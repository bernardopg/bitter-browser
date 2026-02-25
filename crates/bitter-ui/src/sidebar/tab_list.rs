use gtk4::prelude::*;
use crate::sidebar::tab_item::TabItem;

pub struct TabList {
    list_box: gtk4::ListBox,
}

impl TabList {
    pub fn new() -> Self {
        let list_box = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .css_classes(vec!["tab-list"])
            .build();

        Self { list_box }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.list_box.upcast_ref()
    }

    pub fn add_tab(&self, id: &str, title: &str) {
        let item = TabItem::new(id, title);
        self.list_box.append(item.widget());
    }
}
