use crate::sidebar::tab_item::TabItem;
use bitter_core::session::WorkspaceId;
use gtk4::prelude::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub struct TabList {
    stack: gtk4::Stack,
    lists: [gtk4::ListBox; 2],
    items: RefCell<HashMap<String, TabItem>>,
}

impl TabList {
    pub fn new() -> Self {
        let stack = gtk4::Stack::new();
        let lists = WorkspaceId::ALL.map(|workspace| {
            let list = gtk4::ListBox::builder().css_classes(["tab-list"]).build();
            stack.add_named(&list, Some(Self::name(workspace)));
            list
        });
        Self {
            stack,
            lists,
            items: RefCell::new(HashMap::new()),
        }
    }

    fn name(workspace: WorkspaceId) -> &'static str {
        match workspace {
            WorkspaceId::Main => "main",
            WorkspaceId::Second => "second",
        }
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.stack.upcast_ref()
    }

    pub fn insert(
        &self,
        id: &str,
        title: &str,
        workspace: WorkspaceId,
        index: usize,
        close: impl Fn(&str) + 'static,
    ) {
        let item = TabItem::new(id, title);
        item.connect_close_clicked(close);
        self.lists[workspace.index()].insert(item.row(), index as i32);
        self.items.borrow_mut().insert(id.to_owned(), item);
    }

    pub fn remove(&self, workspace: WorkspaceId, id: &str) {
        let item = self.items.borrow_mut().remove(id);
        if let Some(item) = item {
            self.lists[workspace.index()].remove(item.row());
        }
    }

    pub fn move_to(&self, workspace: WorkspaceId, id: &str, index: usize) {
        let row = self.items.borrow().get(id).map(|item| item.row().clone());
        if let Some(row) = row {
            let list = &self.lists[workspace.index()];
            list.remove(&row);
            list.insert(&row, index as i32);
        }
    }

    pub fn show(&self, workspace: WorkspaceId, id: &str) {
        let row = self.items.borrow().get(id).map(|item| item.row().clone());
        self.stack.set_visible_child_name(Self::name(workspace));
        if let Some(row) = row {
            self.lists[workspace.index()].select_row(Some(&row));
        }
    }

    pub fn update(&self, id: &str, title: &str, loading: bool) {
        if let Some(item) = self.items.borrow().get(id) {
            item.update(title, loading);
        }
    }

    pub fn set_favicon(&self, id: &str, favicon: Option<&gtk4::gdk::Texture>) {
        if let Some(item) = self.items.borrow().get(id) {
            item.set_favicon(favicon);
        }
    }

    pub fn connect_selected(&self, callback: impl Fn(&str) + 'static) {
        let callback = Rc::new(callback);
        for list in &self.lists {
            let callback = callback.clone();
            list.connect_row_selected(move |_, row| {
                if let Some(row) = row {
                    callback(&row.widget_name());
                }
            });
        }
    }
}
