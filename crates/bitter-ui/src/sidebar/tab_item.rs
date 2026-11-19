use gtk4::prelude::*;

const FALLBACK_ICON: &str = "web-browser-symbolic";

pub struct TabItem {
    row: gtk4::ListBoxRow,
    id: String,
    icon: gtk4::Image,
    spinner: gtk4::Spinner,
    close_button: gtk4::Button,
    label: gtk4::Label,
}

impl TabItem {
    pub fn new(id: &str, title: &str) -> Self {
        let row = gtk4::ListBoxRow::builder()
            .name(id)
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
            .icon_name(FALLBACK_ICON)
            .pixel_size(16)
            .build();

        let label = gtk4::Label::builder()
            .label(title)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .xalign(0.0)
            .max_width_chars(20)
            .hexpand(true)
            .build();

        let close_button = gtk4::Button::builder()
            .icon_name("window-close-symbolic")
            .css_classes(vec!["flat", "circular"])
            .valign(gtk4::Align::Center)
            .build();

        let spinner = gtk4::Spinner::new();
        spinner.set_visible(false);
        box_container.append(&favicon);
        box_container.append(&spinner);
        box_container.append(&label);
        box_container.append(&close_button);
        row.set_child(Some(&box_container));

        Self {
            row,
            id: id.to_string(),
            icon: favicon,
            spinner,
            close_button,
            label,
        }
    }

    pub fn row(&self) -> &gtk4::ListBoxRow {
        &self.row
    }

    pub fn update(&self, title: &str, loading: bool) {
        self.label.set_label(title);
        self.row.set_tooltip_text(Some(title));
        self.close_button
            .set_tooltip_text(Some(&format!("Close {title}")));
        self.icon.set_visible(!loading);
        self.spinner.set_visible(loading);
        self.spinner.set_spinning(loading);
    }

    /// Shows the site icon, or the generic page icon when there is none.
    pub fn set_favicon(&self, favicon: Option<&gtk4::gdk::Texture>) {
        match favicon {
            Some(texture) => self.icon.set_paintable(Some(texture)),
            None => self.icon.set_icon_name(Some(FALLBACK_ICON)),
        }
    }

    pub fn connect_close_clicked<F: Fn(&str) + 'static>(&self, f: F) {
        let id = self.id.clone();
        self.close_button.connect_clicked(move |_| {
            f(&id);
        });
    }
}
