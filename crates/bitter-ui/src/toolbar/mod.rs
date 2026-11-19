pub mod actions;
pub mod omnibar;

use gtk4::prelude::*;
use libadwaita as adw;
use std::cell::Cell;

pub struct Toolbar {
    container: gtk4::Box,
    header: adw::HeaderBar,
    omnibar: omnibar::Omnibar,
    actions: actions::Actions,
    progress: gtk4::ProgressBar,
    navigation: Cell<Option<(bool, bool, bool)>>,
}

impl Toolbar {
    pub fn new() -> Self {
        let header = adw::HeaderBar::builder().build();
        let container = gtk4::Box::new(gtk4::Orientation::Vertical, 0);
        let actions = actions::Actions::new();
        let omnibar = omnibar::Omnibar::new();

        let sidebar = gtk4::Button::builder()
            .icon_name("sidebar-show-symbolic")
            .tooltip_text("Toggle sidebar (F9)")
            .action_name("win.toggle_sidebar")
            .build();
        sidebar.update_property(&[gtk4::accessible::Property::Label("Toggle sidebar")]);
        header.pack_start(&sidebar);
        header.pack_start(actions.widget());
        let preferences = gtk4::Button::builder()
            .icon_name("emblem-system-symbolic")
            .tooltip_text("Preferences (Ctrl+,)")
            .action_name("win.preferences")
            .build();
        preferences.update_property(&[gtk4::accessible::Property::Label("Preferences")]);
        header.pack_end(&preferences);
        header.set_title_widget(Some(omnibar.widget()));
        let progress = gtk4::ProgressBar::builder()
            .css_classes(["navigation-progress"])
            .build();
        progress.update_property(&[gtk4::accessible::Property::Label("Page loading progress")]);
        progress.update_state(&[gtk4::accessible::State::Hidden(true)]);
        container.append(&header);
        container.append(&progress);

        Self {
            container,
            header,
            omnibar,
            actions,
            progress,
            navigation: Cell::new(None),
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    /// Adds a widget to the end of the header bar, left of earlier ones.
    pub fn pack_end(&self, widget: &impl IsA<gtk4::Widget>) {
        self.header.pack_end(widget);
    }

    pub fn omnibar(&self) -> &omnibar::Omnibar {
        &self.omnibar
    }

    pub fn refresh(&self, back: bool, forward: bool, loading: bool, fraction: f64) {
        let state = (back, forward, loading);
        if self.navigation.replace(Some(state)) != Some(state) {
            self.actions.refresh(back, forward, loading);
            if loading {
                self.progress.add_css_class("loading");
            } else {
                self.progress.remove_css_class("loading");
            }
            self.progress
                .update_state(&[gtk4::accessible::State::Hidden(!loading)]);
        }
        if loading {
            self.progress.set_fraction(fraction);
        }
    }
}
