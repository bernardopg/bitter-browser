use bitter_core::session::SavedTab;
use gtk4::prelude::*;
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use webkit6::{NetworkSession, WebView};

pub struct Tab {
    pub id: String,
    saved: RefCell<SavedTab>,
    pub view: std::cell::OnceCell<TabView>,
}

pub struct TabView {
    pub container: gtk4::Stack,
    pub web: WebView,
    problem: adw::StatusPage,
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
    pub fn new(session: &NetworkSession, retry: impl Fn() + 'static) -> Self {
        let web = WebView::builder()
            .network_session(session)
            .hexpand(true)
            .vexpand(true)
            .build();
        let container = gtk4::Stack::new();
        let problem = adw::StatusPage::builder().css_classes(["compact"]).build();
        let button = gtk4::Button::builder()
            .label("Try Again")
            .halign(gtk4::Align::Center)
            .css_classes(["pill", "suggested-action"])
            .build();
        button.connect_clicked(move |_| retry());
        problem.set_child(Some(&button));
        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .child(&problem)
            .build();
        container.add_named(&web, Some("website"));
        container.add_named(&scroll, Some("problem"));
        Self {
            container,
            web,
            problem,
            failed: Cell::new(false),
            cancelled: Cell::new(false),
            certificate_failed: Cell::new(false),
            visited: Cell::new(false),
            requested: RefCell::new(String::new()),
        }
    }

    pub fn loading_started(&self) {
        self.failed.set(false);
        self.cancelled.set(false);
        self.certificate_failed.set(false);
        self.visited.set(false);
        self.container.set_visible_child_name("website");
    }

    pub fn show_problem(&self, title: &str, description: &str, icon: &str) {
        self.failed.set(true);
        self.problem.set_title(title);
        self.problem
            .set_description(Some(&gtk4::glib::markup_escape_text(description)));
        self.problem.set_icon_name(Some(icon));
        self.container.set_visible_child_name("problem");
    }
}
