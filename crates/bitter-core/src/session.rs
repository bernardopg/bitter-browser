use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

const VERSION: u32 = 1;
const MAX_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_TABS: usize = 100;
pub const HOME_URI: &str = "bitter://newtab";

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceId {
    #[default]
    Main,
    Second,
}

impl WorkspaceId {
    pub const ALL: [Self; 2] = [Self::Main, Self::Second];

    pub fn index(self) -> usize {
        self as usize
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct SavedTab {
    pub uri: String,
    pub title: String,
}

impl SavedTab {
    pub fn new(uri: &str, title: &str) -> Self {
        let uri = if persistable(uri) { uri } else { HOME_URI };
        let title = if uri == HOME_URI { "New Tab" } else { title };
        Self {
            uri: uri.into(),
            title: title
                .chars()
                .filter(|ch| !ch.is_control())
                .take(256)
                .collect(),
        }
    }
}

fn persistable(uri: &str) -> bool {
    if matches!(uri, HOME_URI | "bitter://history" | "bitter://bookmarks") {
        return true;
    }
    uri.len() <= 4096
        && !uri.chars().any(char::is_control)
        && url::Url::parse(uri).is_ok_and(|url| {
            matches!(url.scheme(), "http" | "https")
                && url.host().is_some()
                && url.username().is_empty()
                && url.password().is_none()
        })
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct SavedWorkspace {
    pub tabs: Vec<SavedTab>,
    pub selected: usize,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Session {
    pub workspace: WorkspaceId,
    pub workspaces: [SavedWorkspace; 2],
}

#[derive(Deserialize, Serialize)]
struct Snapshot {
    version: u32,
    session: Session,
}

pub fn load(path: &Path) -> io::Result<Session> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Session::default()),
        Err(error) => return Err(error),
    };
    if !file.metadata()?.is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "expected a regular session file",
        ));
    }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "session file is too large",
        ));
    }
    let Snapshot {
        version,
        mut session,
    } = serde_json::from_slice(&bytes)?;
    if version != VERSION {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "unsupported session version",
        ));
    }
    for workspace in &mut session.workspaces {
        if workspace.tabs.len() > MAX_TABS {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "too many tabs in the session",
            ));
        }
        workspace.selected = workspace
            .selected
            .min(workspace.tabs.len().saturating_sub(1));
        for tab in &mut workspace.tabs {
            *tab = SavedTab::new(&tab.uri, &tab.title);
        }
    }
    Ok(session)
}

pub fn save(path: &Path, session: Session) -> io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| io::Error::other("missing session directory"))?;
    let directory = File::open(parent)?;
    let bytes = serde_json::to_vec(&Snapshot {
        version: VERSION,
        session,
    })?;
    if bytes.len() > MAX_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "session file is too large",
        ));
    }
    crate::config::stage(parent, ".session-", &bytes)?
        .persist(path)
        .map_err(|error| error.error)?;
    directory.sync_all()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_workspaces_and_excludes_sensitive_or_unsupported_destinations() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        let mut session = Session {
            workspace: WorkspaceId::Second,
            ..Session::default()
        };
        session.workspaces[1].tabs = [
            SavedTab::new("https://example.test/", "Example"),
            SavedTab::new("file:///home/someone/private", "Private file"),
            SavedTab::new("https://user:pass@example.test/", "Credentials"),
        ]
        .into();
        session.workspaces[1].selected = 2;
        save(&path, session.clone()).unwrap();
        let raw = std::fs::read_to_string(&path).unwrap();
        assert!(!raw.contains("pass"));
        assert!(!raw.contains("private"));
        assert_eq!(load(&path).unwrap(), session);
    }

    #[test]
    fn preserves_corrupt_and_future_snapshots() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        assert_eq!(load(&path).unwrap(), Session::default());
        for bytes in [
            "{truncated",
            r#"{"version":2,"session":{"workspace":"main","workspaces":[{"tabs":[],"selected":0},{"tabs":[],"selected":0}]}}"#,
        ] {
            std::fs::write(&path, bytes).unwrap();
            assert!(load(&path).is_err());
            assert_eq!(std::fs::read_to_string(&path).unwrap(), bytes);
        }
    }

    #[test]
    fn saved_tabs_reject_non_persistable_destinations() {
        let saved = SavedTab::new("bitter://history", "History");
        assert_eq!(
            (saved.uri, saved.title),
            ("bitter://history".to_owned(), "History".to_owned())
        );
        assert_eq!(SavedTab::new("bitter://bookmarks", "").title, "");
        assert_eq!(
            SavedTab::new("https://user:pass@example.test/", "Credentials").uri,
            HOME_URI
        );
        assert_eq!(
            SavedTab::new("file:///home/someone/private", "Private file").uri,
            HOME_URI
        );
        let long = format!("https://a.test/{}", "x".repeat(5000));
        assert_eq!(SavedTab::new(&long, "Long").uri, HOME_URI);
        // Titles are shortened, without control characters, at 256 characters.
        let saved = SavedTab::new("https://a.test/", &format!("  {}  ", "y".repeat(300)));
        assert_eq!(saved.title.chars().count(), 256);
        assert_eq!(
            SavedTab::new("https://a.test/", "line\nbreak\u{7}").title,
            "linebreak"
        );
    }

    #[test]
    fn workspace_ids_are_indexed() {
        assert_eq!(WorkspaceId::Main.index(), 0);
        assert_eq!(WorkspaceId::Second.index(), 1);
        assert_eq!(WorkspaceId::ALL, [WorkspaceId::Main, WorkspaceId::Second]);
    }

    #[test]
    fn load_rejects_too_many_tabs_and_clamps_selection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("session.json");
        assert_eq!(load(&path).unwrap(), Session::default());
        assert!(load(dir.path()).is_err()); // a directory is not a session
        let mut session = Session::default();
        session.workspaces[0].tabs = vec![SavedTab::new("https://a.test/", "A")];
        // Selected beyond the tabs clamps to the last one.
        session.workspaces[0].selected = 7;
        save(&path, session.clone()).unwrap();
        let loaded = load(&path).unwrap();
        assert_eq!(loaded.workspaces[0].selected, 0);
        assert_eq!(loaded.workspaces[0].tabs, session.workspaces[0].tabs);
        let mut session = Session::default();
        session.workspaces[0].tabs = vec![SavedTab::new("https://a.test/", "A"); MAX_TABS + 1];
        save(&path, session).unwrap();
        assert!(load(&path).is_err());
    }
}
