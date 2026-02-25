use gtk4::prelude::*;
use libadwaita as adw;
use crate::webview::tab::Tab;
use crate::sidebar::Sidebar;
use crate::toolbar::Toolbar;
use std::rc::Rc;
use std::cell::RefCell;

pub struct BitterWindow {
    window: adw::ApplicationWindow,
}

impl BitterWindow {
    pub fn new(app: &adw::Application) -> Self {
        let window = adw::ApplicationWindow::builder()
            .application(app)
            .title("Bitter Browser")
            .default_width(1200)
            .default_height(800)
            .build();

        let sidebar = Sidebar::new();
        let toolbar = Toolbar::new();

        let main_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Horizontal)
            .build();

        let content_box = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .hexpand(true)
            .vexpand(true)
            .build();

        let stack = gtk4::Stack::builder()
            .hexpand(true)
            .vexpand(true)
            .build();

        content_box.append(toolbar.widget());
        content_box.append(&stack);

        main_box.append(sidebar.widget());
        main_box.append(&content_box);

        window.set_content(Some(&main_box));

        let tabs: Rc<RefCell<Vec<Tab>>> = Rc::new(RefCell::new(Vec::new()));

        let initial_tab = Tab::new("https://duckduckgo.com");
        let initial_id = initial_tab.id().to_string();
        
        let tab_list = sidebar.tab_list().clone();
        
        // Handle tab closing
        let tabs_clone = tabs.clone();
        let stack_clone2 = stack.clone();
        let tab_list_clone = tab_list.clone();
        
        tab_list.connect_tab_closed(move |id| {
            let mut tabs = tabs_clone.borrow_mut();
            if let Some(index) = tabs.iter().position(|t| t.id() == id) {
                let tab = tabs.remove(index);
                stack_clone2.remove(tab.widget());
                tab_list_clone.remove_tab(id);
                
                // Select another tab in the same workspace if available
                let active_workspace = tab_list_clone.active_workspace();
                if let Some(next_tab_id) = tab_list_clone.first_tab_in_workspace(&active_workspace) {
                    tab_list_clone.select_tab(&next_tab_id);
                    stack_clone2.set_visible_child_name(&next_tab_id);
                } else {
                    // Create a new tab if the workspace is empty
                    let new_tab = Tab::new("https://duckduckgo.com");
                    let new_id = new_tab.id().to_string();
                    
                    tab_list_clone.add_tab(&new_id, "New Tab", &active_workspace);
                    stack_clone2.add_named(new_tab.widget(), Some(&new_id));
                    
                    let tab_list_clone_inner = tab_list_clone.clone();
                    let new_id_clone = new_id.clone();
                    new_tab.connect_title_changed(move |title| {
                        tab_list_clone_inner.set_tab_title(&new_id_clone, title);
                    });
                    
                    tabs.push(new_tab);
                    stack_clone2.set_visible_child_name(&new_id);
                    tab_list_clone.select_tab(&new_id);
                }
            }
        });

        // Add initial tab
        let active_workspace = tab_list.active_workspace();
        tab_list.add_tab(&initial_id, "DuckDuckGo", &active_workspace);
        stack.add_named(initial_tab.widget(), Some(&initial_id));
        
        let tab_list_clone3 = tab_list.clone();
        let initial_id_clone = initial_id.clone();
        initial_tab.connect_title_changed(move |title| {
            tab_list_clone3.set_tab_title(&initial_id_clone, title);
        });
        
        tabs.borrow_mut().push(initial_tab);
        stack.set_visible_child_name(&initial_id);
        tab_list.select_tab(&initial_id);

        // Handle tab selection
        let stack_clone = stack.clone();
        tab_list.connect_tab_selected(move |id| {
            stack_clone.set_visible_child_name(id);
        });

        // Handle new tab button
        let tabs_clone2 = tabs.clone();
        let stack_clone3 = stack.clone();
        let tab_list_clone2 = tab_list.clone();
        sidebar.new_tab_button().connect_clicked(move |_| {
            let new_tab = Tab::new("https://duckduckgo.com");
            let new_id = new_tab.id().to_string();
            let active_workspace = tab_list_clone2.active_workspace();
            
            tab_list_clone2.add_tab(&new_id, "New Tab", &active_workspace);
            stack_clone3.add_named(new_tab.widget(), Some(&new_id));
            
            let tab_list_clone4 = tab_list_clone2.clone();
            let new_id_clone = new_id.clone();
            new_tab.connect_title_changed(move |title| {
                tab_list_clone4.set_tab_title(&new_id_clone, title);
            });
            
            tabs_clone2.borrow_mut().push(new_tab);
            stack_clone3.set_visible_child_name(&new_id);
            tab_list_clone2.select_tab(&new_id);
        });

        // Handle workspace selection
        let tab_list_clone5 = tab_list.clone();
        let tabs_clone3 = tabs.clone();
        let stack_clone4 = stack.clone();
        sidebar.workspace().connect_workspace_selected(move |workspace_id| {
            tab_list_clone5.set_active_workspace(workspace_id);
            
            if let Some(first_tab_id) = tab_list_clone5.first_tab_in_workspace(workspace_id) {
                tab_list_clone5.select_tab(&first_tab_id);
                stack_clone4.set_visible_child_name(&first_tab_id);
            } else {
                // Create a new tab if the workspace is empty
                let new_tab = Tab::new("https://duckduckgo.com");
                let new_id = new_tab.id().to_string();
                
                tab_list_clone5.add_tab(&new_id, "New Tab", workspace_id);
                stack_clone4.add_named(new_tab.widget(), Some(&new_id));
                
                let tab_list_clone6 = tab_list_clone5.clone();
                let new_id_clone = new_id.clone();
                new_tab.connect_title_changed(move |title| {
                    tab_list_clone6.set_tab_title(&new_id_clone, title);
                });
                
                tabs_clone3.borrow_mut().push(new_tab);
                stack_clone4.set_visible_child_name(&new_id);
                tab_list_clone5.select_tab(&new_id);
            }
        });

        Self { window }
    }

    pub fn present(&self) {
        self.window.present();
    }
}
