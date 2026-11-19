use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Clone)]
struct Command {
    id: &'static str,
    title: &'static str,
    icon: &'static str,
    shortcut: &'static str,
    keywords: &'static [&'static str],
}

pub struct CommandPalette {
    window: gtk4::Window,
    search_entry: gtk4::SearchEntry,
    list_box: gtk4::ListBox,
    commands: Rc<Vec<Command>>,
    visible_commands: Rc<RefCell<Vec<Command>>>,
}

impl CommandPalette {
    pub fn new(parent: &impl IsA<gtk4::Window>) -> Self {
        let window = gtk4::Window::builder()
            .transient_for(parent)
            .modal(true)
            .decorated(false)
            .hide_on_close(true)
            .default_width(640)
            .default_height(420)
            .css_classes(vec!["command-palette"])
            .build();

        let box_container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(8)
            .margin_start(12)
            .margin_end(12)
            .margin_top(12)
            .margin_bottom(12)
            .build();

        let search_entry = gtk4::SearchEntry::builder()
            .placeholder_text("Search browser commands")
            .hexpand(true)
            .build();

        let list_box = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .css_classes(vec!["boxed-list"])
            .build();

        let scrolled_window = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .child(&list_box)
            .build();

        box_container.append(&search_entry);
        box_container.append(&scrolled_window);
        window.set_child(Some(&box_container));

        let commands = Rc::new(vec![
            Command {
                id: "new_tab",
                title: "New Tab",
                icon: "tab-new-symbolic",
                shortcut: "Ctrl+T",
                keywords: &["open", "create", "tab"],
            },
            Command {
                id: "close_tab",
                title: "Close Tab",
                icon: "window-close-symbolic",
                shortcut: "Ctrl+W",
                keywords: &["remove", "tab"],
            },
            Command {
                id: "reopen_tab",
                title: "Reopen Closed Tab",
                icon: "edit-undo-symbolic",
                shortcut: "Ctrl+Shift+T",
                keywords: &["restore", "closed"],
            },
            Command {
                id: "move_left",
                title: "Move Tab Earlier",
                icon: "go-up-symbolic",
                shortcut: "Alt+Shift+Left",
                keywords: &["reorder", "left", "previous"],
            },
            Command {
                id: "move_right",
                title: "Move Tab Later",
                icon: "go-down-symbolic",
                shortcut: "Alt+Shift+Right",
                keywords: &["reorder", "right", "next"],
            },
            Command {
                id: "focus_omnibar",
                title: "Focus Address Bar",
                icon: "edit-find-symbolic",
                shortcut: "Ctrl+L",
                keywords: &["url", "address", "search", "omnibar"],
            },
            Command {
                id: "reload",
                title: "Reload Page",
                icon: "view-refresh-symbolic",
                shortcut: "Ctrl+R",
                keywords: &["refresh", "page"],
            },
            Command {
                id: "go_back",
                title: "Go Back",
                icon: "go-previous-symbolic",
                shortcut: "Alt+Left",
                keywords: &["history", "previous"],
            },
            Command {
                id: "go_forward",
                title: "Go Forward",
                icon: "go-next-symbolic",
                shortcut: "Alt+Right",
                keywords: &["history", "next"],
            },
            Command {
                id: "copy_url",
                title: "Copy Current URL",
                icon: "edit-copy-symbolic",
                shortcut: "",
                keywords: &["clipboard", "link", "address"],
            },
            Command {
                id: "bookmark_page",
                title: "Bookmark Current Page",
                icon: "bookmark-new-symbolic",
                shortcut: "Ctrl+D",
                keywords: &["favorite", "save", "star"],
            },
            Command {
                id: "history",
                title: "Open History",
                icon: "document-open-recent-symbolic",
                shortcut: "Ctrl+H",
                keywords: &["recent", "visited"],
            },
            Command {
                id: "bookmarks",
                title: "Open Bookmarks",
                icon: "user-bookmarks-symbolic",
                shortcut: "Ctrl+B",
                keywords: &["favorites", "saved"],
            },
            Command {
                id: "find",
                title: "Find in Page",
                icon: "edit-find-symbolic",
                shortcut: "Ctrl+F",
                keywords: &["search", "text", "match"],
            },
            Command {
                id: "zoom_in",
                title: "Zoom In",
                icon: "zoom-in-symbolic",
                shortcut: "Ctrl++",
                keywords: &["larger", "bigger", "magnify"],
            },
            Command {
                id: "zoom_out",
                title: "Zoom Out",
                icon: "zoom-out-symbolic",
                shortcut: "Ctrl+-",
                keywords: &["smaller"],
            },
            Command {
                id: "zoom_reset",
                title: "Reset Zoom",
                icon: "zoom-original-symbolic",
                shortcut: "Ctrl+0",
                keywords: &["actual", "size", "100"],
            },
            Command {
                id: "fullscreen",
                title: "Toggle Full Screen",
                icon: "view-fullscreen-symbolic",
                shortcut: "F11",
                keywords: &["fullscreen", "immersive", "kiosk"],
            },
            Command {
                id: "downloads",
                title: "Show Downloads",
                icon: "folder-download-symbolic",
                shortcut: "Ctrl+J",
                keywords: &["files", "saved", "transfer"],
            },
            Command {
                id: "print",
                title: "Print Page",
                icon: "printer-symbolic",
                shortcut: "Ctrl+P",
                keywords: &["paper", "pdf", "save"],
            },
            Command {
                id: "devtools",
                title: "Developer Tools",
                icon: "applications-engineering-symbolic",
                shortcut: "F12",
                keywords: &["inspect", "inspector", "console", "debug"],
            },
            Command {
                id: "clear_data",
                title: "Clear Browsing Data",
                icon: "user-trash-symbolic",
                shortcut: "Ctrl+Shift+Delete",
                keywords: &["history", "cookies", "cache", "privacy", "delete"],
            },
            Command {
                id: "site_permissions",
                title: "Site Permissions",
                icon: "preferences-system-privacy-symbolic",
                shortcut: "",
                keywords: &["camera", "microphone", "location", "notifications"],
            },
            Command {
                id: "settings",
                title: "Open Preferences",
                icon: "emblem-system-symbolic",
                shortcut: "Ctrl+,",
                keywords: &["settings", "config"],
            },
        ]);

        let visible_commands = Rc::new(RefCell::new(Vec::new()));
        rebuild_command_rows(&list_box, &commands, &visible_commands, "");

        let key_controller = gtk4::EventControllerKey::new();
        let window_clone = window.downgrade();
        key_controller.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gtk4::gdk::Key::Escape
                && let Some(window) = window_clone.upgrade()
            {
                window.set_visible(false);
                return gtk4::glib::Propagation::Stop;
            }
            gtk4::glib::Propagation::Proceed
        });
        window.add_controller(key_controller);

        let commands_clone = commands.clone();
        let visible_commands_clone = visible_commands.clone();
        let list_box_clone = list_box.clone();
        search_entry.connect_search_changed(move |entry| {
            rebuild_command_rows(
                &list_box_clone,
                &commands_clone,
                &visible_commands_clone,
                entry.text().as_str(),
            );
        });

        let key_controller_entry = gtk4::EventControllerKey::new();
        let list_box_clone = list_box.clone();
        key_controller_entry.connect_key_pressed(move |_, keyval, _, _| {
            if keyval == gtk4::gdk::Key::Down || keyval == gtk4::gdk::Key::Up {
                list_box_clone.grab_focus();
                return gtk4::glib::Propagation::Proceed;
            }

            if (keyval == gtk4::gdk::Key::Return || keyval == gtk4::gdk::Key::KP_Enter)
                && let Some(row) = list_box_clone.selected_row()
            {
                row.activate();
                return gtk4::glib::Propagation::Stop;
            }

            gtk4::glib::Propagation::Proceed
        });
        search_entry.add_controller(key_controller_entry);
        // The entry handles Enter itself, so the key controller never sees it.
        let list_box_clone = list_box.clone();
        search_entry.connect_activate(move |_| {
            let row = list_box_clone
                .selected_row()
                .or_else(|| list_box_clone.row_at_index(0));
            if let Some(row) = row {
                row.activate();
            }
        });

        Self {
            window,
            search_entry,
            list_box,
            commands,
            visible_commands,
        }
    }

    pub fn show(&self) {
        self.search_entry.set_text("");
        rebuild_command_rows(&self.list_box, &self.commands, &self.visible_commands, "");
        self.window.present();
        self.search_entry.grab_focus();
    }

    pub fn connect_command_activated<F: Fn(&str) + 'static>(&self, f: F) {
        let visible_commands = self.visible_commands.clone();
        let window = self.window.downgrade();
        self.list_box.connect_row_activated(move |_, row| {
            let index = row.index() as usize;
            let id = visible_commands
                .borrow()
                .get(index)
                .map(|command| command.id);
            if let Some(id) = id {
                if let Some(window) = window.upgrade() {
                    window.set_visible(false);
                }
                f(id);
            }
        });
    }
}

fn rebuild_command_rows(
    list_box: &gtk4::ListBox,
    commands: &[Command],
    visible_commands: &Rc<RefCell<Vec<Command>>>,
    query: &str,
) {
    while let Some(child) = list_box.first_child() {
        list_box.remove(&child);
    }

    let query = query.trim().to_lowercase();
    let matches: Vec<Command> = commands
        .iter()
        .filter(|cmd| command_matches(cmd, &query))
        .cloned()
        .collect();

    for cmd in &matches {
        list_box.append(&build_command_row(cmd));
    }

    *visible_commands.borrow_mut() = matches;

    if let Some(row) = list_box.row_at_index(0) {
        list_box.select_row(Some(&row));
    }
}

fn command_matches(command: &Command, query: &str) -> bool {
    if query.is_empty() {
        return true;
    }

    command.title.to_lowercase().contains(query)
        || command
            .keywords
            .iter()
            .any(|keyword| keyword.to_lowercase().contains(query))
}

fn build_command_row(command: &Command) -> gtk4::ListBoxRow {
    let row = adw::ActionRow::builder()
        .title(command.title)
        .activatable(true)
        .build();

    let icon = gtk4::Image::builder()
        .icon_name(command.icon)
        .pixel_size(16)
        .build();
    row.add_prefix(&icon);

    if !command.shortcut.is_empty() {
        let shortcut = gtk4::Label::builder()
            .label(command.shortcut)
            .css_classes(vec!["dim-label", "caption"])
            .build();
        row.add_suffix(&shortcut);
    }

    row.upcast()
}

#[cfg(test)]
mod tests {
    use super::{Command, command_matches};

    fn command(title: &'static str, keywords: &'static [&'static str]) -> Command {
        Command {
            id: "test",
            title,
            icon: "",
            shortcut: "",
            keywords,
        }
    }

    #[test]
    fn command_matches_title_or_keywords() {
        let cmd = command("Close Tab", &["remove", "tab"]);
        assert!(command_matches(&cmd, "close"));
        assert!(command_matches(&cmd, "tab"));
        assert!(command_matches(&cmd, "remov"));
        assert!(!command_matches(&cmd, "open"));
        assert!(!command_matches(&cmd, "nothing here"));
        assert!(command_matches(&cmd, ""));
    }
}
