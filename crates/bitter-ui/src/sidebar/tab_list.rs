use gtk4::prelude::*;
use crate::sidebar::tab_item::TabItem;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Clone)]
pub struct TabList {
    list_box: gtk4::ListBox,
    items: Rc<RefCell<Vec<TabItem>>>,
    on_tab_closed: Rc<RefCell<Option<Rc<dyn Fn(&str)>>>>,
    active_workspace_id: Rc<RefCell<String>>,
}

impl TabList {
    pub fn new() -> Self {
        let list_box = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::Single)
            .css_classes(vec!["tab-list"])
            .build();

        let active_workspace_id = Rc::new(RefCell::new("default".to_string()));
        
        let items: Rc<RefCell<Vec<TabItem>>> = Rc::new(RefCell::new(Vec::new()));
        
        let items_clone = items.clone();
        let active_workspace_id_clone = active_workspace_id.clone();
        
        list_box.set_filter_func(move |row| {
            let items = items_clone.borrow();
            if let Some(item) = items.iter().find(|item| item.widget() == row.upcast_ref::<gtk4::Widget>()) {
                item.workspace_id() == *active_workspace_id_clone.borrow()
            } else {
                false
            }
        });

        Self {
            list_box,
            items,
            on_tab_closed: Rc::new(RefCell::new(None)),
            active_workspace_id,
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.list_box.upcast_ref()
    }

    pub fn add_tab(&self, id: &str, title: &str, workspace_id: &str) {
        let item = TabItem::new(id, title, workspace_id);
        
        if let Some(cb) = self.on_tab_closed.borrow().as_ref() {
            let cb = cb.clone();
            item.connect_close_clicked(move |id| {
                cb(id);
            });
        }

        self.list_box.append(item.widget());
        self.items.borrow_mut().push(item);
        self.list_box.invalidate_filter();
    }

    pub fn remove_tab(&self, id: &str) {
        let mut items = self.items.borrow_mut();
        if let Some(index) = items.iter().position(|item| item.id() == id) {
            let item = items.remove(index);
            self.list_box.remove(item.widget());
            self.list_box.invalidate_filter();
        }
    }

    pub fn set_active_workspace(&self, workspace_id: &str) {
        *self.active_workspace_id.borrow_mut() = workspace_id.to_string();
        self.list_box.invalidate_filter();
    }

    pub fn active_workspace(&self) -> String {
        self.active_workspace_id.borrow().clone()
    }

    pub fn first_tab_in_workspace(&self, workspace_id: &str) -> Option<String> {
        let items = self.items.borrow();
        items.iter().find(|item| item.workspace_id() == workspace_id).map(|item| item.id().to_string())
    }

    pub fn set_tab_title(&self, id: &str, title: &str) {
        let items = self.items.borrow();
        if let Some(item) = items.iter().find(|item| item.id() == id) {
            item.set_title(title);
        }
    }

    pub fn select_tab(&self, id: &str) {
        let items = self.items.borrow();
        if let Some(item) = items.iter().find(|item| item.id() == id) {
            self.list_box.select_row(Some(item.row()));
        }
    }

    pub fn connect_tab_selected<F: Fn(&str) + 'static>(&self, f: F) {
        let items = self.items.clone();
        self.list_box.connect_row_selected(move |_, row| {
            if let Some(row) = row {
                let items = items.borrow();
                if let Some(item) = items.iter().find(|item| item.widget() == row.upcast_ref::<gtk4::Widget>()) {
                    f(item.id());
                }
            }
        });
    }

    pub fn connect_tab_closed<F: Fn(&str) + 'static>(&self, f: F) {
        *self.on_tab_closed.borrow_mut() = Some(Rc::new(f));
    }
}
