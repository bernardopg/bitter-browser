pub mod tab_list;
pub mod tab_item;
pub mod workspace;

use gtk4::prelude::*;
use libadwaita as adw;

pub struct Sidebar {
    container: gtk4::Box,
    tab_list: tab_list::TabList,
    workspace: workspace::Workspace,
    new_tab_button: gtk4::Button,
}

impl Sidebar {
    pub fn new() -> Self {
        let container = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .width_request(220)
            .css_classes(vec!["sidebar"])
            .build();

        let workspace = workspace::Workspace::new();
        let tab_list = tab_list::TabList::new();
        
        let new_tab_button = gtk4::Button::builder()
            .label("New Tab")
            .icon_name("tab-new-symbolic")
            .margin_start(8)
            .margin_end(8)
            .margin_top(8)
            .margin_bottom(8)
            .build();

        let scrolled_window = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .vscrollbar_policy(gtk4::PolicyType::Automatic)
            .vexpand(true)
            .child(tab_list.widget())
            .build();

        container.append(workspace.widget());
        container.append(&gtk4::Separator::new(gtk4::Orientation::Horizontal));
        container.append(&new_tab_button);
        container.append(&scrolled_window);

        Self {
            container,
            tab_list,
            workspace,
            new_tab_button,
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.container.upcast_ref()
    }

    pub fn tab_list(&self) -> &tab_list::TabList {
        &self.tab_list
    }

    pub fn workspace(&self) -> &workspace::Workspace {
        &self.workspace
    }

    pub fn new_tab_button(&self) -> &gtk4::Button {
        &self.new_tab_button
    }
}
