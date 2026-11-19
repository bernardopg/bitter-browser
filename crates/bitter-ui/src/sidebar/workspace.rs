use bitter_core::session::WorkspaceId;
use gtk4::prelude::*;
use std::rc::Rc;

pub struct Workspace {
    container: gtk4::Box,
    buttons: [gtk4::ToggleButton; 2],
}

impl Workspace {
    pub fn new() -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .margin_start(12)
            .margin_end(12)
            .margin_top(10)
            .margin_bottom(10)
            .css_classes(["linked", "workspace-switcher"])
            .build();
        let buttons = ["Main", "Second"].map(|label| {
            let button = gtk4::ToggleButton::builder()
                .label(label)
                .hexpand(true)
                .build();
            container.append(&button);
            button
        });
        buttons[1].set_group(Some(&buttons[0]));
        buttons[0].set_active(true);
        Self { container, buttons }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    pub fn show(&self, id: WorkspaceId) {
        self.buttons[id.index()].set_active(true);
    }

    pub fn connect_selected(&self, callback: impl Fn(WorkspaceId) + 'static) {
        let callback = Rc::new(callback);
        for workspace in WorkspaceId::ALL {
            let callback = callback.clone();
            self.buttons[workspace.index()].connect_toggled(move |button| {
                if button.is_active() {
                    callback(workspace);
                }
            });
        }
    }
}
