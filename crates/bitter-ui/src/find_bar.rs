//! Find-in-page bar bound to the active tab's WebKit find controller.

use gtk4::{glib, prelude::*};
use std::cell::RefCell;
use std::rc::Rc;
use webkit6::{FindController, FindOptions};

const MAX_MATCHES: u32 = 1000;

pub struct FindBar {
    bar: gtk4::SearchBar,
    entry: gtk4::SearchEntry,
    matches: gtk4::Label,
    controller: RefCell<Option<FindController>>,
    handlers: RefCell<Vec<glib::SignalHandlerId>>,
}

impl FindBar {
    pub fn new() -> Rc<Self> {
        let entry = gtk4::SearchEntry::builder()
            .placeholder_text("Find in page")
            .width_chars(28)
            .build();
        entry.update_property(&[gtk4::accessible::Property::Label("Find in page")]);
        let matches = gtk4::Label::builder()
            .css_classes(["dim-label", "numeric"])
            .width_chars(12)
            .xalign(0.0)
            .build();
        let button = |icon: &str, tooltip: &str| {
            let button = gtk4::Button::builder()
                .icon_name(icon)
                .tooltip_text(tooltip)
                .css_classes(["flat"])
                .build();
            button.update_property(&[gtk4::accessible::Property::Label(tooltip)]);
            button
        };
        let previous = button("go-up-symbolic", "Previous match (Shift+Enter)");
        let next = button("go-down-symbolic", "Next match (Enter)");
        let navigation = gtk4::Box::builder().css_classes(["linked"]).build();
        navigation.append(&previous);
        navigation.append(&next);
        let content = gtk4::Box::builder().spacing(8).build();
        content.append(&entry);
        content.append(&navigation);
        content.append(&matches);
        let bar = gtk4::SearchBar::builder()
            .child(&content)
            .show_close_button(true)
            .css_classes(["find-bar"])
            .build();
        bar.connect_entry(&entry);

        let find = Rc::new(Self {
            bar,
            entry,
            matches,
            controller: Default::default(),
            handlers: Default::default(),
        });
        let weak = Rc::downgrade(&find);
        find.entry.connect_search_changed(move |_| {
            if let Some(find) = weak.upgrade() {
                find.search();
            }
        });
        let weak = Rc::downgrade(&find);
        find.entry.connect_activate(move |_| {
            if let Some(find) = weak.upgrade() {
                find.step(true);
            }
        });
        let keys = gtk4::EventControllerKey::new();
        let weak = Rc::downgrade(&find);
        keys.connect_key_pressed(move |_, key, _, modifiers| {
            let shift = modifiers.contains(gtk4::gdk::ModifierType::SHIFT_MASK);
            if matches!(key, gtk4::gdk::Key::Return | gtk4::gdk::Key::KP_Enter)
                && shift
                && let Some(find) = weak.upgrade()
            {
                find.step(false);
                return glib::Propagation::Stop;
            }
            glib::Propagation::Proceed
        });
        find.entry.add_controller(keys);
        for (button, forward) in [(&previous, false), (&next, true)] {
            let weak = Rc::downgrade(&find);
            button.connect_clicked(move |_| {
                if let Some(find) = weak.upgrade() {
                    find.step(forward);
                }
            });
        }
        let weak = Rc::downgrade(&find);
        find.bar.connect_search_mode_enabled_notify(move |bar| {
            if !bar.is_search_mode()
                && let Some(find) = weak.upgrade()
            {
                find.detach();
            }
        });
        find
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.bar.upcast_ref()
    }

    pub fn is_open(&self) -> bool {
        self.bar.is_search_mode()
    }

    /// Opens the bar for `controller`, keeping any text already entered.
    pub fn open(self: &Rc<Self>, controller: FindController) {
        if self.controller.borrow().as_ref() != Some(&controller) {
            self.detach();
            self.connect(&controller);
            *self.controller.borrow_mut() = Some(controller);
        }
        self.bar.set_search_mode(true);
        self.entry.grab_focus();
        self.entry.select_region(0, -1);
        self.search();
    }

    pub fn close(&self) {
        self.bar.set_search_mode(false);
    }

    pub fn step(&self, forward: bool) {
        let Some(controller) = self.controller.borrow().clone() else {
            return;
        };
        if self.entry.text().is_empty() {
            return;
        }
        if forward {
            controller.search_next();
        } else {
            controller.search_previous();
        }
    }

    fn connect(self: &Rc<Self>, controller: &FindController) {
        let mut handlers = self.handlers.borrow_mut();
        let weak = Rc::downgrade(self);
        handlers.push(controller.connect_counted_matches(move |_, count| {
            if let Some(find) = weak.upgrade() {
                find.show_count(count);
            }
        }));
        let weak = Rc::downgrade(self);
        handlers.push(controller.connect_failed_to_find_text(move |_| {
            if let Some(find) = weak.upgrade() {
                find.show_count(0);
            }
        }));
    }

    fn detach(&self) {
        if let Some(controller) = self.controller.take() {
            controller.search_finish();
            for handler in self.handlers.take() {
                controller.disconnect(handler);
            }
        }
        self.matches.set_label("");
        self.entry.remove_css_class("error");
    }

    fn search(&self) {
        let Some(controller) = self.controller.borrow().clone() else {
            return;
        };
        let text = self.entry.text();
        if text.is_empty() {
            controller.search_finish();
            self.matches.set_label("");
            self.entry.remove_css_class("error");
            return;
        }
        let options = (FindOptions::CASE_INSENSITIVE | FindOptions::WRAP_AROUND).bits();
        controller.count_matches(&text, options, MAX_MATCHES);
        controller.search(&text, options, MAX_MATCHES);
    }

    fn show_count(&self, count: u32) {
        self.matches.set_label(&match_label(count));
        if count == 0 {
            self.entry.add_css_class("error");
        } else {
            self.entry.remove_css_class("error");
        }
    }
}

fn match_label(count: u32) -> String {
    match count {
        0 => "No matches".to_owned(),
        1 => "1 match".to_owned(),
        MAX_MATCHES.. => format!("{MAX_MATCHES}+ matches"),
        count => format!("{count} matches"),
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_MATCHES, match_label};

    #[test]
    fn match_label_reports_counts_and_caps_at_limit() {
        assert_eq!(match_label(0), "No matches");
        assert_eq!(match_label(1), "1 match");
        assert_eq!(match_label(2), "2 matches");
        assert_eq!(match_label(MAX_MATCHES - 1), "999 matches");
        assert_eq!(match_label(MAX_MATCHES), "1000+ matches");
        assert_eq!(match_label(MAX_MATCHES + 5), "1000+ matches");
    }
}
