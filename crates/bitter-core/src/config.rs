use std::fs::{File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;
use toml_edit::{DocumentMut, Table, TableLike, value};

const VERSION: i64 = 1;
const MAX_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preferences {
    pub search: SearchEngine,
    pub theme: Theme,
    pub restore_session: bool,
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            search: SearchEngine::default(),
            theme: Theme::default(),
            restore_session: true,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum SearchEngine {
    #[default]
    DuckDuckGo,
    Brave,
    Google,
}

impl SearchEngine {
    pub const ALL: [Self; 3] = [Self::DuckDuckGo, Self::Brave, Self::Google];

    pub fn label(self) -> &'static str {
        match self {
            Self::DuckDuckGo => "DuckDuckGo",
            Self::Brave => "Brave Search",
            Self::Google => "Google",
        }
    }

    pub fn search_url(self, query: &str) -> String {
        let base = match self {
            Self::DuckDuckGo => "https://duckduckgo.com/",
            Self::Brave => "https://search.brave.com/search",
            Self::Google => "https://www.google.com/search",
        };
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("q", query)
            .finish();
        format!("{base}?{query}")
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    pub const ALL: [Self; 3] = [Self::System, Self::Light, Self::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Self::System => "System",
            Self::Light => "Light",
            Self::Dark => "Dark",
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::Light => "light",
            Self::Dark => "dark",
        }
    }
}

#[derive(Debug, thiserror::Error)]
#[error("{path}: {source}")]
pub struct ConfigError {
    path: PathBuf,
    source: ConfigFailure,
}

#[derive(Debug, thiserror::Error)]
enum ConfigFailure {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("could not read TOML at line {0}")]
    Syntax(usize),
    #[error("{0} must be a table")]
    InvalidTable(&'static str),
    #[error("{0}.{1} has an invalid or unsupported value")]
    InvalidValue(&'static str, &'static str),
    #[error("configuration version is invalid or unsupported")]
    Version,
    #[error("configuration exceeds 256 KiB")]
    TooLarge,
    #[error("configuration changed outside the browser. Restart to reload it before saving")]
    Changed,
}

pub struct ConfigStore {
    path: PathBuf,
    original: Option<Vec<u8>>,
    document: DocumentMut,
    current: Preferences,
}

#[derive(Debug)]
pub enum SaveOutcome {
    Durable,
    // The rename is visible, but the filesystem did not confirm crash durability.
    DirectorySyncFailed(io::Error),
}

impl ConfigStore {
    pub fn open(path: PathBuf) -> Result<Self, ConfigError> {
        let result = (|| {
            let original = read_config(&path)?;
            let document = match original.as_deref() {
                Some(bytes) => parse(bytes)?,
                None => DocumentMut::new(),
            };
            let current = validate(&document)?;
            Ok(Self {
                path: path.clone(),
                document,
                current,
                original,
            })
        })();
        result.map_err(|source| ConfigError { path, source })
    }

    pub fn current(&self) -> Preferences {
        self.current
    }

    pub fn save(&mut self, preferences: Preferences) -> Result<SaveOutcome, ConfigError> {
        self.save_inner(preferences).map_err(|source| ConfigError {
            path: self.path.clone(),
            source,
        })
    }

    fn save_inner(&mut self, preferences: Preferences) -> Result<SaveOutcome, ConfigFailure> {
        let mut document = self.document.clone();
        document["version"] = value(VERSION);
        let search = ensure_table(&mut document, "general")?;
        search.insert("default_search_engine", value(preferences.search.label()));
        search.insert("restore_session", value(preferences.restore_session));
        let appearance = ensure_table(&mut document, "appearance")?;
        appearance.insert("theme", value(preferences.theme.key()));

        let bytes = document.to_string().into_bytes();
        if bytes.len() > MAX_BYTES {
            return Err(ConfigFailure::TooLarge);
        }
        let parent = self
            .path
            .parent()
            .ok_or_else(|| io::Error::other("missing parent directory"))?;
        let directory = File::open(parent)?;
        let temp = stage(parent, ".config-", &bytes)?;
        if read_config(&self.path)? != self.original {
            return Err(ConfigFailure::Changed);
        }
        temp.persist(&self.path).map_err(|err| err.error)?;
        self.current = preferences;
        self.document = document;
        self.original = Some(bytes);
        Ok(match directory.sync_all() {
            Ok(()) => SaveOutcome::Durable,
            Err(error) => SaveOutcome::DirectorySyncFailed(error),
        })
    }
}

fn read_config(path: &Path) -> Result<Option<Vec<u8>>, ConfigFailure> {
    let file = match OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
    {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    if !file.metadata()?.is_file() {
        return Err(io::Error::other("expected a regular configuration file").into());
    }
    let mut bytes = Vec::new();
    file.take((MAX_BYTES + 1) as u64).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_BYTES {
        return Err(ConfigFailure::TooLarge);
    }
    Ok(Some(bytes))
}

fn parse(bytes: &[u8]) -> Result<DocumentMut, ConfigFailure> {
    fn line(bytes: &[u8], position: usize) -> usize {
        1 + bytes
            .iter()
            .take(position)
            .filter(|&&byte| byte == b'\n')
            .count()
    }

    let contents = std::str::from_utf8(bytes)
        .map_err(|error| ConfigFailure::Syntax(line(bytes, error.valid_up_to())))?;
    contents.parse().map_err(|error: toml_edit::TomlError| {
        ConfigFailure::Syntax(line(bytes, error.span().map_or(0, |span| span.start)))
    })
}

fn validate(document: &DocumentMut) -> Result<Preferences, ConfigFailure> {
    if let Some(item) = document.get("version")
        && !matches!(item.as_integer(), Some(0 | VERSION))
    {
        return Err(ConfigFailure::Version);
    }
    let search = read_setting(document, "general", "default_search_engine", |key| {
        SearchEngine::ALL
            .into_iter()
            .find(|engine| engine.label() == key)
    })?;
    let theme = read_setting(document, "appearance", "theme", |key| {
        Theme::ALL.into_iter().find(|theme| theme.key() == key)
    })?;
    let restore_session = document
        .get("general")
        .and_then(toml_edit::Item::as_table_like)
        .and_then(|table| table.get("restore_session"))
        .map(|item| {
            item.as_bool()
                .ok_or(ConfigFailure::InvalidValue("general", "restore_session"))
        })
        .transpose()?
        .unwrap_or(true);
    Ok(Preferences {
        search,
        theme,
        restore_session,
    })
}

fn read_setting<T: Default>(
    document: &DocumentMut,
    section: &'static str,
    field: &'static str,
    parse: impl FnOnce(&str) -> Option<T>,
) -> Result<T, ConfigFailure> {
    let Some(item) = document.get(section) else {
        return Ok(T::default());
    };
    let table = item
        .as_table_like()
        .ok_or(ConfigFailure::InvalidTable(section))?;
    let Some(item) = table.get(field) else {
        return Ok(T::default());
    };
    item.as_str()
        .and_then(parse)
        .ok_or(ConfigFailure::InvalidValue(section, field))
}

fn ensure_table<'a>(
    document: &'a mut DocumentMut,
    name: &'static str,
) -> Result<&'a mut dyn TableLike, ConfigFailure> {
    document
        .entry(name)
        .or_insert(Table::new().into())
        .as_table_like_mut()
        .ok_or(ConfigFailure::InvalidTable(name))
}

pub(crate) fn stage(parent: &Path, prefix: &str, bytes: &[u8]) -> io::Result<NamedTempFile> {
    let mut temp = tempfile::Builder::new()
        .prefix(prefix)
        .tempfile_in(parent)?;
    temp.write_all(bytes)?;
    temp.as_file().sync_all()?;
    Ok(temp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn upgrades_partial_legacy_config_and_preserves_unknown_fields_and_comments() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("config.toml");
        fs::write(
            &path,
            "# Browser profile\n[general]\nrestore_session = true\n\n[privacy]\nblock_ads = true\n",
        )
        .unwrap();
        let mut store = ConfigStore::open(path.clone()).unwrap();
        assert_eq!(store.current(), Preferences::default());
        let next = Preferences {
            search: SearchEngine::Brave,
            theme: Theme::Light,
            ..Preferences::default()
        };
        assert!(matches!(store.save(next).unwrap(), SaveOutcome::Durable));
        let text = fs::read_to_string(&path).unwrap();
        for expected in [
            "version = 1",
            "# Browser profile",
            "restore_session = true",
            "block_ads = true",
        ] {
            assert!(text.contains(expected), "{text}");
        }
        assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        assert_eq!(ConfigStore::open(path).unwrap().current(), next);
    }

    #[test]
    fn malformed_invalid_future_and_oversized_config_remain_untouched() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("config.toml");
        let oversized = vec![b' '; MAX_BYTES + 1];
        for original in [
            b"[general\n".as_slice(),
            b"version = 2",
            b"version = '1'",
            b"[appearance]\ntheme = 3",
            b"general = 'broken'",
            b"[general]\ndefault_search_engine = 'unsupported'",
            &oversized,
        ] {
            fs::write(&path, original).unwrap();
            assert!(ConfigStore::open(path.clone()).is_err());
            assert_eq!(fs::read(&path).unwrap(), original);
        }
    }

    #[test]
    fn interrupted_save_and_external_edits_keep_the_original_and_effective_values() {
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("config.toml");
        let mut store = ConfigStore::open(path.clone()).unwrap();
        store.save(Preferences::default()).unwrap();

        let bytes = fs::read(&path).unwrap();
        let staged = stage(profile.path(), ".config-", b"[appearance]\ntheme = 'dark'").unwrap();
        assert_eq!(fs::read(&path).unwrap(), bytes);
        drop(staged);
        assert_eq!(
            ConfigStore::open(path.clone()).unwrap().current(),
            Preferences::default()
        );

        let edited = b"[appearance]\ntheme = 'light'";
        fs::write(&path, edited).unwrap();
        let requested = Preferences {
            theme: Theme::Dark,
            ..Preferences::default()
        };
        assert!(matches!(
            store.save(requested),
            Err(ConfigError {
                source: ConfigFailure::Changed,
                ..
            })
        ));
        assert_eq!(store.current(), Preferences::default());
        assert_eq!(fs::read(&path).unwrap(), edited);
        assert_eq!(
            ConfigStore::open(path).unwrap().current().theme,
            Theme::Light
        );
    }

    #[test]
    fn search_terms_form_one_parameter_for_each_provider() {
        for engine in SearchEngine::ALL {
            let query = "férias & café + ?";
            let uri = url::Url::parse(&engine.search_url(query)).unwrap();
            assert_eq!(uri.scheme(), "https");
            assert_eq!(
                uri.query_pairs().collect::<Vec<_>>(),
                [("q".into(), query.into())]
            );
        }
    }
}
