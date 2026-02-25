use gtk4::prelude::*;

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

    pub fn text(&self) -> String {
        self.entry.text().to_string()
    }

    pub fn connect_activate<F: Fn(&Self) + 'static>(&self, f: F) {
        // We need to clone the entry to pass it to the closure, but we can't easily pass `Self`.
        // For simplicity in this scaffold, we just connect to the entry's activate signal.
        let entry_clone = self.entry.clone();
        self.entry.connect_activate(move |_| {
            let omnibar = Self { entry: entry_clone.clone() };
            f(&omnibar);
        });
    }
}
