use crate::command_palette::CommandPalette;
use crate::downloads::Downloads;
use crate::find_bar::FindBar;
use crate::preferences::{self, PreferenceState};
use crate::session_writer::SessionWriter;
use crate::sidebar::Sidebar;
use crate::suggestion_worker::SuggestionWorker;
use crate::toolbar::Toolbar;
use crate::webview::tab::{Tab, TabView};
use bitter_core::session::{
    self, HOME_URI, MAX_TABS, SavedTab, SavedWorkspace, Session, WorkspaceId,
};
use bitter_core::{bookmarks::Bookmarks, config::SearchEngine, history::History, paths::Profile};
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::rc::{Rc, Weak};
use std::time::Duration;
use webkit6::prelude::*;

mod suggestions;
mod window_state;

const MAX_CLOSED_TABS: usize = 20;

pub struct BitterWindow {
    window: adw::ApplicationWindow,
}

#[derive(Default)]
struct WorkspaceTabs {
    tabs: Vec<Rc<Tab>>,
    selected: usize,
}

struct ClosedTab {
    workspace: WorkspaceId,
    index: usize,
    tab: Rc<Tab>,
}

struct Browser {
    window: glib::WeakRef<adw::ApplicationWindow>,
    stack: gtk4::Stack,
    toolbar: Toolbar,
    sidebar: Sidebar,
    palette: CommandPalette,
    feedback: adw::ToastOverlay,
    downloads: Rc<Downloads>,
    find: Rc<FindBar>,
    fullscreen: Cell<bool>,
    page_fullscreen: Cell<bool>,
    zoom_toast: RefCell<Option<adw::Toast>>,
    history: History,
    bookmarks: Bookmarks,
    preferences: PreferenceState,
    network: webkit6::NetworkSession,
    workspaces: RefCell<[WorkspaceTabs; 2]>,
    workspace: Cell<WorkspaceId>,
    selecting: Cell<bool>,
    closed: RefCell<VecDeque<ClosedTab>>,
    writer: RefCell<Option<SessionWriter>>,
    suggestions: Option<SuggestionWorker>,
    history_failed: Cell<bool>,
    session_failed: Cell<bool>,
}

impl BitterWindow {
    pub fn new(
        app: &adw::Application,
        profile: &Profile,
        history: History,
        bookmarks: Bookmarks,
        config: PreferenceState,
    ) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Bitter Browser")
            .default_width(1200)
            .default_height(800)
            .build();
        // Builder `css_classes` would replace the window's default `background`
        // class, leaving the content area unpainted and text colors unset.
        window.add_css_class("browser-chrome");
        let window_state = window_state::load(&window);
        let sidebar = Sidebar::new();
        let toolbar = Toolbar::new();
        let stack = gtk4::Stack::builder().hexpand(true).vexpand(true).build();
        let find = FindBar::new();
        let content = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        content.append(toolbar.widget());
        content.append(find.widget());
        content.append(&stack);
        let main = gtk4::Box::new(gtk4::Orientation::Horizontal, 0);
        main.append(sidebar.widget());
        main.append(&content);
        let feedback = adw::ToastOverlay::new();
        feedback.set_child(Some(&main));
        window.set_content(Some(&feedback));

        if config.borrow().is_err() {
            feedback.add_toast(
                adw::Toast::builder()
                    .title("Settings could not be loaded. Defaults are active.")
                    .button_label("Preferences")
                    .action_name("win.preferences")
                    .timeout(0)
                    .build(),
            );
        }

        let mut writer = None;
        let mut restored = Session::default();
        if preferences::current(&config).restore_session {
            let path = profile.data_dir().join("session.json");
            match session::load(&path) {
                Ok(snapshot) => {
                    restored = snapshot;
                    writer = SessionWriter::new(path)
                        .map_err(|error| {
                            tracing::warn!("Could not start session writer: {error}");
                            feedback
                                .add_toast(adw::Toast::new("Session changes could not be saved."));
                        })
                        .ok();
                }
                Err(error) => {
                    tracing::warn!("Could not restore session: {error}");
                    feedback.add_toast(
                        adw::Toast::builder()
                            .timeout(0)
                            .title("Could not restore session. The saved file was preserved.")
                            .build(),
                    );
                }
            }
        }
        let network = webkit6::NetworkSession::new(
            Some(profile.web_data_dir()),
            Some(profile.web_cache_dir()),
        );
        if let Some(data) = network.website_data_manager() {
            data.set_favicons_enabled(true);
        }
        let downloads = Downloads::new(&feedback);
        downloads.attach(&network);
        toolbar.pack_end(downloads.widget());
        let browser = Rc::new(Browser {
            window: window.downgrade(),
            palette: CommandPalette::new(&window),
            stack,
            toolbar,
            sidebar,
            feedback,
            downloads,
            find,
            fullscreen: Cell::new(false),
            page_fullscreen: Cell::new(false),
            zoom_toast: Default::default(),
            history,
            bookmarks,
            preferences: config,
            network,
            workspaces: Default::default(),
            workspace: Cell::new(restored.workspace),
            selecting: Cell::new(false),
            closed: Default::default(),
            writer: RefCell::new(writer),
            suggestions: SuggestionWorker::new(profile.data_dir().to_owned())
                .map_err(|error| tracing::warn!("Could not start address suggestions: {error}"))
                .ok(),
            history_failed: Cell::new(false),
            session_failed: Cell::new(false),
        });
        browser.connect(app, &window);
        for (workspace, saved) in WorkspaceId::ALL.into_iter().zip(restored.workspaces) {
            for tab in saved.tabs {
                browser.insert(workspace, Rc::new(Tab::new(tab)), None);
            }
            browser.workspaces.borrow_mut()[workspace.index()].selected = saved.selected;
        }
        browser.show_workspace(restored.workspace);
        {
            let browser = browser.clone();
            window.connect_close_request(move |window| {
                if let Some(settings) = &window_state {
                    window_state::save(window, settings);
                }
                browser.save_snapshot();
                if let Some(mut writer) = browser.writer.borrow_mut().take() {
                    writer.finish();
                }
                glib::Propagation::Proceed
            });
        }
        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}

impl Browser {
    fn connect(self: &Rc<Self>, app: &adw::Application, window: &adw::ApplicationWindow) {
        let weak = Rc::downgrade(self);
        self.sidebar.tab_list().connect_selected(move |id| {
            if let Some(browser) = weak.upgrade()
                && !browser.selecting.get()
            {
                browser.select_id(id);
            }
        });
        let weak = Rc::downgrade(self);
        self.sidebar.workspace().connect_selected(move |workspace| {
            if let Some(browser) = weak.upgrade()
                && !browser.selecting.get()
            {
                browser.show_workspace(workspace);
            }
        });
        self.sidebar
            .new_tab_button()
            .set_action_name(Some("win.new_tab"));
        let weak = Rc::downgrade(self);
        self.palette.connect_command_activated(move |command| {
            if let Some(browser) = weak.upgrade() {
                browser.command(command);
            }
        });

        for (id, accelerators) in [
            ("new_tab", &["<Control>t"][..]),
            ("close_tab", &["<Control>w"]),
            ("reopen_tab", &["<Control><Shift>t"]),
            ("next_tab", &["<Control>Page_Down", "<Control>Tab"]),
            ("previous_tab", &["<Control>Page_Up", "<Control><Shift>Tab"]),
            ("move_left", &["<Alt><Shift>Left"]),
            ("move_right", &["<Alt><Shift>Right"]),
            ("focus_omnibar", &["<Control>l"]),
            ("refresh", &["<Control>r", "F5"]),
            ("reload", &[]),
            ("back", &["<Alt>Left"]),
            ("forward", &["<Alt>Right"]),
            ("palette", &["<Control>k"]),
            ("history", &["<Control>h"]),
            ("bookmarks", &["<Control>b"]),
            ("bookmark_page", &["<Control>d"]),
            ("downloads", &["<Control>j"]),
            ("find", &["<Control>f"]),
            ("find_next", &["<Control>g"]),
            ("find_previous", &["<Control><Shift>g"]),
            (
                "zoom_in",
                &["<Control>plus", "<Control>equal", "<Control>KP_Add"],
            ),
            ("zoom_out", &["<Control>minus", "<Control>KP_Subtract"]),
            ("zoom_reset", &["<Control>0", "<Control>KP_0"]),
            ("fullscreen", &["F11"]),
            ("preferences", &["<Control>comma"]),
        ] {
            let action = gio::SimpleAction::new(id, None);
            let weak = Rc::downgrade(self);
            action.connect_activate(move |_, _| {
                if let Some(browser) = weak.upgrade() {
                    browser.command(id);
                }
            });
            window.add_action(&action);
            app.set_accels_for_action(&format!("win.{id}"), accelerators);
        }
        let open_uri = gio::SimpleAction::new("open_uri", Some(glib::VariantTy::STRING));
        let weak = Rc::downgrade(self);
        open_uri.connect_activate(move |_, parameter| {
            let Some(browser) = weak.upgrade() else {
                return;
            };
            match parameter
                .and_then(|value| value.str())
                .and_then(external_uri)
            {
                Some(uri) => {
                    browser.new_tab(browser.workspace.get(), &uri, true);
                }
                None => browser.toast("This address scheme is unavailable."),
            }
        });
        window.add_action(&open_uri);
        let keys = gtk4::EventControllerKey::new();
        let weak = Rc::downgrade(self);
        keys.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape
                && let Some(browser) = weak.upgrade()
                && let Some(tab) = browser.active()
                && let Some(view) = tab.view.get()
                && view.web.is_loading()
            {
                view.web.stop_loading();
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(keys);
        // The web view consumes Escape, so leave browser full screen before it
        // sees the key. Page-requested full screen is left to WebKit.
        let escape = gtk4::EventControllerKey::new();
        escape.set_propagation_phase(gtk4::PropagationPhase::Capture);
        let weak = Rc::downgrade(self);
        escape.connect_key_pressed(move |_, key, _, _| {
            if key == gtk4::gdk::Key::Escape
                && let Some(browser) = weak.upgrade()
                && browser.fullscreen.get()
                && !browser.page_fullscreen.get()
                && !browser.find.is_open()
            {
                browser.set_fullscreen(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        window.add_controller(escape);
        // Follow the window manager only after it confirmed full screen, so a
        // request it has not applied yet does not restore the chrome.
        let weak = Rc::downgrade(self);
        let confirmed = Cell::new(false);
        window.connect_fullscreened_notify(move |window| {
            if window.is_fullscreen() {
                confirmed.set(true);
            } else if confirmed.replace(false)
                && let Some(browser) = weak.upgrade()
            {
                browser.set_fullscreen(false);
            }
        });
        suggestions::connect(self);

        if self.writer.borrow().is_some() {
            let weak = Rc::downgrade(self);
            glib::timeout_add_local(Duration::from_millis(500), move || {
                let Some(browser) = weak.upgrade() else {
                    return glib::ControlFlow::Break;
                };
                let result = browser
                    .writer
                    .borrow()
                    .as_ref()
                    .and_then(SessionWriter::newest_result);
                if let Some(result) = result {
                    match result {
                        Ok(()) => browser.session_failed.set(false),
                        Err(error) if !browser.session_failed.replace(true) => {
                            tracing::warn!("Could not save session: {error}");
                            browser.toast("Session changes could not be saved.");
                        }
                        Err(_) => {}
                    }
                }
                glib::ControlFlow::Continue
            });
        }
    }

    fn command(self: &Rc<Self>, command: &str) {
        match command {
            "new_tab" => {
                if self.new_tab(self.workspace.get(), HOME_URI, true) {
                    self.focus_address();
                }
            }
            "close_tab" => {
                if let Some(tab) = self.active() {
                    self.close(&tab.id);
                }
            }
            "reopen_tab" => self.reopen(),
            "next_tab" => self.cycle(1),
            "previous_tab" => self.cycle(-1),
            "move_left" => self.reorder(-1),
            "move_right" => self.reorder(1),
            "focus_omnibar" => self.focus_address(),
            "reload" => self.with_view(|tab, view| {
                if view.web.is_loading() && !view.failed.get() {
                    view.web.stop_loading();
                } else {
                    self.retry(tab, view);
                }
            }),
            "refresh" => self.with_view(|tab, view| self.retry(tab, view)),
            "go_back" | "back" => self.with_view(|_, view| view.web.go_back()),
            "go_forward" | "forward" => self.with_view(|_, view| view.web.go_forward()),
            "palette" => self.palette.show(),
            "history" => self.navigate_active("bitter://history"),
            "bookmarks" => self.navigate_active("bitter://bookmarks"),
            "bookmark_page" => self.toggle_bookmark(),
            "settings" | "preferences" => self.show_preferences(),
            "downloads" => self.downloads.show(),
            "find" => {
                if let Some(tab) = self.active() {
                    let view = self.materialize(&tab);
                    if let Some(controller) = view.web.find_controller() {
                        self.find.open(controller);
                    }
                }
            }
            "find_next" | "find_previous" => {
                if self.find.is_open() {
                    self.find.step(command == "find_next");
                } else {
                    self.command("find");
                }
            }
            "zoom_in" => self.zoom(Some(true)),
            "zoom_out" => self.zoom(Some(false)),
            "zoom_reset" => self.zoom(None),
            "fullscreen" => self.set_fullscreen(!self.fullscreen.get()),
            "copy_url" => {
                if let (Some(display), Some(tab)) = (gtk4::gdk::Display::default(), self.active()) {
                    display.clipboard().set_text(&tab.uri());
                }
            }
            _ => {}
        }
    }

    fn active(&self) -> Option<Rc<Tab>> {
        let all = self.workspaces.borrow();
        let group = &all[self.workspace.get().index()];
        group.tabs.get(group.selected).cloned()
    }

    fn tab_position(&self, id: &str) -> Option<(WorkspaceId, usize)> {
        let all = self.workspaces.borrow();
        WorkspaceId::ALL.into_iter().find_map(|workspace| {
            all[workspace.index()]
                .tabs
                .iter()
                .position(|tab| tab.id == id)
                .map(|index| (workspace, index))
        })
    }

    fn insert(
        self: &Rc<Self>,
        workspace: WorkspaceId,
        tab: Rc<Tab>,
        index: Option<usize>,
    ) -> usize {
        let index = {
            let mut all = self.workspaces.borrow_mut();
            let group = &mut all[workspace.index()];
            let index = index.unwrap_or(group.tabs.len()).min(group.tabs.len());
            group.tabs.insert(index, tab.clone());
            index
        };
        let weak = Rc::downgrade(self);
        self.sidebar
            .tab_list()
            .insert(&tab.id, &tab.title(), workspace, index, move |id| {
                if let Some(browser) = weak.upgrade() {
                    browser.close(id);
                }
            });
        if tab.view.get().is_none() {
            self.load_saved_favicon(&tab);
        }
        index
    }

    /// Shows the stored icon of a tab whose page has not been loaded yet.
    fn load_saved_favicon(self: &Rc<Self>, tab: &Rc<Tab>) {
        let uri = tab.uri();
        if !is_recordable_uri(&uri) {
            return;
        }
        let Some(database) = self
            .network
            .website_data_manager()
            .and_then(|data| data.favicon_database())
        else {
            return;
        };
        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        database.favicon(&uri, gio::Cancellable::NONE, move |result| {
            if let Ok(texture) = result
                && let Some((browser, tab)) = upgrade(&weak, &weak_tab)
                && tab.view.get().is_none()
            {
                browser
                    .sidebar
                    .tab_list()
                    .set_favicon(&tab.id, Some(&texture));
            }
        });
    }

    fn new_tab(self: &Rc<Self>, workspace: WorkspaceId, uri: &str, foreground: bool) -> bool {
        if self.workspaces.borrow()[workspace.index()].tabs.len() >= MAX_TABS {
            self.toast("This workspace has reached its 100-tab limit.");
            return false;
        }
        let title = if uri == HOME_URI { "New Tab" } else { uri };
        let tab = Rc::new(Tab::new(SavedTab {
            uri: uri.to_string(),
            title: title.to_string(),
        }));
        let index = self.insert(workspace, tab, None);
        if foreground {
            self.select(workspace, index);
        } else {
            self.save_snapshot();
        }
        true
    }

    fn show_workspace(self: &Rc<Self>, workspace: WorkspaceId) {
        let (empty, index) = {
            let all = self.workspaces.borrow();
            let group = &all[workspace.index()];
            (group.tabs.is_empty(), group.selected)
        };
        if empty {
            self.new_tab(workspace, HOME_URI, true);
        } else {
            self.select(workspace, index);
        }
    }

    fn select_id(self: &Rc<Self>, id: &str) {
        if let Some((workspace, index)) = self.tab_position(id) {
            self.select(workspace, index);
        }
    }

    fn select(self: &Rc<Self>, workspace: WorkspaceId, index: usize) {
        let tab = {
            let mut all = self.workspaces.borrow_mut();
            let group = &mut all[workspace.index()];
            let Some(tab) = group.tabs.get(index).cloned() else {
                return;
            };
            group.selected = index;
            tab
        };
        self.find.close();
        self.workspace.set(workspace);
        self.selecting.set(true);
        self.sidebar.workspace().show(workspace);
        self.sidebar.tab_list().show(workspace, &tab.id);
        self.selecting.set(false);
        let view = self.materialize(&tab);
        self.stack.set_visible_child(&view.container);
        self.display_address(&tab);
        self.refresh_view(&tab, view);
        view.web.grab_focus();
        self.save_snapshot();
    }

    fn close(self: &Rc<Self>, id: &str) {
        let Some((workspace, index)) = self.tab_position(id) else {
            return;
        };
        let (tab, selected, empty, was_active) = {
            let mut all = self.workspaces.borrow_mut();
            let group = &mut all[workspace.index()];
            let was_active = self.workspace.get() == workspace && group.selected == index;
            let tab = group.tabs.remove(index);
            group.selected = selection_after_close(group.selected, index, group.tabs.len());
            (tab, group.selected, group.tabs.is_empty(), was_active)
        };
        if let Some(view) = tab.view.get() {
            view.web.stop_loading();
            self.stack.remove(&view.container);
        }
        self.selecting.set(true);
        self.sidebar.tab_list().remove(workspace, id);
        self.selecting.set(false);
        if tab.uri() != HOME_URI {
            let mut closed = self.closed.borrow_mut();
            // Closed tabs retain metadata only; their WebKit page can be released.
            closed.push_front(ClosedTab {
                workspace,
                index,
                tab: Rc::new(Tab::new(tab.saved())),
            });
            closed.truncate(MAX_CLOSED_TABS);
        }
        if was_active {
            if empty {
                self.new_tab(workspace, HOME_URI, true);
            } else {
                self.select(workspace, selected);
            }
        } else {
            self.save_snapshot();
        }
    }

    fn reopen(self: &Rc<Self>) {
        let Some(closed) = self.closed.borrow_mut().pop_front() else {
            return;
        };
        if self.workspaces.borrow()[closed.workspace.index()]
            .tabs
            .len()
            >= MAX_TABS
        {
            self.closed.borrow_mut().push_front(closed);
            self.toast("Close a tab in that workspace before reopening this one.");
            return;
        }
        let index = self.insert(closed.workspace, closed.tab, Some(closed.index));
        self.select(closed.workspace, index);
    }

    fn cycle(self: &Rc<Self>, offset: isize) {
        let workspace = self.workspace.get();
        let index = {
            let all = self.workspaces.borrow();
            let group = &all[workspace.index()];
            (group.selected as isize + offset).rem_euclid(group.tabs.len() as isize) as usize
        };
        self.select(workspace, index);
    }

    fn reorder(self: &Rc<Self>, offset: isize) {
        let workspace = self.workspace.get();
        let (index, tab) = {
            let mut all = self.workspaces.borrow_mut();
            let group = &mut all[workspace.index()];
            let current = group.selected;
            let next = (current as isize + offset).clamp(0, group.tabs.len() as isize - 1) as usize;
            if next == current {
                return;
            }
            group.tabs.swap(current, next);
            group.selected = next;
            (next, group.tabs[next].clone())
        };
        self.selecting.set(true);
        self.sidebar.tab_list().move_to(workspace, &tab.id, index);
        self.selecting.set(false);
        self.select(workspace, index);
    }

    fn save_snapshot(&self) {
        let writer = self.writer.borrow();
        let Some(writer) = writer.as_ref() else {
            return;
        };
        let all = self.workspaces.borrow();
        let snapshot = Session {
            workspace: self.workspace.get(),
            workspaces: WorkspaceId::ALL.map(|workspace| {
                let group = &all[workspace.index()];
                SavedWorkspace {
                    selected: group.selected,
                    tabs: group
                        .tabs
                        .iter()
                        .map(|tab| {
                            let saved = tab.saved();
                            SavedTab::new(&saved.uri, &saved.title)
                        })
                        .collect(),
                }
            }),
        };
        writer.queue(snapshot);
    }

    fn toast(&self, message: &str) {
        self.feedback.add_toast(adw::Toast::new(message));
    }

    /// Steps the active tab's zoom, or resets it when `zoom_in` is `None`.
    fn zoom(&self, zoom_in: Option<bool>) {
        self.with_view(|_, view| {
            let level = match zoom_in {
                Some(zoom_in) => next_zoom(view.web.zoom_level(), zoom_in),
                None => 1.0,
            };
            view.web.set_zoom_level(level);
            let toast = adw::Toast::builder()
                .title(format!("Zoom {:.0}%", level * 100.0))
                .timeout(2)
                .build();
            if let Some(previous) = self.zoom_toast.replace(Some(toast.clone())) {
                previous.dismiss();
            }
            self.feedback.add_toast(toast);
        });
    }

    /// Fullscreen hides the browser chrome so the page fills the screen.
    fn set_fullscreen(&self, fullscreen: bool) {
        if self.fullscreen.replace(fullscreen) == fullscreen {
            return;
        }
        self.sidebar.widget().set_visible(!fullscreen);
        self.toolbar.widget().set_visible(!fullscreen);
        if let Some(window) = self.window.upgrade() {
            if fullscreen {
                window.fullscreen();
            } else {
                window.unfullscreen();
            }
        }
        if fullscreen {
            self.find.close();
            if !self.page_fullscreen.get() {
                self.toast("Press F11 or Esc to exit full screen");
            }
        }
    }

    fn focus_address(&self) {
        self.toolbar.omnibar().grab_focus();
        self.toolbar.omnibar().select_all();
    }

    fn show_preferences(&self) {
        if let Some(window) = self.window.upgrade()
            && window.visible_dialog().is_none()
        {
            preferences::present(&window, &self.preferences);
        }
    }

    fn with_view(&self, f: impl FnOnce(&Tab, &TabView)) {
        if let Some(tab) = self.active()
            && let Some(view) = tab.view.get()
        {
            f(&tab, view);
        }
    }

    fn navigate_active(&self, uri: &str) {
        if uri == "bitter://settings" {
            self.with_view(|tab, _| self.display_address(tab));
            self.show_preferences();
            return;
        }
        self.with_view(|tab, view| {
            self.navigate(tab, view, uri);
            view.web.grab_focus();
        });
    }

    fn navigate(&self, tab: &Tab, view: &TabView, uri: &str) {
        let html = match uri {
            HOME_URI => Some(internal_page_shell(
                "New Tab",
                r#"<section class="empty"><p>Enter an address or search in the address bar.</p><p><kbd>Ctrl</kbd> + <kbd>L</kbd> focuses the address bar.</p></section>"#,
            )),
            "bitter://history" => Some(match self.history.recent(50) {
                Ok(rows) => internal_list_page(
                    "History",
                    "Recent pages",
                    &rows,
                    "History will appear here after you visit pages.",
                ),
                Err(error) => unavailable_list_page("History", &error.to_string()),
            }),
            "bitter://bookmarks" => Some(match self.bookmarks.get_all() {
                Ok(rows) => internal_list_page(
                    "Bookmarks",
                    "Saved pages",
                    &rows,
                    "Bookmarks will appear here after you save pages with Ctrl+D.",
                ),
                Err(error) => unavailable_list_page("Bookmarks", &error.to_string()),
            }),
            _ => None,
        };
        let title = match uri {
            HOME_URI => "New Tab",
            "bitter://history" => "History",
            "bitter://bookmarks" => "Bookmarks",
            _ => uri,
        };
        *view.requested.borrow_mut() = uri.to_owned();
        if tab.uri() != uri {
            tab.update(uri, title);
        }
        if let Some(html) = html {
            view.web.load_html(&html, Some(uri));
        } else {
            view.web.load_uri(uri);
        }
    }

    fn retry(&self, tab: &Tab, view: &TabView) {
        if view.failed.get() || tab.uri().starts_with("bitter://") {
            let uri = view.requested.borrow().clone();
            self.navigate(tab, view, &uri);
        } else {
            view.web.reload();
        }
    }

    fn materialize<'a>(self: &Rc<Self>, tab: &'a Rc<Tab>) -> &'a TabView {
        if let Some(view) = tab.view.get() {
            return view;
        }
        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        let view = tab.view.get_or_init(|| {
            TabView::new(&self.network, move || {
                if let Some((browser, tab)) = upgrade(&weak, &weak_tab)
                    && let Some(view) = tab.view.get()
                {
                    browser.retry(&tab, view);
                }
            })
        });
        self.connect_view(tab, view);
        self.stack.add_child(&view.container);
        self.navigate(tab, view, &tab.uri());
        view
    }

    fn connect_view(self: &Rc<Self>, tab: &Rc<Tab>, view: &TabView) {
        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_load_changed(move |web, event| {
            let Some((browser, tab)) = upgrade(&weak, &weak_tab) else {
                return;
            };
            let view = tab.view.get().expect("view attached before loading");
            match event {
                webkit6::LoadEvent::Started => {
                    view.loading_started();
                    if let Some(uri) = web.uri() {
                        *view.requested.borrow_mut() = uri.to_string();
                    }
                }
                webkit6::LoadEvent::Redirected => {
                    if let Some(uri) = web.uri() {
                        *view.requested.borrow_mut() = uri.to_string();
                    }
                }
                webkit6::LoadEvent::Committed => {
                    if let Some(uri) = web.uri() {
                        tab.update(&uri, &uri);
                    }
                }
                webkit6::LoadEvent::Finished if !view.failed.get() => {
                    if let Some(uri) = web.uri() {
                        let title = web.title().filter(|title| !title.trim().is_empty());
                        tab.update(&uri, title.as_deref().unwrap_or(&uri));
                        browser.display_active_address(&tab);
                        if !view.cancelled.get() {
                            view.visited.set(true);
                            browser.record_history(&uri, title.as_deref());
                        }
                        browser.save_snapshot();
                    }
                }
                _ => {}
            }
            browser.refresh_view(&tab, view);
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_load_failed(move |_, _, uri, error| {
            let Some((browser, tab)) = upgrade(&weak, &weak_tab) else {
                return false;
            };
            let view = tab.view.get().expect("view attached before loading");
            if error.matches(webkit6::NetworkError::Cancelled)
                || error.matches(webkit6::PolicyError::FrameLoadInterruptedByPolicyChange)
            {
                view.cancelled.set(true);
                return true;
            }
            if !view.failed.get() {
                *view.requested.borrow_mut() = uri.to_owned();
                tab.update(uri, uri);
                view.show_problem(
                    "Could not load this page",
                    "Check your connection and try again.",
                    "network-error-symbolic",
                );
                browser.display_active_address(&tab);
                browser.refresh_view(&tab, view);
                browser.save_snapshot();
            }
            true
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web
            .connect_load_failed_with_tls_errors(move |_, uri, _, _| {
                let Some((browser, tab)) = upgrade(&weak, &weak_tab) else {
                    return false;
                };
                let view = tab.view.get().expect("view attached before loading");
                *view.requested.borrow_mut() = uri.to_owned();
                tab.update(uri, uri);
                view.show_problem(
                    "Secure connection failed",
                    "The site's certificate could not be verified.",
                    "channel-insecure-symbolic",
                );
                browser.display_active_address(&tab);
                browser.refresh_view(&tab, view);
                browser.save_snapshot();
                true
            });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_web_process_terminated(move |_, _| {
            if let Some((browser, tab)) = upgrade(&weak, &weak_tab) {
                let view = tab.view.get().expect("view attached before loading");
                *view.requested.borrow_mut() = tab.uri();
                view.show_problem(
                    "This tab stopped responding",
                    "Reload this tab to continue browsing.",
                    "dialog-warning-symbolic",
                );
                browser.refresh_view(&tab, view);
            }
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_uri_notify(move |web| {
            if let Some((browser, tab)) = upgrade(&weak, &weak_tab)
                && let Some(uri) = web.uri()
            {
                if browser.is_active(&tab) && !browser.toolbar.omnibar().editing() {
                    browser.toolbar.omnibar().set_text(address_text(&uri));
                }
                // Same-document changes have no load-changed signal.
                if tab.uri() != uri && !web.is_loading() {
                    let weak = Rc::downgrade(&browser);
                    let weak_tab = Rc::downgrade(&tab);
                    glib::idle_add_local_once(move || {
                        if let Some((browser, tab)) = upgrade(&weak, &weak_tab)
                            && let Some(view) = tab.view.get()
                            && !view.web.is_loading()
                            && !view.failed.get()
                            && view.visited.get()
                            && view.web.uri().as_deref() == Some(uri.as_str())
                            && tab.uri() != uri
                        {
                            tab.update(&uri, &tab.title());
                            *view.requested.borrow_mut() = uri.to_string();
                            browser.save_snapshot();
                        }
                    });
                }
            }
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_title_notify(move |web| {
            if let Some((browser, tab)) = upgrade(&weak, &weak_tab)
                && let Some(view) = tab.view.get()
                && !view.failed.get()
                && let Some(uri) = web.uri()
                && let Some(title) = web.title().filter(|title| !title.trim().is_empty())
                && tab.uri() == uri
            {
                tab.update(&uri, &title);
                browser
                    .sidebar
                    .tab_list()
                    .update(&tab.id, &title, web.is_loading());
                if view.visited.get() && is_recordable_uri(&uri) {
                    let result = browser.history.update_title(&uri, &title);
                    browser.report_history_write(result);
                    browser.save_snapshot();
                }
            }
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_estimated_load_progress_notify(move |_| {
            if let Some((browser, tab)) = upgrade(&weak, &weak_tab)
                && let Some(view) = tab.view.get()
            {
                browser.refresh_toolbar(&tab, view);
            }
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_is_loading_notify(move |_| {
            if let Some((browser, tab)) = upgrade(&weak, &weak_tab)
                && let Some(view) = tab.view.get()
            {
                browser.refresh_view(&tab, view);
            }
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_favicon_notify(move |web| {
            if let Some((browser, tab)) = upgrade(&weak, &weak_tab) {
                browser
                    .sidebar
                    .tab_list()
                    .set_favicon(&tab.id, web.favicon().as_ref());
            }
        });

        let weak = Rc::downgrade(self);
        view.web.connect_enter_fullscreen(move |_| {
            if let Some(browser) = weak.upgrade() {
                browser.page_fullscreen.set(true);
                browser.set_fullscreen(true);
            }
            false
        });
        let weak = Rc::downgrade(self);
        view.web.connect_leave_fullscreen(move |_| {
            if let Some(browser) = weak.upgrade() {
                browser.page_fullscreen.set(false);
                browser.set_fullscreen(false);
            }
            false
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_decide_policy(move |_, decision, kind| {
            use webkit6::PolicyDecisionType::*;
            if kind == Response {
                return decide_response(decision);
            }
            if !matches!(kind, NavigationAction | NewWindowAction) {
                return false;
            }
            let Some(policy) = decision.downcast_ref::<webkit6::NavigationPolicyDecision>() else {
                return false;
            };
            let Some(action) = policy.navigation_action() else {
                return false;
            };
            let Some((browser, tab)) = upgrade(&weak, &weak_tab) else {
                return false;
            };
            let Some((workspace, _)) = browser.tab_position(&tab.id) else {
                return false;
            };
            let modifiers = gtk4::gdk::ModifierType::from_bits_truncate(action.modifiers());
            let background = action.mouse_button() == 2
                || modifiers.contains(gtk4::gdk::ModifierType::CONTROL_MASK);
            if kind != NewWindowAction && !background {
                return false;
            }
            if action.is_user_gesture()
                && action.navigation_type() == webkit6::NavigationType::LinkClicked
                && let Some(uri) = action.request().and_then(|request| request.uri())
                && is_recordable_uri(&uri)
            {
                let foreground =
                    !background || modifiers.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
                let weak = Rc::downgrade(&browser);
                glib::idle_add_local_once(move || {
                    if let Some(browser) = weak.upgrade() {
                        browser.new_tab(workspace, &uri, foreground);
                    }
                });
                decision.ignore();
                return true;
            }
            if kind == NewWindowAction {
                decision.ignore();
                return true;
            }
            false
        });
    }

    fn is_active(&self, tab: &Tab) -> bool {
        self.active().is_some_and(|active| active.id == tab.id)
    }

    fn display_address(&self, tab: &Tab) {
        self.toolbar.omnibar().set_text(address_text(&tab.uri()));
    }

    fn display_active_address(&self, tab: &Tab) {
        if self.is_active(tab) && !self.toolbar.omnibar().editing() {
            self.display_address(tab);
        }
    }

    fn refresh_view(&self, tab: &Tab, view: &TabView) {
        let loading = view.web.is_loading() && !view.failed.get();
        self.sidebar
            .tab_list()
            .update(&tab.id, &tab.title(), loading);
        self.refresh_toolbar(tab, view);
    }

    fn refresh_toolbar(&self, tab: &Tab, view: &TabView) {
        if self.is_active(tab) {
            self.toolbar.refresh(
                view.web.can_go_back(),
                view.web.can_go_forward(),
                view.web.is_loading() && !view.failed.get(),
                view.web.estimated_load_progress(),
            );
        }
    }

    fn toggle_bookmark(&self) {
        let Some(tab) = self.active() else { return };
        let uri = tab.uri();
        if !is_recordable_uri(&uri) {
            self.toast("Open a web page to bookmark it.");
            return;
        }
        let result = match self.bookmarks.is_bookmarked(&uri) {
            Ok(true) => self
                .bookmarks
                .remove_bookmark(&uri)
                .map(|()| "Bookmark removed"),
            Ok(false) => self
                .bookmarks
                .add_bookmark(&uri, Some(&tab.title()))
                .map(|()| "Bookmark saved"),
            Err(error) => Err(error),
        };
        match result {
            Ok(message) => self.toast(message),
            Err(error) => {
                tracing::warn!("Could not change bookmark: {error}");
                self.toast("Could not change bookmark. Check disk space and profile permissions.");
            }
        }
    }

    fn record_history(&self, uri: &str, title: Option<&str>) {
        if !is_recordable_uri(uri) {
            return;
        }
        self.report_history_write(self.history.add_visit(uri, title));
    }

    fn report_history_write(&self, result: Result<(), impl std::fmt::Display>) {
        match result {
            Ok(()) => self.history_failed.set(false),
            Err(error) if !self.history_failed.replace(true) => {
                tracing::warn!("Could not record a history visit: {error}");
                self.toast("History could not be saved. Check disk space and profile permissions.");
            }
            Err(_) => {}
        }
    }
}

fn upgrade(browser: &Weak<Browser>, tab: &Weak<Tab>) -> Option<(Rc<Browser>, Rc<Tab>)> {
    Some((browser.upgrade()?, tab.upgrade()?))
}

fn selection_after_close(selected: usize, removed: usize, remaining: usize) -> usize {
    (selected - usize::from(removed < selected)).min(remaining.saturating_sub(1))
}

fn address_text(uri: &str) -> &str {
    if matches!(uri, HOME_URI | "bitter://newtab/") {
        ""
    } else {
        uri
    }
}

fn is_recordable_uri(uri: &str) -> bool {
    url::Url::parse(uri)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host().is_some())
}

#[derive(Debug, PartialEq, Eq)]
enum Target {
    Address(String),
    Search(String),
}

const ZOOM_LEVELS: [f64; 13] = [
    0.3, 0.5, 0.67, 0.8, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0, 2.5, 3.0,
];

/// Returns the next preset zoom level above or below `current`.
fn next_zoom(current: f64, zoom_in: bool) -> f64 {
    const EPSILON: f64 = 0.001;
    if zoom_in {
        ZOOM_LEVELS
            .into_iter()
            .find(|level| *level > current + EPSILON)
            .unwrap_or(ZOOM_LEVELS[ZOOM_LEVELS.len() - 1])
    } else {
        ZOOM_LEVELS
            .into_iter()
            .rev()
            .find(|level| *level < current - EPSILON)
            .unwrap_or(ZOOM_LEVELS[0])
    }
}

/// Downloads main-frame responses the engine cannot display and those the
/// server marks as attachments. Other responses keep WebKit's default.
fn decide_response(decision: &webkit6::PolicyDecision) -> bool {
    let Some(policy) = decision.downcast_ref::<webkit6::ResponsePolicyDecision>() else {
        return false;
    };
    if !policy.is_main_frame_main_resource() {
        return false;
    }
    let attachment = policy
        .response()
        .and_then(|response| response.http_headers())
        .and_then(|headers| headers.content_disposition())
        .is_some_and(|(disposition, _)| is_attachment(&disposition));
    if attachment || !policy.is_mime_type_supported() {
        decision.download();
        return true;
    }
    false
}

fn is_attachment(disposition: &str) -> bool {
    disposition.trim().eq_ignore_ascii_case("attachment")
}

/// Accepts addresses handed over by the desktop, such as links opened from
/// other applications, and rejects schemes the browser does not load.
fn external_uri(uri: &str) -> Option<String> {
    let parsed = url::Url::parse(uri.trim()).ok()?;
    let supported = match parsed.scheme() {
        "http" | "https" => parsed.host().is_some(),
        "file" => true,
        _ => false,
    };
    supported.then(|| parsed.into())
}

fn navigation_target(input: &str, engine: SearchEngine) -> Result<Option<Target>, &'static str> {
    let input = input.trim();
    if input.is_empty() {
        return Ok(None);
    }
    if matches!(
        input,
        HOME_URI | "bitter://history" | "bitter://bookmarks" | "bitter://settings" | "about:blank"
    ) {
        return Ok(Some(Target::Address(input.to_owned())));
    }

    let (first, remainder) = input.split_once(':').unwrap_or(("", ""));
    let in_authority =
        first.contains('.') || first.starts_with('[') || first.eq_ignore_ascii_case("localhost");
    let host_suffix = !remainder.is_empty()
        && remainder
            .split('/')
            .next()
            .is_some_and(|port| port.bytes().all(|byte| byte.is_ascii_digit()));
    let explicit = first
        .bytes()
        .next()
        .is_some_and(|byte| byte.is_ascii_alphabetic())
        && first
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"+-.".contains(&byte))
        && !(in_authority && host_suffix);
    if explicit && (remainder.starts_with("//") || !input.contains(char::is_whitespace)) {
        let scheme = first.to_ascii_lowercase();
        if !matches!(scheme.as_str(), "http" | "https" | "file") {
            return Err("This address scheme is unavailable.");
        }
        let authority = remainder
            .strip_prefix("//")
            .and_then(|rest| rest.split(['/', '?', '#']).next());
        if matches!(scheme.as_str(), "http" | "https") && authority.unwrap_or_default().is_empty() {
            return Err("Enter a complete web address.");
        }
        let uri = url::Url::parse(input).map_err(|_| "Enter a valid address.")?;
        return Ok(Some(Target::Address(uri.into())));
    }

    let host = input.split(['/', '?', '#']).next().unwrap_or_default();
    if !input.contains(char::is_whitespace)
        && (host.starts_with('[')
            || host.contains('.')
            || host.eq_ignore_ascii_case("localhost")
            || host.to_ascii_lowercase().starts_with("localhost:"))
    {
        let uri =
            url::Url::parse(&format!("https://{input}")).map_err(|_| "Enter a valid address.")?;
        return Ok(Some(Target::Address(uri.into())));
    }
    Ok(Some(Target::Search(engine.search_url(input))))
}
fn unavailable_list_page(title: &str, error: &str) -> String {
    internal_page_shell(
        title,
        &format!(
            r#"<section class="empty"><h2>Could not load this list</h2><p>Check available disk space and profile permissions, then try again.</p><p>{}</p></section>"#,
            escape_html(error),
        ),
    )
}

fn internal_list_page(
    title: &str,
    heading: &str,
    rows: &[(String, String)],
    empty_message: &str,
) -> String {
    let body = if rows.is_empty() {
        format!(
            r#"<section class="empty"><h2>{}</h2><p>{}</p></section>"#,
            escape_html(heading),
            escape_html(empty_message)
        )
    } else {
        let items = rows
            .iter()
            .map(|(url, title)| {
                let label = if title.trim().is_empty() { url } else { title };
                format!(
                    r#"<a class="row" href="{url}">
                        <span class="title">{title}</span>
                        <span class="url">{url}</span>
                    </a>"#,
                    url = escape_html(url),
                    title = escape_html(label)
                )
            })
            .collect::<String>();

        format!(
            r#"<section><h2>{}</h2><div class="list">{}</div></section>"#,
            escape_html(heading),
            items
        )
    };

    internal_page_shell(title, &body)
}

fn internal_page_shell(title: &str, body: &str) -> String {
    format!(
        r#"<!doctype html>
<html class="{theme}">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title}</title>
  <style>
    :root {{
    color-scheme: light dark;
    font-family: system-ui, sans-serif;
    --surface: #ffffff;
    --outline: #d9d9dc;
    --hover: #f2f2f5;
    --secondary: #54545d;
    background: #f6f6f8;
    color: #202026;
  }}
  :root.dark {{
    --surface: #232327;
    --outline: #3a3a41;
    --hover: #2e2e35;
    --secondary: #b5b5c0;
    background: #19191d;
    color: #f0f0f2;
  }}
  body {{
    margin: 0;
    padding: clamp(20px, 5vw, 48px);
  }}
  main {{
      max-width: 880px;
      margin: 0 auto;
    }}
    h1 {{
      margin: 0 0 28px;
      font-size: 28px;
      font-weight: 650;
    }}
    h2 {{
      margin: 0 0 16px;
      font-size: 16px;
      font-weight: 650;
      }}
    p {{
      margin: 0;
      color: var(--secondary);
    }}
    p + p {{
    margin-top: 12px;
  }}
  kbd {{
    padding: 2px 5px;
    border: 1px solid var(--outline);
    border-radius: 4px;
  }}
  .list {{
      display: grid;
      gap: 1px;
      overflow: hidden;
      border: 1px solid var(--outline);
      border-radius: 8px;
      background: var(--outline);
    }}
    .row {{
      display: grid;
      gap: 4px;
      padding: 12px 14px;
      color: inherit;
      text-decoration: none;
      background: var(--surface);
    }}
    .row:hover {{
      background: var(--hover);
    }}
    .title {{
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 14px;
    }}
    .url {{
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      color: var(--secondary);
      font-size: 12px;
    }}
    .empty {{
      padding: 32px;
      border: 1px solid var(--outline);
      border-radius: 8px;
      background: var(--surface);
    }}
  </style>
</head>
<body>
  <main>
    <h1>{title}</h1>
    {body}
  </main>
</body>
</html>"#,
        title = escape_html(title),
        theme = if adw::StyleManager::default().is_dark() {
            "dark"
        } else {
            "light"
        },
        body = body
    )
}

fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_steps_through_presets_and_stops_at_the_limits() {
        assert_eq!(next_zoom(1.0, true), 1.1);
        assert_eq!(next_zoom(1.0, false), 0.9);
        assert_eq!(next_zoom(1.17, true), 1.25);
        assert_eq!(next_zoom(1.17, false), 1.1);
        assert_eq!(next_zoom(3.0, true), 3.0);
        assert_eq!(next_zoom(0.3, false), 0.3);
        assert_eq!(next_zoom(5.0, false), 3.0);
    }

    #[test]
    fn only_attachment_dispositions_force_downloads() {
        assert!(is_attachment("attachment"));
        assert!(is_attachment(" Attachment "));
        assert!(!is_attachment("inline"));
        assert!(!is_attachment(""));
    }

    #[test]
    fn external_addresses_keep_supported_schemes_only() {
        assert_eq!(
            external_uri("HTTPS://Example.test/a?b#c").as_deref(),
            Some("https://example.test/a?b#c")
        );
        assert_eq!(
            external_uri("file:///tmp/page%20one.html").as_deref(),
            Some("file:///tmp/page%20one.html")
        );
        for rejected in [
            "javascript:alert(1)",
            "data:text/html,hi",
            "bitter://settings",
            "https://",
            "ftp://example.test/",
            "not a uri",
        ] {
            assert_eq!(external_uri(rejected), None, "{rejected}");
        }
    }

    #[test]
    fn distinguishes_search_urls_hosts_and_unsupported_explicit_schemes() {
        for engine in SearchEngine::ALL {
            assert_eq!(
                navigation_target(" português café ", engine),
                Ok(Some(Target::Search(engine.search_url("português café"))))
            );
            assert_eq!(
                navigation_target(" HTTPS://example.test/route ", engine),
                Ok(Some(Target::Address("https://example.test/route".into())))
            );
            assert_eq!(
                navigation_target("example.test:8443", engine),
                Ok(Some(Target::Address("https://example.test:8443/".into())))
            );
            assert_eq!(
                navigation_target("localhost:8080/path", engine),
                Ok(Some(Target::Address("https://localhost:8080/path".into())))
            );
            assert_eq!(
                navigation_target("[::1]:8080", engine),
                Ok(Some(Target::Address("https://[::1]:8080/".into())))
            );
            assert_eq!(
                navigation_target("bitter://settings", engine),
                Ok(Some(Target::Address("bitter://settings".into())))
            );
            assert_eq!(navigation_target("  ", engine), Ok(None));
            for invalid in [
                "https://",
                "https:///path",
                "custom:destination",
                "bitter://unknown",
                "example.test:99999",
            ] {
                assert!(navigation_target(invalid, engine).is_err(), "{invalid}");
            }
        }
    }

    #[test]
    fn closing_uses_the_nearest_tab_and_keeps_other_selections() {
        assert_eq!(selection_after_close(2, 0, 3), 1);
        assert_eq!(selection_after_close(1, 1, 3), 1);
        assert_eq!(selection_after_close(3, 3, 3), 2);
        assert_eq!(selection_after_close(1, 2, 3), 1);
        assert_eq!(selection_after_close(0, 0, 0), 0);
    }
}
