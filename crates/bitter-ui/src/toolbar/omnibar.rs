use gtk4::prelude::*;
use std::{cell::Cell, rc::Rc};

/// What the address bar says about the current page's connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Security {
    /// A web page loaded over HTTPS without certificate errors.
    Secure,
    /// Plain HTTP, or HTTPS whose certificate failed.
    Insecure,
    /// Internal pages and local files.
    Local,
}

impl Security {
    pub fn for_address(uri: &str, certificate_failed: bool) -> Self {
        match uri
            .split_once(':')
            .map(|(scheme, _)| scheme.to_ascii_lowercase())
        {
            Some(scheme) if scheme == "https" && !certificate_failed => Self::Secure,
            Some(scheme) if scheme == "https" || scheme == "http" => Self::Insecure,
            _ => Self::Local,
        }
    }

    fn icon(self) -> (&'static str, &'static str) {
        match self {
            Self::Secure => ("channel-secure-symbolic", "Secure connection"),
            Self::Insecure => ("channel-insecure-symbolic", "Connection is not secure"),
            Self::Local => ("system-search-symbolic", "Search or enter address"),
        }
    }
}

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
            .primary_icon_name("system-search-symbolic")
            .primary_icon_activatable(false)
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

    pub fn set_security(&self, security: Security) {
        let (icon, tooltip) = security.icon();
        self.entry.set_primary_icon_name(Some(icon));
        self.entry.set_primary_icon_tooltip_text(Some(tooltip));
        for (class, active) in [
            ("secure", security == Security::Secure),
            ("insecure", security == Security::Insecure),
        ] {
            if active {
                self.entry.add_css_class(class);
            } else {
                self.entry.remove_css_class(class);
            }
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
                entry.set_primary_icon_name(Some("system-search-symbolic"));
                entry.remove_css_class("secure");
                entry.remove_css_class("insecure");
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

    /// Clicks on the connection icon at the start of the bar.
    pub fn connect_icon_pressed<F: Fn(&gtk4::Entry) + 'static>(&self, f: F) {
        self.entry.set_primary_icon_activatable(true);
        self.entry.connect_icon_press(move |entry, position| {
            if position == gtk4::EntryIconPosition::Primary {
                f(entry);
            }
        });
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

#[cfg(test)]
mod tests {
    use super::Security;

    #[test]
    fn security_follows_the_scheme_and_certificate_state() {
        assert_eq!(
            Security::for_address("https://a.test/", false),
            Security::Secure
        );
        assert_eq!(
            Security::for_address("HTTPS://a.test/", false),
            Security::Secure
        );
        assert_eq!(
            Security::for_address("https://a.test/", true),
            Security::Insecure
        );
        assert_eq!(
            Security::for_address("http://a.test/", false),
            Security::Insecure
        );
        assert_eq!(
            Security::for_address("bitter://history", false),
            Security::Local
        );
        assert_eq!(
            Security::for_address("file:///tmp/a", false),
            Security::Local
        );
        assert_eq!(Security::for_address("", false), Security::Local);
    }
}
