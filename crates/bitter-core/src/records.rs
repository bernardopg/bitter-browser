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

    /// Remembers `level` for `host`; 100% forgets the host.
    pub fn set(&mut self, host: &str, level: f64) -> io::Result<()> {
        let host = host.to_ascii_lowercase();
        if !valid_host(&host) || !valid_zoom(level) {
            return Err(invalid("invalid zoom entry"));
        }
        self.sites.retain(|(site, _)| *site != host);
        if (level - 1.0).abs() > f64::EPSILON {
            self.sites.insert(0, (host, level));
            self.sites.truncate(MAX_ZOOM_SITES);
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zoom_levels_persist_per_host_and_forget_the_default() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("zoom.json");
        let (mut zoom, error) = ZoomLevels::open(path.clone());
        assert!(error.is_none());
        zoom.set("Example.TEST", 1.25).unwrap();
        zoom.set("other.test", 0.8).unwrap();
        let (mut zoom, _) = ZoomLevels::open(path.clone());
        assert_eq!(zoom.get("example.test"), Some(1.25));
        assert_eq!(zoom.get("other.test"), Some(0.8));
        zoom.set("example.test", 1.0).unwrap();
        let (zoom, _) = ZoomLevels::open(path);
        assert_eq!(zoom.get("example.test"), None);
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
        assert!(ZoomLevels::open(path).1.is_some());
    }
}
