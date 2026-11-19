use super::{Browser, Target, WorkspaceId, navigation_target, preferences};
use crate::suggestion_worker::{Matches, ResultSet};
use gtk4::{glib, prelude::*};
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::rc::{Rc, Weak};
use std::time::Duration;

const DEBOUNCE: Duration = Duration::from_millis(90);
const MAX_RESULTS: usize = 8;

#[derive(PartialEq, Eq)]
enum Destination {
    Navigate(String),
    Switch(String),
}

#[derive(PartialEq, Eq)]
struct Suggestion {
    title: String,
    detail: String,
    icon: &'static str,
    destination: Destination,
}

struct Suggestions {
    browser: Weak<Browser>,
    popover: gtk4::Popover,
    list: gtk4::ListBox,
    scroll: gtk4::ScrolledWindow,
    rows: RefCell<Vec<Suggestion>>,
    generation: Cell<u64>,
    scheduled: RefCell<Option<glib::SourceId>>,
    pending: RefCell<Option<glib::JoinHandle<()>>>,
}

pub(super) fn connect(browser: &Rc<Browser>) {
    let omnibar = browser.toolbar.omnibar();
    let list = gtk4::ListBox::builder()
        .selection_mode(gtk4::SelectionMode::Single)
        .activate_on_single_click(true)
        .focusable(false)
        .build();
    let popover = gtk4::Popover::builder()
        .autohide(false)
        .has_arrow(false)
        .position(gtk4::PositionType::Bottom)
        .css_classes(["omnibar-suggestions"])
        .build();
    popover.set_parent(omnibar.widget());
    let scroll = gtk4::ScrolledWindow::builder()
        .width_request(300)
        .max_content_height(310)
        .propagate_natural_height(true)
        .hscrollbar_policy(gtk4::PolicyType::Never)
        .child(&list)
        .build();
    popover.set_child(Some(&scroll));
    let state = Rc::new(Suggestions {
        browser: Rc::downgrade(browser),
        popover,
        list,
        scroll,
        rows: Default::default(),
        generation: Cell::new(0),
        scheduled: Default::default(),
        pending: Default::default(),
    });

    let handlers = state.clone();
    omnibar.connect_changed(move |query| handlers.edit(query));
    let handlers = state.clone();
    omnibar.connect_focus_out(move || handlers.dismiss());
    let handlers = state.clone();
    omnibar.connect_activate(move |input| handlers.activate(&input));
    let handlers = state.clone();
    omnibar.connect_key_pressed(move |key, _| {
        use gtk4::gdk::Key;
        if !handlers.popover.is_visible() {
            return glib::Propagation::Proceed;
        }
        match key {
            Key::Down => handlers.move_selection(1),
            Key::Up => handlers.move_selection(-1),
            Key::Escape => handlers.dismiss(),
            _ => return glib::Propagation::Proceed,
        }
        glib::Propagation::Stop
    });
    let weak = Rc::downgrade(&state);
    state.list.connect_row_activated(move |_, row| {
        if let Some(state) = weak.upgrade() {
            state.activate_row(row.index() as usize);
        }
    });
}

impl Suggestions {
    fn invalidate(&self) {
        self.generation.set(self.generation.get().wrapping_add(1));
        if let Some(source) = self.scheduled.borrow_mut().take() {
            source.remove();
        }
        if let Some(pending) = self.pending.borrow_mut().take() {
            pending.abort();
        }
    }

    fn reset(&self) {
        self.invalidate();
        self.popover.popdown();
        self.rows.borrow_mut().clear();
        while let Some(child) = self.list.first_child() {
            self.list.remove(&child);
        }
    }

    fn dismiss(&self) {
        self.reset();
        if let Some(browser) = self.browser.upgrade()
            && let Some(tab) = browser.active()
        {
            browser.display_address(&tab);
        }
    }

    fn edit(self: &Rc<Self>, input: String) {
        self.invalidate();
        let Some(browser) = self.browser.upgrade() else {
            return;
        };
        let trimmed = input.trim();
        if trimmed.is_empty() || trimmed.starts_with("bitter://") {
            self.reset();
            return;
        }
        let generation = self.generation.get();
        self.render(local_results(&browser, trimmed, None));
        if trimmed.chars().take(2).count() < 2
            || trimmed.len() > 512
            || browser.suggestions.is_none()
        {
            return;
        }
        let weak = Rc::downgrade(self);
        let source = glib::timeout_add_local_once(DEBOUNCE, move || {
            let Some(state) = weak.upgrade() else { return };
            state.scheduled.borrow_mut().take();
            let Some(browser) = state.browser.upgrade() else {
                return;
            };
            let Some(worker) = &browser.suggestions else {
                return;
            };
            let reply = worker.submit(input.clone());
            let weak = Rc::downgrade(&state);
            let task = glib::spawn_future_local(async move {
                let Ok(result) = reply.await else { return };
                let Some(state) = weak.upgrade() else { return };
                let Some(browser) = state.browser.upgrade() else {
                    return;
                };
                let omnibar = browser.toolbar.omnibar();
                if generation == state.generation.get()
                    && omnibar.editing()
                    && omnibar.text() == input
                {
                    state.render(local_results(&browser, input.trim(), Some(result)));
                }
            });
            *state.pending.borrow_mut() = Some(task);
        });
        *self.scheduled.borrow_mut() = Some(source);
    }

    fn render(&self, next: Vec<Suggestion>) {
        if next.is_empty() {
            self.popover.popdown();
            self.rows.borrow_mut().clear();
            while let Some(child) = self.list.first_child() {
                self.list.remove(&child);
            }
            return;
        }
        let same = *self.rows.borrow() == next;
        if !same {
            let previous = self.list.selected_row().map(|row| row.index() as usize);
            let preferred = {
                let old = self.rows.borrow();
                previous
                    .and_then(|index| old.get(index))
                    .and_then(|selected| {
                        next.iter()
                            .position(|candidate| candidate.destination == selected.destination)
                    })
                    .unwrap_or(0)
            };
            while let Some(child) = self.list.first_child() {
                self.list.remove(&child);
            }
            for suggestion in &next {
                self.list.append(&build_row(suggestion));
            }
            *self.rows.borrow_mut() = next;
            self.list
                .select_row(self.list.row_at_index(preferred as i32).as_ref());
        }
        if let Some(browser) = self.browser.upgrade() {
            let width = browser.toolbar.omnibar().widget().width().clamp(260, 540);
            self.scroll.set_width_request(width);
        }
        if !self.popover.is_visible() {
            self.popover.popup();
        }
    }

    fn move_selection(&self, offset: i32) {
        let count = self.rows.borrow().len() as i32;
        if count == 0 {
            return;
        }
        let current = self.list.selected_row().map_or(0, |row| row.index());
        let next = (current + offset).rem_euclid(count);
        self.list.select_row(self.list.row_at_index(next).as_ref());
    }

    fn activate(&self, input: &str) {
        if self.popover.is_visible()
            && self
                .browser
                .upgrade()
                .is_some_and(|browser| browser.toolbar.omnibar().editing())
            && let Some(row) = self.list.selected_row()
        {
            self.activate_row(row.index() as usize);
            return;
        }
        self.reset();
        if let Some(browser) = self.browser.upgrade() {
            match navigation_target(input, preferences::current(&browser.preferences).search) {
                Ok(Some(Target::Address(uri) | Target::Search(uri))) => {
                    browser.navigate_active(&uri)
                }
                Ok(None) => {}
                Err(message) => browser.toast(message),
            }
        }
    }

    fn activate_row(&self, index: usize) {
        let destination = self
            .rows
            .borrow()
            .get(index)
            .map(|row| match &row.destination {
                Destination::Navigate(uri) => Destination::Navigate(uri.clone()),
                Destination::Switch(id) => Destination::Switch(id.clone()),
            });
        self.reset();
        if let Some(browser) = self.browser.upgrade() {
            match destination {
                Some(Destination::Navigate(uri)) => browser.navigate_active(&uri),
                Some(Destination::Switch(id)) => browser.select_id(&id),
                None => {}
            }
        }
    }
}

impl Drop for Suggestions {
    fn drop(&mut self) {
        if let Some(source) = self.scheduled.get_mut().take() {
            source.remove();
        }
        if let Some(pending) = self.pending.get_mut().take() {
            pending.abort();
        }
    }
}

fn local_results(browser: &Browser, input: &str, result: Option<ResultSet>) -> Vec<Suggestion> {
    let engine = preferences::current(&browser.preferences).search;
    let mut results = Vec::new();
    let query = input.to_lowercase();
    let direct = match navigation_target(input, engine) {
        Ok(Some(Target::Address(uri))) => {
            Some((format!("Open {input}"), "web-browser-symbolic", uri))
        }
        Ok(Some(Target::Search(uri))) => Some((
            format!("Search {} for “{input}”", engine.label()),
            "edit-find-symbolic",
            uri,
        )),
        _ => None,
    };
    if let Some((title, icon, target)) = direct {
        results.push(Suggestion {
            title,
            detail: target.clone(),
            icon,
            destination: Destination::Navigate(target),
        });
    }
    let workspace = browser.workspace.get();
    let all = browser.workspaces.borrow();
    let active_id = all[workspace.index()]
        .tabs
        .get(all[workspace.index()].selected)
        .map(|tab| tab.id.as_str());
    for candidate in [
        workspace,
        if workspace == WorkspaceId::Main {
            WorkspaceId::Second
        } else {
            WorkspaceId::Main
        },
    ] {
        let entries = all[candidate.index()].tabs.iter().filter_map(|tab| {
            if Some(tab.id.as_str()) == active_id {
                return None;
            }
            let uri = tab.uri();
            let title = tab.title();
            if !title.to_lowercase().contains(&query) && !uri.to_lowercase().contains(&query) {
                return None;
            }
            let workspace = if candidate == WorkspaceId::Main {
                "Main"
            } else {
                "Second"
            };
            Some(Suggestion {
                title: format!("Switch to {title}"),
                detail: format!("{workspace} · {uri}"),
                icon: "tab-new-symbolic",
                destination: Destination::Switch(tab.id.clone()),
            })
        });
        results.extend(entries.take(MAX_RESULTS.saturating_sub(results.len())));
    }
    for (title, target, icon) in [
        (
            "History",
            "bitter://history",
            "document-open-recent-symbolic",
        ),
        ("Bookmarks", "bitter://bookmarks", "user-bookmarks-symbolic"),
        ("Preferences", "bitter://settings", "emblem-system-symbolic"),
    ] {
        if title.to_lowercase().contains(&query) {
            results.push(Suggestion {
                title: title.to_owned(),
                detail: target.to_owned(),
                icon,
                destination: Destination::Navigate(target.to_owned()),
            });
        }
    }
    if let Some(result) = result {
        append_saved(&mut results, result.bookmarks, "user-bookmarks-symbolic");
        append_saved(
            &mut results,
            result.history,
            "document-open-recent-symbolic",
        );
    }
    results.truncate(MAX_RESULTS);
    results
}

fn append_saved(results: &mut Vec<Suggestion>, matches: Matches, icon: &'static str) {
    let mut seen: HashSet<String> = results
        .iter()
        .filter_map(|row| match &row.destination {
            Destination::Navigate(uri) => Some(uri.clone()),
            _ => None,
        })
        .collect();
    for (uri, title) in matches {
        if results.len() >= MAX_RESULTS {
            break;
        }
        if super::is_recordable_uri(&uri) && seen.insert(uri.clone()) {
            results.push(Suggestion {
                title: if title.trim().is_empty() {
                    uri.clone()
                } else {
                    title
                },
                detail: uri.clone(),
                icon,
                destination: Destination::Navigate(uri),
            });
        }
    }
}

fn build_row(suggestion: &Suggestion) -> gtk4::ListBoxRow {
    let row = gtk4::ListBoxRow::builder()
        .activatable(true)
        .focusable(false)
        .build();
    let container = gtk4::Box::builder()
        .spacing(10)
        .margin_start(10)
        .margin_end(10)
        .margin_top(6)
        .margin_bottom(6)
        .build();
    let icon = gtk4::Image::builder()
        .icon_name(suggestion.icon)
        .pixel_size(16)
        .build();
    let text = gtk4::Box::new(gtk4::Orientation::Vertical, 2);
    let title = label(&suggestion.title, "omnibar-suggestion-title");
    let detail = label(&suggestion.detail, "omnibar-suggestion-detail");
    text.append(&title);
    text.append(&detail);
    container.append(&icon);
    container.append(&text);
    row.set_child(Some(&container));
    row
}

fn label(value: &str, class: &str) -> gtk4::Label {
    gtk4::Label::builder()
        .label(value)
        .ellipsize(gtk4::pango::EllipsizeMode::End)
        .xalign(0.0)
        .hexpand(true)
        .css_classes([class])
        .build()
}
