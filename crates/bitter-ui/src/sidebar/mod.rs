pub mod tab_list;
pub mod tab_item;

use gtk4::prelude::*;
use libadwaita as adw;

pub struct Sidebar {
    container: gtk4::Box,
    tab_list: tab_list::TabList,
}

impl Sidebar {
    pub fn new() -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .width_request(220)
            .css_classes(vec!["sidebar"])
            .build();

        let tab_list = tab_list::TabList::new();
        container.append(tab_list.widget());

        Self {
            container,
            tab_list,
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    pub fn tab_list(&self) -> &tab_list::TabList {
        &self.tab_list
    }
}
