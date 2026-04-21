use crate::window::BitterWindow;
use gtk4::prelude::*;
use libadwaita as adw;

pub struct BitterApp {
    app: adw::Application,
}

impl BitterApp {
    pub fn new() -> Self {
        let app = adw::Application::builder()
            .application_id("com.bitterbrowser.Browser")
            .build();

        app.connect_startup(|_| {
            adw::init().expect("Failed to initialize libadwaita");
            adw::StyleManager::default().set_color_scheme(adw::ColorScheme::ForceDark);
            crate::style::load();
        });

        app.connect_activate(|app| {
            let window = BitterWindow::new(app);
            window.present();
        });

        Self { app }
    }

    pub fn run(&self) {
        self.app.run();
    }
}
