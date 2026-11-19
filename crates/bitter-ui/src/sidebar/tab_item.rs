use gtk4::prelude::*;

const FALLBACK_ICON: &[u8] = include_bytes!("../../data/icons/ui/tab-fallback.svg");

/// A neutral globe that reads well in light and dark schemes; icon themes'
/// generic web icons vary too much to stand in for a site.
fn fallback_icon() -> gtk4::gio::BytesIcon {
    gtk4::gio::BytesIcon::new(&gtk4::glib::Bytes::from_static(FALLBACK_ICON))
}

pub struct TabItem {
    row: gtk4::ListBoxRow,
    id: String,
    icon: gtk4::Image,
    spinner: libadwaita::Spinner,
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
            .spacing(10)
            .margin_start(10)
            .margin_end(4)
            .build();

        let favicon = gtk4::Image::builder()
            .gicon(&fallback_icon())
            .pixel_size(16)
            .css_classes(["tab-icon"])
            .build();

        let label = gtk4::Label::builder()
            .label(title)
            .ellipsize(gtk4::pango::EllipsizeMode::End)
            .xalign(0.0)
            .max_width_chars(20)
            .hexpand(true)
            .css_classes(["tab-title"])
            .build();

        let close_button = gtk4::Button::builder()
            .icon_name("window-close-symbolic")
            .css_classes(vec!["flat", "circular", "tab-close"])
            .valign(gtk4::Align::Center)
            .build();

        let spinner = libadwaita::Spinner::builder()
            .width_request(16)
            .height_request(16)
            .visible(false)
            .build();
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
    }

    /// Shows the site icon, or the generic page icon when there is none.
    pub fn set_favicon(&self, favicon: Option<&gtk4::gdk::Texture>) {
        match favicon {
            Some(texture) => self.icon.set_paintable(Some(texture)),
            None => self.icon.set_from_gicon(&fallback_icon()),
        }
    }

    pub fn connect_close_clicked<F: Fn(&str) + 'static>(&self, f: F) {
        let id = self.id.clone();
        self.close_button.connect_clicked(move |_| {
            f(&id);
        });
    }
}
