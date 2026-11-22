//! Small versioned JSON files in the profile: per-site zoom and the download
//! list. Files are read without following symlinks, size-capped and written
//! atomically, like the session snapshot.

use serde::{Deserialize, Serialize, de::DeserializeOwned};
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

const MAX_BYTES: usize = 1024 * 1024;
pub const MAX_ZOOM_SITES: usize = 500;
pub const MAX_DOWNLOADS: usize = 100;
const ZOOM_VERSION: u32 = 1;
const DOWNLOADS_VERSION: u32 = 1;

#[derive(Deserialize, Serialize)]
struct Envelope<T> {
    version: u32,
    #[serde(flatten)]
    data: T,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

fn load<T: DeserializeOwned + Default>(path: &Path, version: u32) -> io::Result<T> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(T::default()),
        Err(error) => return Err(error),
    };
    if !file.metadata()?.is_file() {
        return Err(invalid("expected a regular file"));
    }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(invalid("file is too large"));
    }
    let envelope: Envelope<T> = serde_json::from_slice(&bytes)?;
    if envelope.version != version {
        return Err(invalid("unsupported file version"));
    }
    Ok(envelope.data)
}

fn save<T: Serialize>(path: &Path, version: u32, data: &T) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing parent directory"))?;
    let directory = File::open(parent)?;
    let bytes = serde_json::to_vec(&Envelope { version, data })?;
    if bytes.len() > MAX_BYTES {
        return Err(invalid("file is too large"));
    }
    crate::config::stage(parent, ".record-", &bytes)?
        .persist(path)
        .map_err(|error| error.error)?;
    directory.sync_all()
}

/// Zoom levels by host, most recently changed first.
#[derive(Debug, Default)]
pub struct ZoomLevels {
    path: PathBuf,
    sites: Vec<(String, f64)>,
}

#[derive(Default, Deserialize, Serialize)]
struct ZoomData {
    sites: Vec<(String, f64)>,
}

impl ZoomLevels {
    /// Opens the file; an unreadable file is reported and replaced on the
    /// next change rather than blocking the browser.
    pub fn open(path: PathBuf) -> (Self, Option<io::Error>) {
        let (sites, error) = match load::<ZoomData>(&path, ZOOM_VERSION) {
            Ok(data) => (data.sites, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let sites = sites
            .into_iter()
            .filter(|(host, level)| valid_host(host) && valid_zoom(*level))
            .take(MAX_ZOOM_SITES)
            .collect();
        (Self { path, sites }, error)
    }

    pub fn get(&self, host: &str) -> Option<f64> {
        let host = host.to_ascii_lowercase();
        self.sites
            .iter()
            .find(|(site, _)| *site == host)
            .map(|(_, level)| *level)
    }

    /// Remembers `level` for `host`.
    pub fn set(&mut self, host: &str, level: f64) -> io::Result<()> {
        let host = host.to_ascii_lowercase();
        if !valid_host(&host) || !valid_zoom(level) {
            return Err(invalid("invalid zoom entry"));
        }
        self.sites.retain(|(site, _)| *site != host);
        self.sites.insert(0, (host, level));
        self.sites.truncate(MAX_ZOOM_SITES);
        self.write()
    }

    /// Forgets `host`, so it uses the default zoom again.
    pub fn forget(&mut self, host: &str) -> io::Result<()> {
        let host = host.to_ascii_lowercase();
        let before = self.sites.len();
        self.sites.retain(|(site, _)| *site != host);
        if self.sites.len() == before {
            return Ok(());
        }
        self.write()
    }

    fn write(&self) -> io::Result<()> {
        save(
            &self.path,
            ZOOM_VERSION,
            &ZoomData {
                sites: self.sites.clone(),
            },
        )
    }
}

fn valid_host(host: &str) -> bool {
    !host.is_empty() && host.len() <= 253 && !host.chars().any(|ch| ch.is_control() || ch == '/')
}

fn valid_zoom(level: f64) -> bool {
    level.is_finite() && (0.25..=5.0).contains(&level)
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DownloadOutcome {
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct DownloadRecord {
    pub path: PathBuf,
    pub source: String,
    pub size: u64,
    /// Seconds since the Unix epoch.
    pub finished: i64,
    pub outcome: DownloadOutcome,
}

/// Finished downloads, newest first.
#[derive(Debug, Default)]
pub struct DownloadList {
    path: PathBuf,
    records: Vec<DownloadRecord>,
}

#[derive(Default, Deserialize, Serialize)]
struct DownloadData {
    downloads: Vec<DownloadRecord>,
}

impl DownloadList {
    pub fn open(path: PathBuf) -> (Self, Option<io::Error>) {
        let (records, error) = match load::<DownloadData>(&path, DOWNLOADS_VERSION) {
            Ok(data) => (data.downloads, None),
            Err(error) => (Vec::new(), Some(error)),
        };
        let records = records
            .into_iter()
            .filter(|record| record.path.is_absolute())
            .take(MAX_DOWNLOADS)
            .collect();
        (Self { path, records }, error)
    }

    pub fn records(&self) -> &[DownloadRecord] {
        &self.records
    }

    pub fn push(&mut self, record: DownloadRecord) -> io::Result<()> {
        if !record.path.is_absolute() {
            return Err(invalid("download paths must be absolute"));
        }
        self.records.insert(0, record);
        self.records.truncate(MAX_DOWNLOADS);
        self.write()
    }

    pub fn clear(&mut self) -> io::Result<()> {
        self.records.clear();
        self.write()
    }

    fn write(&self) -> io::Result<()> {
        save(
            &self.path,
            DOWNLOADS_VERSION,
            &DownloadData {
                downloads: self.records.clone(),
            },
        )
    }
}

/// A capability a site asks for. Screen sharing is never remembered.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    Notifications,
    Location,
    Camera,
    Microphone,
    Clipboard,
}

impl Permission {
    pub const ALL: [Self; 5] = [
        Self::Notifications,
        Self::Location,
        Self::Camera,
        Self::Microphone,
        Self::Clipboard,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Notifications => "Notifications",
            Self::Location => "Location",
            Self::Camera => "Camera",
            Self::Microphone => "Microphone",
            Self::Clipboard => "Read clipboard",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SitePermission {
    /// `scheme://host[:port]`, as returned by [`site_origin`].
    pub origin: String,
    pub permission: Permission,
    pub allowed: bool,
}

pub const MAX_SITE_PERMISSIONS: usize = 1000;
const SITE_SETTINGS_VERSION: u32 = 1;

/// Remembered permission answers and the sites where content blocking is off.
#[derive(Debug, Default)]
pub struct SiteSettings {
    path: PathBuf,
    permissions: Vec<SitePermission>,
    unblocked: Vec<String>,
}

#[derive(Default, Deserialize, Serialize)]
struct SiteData {
    #[serde(default)]
    permissions: Vec<SitePermission>,
    #[serde(default)]
    unblocked: Vec<String>,
}

/// The origin a permission is remembered for: web origins only, normalized
/// to lower case without a default port.
pub fn site_origin(uri: &str) -> Option<String> {
    let url = url::Url::parse(uri).ok()?;
    if !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let origin = url.origin().ascii_serialization();
    (origin != "null").then_some(origin)
}

impl SiteSettings {
    pub fn open(path: PathBuf) -> (Self, Option<io::Error>) {
        let (data, error) = match load::<SiteData>(&path, SITE_SETTINGS_VERSION) {
            Ok(data) => (data, None),
            Err(error) => (SiteData::default(), Some(error)),
        };
        let mut permissions: Vec<SitePermission> = Vec::new();
        for entry in data.permissions {
            let normalized = site_origin(&entry.origin);
            if normalized.as_deref() == Some(entry.origin.as_str())
                && !permissions.iter().any(|known| {
                    known.origin == entry.origin && known.permission == entry.permission
                })
            {
                permissions.push(entry);
            }
        }
        permissions.truncate(MAX_SITE_PERMISSIONS);
        let mut unblocked: Vec<String> = Vec::new();
        for host in data.unblocked {
            let host = host.to_ascii_lowercase();
            if valid_host(&host) && !unblocked.contains(&host) {
                unblocked.push(host);
            }
        }
        unblocked.truncate(MAX_SITE_PERMISSIONS);
        let settings = Self {
            path,
            permissions,
            unblocked,
        };
        (settings, error)
    }

    pub fn permissions(&self) -> &[SitePermission] {
        &self.permissions
    }

    pub fn decision(&self, origin: &str, permission: Permission) -> Option<bool> {
        self.permissions
            .iter()
            .find(|entry| entry.origin == origin && entry.permission == permission)
            .map(|entry| entry.allowed)
    }

    pub fn set(&mut self, origin: &str, permission: Permission, allowed: bool) -> io::Result<()> {
        if site_origin(origin).as_deref() != Some(origin) {
            return Err(invalid("invalid site origin"));
        }
        self.permissions
            .retain(|entry| !(entry.origin == origin && entry.permission == permission));
        self.permissions.insert(
            0,
            SitePermission {
                origin: origin.to_owned(),
                permission,
                allowed,
            },
        );
        self.permissions.truncate(MAX_SITE_PERMISSIONS);
        self.write()
    }

    pub fn forget(&mut self, origin: &str, permission: Permission) -> io::Result<()> {
        self.permissions
            .retain(|entry| !(entry.origin == origin && entry.permission == permission));
        self.write()
    }

    pub fn unblocked(&self) -> &[String] {
        &self.unblocked
    }

    pub fn is_unblocked(&self, host: &str) -> bool {
        let host = host.to_ascii_lowercase();
        self.unblocked.contains(&host)
    }

    /// Turns content blocking off (`true`) or back on for `host`.
    pub fn set_unblocked(&mut self, host: &str, unblocked: bool) -> io::Result<()> {
        let host = host.to_ascii_lowercase();
        if !valid_host(&host) {
            return Err(invalid("invalid host"));
        }
        self.unblocked.retain(|known| *known != host);
        if unblocked {
            self.unblocked.insert(0, host);
            self.unblocked.truncate(MAX_SITE_PERMISSIONS);
        }
        self.write()
    }

    /// Forgets every permission answer, keeping the blocking exceptions.
    pub fn clear_permissions(&mut self) -> io::Result<()> {
        self.permissions.clear();
        self.write()
    }

    fn write(&self) -> io::Result<()> {
        save(
            &self.path,
            SITE_SETTINGS_VERSION,
            &SiteData {
                permissions: self.permissions.clone(),
                unblocked: self.unblocked.clone(),
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn permission_labels_are_human_readable() {
        assert_eq!(Permission::Notifications.label(), "Notifications");
        assert_eq!(Permission::Location.label(), "Location");
        assert_eq!(Permission::Camera.label(), "Camera");
        assert_eq!(Permission::Microphone.label(), "Microphone");
        assert_eq!(Permission::Clipboard.label(), "Read clipboard");
    }

    #[test]
    fn site_permissions_persist_per_origin_and_reject_non_web_origins() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("site-settings.json");
        let (mut sites, error) = SiteSettings::open(path.clone());
        assert!(error.is_none());
        let meet = site_origin("HTTPS://Meet.Example.test:443/room?x").unwrap();
        assert_eq!(meet, "https://meet.example.test");
        assert_eq!(
            site_origin("http://127.0.0.1:8080/a").as_deref(),
            Some("http://127.0.0.1:8080")
        );
        for rejected in [
            "bitter://history",
            "file:///tmp/a",
            "data:text/html,x",
            "nope",
        ] {
            assert_eq!(site_origin(rejected), None, "{rejected}");
        }
        sites.set(&meet, Permission::Camera, true).unwrap();
        sites.set(&meet, Permission::Notifications, false).unwrap();
        sites.set(&meet, Permission::Camera, false).unwrap();
        assert!(
            sites
                .set("https://Meet.example.test", Permission::Camera, true)
                .is_err()
        );
        sites.set_unblocked("News.Example.test", true).unwrap();
        assert!(sites.set_unblocked("", true).is_err());

        let (mut sites, _) = SiteSettings::open(path.clone());
        assert_eq!(sites.permissions().len(), 2);
        assert_eq!(sites.decision(&meet, Permission::Camera), Some(false));
        assert_eq!(
            sites.decision(&meet, Permission::Notifications),
            Some(false)
        );
        assert_eq!(sites.decision(&meet, Permission::Location), None);
        assert!(sites.is_unblocked("news.example.test"));
        sites.forget(&meet, Permission::Camera).unwrap();
        sites.set_unblocked("news.example.test", false).unwrap();
        sites.clear_permissions().unwrap();
        let (sites, _) = SiteSettings::open(path);
        assert!(sites.permissions().is_empty() && sites.unblocked().is_empty());
    }

    #[test]
    fn site_settings_drop_invalid_and_duplicate_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("site-settings.json");
        std::fs::write(
            &path,
            br#"{"version":1,"permissions":[
                {"origin":"https://a.test","permission":"camera","allowed":true},
                {"origin":"https://a.test","permission":"camera","allowed":false},
                {"origin":"HTTPS://B.test","permission":"location","allowed":true},
                {"origin":"file:///x","permission":"location","allowed":true}],
              "unblocked":["A.test","a.test","bad/host"]}"#,
        )
        .unwrap();
        let (sites, error) = SiteSettings::open(path.clone());
        assert!(error.is_none());
        assert_eq!(sites.permissions().len(), 1);
        assert_eq!(
            sites.decision("https://a.test", Permission::Camera),
            Some(true)
        );
        assert_eq!(sites.unblocked(), ["a.test"]);
        std::fs::write(&path, br#"{"version":1,"permissions":[{"origin":"https://a.test","permission":"teleport","allowed":true}]}"#).unwrap();
        let (sites, error) = SiteSettings::open(path);
        assert!(error.is_some() && sites.permissions().is_empty());
    }

    #[test]
    fn zoom_levels_persist_per_host_and_can_be_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zoom.json");
        let (mut zoom, error) = ZoomLevels::open(path.clone());
        assert!(error.is_none());
        zoom.set("Example.TEST", 1.25).unwrap();
        zoom.set("other.test", 0.8).unwrap();
        let (mut zoom, _) = ZoomLevels::open(path.clone());
        assert_eq!(zoom.get("example.test"), Some(1.25));
        assert_eq!(zoom.get("other.test"), Some(0.8));
        zoom.set("other.test", 1.0).unwrap();
        zoom.forget("Example.test").unwrap();
        let (zoom, _) = ZoomLevels::open(path);
        assert_eq!(zoom.get("example.test"), None);
        assert_eq!(zoom.get("other.test"), Some(1.0));
        assert!(zoom.set_is_rejected_for_invalid_input());
    }

    impl ZoomLevels {
        fn set_is_rejected_for_invalid_input(mut self) -> bool {
            self.set("", 1.5).is_err()
                && self.set("a/b", 1.5).is_err()
                && self.set("ok.test", f64::NAN).is_err()
                && self.set("ok.test", 40.0).is_err()
        }
    }

    #[test]
    fn zoom_keeps_the_most_recent_sites() {
        let dir = tempfile::tempdir().unwrap();
        let (mut zoom, _) = ZoomLevels::open(dir.path().join("zoom.json"));
        for index in 0..=MAX_ZOOM_SITES {
            zoom.sites.insert(0, (format!("site{index}.test"), 1.5));
        }
        zoom.set("latest.test", 2.0).unwrap();
        let (zoom, _) = ZoomLevels::open(dir.path().join("zoom.json"));
        assert_eq!(zoom.sites.len(), MAX_ZOOM_SITES);
        assert_eq!(zoom.get("latest.test"), Some(2.0));
        assert_eq!(zoom.get("site0.test"), None);
    }

    #[test]
    fn download_list_round_trips_and_caps_entries() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("downloads.json");
        let (mut list, _) = DownloadList::open(path.clone());
        let record = |index: usize| DownloadRecord {
            path: dir.path().join(format!("file{index}.txt")),
            source: format!("https://example.test/{index}"),
            size: index as u64,
            finished: index as i64,
            outcome: DownloadOutcome::Completed,
        };
        for index in 0..MAX_DOWNLOADS + 5 {
            list.push(record(index)).unwrap();
        }
        assert!(
            list.push(DownloadRecord {
                path: "relative.txt".into(),
                ..record(0)
            })
            .is_err()
        );
        let (list, error) = DownloadList::open(path.clone());
        assert!(error.is_none());
        assert_eq!(list.records().len(), MAX_DOWNLOADS);
        assert_eq!(list.records()[0], record(MAX_DOWNLOADS + 4));
        let (mut list, _) = DownloadList::open(path.clone());
        list.clear().unwrap();
        assert!(DownloadList::open(path).0.records().is_empty());
    }

    #[test]
    fn unreadable_files_report_an_error_and_start_empty() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zoom.json");
        std::fs::write(&path, b"{not json").unwrap();
        let (zoom, error) = ZoomLevels::open(path.clone());
        assert!(error.is_some() && zoom.sites.is_empty());
        std::fs::write(&path, br#"{"version":9,"sites":[]}"#).unwrap();
        assert!(ZoomLevels::open(path.clone()).1.is_some());
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(dir.path().join("elsewhere"), &path).unwrap();
        assert!(ZoomLevels::open(path.clone()).1.is_some());
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(ZoomLevels::open(path.clone()).1.is_some());
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, vec![b' '; super::MAX_BYTES + 1]).unwrap();
        assert!(ZoomLevels::open(path).1.is_some());
    }

    #[test]
    fn forget_unknown_host_is_a_noop() {
        let dir = tempfile::tempdir().unwrap();
        let (mut zoom, _) = ZoomLevels::open(dir.path().join("zoom.json"));
        zoom.forget("missing.test").unwrap();
        assert!(zoom.sites.is_empty());
    }

    #[test]
    fn save_rejects_paths_without_parent_and_oversized_payloads() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zoom.json");
        let (mut zoom, _) = ZoomLevels::open(path);
        assert!(save(Path::new("/"), ZOOM_VERSION, &ZoomData::default()).is_err());
        zoom.sites = (0..60_000)
            .map(|index| (format!("site-{index}-{}.example.test", "x".repeat(20)), 1.0))
            .collect();
        assert!(zoom.write().is_err());
        let target = dir.path().join("dir-target.json");
        std::fs::create_dir(&target).unwrap();
        let (mut list, _) = DownloadList::open(target.clone());
        list.records.push(DownloadRecord {
            path: dir.path().join("file"),
            source: "https://example.test/".into(),
            size: 0,
            finished: 0,
            outcome: DownloadOutcome::Completed,
        });
        assert!(list.write().is_err());
    }

    #[test]
    fn zoom_input_is_validated_and_hosts_are_capped() {
        let dir = tempfile::tempdir().unwrap();
        let (mut zoom, error) = ZoomLevels::open(dir.path().join("zoom.json"));
        assert!(error.is_none());
        let host253 = format!("{}.test", "a".repeat(248));
        let host254 = format!("{}.test", "a".repeat(249));
        assert_eq!(host253.len(), 253);
        assert_eq!(host254.len(), 254);
        assert!(zoom.set(&host254, 1.5).is_err());
        assert!(zoom.set(&host253, 1.5).is_ok());
        assert_eq!(zoom.get(&host253), Some(1.5));
        assert_eq!(zoom.get("UPPER.test"), None);
        assert!(zoom.set("UPPER.test", 1.5).is_ok());
        assert_eq!(zoom.get("upper.test"), Some(1.5));
        // The valid zoom range is 0.25 to 5.0, inclusive.
        for level in [0.25, 5.0] {
            assert!(zoom.set("bounds.test", level).is_ok(), "{level}");
        }
        for level in [0.1, 5.01, f64::NAN] {
            assert!(zoom.set("bounds.test", level).is_err(), "{level}");
        }
    }

    #[test]
    fn download_list_drops_non_absolute_and_wrong_version_records() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("downloads.json");
        std::fs::write(
            &path,
            br#"{"version":1,"downloads":[
                {"path":"relative.txt","source":"","size":0,"finished":0,"outcome":"completed"},
                {"path":"/abs/ok","source":"","size":0,"finished":0,"outcome":"completed"}]}"#,
        )
        .unwrap();
        let (list, error) = DownloadList::open(path.clone());
        assert!(error.is_none());
        assert_eq!(list.records().len(), 1);
        assert_eq!(list.records()[0].path, PathBuf::from("/abs/ok"));
        std::fs::write(&path, br#"{"version":9,"downloads":[]}"#).unwrap();
        let (list, error) = DownloadList::open(path);
        assert!(error.is_some() && list.records().is_empty());
    }

    #[test]
    fn site_origins_normalize_ports_and_schemes() {
        assert_eq!(
            site_origin("HTTPS://A.TEST:443/x").as_deref(),
            Some("https://a.test")
        );
        assert_eq!(
            site_origin("http://Example.test:8080").as_deref(),
            Some("http://example.test:8080")
        );
        assert_eq!(
            site_origin("https://a.test").as_deref(),
            Some("https://a.test")
        );
        for rejected in ["ftp://a.test", "bitter://history", "", "file:///tmp/a"] {
            assert_eq!(site_origin(rejected), None, "{rejected}");
        }
    }

    #[test]
    fn site_settings_replace_answers_and_forget_permissions() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("site-settings.json");
        let (mut sites, error) = SiteSettings::open(path.clone());
        assert!(error.is_none());
        let origin = "https://a.test";
        sites.set(origin, Permission::Camera, true).unwrap();
        sites.set(origin, Permission::Camera, false).unwrap();
        assert_eq!(sites.decision(origin, Permission::Camera), Some(false));
        sites.set(origin, Permission::Camera, true).unwrap();
        assert_eq!(sites.permissions().len(), 1);
        sites.set(origin, Permission::Notifications, false).unwrap();
        assert_eq!(
            sites.decision(origin, Permission::Notifications),
            Some(false)
        );
        sites.forget(origin, Permission::Camera).unwrap();
        assert_eq!(sites.decision(origin, Permission::Camera), None);
        let (sites, _) = SiteSettings::open(path);
        assert_eq!(sites.decision(origin, Permission::Camera), None);
        assert_eq!(
            sites.decision(origin, Permission::Notifications),
            Some(false)
        );
    }
}
