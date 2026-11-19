//! Download manager: saves files under safe, unique names in the download
//! directory, or where the user chooses, lists progress in a toolbar popover
//! and keeps finished downloads across restarts.

use crate::preferences::{self, PreferenceState};
use bitter_core::downloads::{safe_file_name, unique_destination};
use bitter_core::records::{DownloadList, DownloadOutcome, DownloadRecord};
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use webkit6::{Download, NetworkSession};

pub struct Downloads {
    button: gtk4::MenuButton,
    popover: gtk4::Popover,
    list: gtk4::ListBox,
    clear: gtk4::Button,
    directory: PathBuf,
    preferences: PreferenceState,
    history: RefCell<DownloadList>,
    reserved: RefCell<HashSet<PathBuf>>,
    rows: RefCell<Vec<Rc<Row>>>,
    active: Cell<usize>,
    feedback: adw::ToastOverlay,
    toast: RefCell<Option<adw::Toast>>,
}

struct Row {
    row: gtk4::ListBoxRow,
    icon: gtk4::Image,
    name: gtk4::Label,
    status: gtk4::Label,
    progress: gtk4::ProgressBar,
    cancel: gtk4::Button,
    folder: gtk4::Button,
    path: RefCell<Option<PathBuf>>,
    source: RefCell<String>,
    failed: Cell<bool>,
    finished: Cell<bool>,
}

/// The user's download directory: the XDG setting when present, otherwise
/// `~/Downloads`.
pub fn download_directory() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Downloads)
        .filter(|path| path.is_absolute() && path != &glib::home_dir())
        .unwrap_or_else(|| glib::home_dir().join("Downloads"))
}

impl Downloads {
    pub fn new(
        feedback: &adw::ToastOverlay,
        preferences: PreferenceState,
        list_path: PathBuf,
    ) -> Rc<Self> {
        let list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(["boxed-list", "download-list"])
            .build();
        let empty = adw::StatusPage::builder()
            .icon_name("folder-download-symbolic")
            .title("No downloads")
            .description("Files you download appear here")
            .css_classes(["compact", "downloads-empty"])
            .build();
        list.set_placeholder(Some(&empty));
        let scroll = gtk4::ScrolledWindow::builder()
            .hscrollbar_policy(gtk4::PolicyType::Never)
            .propagate_natural_height(true)
            .max_content_height(420)
            .child(&list)
            .build();
        let heading = gtk4::Label::builder()
            .label("Downloads")
            .xalign(0.0)
            .hexpand(true)
            .css_classes(["heading"])
            .build();
        let header_button = |icon: &str, tooltip: &str| {
            let button = gtk4::Button::builder()
                .icon_name(icon)
                .tooltip_text(tooltip)
                .css_classes(["flat", "circular"])
                .build();
            button.update_property(&[gtk4::accessible::Property::Label(tooltip)]);
            button
        };
        let clear = header_button("edit-clear-all-symbolic", "Clear finished downloads");
        let open_dir = header_button("folder-open-symbolic", "Open downloads folder");
        let header = gtk4::Box::builder().spacing(4).margin_start(6).build();
        header.append(&heading);
        header.append(&clear);
        header.append(&open_dir);
        let content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(10)
            .width_request(380)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .margin_end(6)
            .build();
        content.append(&header);
        content.append(&scroll);
        let popover = gtk4::Popover::builder()
            .child(&content)
            .css_classes(["downloads-popover"])
            .build();
        let button = gtk4::MenuButton::builder()
            .icon_name("folder-download-symbolic")
            .tooltip_text("Downloads (Ctrl+J)")
            .popover(&popover)
            .visible(false)
            .css_classes(["flat"])
            .build();
        button.update_property(&[gtk4::accessible::Property::Label("Downloads")]);

        let (history, error) = DownloadList::open(list_path);
        if let Some(error) = error {
            tracing::warn!("Could not read the download list: {error}");
        }
        let downloads = Rc::new(Self {
            button,
            popover,
            list,
            clear,
            directory: download_directory(),
            preferences,
            history: RefCell::new(history),
            reserved: Default::default(),
            rows: Default::default(),
            active: Cell::new(0),
            feedback: feedback.clone(),
            toast: Default::default(),
        });
        downloads.restore();
        let weak = Rc::downgrade(&downloads);
        downloads.list.connect_row_activated(move |_, activated| {
            if let Some(downloads) = weak.upgrade() {
                let row = downloads
                    .rows
                    .borrow()
                    .iter()
                    .find(|row| &row.row == activated)
                    .cloned();
                if let Some(row) = row {
                    downloads.open(&row, false);
                }
            }
        });

        let weak = Rc::downgrade(&downloads);
        open_dir.connect_clicked(move |_| {
            if let Some(downloads) = weak.upgrade() {
                downloads.popover.popdown();
                let _ = std::fs::create_dir_all(&downloads.directory);
                downloads.launch(&downloads.directory, false);
            }
        });
        let weak = Rc::downgrade(&downloads);
        downloads.clear.connect_clicked(move |_| {
            if let Some(downloads) = weak.upgrade() {
                downloads.clear_finished();
            }
        });
        downloads.refresh_button();
        downloads
    }

    pub fn widget(&self) -> &gtk4::Widget {
        self.button.upcast_ref()
    }

    pub fn show(&self) {
        self.button.set_visible(true);
        self.button.popup();
    }

    pub fn attach(self: &Rc<Self>, session: &NetworkSession) {
        let weak = Rc::downgrade(self);
        session.connect_download_started(move |_, download| {
            if let Some(downloads) = weak.upgrade() {
                downloads.track(download);
            }
        });
    }

    /// Shows the downloads saved by earlier runs.
    fn restore(self: &Rc<Self>) {
        let records = self.history.borrow().records().to_vec();
        for record in records.iter().rev() {
            let row = self.add_row();
            row.set_name(&record.path);
            *row.path.borrow_mut() = Some(record.path.clone());
            *row.source.borrow_mut() = record.source.clone();
            row.finished.set(true);
            let when = glib::DateTime::from_unix_local(record.finished)
                .ok()
                .and_then(|time| time.format("%e %b").ok())
                .map(|text| text.trim().to_owned())
                .unwrap_or_default();
            match record.outcome {
                DownloadOutcome::Completed if record.path.exists() => {
                    row.finish(format!("{} · {when}", glib::format_size(record.size)));
                    row.ready();
                }
                DownloadOutcome::Completed => row.finish(format!("Deleted · {when}")),
                DownloadOutcome::Failed => {
                    row.failed.set(true);
                    row.finish(format!("Failed · {when}"));
                }
                DownloadOutcome::Cancelled => {
                    row.failed.set(true);
                    row.finish(format!("Cancelled · {when}"));
                }
            }
        }
        if !records.is_empty() {
            self.button.set_visible(true);
        }
    }

    fn add_row(self: &Rc<Self>) -> Rc<Row> {
        let row = Rc::new(Row::new());
        self.list.prepend(&row.row);
        let weak = Rc::downgrade(self);
        let weak_row = Rc::downgrade(&row);
        row.folder.connect_clicked(move |_| {
            if let (Some(downloads), Some(row)) = (weak.upgrade(), weak_row.upgrade()) {
                downloads.open(&row, true);
            }
        });
        self.rows.borrow_mut().push(row.clone());
        row
    }

    /// Opens a finished download, or its folder.
    fn open(&self, row: &Row, folder: bool) {
        if !row.openable() {
            return;
        }
        if let Some(path) = row.path.borrow().as_ref() {
            self.popover.popdown();
            self.launch(path, folder);
        }
    }

    fn track(self: &Rc<Self>, download: &Download) {
        let row = self.add_row();
        if let Some(uri) = download.request().and_then(|request| request.uri()) {
            *row.source.borrow_mut() = uri.to_string();
        }
        self.active.set(self.active.get() + 1);
        // Reveal the button without opening the popover, which would take
        // keyboard focus away from the page.
        self.button.set_visible(true);
        self.refresh_button();

        let weak = Rc::downgrade(self);
        let weak_row = Rc::downgrade(&row);
        download.connect_decide_destination(move |download, suggested| {
            let (Some(downloads), Some(row)) = (weak.upgrade(), weak_row.upgrade()) else {
                return false;
            };
            if let Err(error) = std::fs::create_dir_all(&downloads.directory) {
                tracing::warn!("Could not create the download directory: {error}");
            }
            if preferences::current(&downloads.preferences).ask_download_location {
                downloads.choose_destination(download, &row, suggested);
                return true;
            }
            let path = unique_destination(&downloads.directory, suggested, |path| {
                downloads.reserved.borrow().contains(path)
            });
            downloads.start(download, &row, path, false)
        });

        let weak_row = Rc::downgrade(&row);
        download.connect_received_data(move |download, _| {
            if let Some(row) = weak_row.upgrade() {
                row.progressed(download);
            }
        });

        let weak = Rc::downgrade(self);
        let weak_row = Rc::downgrade(&row);
        download.connect_failed(move |download, error| {
            if let (Some(downloads), Some(row)) = (weak.upgrade(), weak_row.upgrade()) {
                row.failed.set(true);
                let cancelled = error.matches(webkit6::DownloadError::CancelledByUser);
                row.finish(if cancelled {
                    "Cancelled".to_owned()
                } else {
                    format!("Failed: {}", error.message())
                });
                let outcome = if cancelled {
                    DownloadOutcome::Cancelled
                } else {
                    tracing::warn!("Download failed: {error}");
                    downloads.toast(&format!("Download failed: {}", row.name.label()), None);
                    DownloadOutcome::Failed
                };
                downloads.remember(&row, download, outcome);
            }
        });

        let weak = Rc::downgrade(self);
        let weak_row = Rc::downgrade(&row);
        download.connect_finished(move |download| {
            let (Some(downloads), Some(row)) = (weak.upgrade(), weak_row.upgrade()) else {
                return;
            };
            row.finished.set(true);
            downloads
                .active
                .set(downloads.active.get().saturating_sub(1));
            downloads.refresh_button();
            if let Some(path) = row.path.borrow().as_ref() {
                downloads.reserved.borrow_mut().remove(path);
            }
            if row.failed.get() {
                return;
            }
            let size = glib::format_size(download.received_data_length());
            row.finish(format!("Completed · {size}"));
            row.ready();
            downloads.remember(&row, download, DownloadOutcome::Completed);
            let path = row.path.borrow().clone();
            if let Some(path) = path {
                tracing::info!("Download finished: {}", path.display());
                downloads.toast(&format!("Downloaded {}", row.name.label()), Some(path));
            }
        });

        let weak_download = download.downgrade();
        row.cancel.connect_clicked(move |_| {
            if let Some(download) = weak_download.upgrade() {
                download.cancel();
            }
        });
    }

    /// Asks for a destination. WebKit waits until one is set.
    fn choose_destination(self: &Rc<Self>, download: &Download, row: &Rc<Row>, suggested: &str) {
        let dialog = gtk4::FileDialog::builder()
            .title("Save File")
            .accept_label("Save")
            .modal(true)
            .initial_folder(&gio::File::for_path(&self.directory))
            .initial_name(safe_file_name(suggested))
            .build();
        let window = self.window();
        let weak = Rc::downgrade(self);
        let weak_row = Rc::downgrade(row);
        let download = download.clone();
        dialog.save(window.as_ref(), gio::Cancellable::NONE, move |result| {
            let (Some(downloads), Some(row)) = (weak.upgrade(), weak_row.upgrade()) else {
                download.cancel();
                return;
            };
            match result.ok().and_then(|file| file.path()) {
                // The dialog already confirmed replacing an existing file.
                Some(path) => {
                    if !downloads.start(&download, &row, path, true) {
                        download.cancel();
                    }
                }
                None => download.cancel(),
            }
        });
    }

    fn start(&self, download: &Download, row: &Row, path: PathBuf, overwrite: bool) -> bool {
        let Some(destination) = path.to_str() else {
            tracing::warn!("Download destinations must be UTF-8: {}", path.display());
            return false;
        };
        self.reserved.borrow_mut().insert(path.clone());
        row.set_name(&path);
        download.set_allow_overwrite(overwrite);
        download.set_destination(destination);
        *row.path.borrow_mut() = Some(path);
        true
    }

    fn remember(&self, row: &Row, download: &Download, outcome: DownloadOutcome) {
        let Some(path) = row.path.borrow().clone() else {
            return;
        };
        let record = DownloadRecord {
            path,
            source: row.source.borrow().clone(),
            size: download.received_data_length(),
            finished: glib::real_time() / 1_000_000,
            outcome,
        };
        if let Err(error) = self.history.borrow_mut().push(record) {
            tracing::warn!("Could not save the download list: {error}");
        }
    }

    pub fn clear_finished(&self) {
        let mut rows = self.rows.borrow_mut();
        rows.retain(|row| {
            if row.finished.get() {
                self.list.remove(&row.row);
                false
            } else {
                true
            }
        });
        if let Err(error) = self.history.borrow_mut().clear() {
            tracing::warn!("Could not clear the download list: {error}");
        }
        drop(rows);
        self.refresh_button();
    }

    fn refresh_button(&self) {
        let active = self.active.get() > 0;
        if active {
            self.button.add_css_class("downloads-active");
        } else {
            self.button.remove_css_class("downloads-active");
        }
        self.clear
            .set_sensitive(self.rows.borrow().iter().any(|row| row.finished.get()));
    }

    fn toast(self: &Rc<Self>, title: &str, open: Option<PathBuf>) {
        let toast = adw::Toast::new(title);
        if let Some(path) = open {
            toast.set_button_label(Some("Open"));
            let weak = Rc::downgrade(self);
            toast.connect_button_clicked(move |_| {
                if let Some(downloads) = weak.upgrade() {
                    downloads.launch(&path, false);
                }
            });
        }
        // Replace the previous download toast so queued toasts never go stale.
        if let Some(previous) = self.toast.replace(Some(toast.clone())) {
            previous.dismiss();
        }
        self.feedback.add_toast(toast);
    }

    fn window(&self) -> Option<gtk4::Window> {
        self.feedback
            .root()
            .and_then(|root| root.downcast::<gtk4::Window>().ok())
    }

    fn launch(&self, path: &Path, containing_folder: bool) {
        let launcher = gtk4::FileLauncher::new(Some(&gio::File::for_path(path)));
        let window = self.window();
        let report = |result: Result<(), glib::Error>| {
            if let Err(error) = result
                && !error.matches(gtk4::DialogError::Dismissed)
            {
                tracing::warn!("Could not open the download: {error}");
            }
        };
        if containing_folder {
            launcher.open_containing_folder(window.as_ref(), gio::Cancellable::NONE, report);
        } else {
            launcher.launch(window.as_ref(), gio::Cancellable::NONE, report);
        }
    }
}

impl Row {
    fn new() -> Self {
        let name = gtk4::Label::builder()
            .label("Starting download…")
            .xalign(0.0)
            .ellipsize(gtk4::pango::EllipsizeMode::Middle)
            .css_classes(["download-name"])
            .build();
        let status = gtk4::Label::builder()
            .xalign(0.0)
            .css_classes(["caption", "dim-label", "numeric"])
            .build();
        let progress = gtk4::ProgressBar::builder()
            .css_classes(["download-progress"])
            .build();
        let text = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(3)
            .hexpand(true)
            .valign(gtk4::Align::Center)
            .build();
        text.append(&name);
        text.append(&status);
        text.append(&progress);
        let icon = gtk4::Image::builder()
            .icon_name("folder-download-symbolic")
            .pixel_size(24)
            .css_classes(["download-icon"])
            .build();
        let button = |icon: &str, tooltip: &str, visible: bool| {
            let button = gtk4::Button::builder()
                .icon_name(icon)
                .tooltip_text(tooltip)
                .valign(gtk4::Align::Center)
                .css_classes(["flat", "circular"])
                .visible(visible)
                .build();
            button.update_property(&[gtk4::accessible::Property::Label(tooltip)]);
            button
        };
        let cancel = button("process-stop-symbolic", "Cancel download", true);
        let folder = button("folder-open-symbolic", "Show in folder", false);
        let layout = gtk4::Box::builder()
            .spacing(12)
            .margin_top(10)
            .margin_bottom(10)
            .margin_start(12)
            .margin_end(8)
            .build();
        layout.append(&icon);
        layout.append(&text);
        layout.append(&cancel);
        layout.append(&folder);
        let row = gtk4::ListBoxRow::builder()
            .activatable(false)
            .child(&layout)
            .css_classes(["download-row"])
            .build();
        Self {
            row,
            icon,
            name,
            status,
            progress,
            cancel,
            folder,
            path: Default::default(),
            source: Default::default(),
            failed: Cell::new(false),
            finished: Cell::new(false),
        }
    }

    fn set_name(&self, path: &Path) {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.name.set_label(&name);
        self.row.set_tooltip_text(Some(&path.display().to_string()));
        let (content_type, _) = gio::content_type_guess(Some(path), None);
        self.icon
            .set_from_gicon(&gio::content_type_get_symbolic_icon(&content_type));
    }

    fn progressed(&self, download: &Download) {
        let received = download.received_data_length();
        let total = download
            .response()
            .map_or(0, |response| response.content_length());
        let text = if total > 0 {
            format!(
                "{} of {}",
                glib::format_size(received),
                glib::format_size(total)
            )
        } else {
            glib::format_size(received).to_string()
        };
        self.status.set_label(&text);
        if total > 0 {
            self.progress.set_fraction(download.estimated_progress());
        } else {
            self.progress.pulse();
        }
    }

    /// A finished file on disk: the row opens it and shows its folder.
    fn ready(&self) {
        self.folder.set_visible(true);
        self.row.set_activatable(true);
        self.row.set_tooltip_text(
            self.path
                .borrow()
                .as_ref()
                .map(|path| format!("Open {}", path.display()))
                .as_deref(),
        );
    }

    fn openable(&self) -> bool {
        self.row.is_activatable()
    }

    fn finish(&self, status: String) {
        self.status.set_label(&status);
        self.progress.set_visible(false);
        self.cancel.set_visible(false);
        if self.failed.get() {
            self.row.add_css_class("download-failed");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::download_directory;

    #[test]
    fn download_directory_is_absolute() {
        let directory = download_directory();
        assert!(directory.is_absolute(), "{}", directory.display());
    }
}
