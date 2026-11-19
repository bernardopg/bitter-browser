use gtk4::prelude::*;
use std::{cell::Cell, rc::Rc};

#[derive(Clone)]
pub struct Omnibar {
    entry: gtk4::Entry,
    updating: Rc<Cell<bool>>,
    editing: Rc<Cell<bool>>,
}

impl Omnibar {
    pub fn new() -> Self {
        let entry = gtk4::Entry::builder()
            .placeholder_text("Search or enter address")
            .width_chars(24)
            .max_width_chars(45)
            .hexpand(true)
            .css_classes(vec!["omnibar"])
            .build();

        entry.update_property(&[gtk4::accessible::Property::Label("Address and search")]);
        Self {
            entry,
            updating: Rc::new(Cell::new(false)),
            editing: Rc::new(Cell::new(false)),
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.entry.upcast_ref()
    }

    pub fn set_text(&self, text: &str) {
        self.editing.set(false);
        if self.entry.text() != text {
            self.updating.set(true);
            self.entry.set_text(text);
            self.updating.set(false);
        }
    }

    pub fn editing(&self) -> bool {
        self.editing.get()
    }

    pub fn text(&self) -> gtk4::glib::GString {
        self.entry.text()
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
        let updating = self.updating.clone();
        let editing = self.editing.clone();
        self.entry.connect_changed(move |entry| {
            if !updating.get() {
                editing.set(true);
                f(entry.text().to_string());
            }
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
        key_controller.set_propagation_phase(gtk4::PropagationPhase::Capture);
        self.entry.add_controller(key_controller);
    }
}
