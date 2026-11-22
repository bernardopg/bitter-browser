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
    /// Ask for a destination for every download instead of saving to the
    /// download directory.
    pub ask_download_location: bool,
    /// Underline misspelled words in text fields.
    pub spell_check: bool,
    /// Page zoom, in percent, for sites without a remembered level.
    pub default_zoom: u16,
    /// Block ads and trackers with the content filter lists.
    pub block_ads: bool,
    /// Load sites over HTTPS and ask before falling back to HTTP.
    pub https_only: bool,
    /// Refuse cookies from sites other than the one in the address bar.
    pub block_third_party_cookies: bool,
    /// WebKit's Intelligent Tracking Prevention.
    pub prevent_tracking: bool,
    /// Web Inspector and the Inspect Element menu item.
    pub developer_tools: bool,
}

/// The zoom presets, in percent, that the browser steps through.
pub const ZOOM_PERCENTS: [u16; 13] = [30, 50, 67, 80, 90, 100, 110, 125, 150, 175, 200, 250, 300];

impl Default for Preferences {
    fn default() -> Self {
        Self {
            search: SearchEngine::default(),
            theme: Theme::default(),
            restore_session: true,
            ask_download_location: false,
            spell_check: true,
            default_zoom: 100,
            block_ads: true,
            https_only: false,
            block_third_party_cookies: true,
            prevent_tracking: true,
            developer_tools: true,
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
        search.insert("spell_check", value(preferences.spell_check));
        let appearance = ensure_table(&mut document, "appearance")?;
        appearance.insert("theme", value(preferences.theme.key()));
        appearance.insert("default_zoom", value(i64::from(preferences.default_zoom)));
        let downloads = ensure_table(&mut document, "downloads")?;
        downloads.insert("ask_location", value(preferences.ask_download_location));
        let privacy = ensure_table(&mut document, "privacy")?;
        privacy.insert("block_ads", value(preferences.block_ads));
        privacy.insert("https_only", value(preferences.https_only));
        privacy.insert(
            "block_third_party_cookies",
            value(preferences.block_third_party_cookies),
        );
        privacy.insert(
            "prevent_cross_site_tracking",
            value(preferences.prevent_tracking),
        );
        let advanced = ensure_table(&mut document, "advanced")?;
        advanced.insert("developer_tools", value(preferences.developer_tools));

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
    let restore_session = read_bool(document, "general", "restore_session")?.unwrap_or(true);
    let ask_download_location = read_bool(document, "downloads", "ask_location")?.unwrap_or(false);
    let defaults = Preferences::default();
    let flag = |table, key, default: bool| -> Result<bool, ConfigFailure> {
        Ok(read_bool(document, table, key)?.unwrap_or(default))
    };
    let default_zoom = match document
        .get("appearance")
        .and_then(toml_edit::Item::as_table_like)
        .and_then(|table| table.get("default_zoom"))
    {
        None => defaults.default_zoom,
        Some(item) => item
            .as_integer()
            .and_then(|percent| u16::try_from(percent).ok())
            .filter(|percent| ZOOM_PERCENTS.contains(percent))
            .ok_or(ConfigFailure::InvalidValue("appearance", "default_zoom"))?,
    };
    Ok(Preferences {
        search,
        theme,
        restore_session,
        ask_download_location,
        spell_check: flag("general", "spell_check", defaults.spell_check)?,
        default_zoom,
        block_ads: flag("privacy", "block_ads", defaults.block_ads)?,
        https_only: flag("privacy", "https_only", defaults.https_only)?,
        block_third_party_cookies: flag(
            "privacy",
            "block_third_party_cookies",
            defaults.block_third_party_cookies,
        )?,
        prevent_tracking: flag(
            "privacy",
            "prevent_cross_site_tracking",
            defaults.prevent_tracking,
        )?,
        developer_tools: flag("advanced", "developer_tools", defaults.developer_tools)?,
    })
}

fn read_bool(
    document: &DocumentMut,
    table: &'static str,
    key: &'static str,
) -> Result<Option<bool>, ConfigFailure> {
    let Some(item) = document.get(table) else {
        return Ok(None);
    };
    item.as_table_like()
        .ok_or(ConfigFailure::InvalidTable(table))?
        .get(key)
        .map(|item| {
            item.as_bool()
                .ok_or(ConfigFailure::InvalidValue(table, key))
        })
        .transpose()
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
    fn read_config_rejects_invalid_utf8_and_non_regular_files() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        std::fs::write(&path, [0xff, 0xfe, 0xfd]).unwrap();
        assert!(ConfigStore::open(path.clone()).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(ConfigStore::open(path.clone()).is_err());
        std::fs::remove_dir(&path).unwrap();
        std::fs::write(&path, b"version = 1").unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o000);
        std::fs::set_permissions(&path, perms.clone()).unwrap();
        let result = ConfigStore::open(path.clone());
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o644);
        std::fs::set_permissions(&path, perms).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn save_rejects_missing_parent_and_oversized_documents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut store = ConfigStore::open(path).unwrap();
        store.document["padding"] = value("x".repeat(MAX_BYTES));
        assert!(matches!(
            store.save(Preferences::default()),
            Err(ConfigError {
                source: ConfigFailure::TooLarge,
                ..
            })
        ));

        let mut root_store = ConfigStore {
            path: PathBuf::from("/"),
            original: None,
            document: DocumentMut::new(),
            current: Preferences::default(),
        };
        assert!(root_store.save(Preferences::default()).is_err());
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
    fn download_location_preference_round_trips_and_rejects_bad_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut store = ConfigStore::open(path.clone()).unwrap();
        assert!(!store.current().ask_download_location);
        store
            .save(Preferences {
                ask_download_location: true,
                ..store.current()
            })
            .unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("ask_location = true")
        );
        assert!(
            ConfigStore::open(path.clone())
                .unwrap()
                .current()
                .ask_download_location
        );
        std::fs::write(&path, "version = 1\n[downloads]\nask_location = \"yes\"\n").unwrap();
        assert!(ConfigStore::open(path).is_err());
    }

    #[test]
    fn privacy_zoom_and_advanced_settings_round_trip_and_reject_bad_values() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.toml");
        let mut store = ConfigStore::open(path.clone()).unwrap();
        let next = Preferences {
            spell_check: false,
            default_zoom: 125,
            block_ads: false,
            https_only: true,
            block_third_party_cookies: false,
            prevent_tracking: false,
            developer_tools: false,
            ..store.current()
        };
        store.save(next).unwrap();
        let text = fs::read_to_string(&path).unwrap();
        for expected in [
            "spell_check = false",
            "default_zoom = 125",
            "[privacy]",
            "https_only = true",
            "prevent_cross_site_tracking = false",
            "[advanced]",
            "developer_tools = false",
        ] {
            assert!(text.contains(expected), "{text}");
        }
        assert_eq!(ConfigStore::open(path.clone()).unwrap().current(), next);
        for invalid in [
            "[appearance]\ndefault_zoom = 120",
            "[appearance]\ndefault_zoom = -100",
            "[appearance]\ndefault_zoom = '100'",
            "[privacy]\nhttps_only = 'yes'",
            "[advanced]\ndeveloper_tools = 1",
            "privacy = true",
        ] {
            fs::write(&path, invalid).unwrap();
            assert!(ConfigStore::open(path.clone()).is_err(), "{invalid}");
        }
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

    #[test]
    fn engine_and_theme_labels_are_shown_in_the_product() {
        for engine in SearchEngine::ALL {
            let uri = url::Url::parse(&engine.search_url("query")).unwrap();
            assert_eq!(uri.scheme(), "https");
            assert!(!engine.label().is_empty());
        }
        assert_eq!(SearchEngine::DuckDuckGo.label(), "DuckDuckGo");
        assert_eq!(SearchEngine::Brave.label(), "Brave Search");
        assert_eq!(SearchEngine::Google.label(), "Google");
        assert_eq!(Theme::System.label(), "System");
        assert_eq!(Theme::Light.label(), "Light");
        assert_eq!(Theme::Dark.label(), "Dark");
    }
}
