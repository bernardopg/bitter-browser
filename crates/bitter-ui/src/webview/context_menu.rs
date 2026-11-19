//! Adapts WebKit's context menu to a tabbed browser: items that would open
//! a window open a tab instead, and selected text can be searched.

use gtk4::{gio, glib, prelude::*};
use webkit6::{ContextMenu, ContextMenuAction, ContextMenuItem, HitTestResult};

/// Browser actions the menu items trigger.
pub struct MenuActions {
    /// Opens the address in its string parameter in a new foreground tab.
    pub open_tab: gio::SimpleAction,
    /// Opens the address in a new tab without switching to it.
    pub open_background: gio::SimpleAction,
    /// Searches the page's selected text in a new tab.
    pub search_selection: gio::SimpleAction,
}

impl MenuActions {
    pub fn new(open: impl Fn(String, bool) + Clone + 'static, search: impl Fn() + 'static) -> Self {
        let tab_action = |name: &str, foreground: bool| {
            let action = gio::SimpleAction::new(name, Some(glib::VariantTy::STRING));
            let open = open.clone();
            action.connect_activate(move |_, parameter| {
                if let Some(uri) = parameter.and_then(|value| value.str()) {
                    open(uri.to_owned(), foreground);
                }
            });
            action
        };
        let search_selection = gio::SimpleAction::new("search-selection", None);
        search_selection.connect_activate(move |_, _| search());
        Self {
            open_tab: tab_action("open-tab", true),
            open_background: tab_action("open-background-tab", false),
            search_selection,
        }
    }
}

/// What replaces a stock item that would open a new window.
fn replacement(action: ContextMenuAction, hit: &HitTestResult) -> Option<(&'static str, String)> {
    match action {
        ContextMenuAction::OpenLinkInNewWindow => {
            Some(("Open Link in New Tab", hit.link_uri()?.to_string()))
        }
        ContextMenuAction::OpenImageInNewWindow => {
            Some(("Open Image in New Tab", hit.image_uri()?.to_string()))
        }
        ContextMenuAction::OpenVideoInNewWindow => {
            Some(("Open Video in New Tab", hit.media_uri()?.to_string()))
        }
        ContextMenuAction::OpenAudioInNewWindow => {
            Some(("Open Audio in New Tab", hit.media_uri()?.to_string()))
        }
        _ => None,
    }
}

/// English labels for WebKit's own items, which otherwise follow the system
/// language while the rest of the browser is in English.
fn label(action: ContextMenuAction) -> Option<&'static str> {
    use ContextMenuAction::*;
    Some(match action {
        OpenLink => "Open Link",
        DownloadLinkToDisk => "Download Linked File",
        CopyLinkToClipboard => "Copy Link",
        DownloadImageToDisk => "Save Image As…",
        CopyImageToClipboard => "Copy Image",
        CopyImageUrlToClipboard => "Copy Image Address",
        GoBack => "Back",
        GoForward => "Forward",
        Stop => "Stop",
        Reload => "Reload",
        Copy => "Copy",
        Cut => "Cut",
        Paste => "Paste",
        PasteAsPlainText => "Paste as Plain Text",
        Delete => "Delete",
        SelectAll => "Select All",
        InsertEmoji => "Insert Emoji",
        InspectElement => "Inspect Element",
        NoGuessesFound => "No Guesses Found",
        IgnoreSpelling => "Ignore Spelling",
        LearnSpelling => "Add to Dictionary",
        IgnoreGrammar => "Ignore Grammar",
        Bold => "Bold",
        Italic => "Italic",
        Underline => "Underline",
        CopyVideoLinkToClipboard => "Copy Video Address",
        CopyAudioLinkToClipboard => "Copy Audio Address",
        DownloadVideoToDisk => "Download Video",
        DownloadAudioToDisk => "Download Audio",
        EnterVideoFullscreen => "Full Screen",
        MediaPlay => "Play",
        MediaPause => "Pause",
        MediaMute => "Mute",
        _ => return None,
    })
}

pub fn customize(menu: &ContextMenu, hit: &HitTestResult, actions: &MenuActions, engine: &str) {
    // Positions move as items are removed and added.
    let mut shift = 0;
    for (index, item) in menu.items().into_iter().enumerate() {
        let action = item.stock_action();
        if item.submenu().is_none()
            && let Some(label) = label(action)
        {
            let position = index as i32 + shift;
            menu.remove(&item);
            menu.insert(
                &ContextMenuItem::from_stock_action_with_label(action, label),
                position,
            );
            continue;
        }
        if action == ContextMenuAction::OpenFrameInNewWindow {
            menu.remove(&item);
            shift -= 1;
            continue;
        }
        let Some((label, uri)) = replacement(action, hit) else {
            continue;
        };
        let target = uri.to_variant();
        let position = index as i32 + shift;
        menu.remove(&item);
        menu.insert(
            &ContextMenuItem::from_gaction(&actions.open_tab, label, Some(&target)),
            position,
        );
        if action == ContextMenuAction::OpenLinkInNewWindow {
            menu.insert(
                &ContextMenuItem::from_gaction(
                    &actions.open_background,
                    "Open Link in Background Tab",
                    Some(&target),
                ),
                position + 1,
            );
            shift += 1;
        }
    }
    if hit.context_is_selection() && !hit.context_is_editable() && !hit.context_is_link() {
        menu.append(&ContextMenuItem::new_separator());
        menu.append(&ContextMenuItem::from_gaction(
            &actions.search_selection,
            &format!("Search {engine} for Selection"),
            None,
        ));
    }
}
