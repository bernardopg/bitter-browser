use gtk4::prelude::*;
use libadwaita as adw;
use crate::webview::tab::Tab;
use crate::sidebar::Sidebar;
use crate::toolbar::Toolbar;

pub struct BitterWindow {
    window: adw::ApplicationWindow,
}

impl BitterWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Bitter Browser")
            .default_width(1200)
            .default_height(800)
            .build();

        let sidebar = Sidebar::new();
        let tab = Tab::new("https://duckduckgo.com");

        sidebar.tab_list().add_tab(tab.id(), "DuckDuckGo");

        let main_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .build();

        let content_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        let toolbar = Toolbar::new();

        content_box.append(toolbar.widget());
        content_box.append(tab.widget());

        main_box.append(sidebar.widget());
        main_box.append(&content_box);

        window.set_content(Some(&main_box));

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}
