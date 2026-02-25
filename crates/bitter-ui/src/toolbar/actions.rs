use gtk4::prelude::*;

pub struct Actions {
    box_container: gtk4::Box,
    back_button: gtk4::Button,
    forward_button: gtk4::Button,
    reload_button: gtk4::Button,
}

impl Actions {
    pub fn new() -> Self {
        let box_container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .build();

        let back_button = gtk4::Button::builder()
            .icon_name("go-previous-symbolic")
            .tooltip_text("Back")
            .build();

        let forward_button = gtk4::Button::builder()
            .icon_name("go-next-symbolic")
            .tooltip_text("Forward")
            .build();

        let reload_button = gtk4::Button::builder()
            .icon_name("view-refresh-symbolic")
            .tooltip_text("Reload")
            .build();

        box_container.append(&back_button);
        box_container.append(&forward_button);
        box_container.append(&reload_button);

        Self {
            box_container,
            back_button,
            forward_button,
            reload_button,
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.box_container.upcast_ref()
    }
}
