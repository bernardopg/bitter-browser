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

    pub fn connect_changed<F: Fn(String) + 'static>(&self, f: F) {
        self.entry.connect_changed(move |entry| {
            f(entry.text().to_string());
        });
    }

    pub fn connect_focus_out<F: Fn() + 'static>(&self, f: F) {
        let focus_controller = gtk4::EventControllerFocus::new();
        focus_controller.connect_leave(move |_| {
            f();
        });
        self.entry.add_controller(focus_controller);
    }

    pub fn connect_key_pressed<
        F: Fn(gtk4::gdk::Key, gtk4::gdk::ModifierType) -> gtk4::glib::Propagation + 'static,
    >(
        &self,
        f: F,
    ) {
        let key_controller = gtk4::EventControllerKey::new();
        key_controller.connect_key_pressed(move |_, keyval, _, state| f(keyval, state));
        self.entry.add_controller(key_controller);
    }
}
