//! Ad and tracker blocking with WebKit content filters.
//!
//! A small list compiled into the browser works immediately. EasyList and
//! EasyPrivacy are downloaded into the profile, converted, compiled by WebKit
//! and refreshed every few days. Compiled filters are cached by WebKit, so a
//! restart only loads them.

use bitter_adblock::{BUILTIN_LIST, CONVERTER_VERSION, REMOTE_LISTS, RemoteList, Scope};
use gtk4::{gio, glib};
use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, SystemTime};
use webkit6::{UserContentFilter, UserContentFilterStore, UserContentManager};

/// How old a downloaded list may get before it is refreshed.
const REFRESH_AFTER: Duration = Duration::from_secs(4 * 24 * 60 * 60);
/// How often the browser checks whether lists need refreshing.
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);
/// Upper bound for a downloaded filter list before conversion. EasyList and
/// EasyPrivacy are a few megabytes; anything much larger is treated as invalid
/// or hostile input rather than read into memory.
const MAX_FILTER_LIST_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Off,
    /// Compiling filters; blocking may be incomplete.
    Preparing,
    Updating,
    /// Blocking with the built-in list and the maintained lists that loaded.
    Ready {
        lists: Vec<&'static str>,
        updated: Option<SystemTime>,
    },
    Failed(String),
}

impl Status {
    pub fn describe(&self) -> String {
        match self {
            Self::Off => "Off".into(),
            Self::Preparing => "Preparing filter lists…".into(),
            Self::Updating => "Updating filter lists…".into(),
            Self::Ready { lists, .. } if lists.is_empty() => {
                "Built-in list only. EasyList and EasyPrivacy will download when online.".into()
            }
            Self::Ready { lists, updated } => {
                let age = updated.and_then(|time| time.elapsed().ok()).map(|elapsed| {
                    match elapsed.as_secs() / 86_400 {
                        0 => "updated today".to_owned(),
                        1 => "updated yesterday".to_owned(),
                        days => format!("updated {days} days ago"),
                    }
                });
                let names = lists.join(" and ");
                match age {
                    Some(age) => format!("{names}, {age}"),
                    None => names,
                }
            }
            Self::Failed(error) => format!("Using saved lists. Update failed: {error}"),
        }
    }
}

pub struct ContentBlocker {
    store: UserContentFilterStore,
    lists: PathBuf,
    enabled: Cell<bool>,
    filters: RefCell<Vec<(String, UserContentFilter)>>,
    status: RefCell<Status>,
    downloads_pending: Cell<usize>,
    listeners: RefCell<Vec<Box<dyn Fn()>>>,
    generation: Cell<u64>,
    revision: Cell<u64>,
}

impl ContentBlocker {
    pub fn new(compiled: &Path, lists: PathBuf) -> Rc<Self> {
        let blocker = Rc::new(Self {
            store: UserContentFilterStore::new(&compiled.to_string_lossy()),
            lists,
            enabled: Cell::new(false),
            filters: RefCell::default(),
            status: RefCell::new(Status::Off),
            downloads_pending: Cell::new(0),
            listeners: RefCell::default(),
            generation: Cell::new(0),
            revision: Cell::new(0),
        });
        let weak = Rc::downgrade(&blocker);
        glib::timeout_add_local(CHECK_EVERY, move || {
            let Some(blocker) = weak.upgrade() else {
                return glib::ControlFlow::Break;
            };
            if blocker.enabled.get() {
                blocker.refresh_stale();
            }
            glib::ControlFlow::Continue
        });
        blocker
    }

    /// Called whenever the set of filters changes.
    pub fn connect_changed(&self, f: impl Fn() + 'static) {
        self.listeners.borrow_mut().push(Box::new(f));
    }

    pub fn status(&self) -> Status {
        self.status.borrow().clone()
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.get()
    }

    pub fn set_enabled(self: &Rc<Self>, enabled: bool) {
        if self.enabled.replace(enabled) == enabled {
            return;
        }
        self.generation.set(self.generation.get() + 1);
        if enabled {
            self.set_status(Status::Preparing);
            self.load_all();
        } else {
            self.filters.borrow_mut().clear();
            self.set_status(Status::Off);
            self.notify();
        }
    }

    /// Puts the current filters into a page's content manager. `blocked`
    /// is false for sites where the user turned blocking off.
    pub fn apply(&self, manager: &UserContentManager, blocked: bool) {
        manager.remove_all_filters();
        if blocked && self.enabled.get() {
            for (_, filter) in self.filters.borrow().iter() {
                manager.add_filter(filter);
            }
        }
    }

    /// Downloads the maintained lists again now.
    pub fn update_now(self: &Rc<Self>) {
        if self.enabled.get() {
            for list in &REMOTE_LISTS {
                self.download(list);
            }
        }
    }

    /// Changes whenever the filters do, so pages know to update theirs.
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }

    fn notify(&self) {
        self.revision.set(self.revision.get() + 1);
        for listener in self.listeners.borrow().iter() {
            listener();
        }
    }

    fn set_status(&self, status: Status) {
        *self.status.borrow_mut() = status;
    }

    fn compiled_id(list: &str) -> String {
        format!("{list}-v{CONVERTER_VERSION}")
    }

    /// Names the built-in list by its contents, so an update recompiles it.
    fn builtin_id() -> String {
        // FNV-1a: stable across builds, unlike the standard library hasher.
        let hash = BUILTIN_LIST
            .bytes()
            .fold(0xcbf2_9ce4_8422_2325_u64, |hash, byte| {
                (hash ^ u64::from(byte)).wrapping_mul(0x0100_0000_01b3)
            });
        format!("builtin-{hash:016x}-v{CONVERTER_VERSION}")
    }

    fn source_path(&self, list: &RemoteList) -> PathBuf {
        self.lists.join(format!("{}.txt", list.id))
    }

    fn load_all(self: &Rc<Self>) {
        let generation = self.generation.get();
        let builtin = Self::builtin_id();
        self.load_or_compile(builtin, None, generation);
        for list in &REMOTE_LISTS {
            let path = self.source_path(list);
            self.load_or_compile(Self::compiled_id(list.id), Some(path), generation);
        }
        self.remove_stale_filters();
        self.refresh_stale();
    }

    /// Loads a compiled filter, compiling its source when WebKit has none.
    fn load_or_compile(self: &Rc<Self>, id: String, source: Option<PathBuf>, generation: u64) {
        let weak = Rc::downgrade(self);
        let store_id = id.clone();
        self.store
            .load(&store_id, gio::Cancellable::NONE, move |result| {
                let Some(blocker) = weak.upgrade() else {
                    return;
                };
                if blocker.generation.get() != generation {
                    return;
                }
                match result {
                    Ok(filter) => blocker.install(id, filter),
                    Err(_) => match source {
                        None => blocker.compile(id, BUILTIN_LIST.to_owned(), generation),
                        Some(path) => match std::fs::read_to_string(&path) {
                            Ok(text) => blocker.compile(id, text, generation),
                            // Not downloaded yet; the refresh fetches it.
                            Err(_) => blocker.refresh_status(),
                        },
                    },
                }
            });
    }

    fn compile(self: &Rc<Self>, id: String, list: String, generation: u64) {
        let weak = Rc::downgrade(self);
        glib::spawn_future_local(async move {
            let Ok((full, network)) = gio::spawn_blocking(move || {
                let full = bitter_adblock::convert(&list, Scope::Full);
                let network = bitter_adblock::convert(&list, Scope::NetworkOnly);
                tracing::info!(
                    "Converted filter list: {} rules, {} filters skipped",
                    full.rules,
                    full.skipped
                );
                (full.json, network.json)
            })
            .await
            else {
                return;
            };
            let Some(blocker) = weak.upgrade() else {
                return;
            };
            blocker.save(id, full, Some(network), generation);
        });
    }

    /// Compiles `json`; when WebKit rejects a selector, retries with the
    /// network rules only so one bad filter cannot disable a whole list.
    fn save(self: &Rc<Self>, id: String, json: String, fallback: Option<String>, generation: u64) {
        let weak = Rc::downgrade(self);
        let bytes = glib::Bytes::from_owned(json.into_bytes());
        let store_id = id.clone();
        self.store
            .save(&store_id, &bytes, gio::Cancellable::NONE, move |result| {
                let Some(blocker) = weak.upgrade() else {
                    return;
                };
                if blocker.generation.get() != generation {
                    return;
                }
                match (result, fallback) {
                    (Ok(filter), _) => blocker.install(id, filter),
                    (Err(error), Some(fallback)) => {
                        tracing::warn!(
                            "Could not compile {id}, retrying without element hiding: {error}"
                        );
                        blocker.save(id, fallback, None, generation);
                    }
                    (Err(error), None) => {
                        tracing::warn!("Could not compile {id}: {error}");
                        blocker.set_status(Status::Failed(error.to_string()));
                    }
                }
            });
    }

    fn install(&self, id: String, filter: UserContentFilter) {
        {
            let mut filters = self.filters.borrow_mut();
            filters.retain(|(known, _)| *known != id);
            filters.push((id, filter));
        }
        self.refresh_status();
        self.notify();
    }

    fn refresh_status(&self) {
        if !self.enabled.get() || self.downloads_pending.get() > 0 {
            return;
        }
        if matches!(*self.status.borrow(), Status::Failed(_)) {
            return;
        }
        let filters = self.filters.borrow();
        let lists = REMOTE_LISTS
            .iter()
            .filter(|list| {
                filters
                    .iter()
                    .any(|(id, _)| *id == Self::compiled_id(list.id))
            })
            .map(|list| list.name)
            .collect();
        let updated = REMOTE_LISTS
            .iter()
            .filter_map(|list| modified(&self.source_path(list)))
            .min();
        self.set_status(Status::Ready { lists, updated });
    }

    /// Downloads lists that are missing or older than [`REFRESH_AFTER`].
    fn refresh_stale(self: &Rc<Self>) {
        for list in &REMOTE_LISTS {
            let fresh = modified(&self.source_path(list))
                .and_then(|time| time.elapsed().ok())
                .is_some_and(|age| age < REFRESH_AFTER);
            if !fresh {
                self.download(list);
            }
        }
    }

    fn download(self: &Rc<Self>, list: &'static RemoteList) {
        // Tests and offline builds block with the built-in list only.
        if std::env::var_os("BITTER_OFFLINE_FILTER_LISTS").is_some() {
            self.refresh_status();
            return;
        }
        if let Err(error) = std::fs::create_dir_all(&self.lists) {
            self.set_status(Status::Failed(error.to_string()));
            return;
        }
        // An ephemeral session keeps list updates out of the user's
        // downloads, cookies and cache.
        let session = webkit6::NetworkSession::new_ephemeral();
        let Some(download) = session.download_uri(list.url) else {
            return;
        };
        self.downloads_pending.set(self.downloads_pending.get() + 1);
        self.set_status(Status::Updating);
        let partial = self.lists.join(format!("{}.txt.part", list.id));
        download.set_allow_overwrite(true);
        let destination = partial.clone();
        download.connect_decide_destination(move |download, _| {
            download.set_destination(&destination.to_string_lossy());
            true
        });
        let failure: Rc<RefCell<Option<String>>> = Rc::default();
        let failed = failure.clone();
        download.connect_failed(move |_, error| {
            failed.replace(Some(error.to_string()));
        });
        let weak = Rc::downgrade(self);
        let generation = self.generation.get();
        download.connect_finished(move |_| {
            // Keep the session alive until the transfer ends.
            let _session = &session;
            let Some(blocker) = weak.upgrade() else {
                return;
            };
            blocker
                .downloads_pending
                .set(blocker.downloads_pending.get().saturating_sub(1));
            let outcome = match failure.take() {
                Some(error) => Err(error),
                None => blocker.accept_download(list, &partial),
            };
            let _ = std::fs::remove_file(&partial);
            match outcome {
                Ok(text) if blocker.generation.get() == generation => {
                    blocker.compile(Self::compiled_id(list.id), text, generation);
                }
                Ok(_) => {}
                Err(error) => {
                    tracing::warn!("Could not update {}: {error}", list.name);
                    blocker.set_status(Status::Failed(error));
                    if blocker.downloads_pending.get() == 0 {
                        blocker.notify();
                    }
                }
            }
        });
    }

    /// Replaces the saved list with a download that looks like a filter list.
    fn accept_download(&self, list: &RemoteList, partial: &Path) -> Result<String, String> {
        let size = std::fs::metadata(partial)
            .map_err(|error| error.to_string())?
            .len();
        if size > MAX_FILTER_LIST_BYTES {
            return Err(format!(
                "{} exceeded the {MAX_FILTER_LIST_BYTES} byte filter list limit",
                list.url
            ));
        }
        let text = std::fs::read_to_string(partial).map_err(|error| error.to_string())?;
        if !bitter_adblock::looks_like_filter_list(&text) {
            return Err(format!("{} did not return a filter list", list.url));
        }
        std::fs::rename(partial, self.source_path(list)).map_err(|error| error.to_string())?;
        if matches!(*self.status.borrow(), Status::Failed(_)) {
            self.set_status(Status::Updating);
        }
        Ok(text)
    }

    /// Drops filters compiled from older lists or by an older converter.
    fn remove_stale_filters(&self) {
        let mut stale: Vec<String> = (1..CONVERTER_VERSION)
            .flat_map(|version| {
                REMOTE_LISTS
                    .iter()
                    .map(move |list| format!("{}-v{version}", list.id))
            })
            .collect();
        let marker = self.lists.join("builtin-filter");
        let current = Self::builtin_id();
        if let Ok(previous) = std::fs::read_to_string(&marker)
            && previous.trim() != current
        {
            stale.push(previous.trim().to_owned());
        }
        if std::fs::create_dir_all(&self.lists).is_ok()
            && let Err(error) = std::fs::write(&marker, &current)
        {
            tracing::warn!("Could not record the built-in filter version: {error}");
        }
        for id in stale {
            self.store.remove(&id, gio::Cancellable::NONE, |_| {});
        }
    }
}

fn modified(path: &Path) -> Option<SystemTime> {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_describes_lists_and_their_age() {
        assert_eq!(Status::Off.describe(), "Off");
        let day = Duration::from_secs(86_400);
        let ready = |lists, age: Option<Duration>| Status::Ready {
            lists,
            updated: age.map(|age| SystemTime::now() - age),
        };
        assert_eq!(ready(vec!["EasyList"], None).describe(), "EasyList");
        assert_eq!(
            ready(vec!["EasyList", "EasyPrivacy"], Some(day * 3)).describe(),
            "EasyList and EasyPrivacy, updated 3 days ago"
        );
        assert_eq!(
            ready(vec!["EasyList"], Some(day * 2)).describe(),
            "EasyList, updated 2 days ago"
        );
        assert_eq!(
            ready(vec!["EasyList"], Some(Duration::from_secs(60))).describe(),
            "EasyList, updated today"
        );
        assert!(
            ready(vec![], None)
                .describe()
                .starts_with("Built-in list only")
        );
        assert!(
            Status::Failed("offline".into())
                .describe()
                .ends_with("offline")
        );
    }

    #[test]
    fn filter_ids_are_namespaced_and_deterministic() {
        assert_eq!(ContentBlocker::compiled_id("easylist"), "easylist-v1");
        assert_eq!(ContentBlocker::compiled_id("easyprivacy"), "easyprivacy-v1");
        let builtin = ContentBlocker::builtin_id();
        assert!(builtin.starts_with("builtin-"));
        assert!(builtin.ends_with("-v1"));
        assert_eq!(builtin.len(), "builtin-".len() + 16 + 3);
        assert!(builtin[8..24].chars().all(|ch| ch.is_ascii_hexdigit()));
    }

    #[test]
    fn accept_download_renames_validated_lists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("filter-lists")).unwrap();
        let blocker = ContentBlocker::new(
            &dir.path().join("compiled"),
            dir.path().join("filter-lists"),
        );
        let list = &REMOTE_LISTS[0];
        let partial = dir.path().join("easylist.txt.part");
        let mut text = String::from("[Adblock Plus 2.0]\n! comment\n");
        for index in 0..12 {
            text.push_str(&format!("||rule{index}.test^\n"));
        }
        std::fs::write(&partial, &text).unwrap();
        let text = blocker.accept_download(list, &partial).unwrap();
        assert!(text.starts_with("[Adblock Plus 2.0]"));
        assert!(blocker.source_path(list).is_file());
        // Anything that is not a filter list is rejected.
        std::fs::write(&partial, "not a filter list").unwrap();
        assert!(blocker.accept_download(list, &partial).is_err());
        std::fs::write(&partial, "[Adblock Plus 2.0]\n||too-few.test^\n").unwrap();
        assert!(blocker.accept_download(list, &partial).is_err());
        // A missing file is an error, too.
        assert!(
            blocker
                .accept_download(list, &dir.path().join("missing.part"))
                .is_err()
        );
    }

    #[test]
    fn accept_download_rejects_oversized_lists() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("filter-lists")).unwrap();
        let blocker = ContentBlocker::new(
            &dir.path().join("compiled"),
            dir.path().join("filter-lists"),
        );
        let list = &REMOTE_LISTS[0];
        let partial = dir.path().join("easylist.txt.part");
        let file = std::fs::File::create(&partial).unwrap();
        file.set_len(MAX_FILTER_LIST_BYTES + 1).unwrap();
        let error = blocker.accept_download(list, &partial).unwrap_err();
        assert!(error.contains("filter list limit"));
        assert!(!blocker.source_path(list).exists());
    }
}
