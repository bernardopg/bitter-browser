use crate::{preferences, window::BitterWindow};
use bitter_core::{bookmarks::Bookmarks, config::ConfigStore, history::History, paths::Profile};
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::{Cell, OnceCell, RefCell};
use std::rc::Rc;

pub struct BitterApp {
    app: adw::Application,
    startup_failed: Rc<Cell<bool>>,
}

impl BitterApp {
    pub fn new() -> Self {
        let app = adw::Application::builder()
            .application_id("com.bitterbrowser.Browser")
            .flags(gio::ApplicationFlags::HANDLES_OPEN)
            .build();
        let startup_failed = Rc::new(Cell::new(false));

        app.connect_startup(|_| crate::style::load());
        app.set_accels_for_action("win.preferences", &["<Control>comma"]);
        app.set_accels_for_action("app.quit", &["<Control>q"]);
        let quit = gio::SimpleAction::new("quit", None);
        let app_ref = app.downgrade();
        quit.connect_activate(move |_, _| {
            if let Some(app) = app_ref.upgrade() {
                for window in app.windows() {
                    window.close();
                }
                app.quit();
            }
        });
        app.add_action(&quit);

        let failure = startup_failed.clone();
        let profile = OnceCell::new();
        app.connect_activate(move |app| {
            if let Some(window) = app.active_window() {
                window.present();
                return;
            }
            let launch = (|| {
                let profile = profile.get_or_init(Profile::open);
                let profile = profile
                    .as_ref()
                    .map_err(|error| ("Browser profile unavailable", error.to_string()))?;
                let history = History::open(profile.data_dir()).map_err(database_issue)?;
                let bookmarks = Bookmarks::open(profile.data_dir()).map_err(database_issue)?;
                let config = Rc::new(RefCell::new(ConfigStore::open(profile.config_path())));
                preferences::apply_theme(preferences::current(&config).theme);
                BitterWindow::new(app, profile, history, bookmarks, config).present();
                Ok::<(), (&str, String)>(())
            })();
            if let Err((heading, detail)) = launch {
                failure.set(true);
                tracing::error!("{heading}: {detail}");
                startup_error(app, heading, &detail);
            }
        });

        app.connect_open(|app, files, _| {
            app.activate();
            let Some(window) = app.active_window() else {
                return;
            };
            for file in files {
                let uri = file.uri();
                if let Err(error) = window.activate_action("win.open_uri", Some(&uri.to_variant()))
                {
                    tracing::warn!("Could not open {uri}: {error}");
                }
            }
        });

        Self {
            app,
            startup_failed,
        }
    }

    pub fn run(&self) -> glib::ExitCode {
        let code = self.app.run();
        if self.startup_failed.get() {
            glib::ExitCode::FAILURE
        } else {
            code
        }
    }
}

fn database_issue(error: bitter_core::DatabaseError) -> (&'static str, String) {
    (
        "Browser data could not be opened",
        format!(
            "{error}\n\nCheck permissions and available disk space. If the file is damaged, back up the profile before trying to recover it."
        ),
    )
}

fn startup_error(app: &adw::Application, heading: &str, detail: &str) {
    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Bitter Browser")
        .default_width(560)
        .default_height(420)
        .build();
    let toolbar = adw::ToolbarView::new();
    toolbar.add_top_bar(&adw::HeaderBar::new());
    let page = adw::StatusPage::builder()
        .icon_name("dialog-warning-symbolic")
        .css_classes(["compact"])
        .title(heading)
        .description(glib::markup_escape_text(detail))
        .build();
    let close = gtk4::Button::builder()
        .label("Close Bitter Browser")
        .halign(gtk4::Align::Center)
        .css_classes(["pill"])
        .build();
    let weak_window = window.downgrade();
    close.connect_clicked(move |_| {
        if let Some(window) = weak_window.upgrade() {
            window.close();
        }
    });
    page.set_child(Some(&close));
    let scroll = gtk4::ScrolledWindow::builder()
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .child(&page)
        .build();
    toolbar.set_content(Some(&scroll));
    window.set_content(Some(&toolbar));
    window.present();
}
