use gtk4::prelude::*;

pub struct TabItem {
    row: gtk4::ListBoxRow,
    id: String,
}

impl TabItem {
    pub fn new(id: &str, title: &str) -> Self {
        let row = gtk4::ListBoxRow::builder()
            .css_classes(vec!["tab-item"])
            .build();

        let box_container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(8)
            .margin_start(8)
            .margin_end(8)
            .margin_top(4)
            .margin_bottom(4)
            .build();

        let label = gtk4::Label::builder()
            .label(title)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .xalign(0.0)
            .hexpand(true)
            .build();

        box_container.append(&label);
        row.set_child(Some(&box_container));

        Self {
            row,
            id: id.to_string(),
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.row.upcast_ref()
    }

    pub fn id(&self) -> &str {
        &self.id
    }
}
