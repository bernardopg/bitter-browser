//! Download manager: saves files to the user's download directory under safe,
//! unique names and lists progress in a toolbar popover.

use bitter_core::downloads::unique_destination;
use gtk4::{gio, glib, prelude::*};
use libadwaita as adw;
use std::cell::{Cell, RefCell};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use webkit6::{Download, NetworkSession};

pub struct Downloads {
    button: gtk4::MenuButton,
    popover: gtk4::Popover,
    list: gtk4::ListBox,
    directory: PathBuf,
    reserved: RefCell<HashSet<PathBuf>>,
    rows: RefCell<Vec<Rc<Row>>>,
    active: Cell<usize>,
    feedback: adw::ToastOverlay,
    toast: RefCell<Option<adw::Toast>>,
}

struct Row {
    row: gtk4::ListBoxRow,
    name: gtk4::Label,
    status: gtk4::Label,
    progress: gtk4::ProgressBar,
    cancel: gtk4::Button,
    open: gtk4::Button,
    folder: gtk4::Button,
    path: RefCell<Option<PathBuf>>,
    failed: Cell<bool>,
}

/// The user's download directory: the XDG setting when present, otherwise
/// `~/Downloads`.
pub fn download_directory() -> PathBuf {
    glib::user_special_dir(glib::UserDirectory::Downloads)
        .filter(|path| path.is_absolute() && path != &glib::home_dir())
        .unwrap_or_else(|| glib::home_dir().join("Downloads"))
}

impl Downloads {
    pub fn new(feedback: &adw::ToastOverlay) -> Rc<Self> {
        let list = gtk4::ListBox::builder()
            .selection_mode(gtk4::SelectionMode::None)
            .css_classes(["boxed-list"])
            .build();
        let empty = gtk4::Label::builder()
            .label("Downloads appear here")
            .css_classes(["dim-label"])
            .margin_top(18)
            .margin_bottom(18)
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
            .css_classes(["heading"])
            .build();
        let open_dir = gtk4::Button::builder()
            .icon_name("folder-open-symbolic")
            .tooltip_text("Open downloads folder")
            .css_classes(["flat"])
            .build();
        let header = gtk4::Box::new(gtk4::Orientation::Horizontal, 6);
        heading.set_hexpand(true);
        header.append(&heading);
        header.append(&open_dir);
        let content = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(10)
            .width_request(360)
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
            .build();
        button.update_property(&[gtk4::accessible::Property::Label("Downloads")]);
        let downloads = Rc::new(Self {
            button,
            popover,
            list,
            directory: download_directory(),
            reserved: Default::default(),
            rows: Default::default(),
            active: Cell::new(0),
            feedback: feedback.clone(),
            toast: Default::default(),
        });
        let weak = Rc::downgrade(&downloads);
        open_dir.connect_clicked(move |_| {
            if let Some(downloads) = weak.upgrade() {
                downloads.popover.popdown();
                let _ = std::fs::create_dir_all(&downloads.directory);
                downloads.launch(&downloads.directory, false);
            }
        });
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

    fn track(self: &Rc<Self>, download: &Download) {
        let row = Rc::new(Row::new());
        self.list.prepend(&row.row);
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
            let path = unique_destination(&downloads.directory, suggested, |path| {
                downloads.reserved.borrow().contains(path)
            });
            let Some(destination) = path.to_str() else {
                return false;
            };
            downloads.reserved.borrow_mut().insert(path.clone());
            row.set_name(&path);
            download.set_allow_overwrite(false);
            download.set_destination(destination);
            *row.path.borrow_mut() = Some(path);
            true
        });

        let weak_row = Rc::downgrade(&row);
        download.connect_received_data(move |download, _| {
            if let Some(row) = weak_row.upgrade() {
                row.progressed(download);
            }
        });

        let weak = Rc::downgrade(self);
        let weak_row = Rc::downgrade(&row);
        download.connect_failed(move |_, error| {
            if let (Some(downloads), Some(row)) = (weak.upgrade(), weak_row.upgrade()) {
                row.failed.set(true);
                let cancelled = error.matches(webkit6::DownloadError::CancelledByUser);
                row.finish(if cancelled {
                    "Cancelled".to_owned()
                } else {
                    format!("Failed: {}", error.message())
                });
                if !cancelled {
                    tracing::warn!("Download failed: {error}");
                    downloads.toast(&format!("Download failed: {}", row.name.label()), None);
                }
            }
        });

        let weak = Rc::downgrade(self);
        let weak_row = Rc::downgrade(&row);
        download.connect_finished(move |download| {
            let (Some(downloads), Some(row)) = (weak.upgrade(), weak_row.upgrade()) else {
                return;
            };
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
            row.open.set_visible(true);
            row.folder.set_visible(true);
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
        for (button, folder) in [(&row.open, false), (&row.folder, true)] {
            let weak = Rc::downgrade(self);
            let weak_row: Weak<Row> = Rc::downgrade(&row);
            button.connect_clicked(move |_| {
                if let (Some(downloads), Some(row)) = (weak.upgrade(), weak_row.upgrade())
                    && let Some(path) = row.path.borrow().as_ref()
                {
                    downloads.popover.popdown();
                    downloads.launch(path, folder);
                }
            });
        }
        self.rows.borrow_mut().push(row);
    }

    fn refresh_button(&self) {
        if self.active.get() > 0 {
            self.button.add_css_class("downloads-active");
        } else {
            self.button.remove_css_class("downloads-active");
        }
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

    fn launch(&self, path: &Path, containing_folder: bool) {
        let launcher = gtk4::FileLauncher::new(Some(&gio::File::for_path(path)));
        let window = self
            .button
            .root()
            .and_then(|root| root.downcast::<gtk4::Window>().ok());
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
            .build();
        let status = gtk4::Label::builder()
            .xalign(0.0)
            .css_classes(["caption", "dim-label"])
            .build();
        let progress = gtk4::ProgressBar::builder()
            .css_classes(["download-progress"])
            .build();
        let text = gtk4::Box::builder()
            .orientation(gtk4::Orientation::Vertical)
            .spacing(4)
            .hexpand(true)
            .valign(gtk4::Align::Center)
            .build();
        text.append(&name);
        text.append(&status);
        text.append(&progress);
        let icon = gtk4::Image::builder()
            .icon_name("folder-download-symbolic")
            .pixel_size(24)
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
        let open = button("document-open-symbolic", "Open file", false);
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
        layout.append(&open);
        layout.append(&folder);
        let row = gtk4::ListBoxRow::builder()
            .activatable(false)
            .child(&layout)
            .build();
        Self {
            row,
            name,
            status,
            progress,
            cancel,
            open,
            folder,
            path: Default::default(),
            failed: Cell::new(false),
        }
    }

    fn set_name(&self, path: &Path) {
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.name.set_label(&name);
        self.row.set_tooltip_text(Some(&path.display().to_string()));
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

    fn finish(&self, status: String) {
        self.status.set_label(&status);
        self.progress.set_visible(false);
        self.cancel.set_visible(false);
    }
}
