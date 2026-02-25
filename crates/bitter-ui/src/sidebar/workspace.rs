use gtk4::prelude::*;
use libadwaita as adw;
use std::rc::Rc;
use std::cell::RefCell;

pub struct Workspace {
    container: gtk4::Box,
    workspace_1: gtk4::Button,
    workspace_2: gtk4::Button,
    add_workspace: gtk4::Button,
}

impl Workspace {
    pub fn new() -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .spacing(4)
            .margin_start(8)
            .margin_end(8)
            .margin_top(8)
            .margin_bottom(8)
            .build();

        let workspace_1 = gtk4::Button::builder()
            .icon_name("folder-symbolic")
            .css_classes(vec!["flat", "circular"])
            .tooltip_text("Workspace 1")
            .build();

        let workspace_2 = gtk4::Button::builder()
            .icon_name("folder-symbolic")
            .css_classes(vec!["flat", "circular"])
            .tooltip_text("Workspace 2")
            .build();

        let add_workspace = gtk4::Button::builder()
            .icon_name("list-add-symbolic")
            .css_classes(vec!["flat", "circular"])
            .tooltip_text("New Workspace")
            .build();

        container.append(&workspace_1);
        container.append(&workspace_2);
        container.append(&add_workspace);

        Self {
            container,
            workspace_1,
            workspace_2,
            add_workspace,
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    pub fn connect_workspace_selected<F: Fn(&str) + 'static>(&self, f: F) {
        let f = Rc::new(f);
        
        let f1 = f.clone();
        self.workspace_1.connect_clicked(move |_| {
            f1("default");
        });

        let f2 = f.clone();
        self.workspace_2.connect_clicked(move |_| {
            f2("workspace_2");
        });
    }
}
