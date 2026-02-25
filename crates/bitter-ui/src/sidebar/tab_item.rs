use gtk4::prelude::*;
use std::rc::Rc;
use std::cell::RefCell;

pub struct TabItem {
    row: gtk4::ListBoxRow,
    id: String,
    workspace_id: String,
    close_button: gtk4::Button,
    label: gtk4::Label,
}

impl TabItem {
    pub fn new(id: &str, title: &str, workspace_id: &str) -> Self {
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

        let favicon = gtk4::Image::builder()
            .icon_name("text-html-symbolic")
            .pixel_size(16)
            .build();

        let label = gtk4::Label::builder()
            .label(title)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .xalign(0.0)
            .hexpand(true)
            .build();

        let close_button = gtk4::Button::builder()
            .icon_name("window-close-symbolic")
            .css_classes(vec!["flat", "circular"])
            .valign(gtk4::Align::Center)
            .build();

        box_container.append(&favicon);
        box_container.append(&label);
        box_container.append(&close_button);
        row.set_child(Some(&box_container));

        Self {
            row,
            id: id.to_string(),
            workspace_id: workspace_id.to_string(),
            close_button,
            label,
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.row.upcast_ref()
    }

    pub fn row(&self) -> &gtk4::ListBoxRow {
        &self.row
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn workspace_id(&self) -> &str {
        &self.workspace_id
    }

    pub fn set_title(&self, title: &str) {
        self.label.set_label(title);
    }

    pub fn connect_close_clicked<F: Fn(&str) + 'static>(&self, f: F) {
        let id = self.id.clone();
        self.close_button.connect_clicked(move |_| {
            f(&id);
        });
    }
}
