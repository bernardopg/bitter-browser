use gtk4::prelude::*;

#[derive(Clone)]
pub struct Omnibar {
    entry: gtk4::Entry,
}

impl Omnibar {
    pub fn new() -> Self {
        let entry = gtk4::Entry::builder()
            .placeholder_text("Search or enter address")
            .width_request(400)
            .css_classes(vec!["omnibar"])
            .build();

        Self { entry }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.entry.upcast_ref()
    }

    pub fn set_text(&self, text: &str) {
        self.entry.set_text(text);
    }

    pub fn grab_focus(&self) {
        self.entry.grab_focus();
    }

    pub fn select_all(&self) {
        self.entry.select_region(0, -1);
    }

    pub fn connect_activate<F: Fn(String) + 'static>(&self, f: F) {
        self.entry.connect_activate(move |entry| {
            f(entry.text().to_string());
        });
    }
}
