pub mod omnibar;
pub mod actions;

use gtk4::prelude::*;
use libadwaita as adw;

pub struct Toolbar {
    header_bar: adw::HeaderBar,
    omnibar: omnibar::Omnibar,
    actions: actions::Actions,
}

impl Toolbar {
    pub fn new() -> Self {
        let header_bar = adw::HeaderBar::builder()
            .show_end_title_buttons(true)
            .show_start_title_buttons(true)
            .build();

        let actions = actions::Actions::new();
        let omnibar = omnibar::Omnibar::new();

        header_bar.pack_start(actions.widget());
        header_bar.set_title_widget(Some(omnibar.widget()));

        Self {
            header_bar,
            omnibar,
            actions,
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.header_bar.upcast_ref()
    }

    pub fn omnibar(&self) -> &omnibar::Omnibar {
        &self.omnibar
    }

    pub fn actions(&self) -> &actions::Actions {
        &self.actions
    }
}
