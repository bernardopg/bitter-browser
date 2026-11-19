use super::permissions::PermissionBar;
use bitter_core::session::SavedTab;
use gtk4::prelude::*;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use webkit6::{NetworkSession, UserContentManager, WebView};

/// The problem page's second action, run at most once.
type Alternative = Rc<RefCell<Option<Box<dyn Fn()>>>>;

/// How a new page view gets its engine state.
pub enum Engine<'a> {
    Session(&'a NetworkSession),
    /// A page opened by another one with `window.open`, sharing its process
    /// so the two can script each other.
    Related(&'a WebView),
}

pub struct Tab {
    pub id: String,
    saved: RefCell<SavedTab>,
    pub view: std::cell::OnceCell<TabView>,
}

pub struct TabView {
    /// The widget shown in the browser's page stack.
    pub root: gtk4::Overlay,
    pages: gtk4::Stack,
    pub web: WebView,
    /// This page's content filters, so blocking can differ per site.
    pub content: UserContentManager,
    pub permissions: Rc<PermissionBar>,
    link_status: gtk4::Label,
    problem: adw::StatusPage,
    problem_action: gtk4::Button,
    alternative: Alternative,
    /// The `http:` address an HTTPS-only upgrade replaced, while loading it.
    pub upgraded_from: RefCell<Option<String>>,
    /// The tab that opened this one with `window.open`.
    pub opener: RefCell<Option<String>>,
    /// The filter revision and blocking state last given to `content`.
    pub filter_state: Cell<Option<(u64, bool)>>,
    pub failed: Cell<bool>,
    pub cancelled: Cell<bool>,
    pub certificate_failed: Cell<bool>,
    pub visited: Cell<bool>,
    pub requested: RefCell<String>,
}

impl Tab {
    pub fn new(saved: SavedTab) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            saved: RefCell::new(saved),
            view: Default::default(),
        }
    }

    pub fn saved(&self) -> SavedTab {
        self.saved.borrow().clone()
    }

    pub fn update(&self, uri: &str, title: &str) {
        // The current tab may display a file even though snapshots exclude it.
        *self.saved.borrow_mut() = SavedTab {
            uri: uri.to_owned(),
            title: title.to_owned(),
        };
    }

    pub fn uri(&self) -> String {
        self.saved.borrow().uri.clone()
    }

    pub fn title(&self) -> String {
        self.saved.borrow().title.clone()
    }
}

impl TabView {
    pub fn new(engine: Engine, settings: &webkit6::Settings, retry: impl Fn() + 'static) -> Self {
        let content = UserContentManager::new();
        let builder = WebView::builder()
            .settings(settings)
            .user_content_manager(&content)
            .hexpand(true)
            .vexpand(true);
        let web = match engine {
            Engine::Session(session) => builder.network_session(session).build(),
            Engine::Related(opener) => builder.related_view(opener).build(),
        };
        let pages = gtk4::Stack::new();
        let problem = adw::StatusPage::builder().css_classes(["compact"]).build();
        let button = gtk4::Button::builder()
            .label("Try Again")
            .halign(gtk4::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        button.connect_clicked(move |_| retry());
        let problem_action = gtk4::Button::builder()
            .halign(gtk4::Align::Center)
            .visible(false)
            .css_classes(["pill"])
            .build();
        let actions = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(12)
            .halign(gtk4::Align::Center)
            .build();
        actions.append(&button);
        actions.append(&problem_action);
        problem.set_child(Some(&actions));
        let alternative: Alternative = Rc::default();
        let pending = alternative.clone();
        problem_action.connect_clicked(move |_| {
            let action = pending.borrow_mut().take();
            if let Some(action) = action {
                action();
            }
        });
        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&problem)
            .build();
        pages.add_named(&web, Some("website"));
        pages.add_named(&scroll, Some("problem"));
        let permissions = PermissionBar::new();
        let column = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        column.append(&permissions.revealer);
        column.append(&pages);
        let link_status = gtk4::Label::builder()
            .halign(gtk4::Align::Start)
            .valign(gtk4::Align::End)
            .ellipsize(gtk4::pango::EllipsizeMode::Middle)
            .max_width_chars(90)
            .can_target(false)
            .visible(false)
            .css_classes(["link-status"])
            .build();
        let root = gtk4::Overlay::builder().child(&column).build();
        root.add_overlay(&link_status);
        // WebKit reports no target when the pointer leaves the page.
        let motion = gtk4::EventControllerMotion::new();
        let weak_status = link_status.downgrade();
        motion.connect_leave(move |_| {
            if let Some(status) = weak_status.upgrade() {
                status.set_visible(false);
            }
        });
        web.add_controller(motion);
        Self {
            root,
            pages,
            web,
            content,
            permissions,
            link_status,
            problem,
            problem_action,
            alternative,
            upgraded_from: RefCell::default(),
            opener: RefCell::default(),
            filter_state: Cell::new(None),
            failed: Cell::new(false),
            cancelled: Cell::new(false),
            certificate_failed: Cell::new(false),
            visited: Cell::new(false),
            requested: RefCell::new(String::new()),
        }
    }

    /// Shows where a hovered link goes, or hides the address.
    pub fn show_link(&self, uri: Option<&str>) {
        match uri {
            Some(uri) if !uri.is_empty() => {
                self.link_status.set_label(uri);
                self.link_status.set_visible(true);
            }
            _ => self.link_status.set_visible(false),
        }
    }

    /// Adds a second button to the problem page, such as opening a site
    /// over HTTP. Cleared by the next problem or load.
    pub fn set_alternative(&self, label: &str, action: impl Fn() + 'static) {
        self.problem_action.set_label(label);
        self.problem_action.set_visible(true);
        self.alternative.replace(Some(Box::new(action)));
    }

    pub fn loading_started(&self) {
        self.failed.set(false);
        self.cancelled.set(false);
        self.certificate_failed.set(false);
        self.visited.set(false);
        self.clear_alternative();
        self.show_link(None);
        self.pages.set_visible_child_name("website");
    }

    fn clear_alternative(&self) {
        self.problem_action.set_visible(false);
        self.alternative.replace(None);
    }

    pub fn show_problem(&self, title: &str, description: &str, icon: &str) {
        self.failed.set(true);
        self.clear_alternative();
        self.show_link(None);
        self.problem.set_title(title);
        self.problem
            .set_description(Some(&gtk4::glib::markup_escape_text(description)));
        self.problem.set_icon_name(Some(icon));
        self.pages.set_visible_child_name("problem");
    }
}
