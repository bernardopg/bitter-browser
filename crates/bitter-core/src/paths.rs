use std::fs::{self, DirBuilder, File, OpenOptions, TryLockError};
use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};

const PROFILE_DIR: &str = "bitter-browser";
// Keep WebKit's previous default directories so existing website data stays available.
const WEBKIT_DIR: &str = "bitter-ui";

#[derive(Debug, thiserror::Error)]
pub enum ProfileError {
    #[error("Could not find an absolute {0} directory. Check HOME and your XDG settings.")]
    MissingBase(&'static str),
    #[error("Could not access {path}: {source}")]
    Access { path: PathBuf, source: io::Error },
    #[error(
        "This profile is already open. Close the other Bitter Browser instance and try again. Profile: {0}"
    )]
    InUse(PathBuf),
    #[error("WebKit requires a UTF-8 profile path. Check your XDG settings: {0}")]
    UnsupportedPath(PathBuf),
}

#[derive(Debug)]
pub struct Profile {
    config: PathBuf,
    data: PathBuf,
    web_data: String,
    web_cache: String,
    // Keep these files open until the application exits. Never unlink a held lock.
    _locks: Vec<File>,
}

impl Profile {
    pub fn open() -> Result<Self, ProfileError> {
        Self::open_at(
            &base(dirs::config_dir(), "configuration")?,
            &base(dirs::data_dir(), "data")?,
            &base(dirs::cache_dir(), "cache")?,
        )
    }

    fn open_at(config: &Path, data: &Path, cache: &Path) -> Result<Self, ProfileError> {
        let data_dir = data.join(PROFILE_DIR);
        let config = config.join(PROFILE_DIR);
        let web_data = data.join(WEBKIT_DIR);
        let web_cache = cache.join(WEBKIT_DIR);
        let web_data_utf8 = webkit_path(&web_data)?;
        let web_cache_utf8 = webkit_path(&web_cache)?;
        let mut locked = std::collections::HashSet::new();
        let mut locks = Vec::new();
        for path in [&data_dir, &config, &web_data, &web_cache] {
            let directory = private_directory(path)?;
            if locked.insert(directory) {
                locks.push(lock_directory(path)?);
            }
        }

        Ok(Self {
            config,
            data: data_dir,
            web_data: web_data_utf8,
            web_cache: web_cache_utf8,
            _locks: locks,
        })
    }

    pub fn config_path(&self) -> PathBuf {
        self.config.join("config.toml")
    }

    pub fn data_dir(&self) -> &Path {
        &self.data
    }

    pub fn web_data_dir(&self) -> &str {
        &self.web_data
    }

    pub fn web_cache_dir(&self) -> &str {
        &self.web_cache
    }
}

fn base(path: Option<PathBuf>, label: &'static str) -> Result<PathBuf, ProfileError> {
    path.filter(|path| path.is_absolute())
        .ok_or(ProfileError::MissingBase(label))
}

fn webkit_path(path: &Path) -> Result<String, ProfileError> {
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| ProfileError::UnsupportedPath(path.to_owned()))
}

fn access(path: &Path, source: io::Error) -> ProfileError {
    ProfileError::Access {
        path: path.to_owned(),
        source,
    }
}

fn owned_by_user(info: &fs::Metadata) -> bool {
    // SAFETY: geteuid has no arguments and returns the process's effective user ID.
    info.uid() == unsafe { libc::geteuid() }
}

fn lock_directory(path: &Path) -> Result<File, ProfileError> {
    let lock_path = path.join("profile.lock");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&lock_path)
        .map_err(|error| access(&lock_path, error))?;
    let info = lock.metadata().map_err(|error| access(&lock_path, error))?;
    if !info.is_file() || !owned_by_user(&info) || info.nlink() != 1 {
        return Err(access(
            &lock_path,
            io::Error::other("expected a private lock file"),
        ));
    }
    match lock.try_lock() {
        Ok(()) => Ok(lock),
        Err(TryLockError::WouldBlock) => Err(ProfileError::InUse(path.to_owned())),
        Err(TryLockError::Error(error)) => Err(access(&lock_path, error)),
    }
}

fn private_directory(path: &Path) -> Result<(u64, u64), ProfileError> {
    fn prepare(path: &Path) -> io::Result<(u64, u64)> {
        fs::create_dir_all(path.parent().expect("profile has a base directory"))?;
        match DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        let directory = OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
            .open(path)?;
        let info = directory.metadata()?;
        if !owned_by_user(&info) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "directory belongs to another user",
            ));
        }
        if info.mode() & 0o7777 != 0o700 {
            directory.set_permissions(fs::Permissions::from_mode(0o700))?;
        }
        Ok((info.dev(), info.ino()))
    }

    prepare(path).map_err(|error| access(path, error))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn locks_and_reopens_an_isolated_profile_with_legacy_paths() {
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config");
        let data = root.path().join("data");
        let cache = root.path().join("cache");
        let old_web_data = data.join(WEBKIT_DIR);
        fs::create_dir_all(&old_web_data).unwrap();
        fs::set_permissions(&old_web_data, fs::Permissions::from_mode(0o755)).unwrap();
        fs::write(old_web_data.join("existing"), b"example").unwrap();

        let profile = Profile::open_at(&config, &data, &cache).unwrap();
        assert_eq!(
            profile.config_path(),
            config.join("bitter-browser/config.toml")
        );
        assert_eq!(profile.data_dir(), data.join("bitter-browser"));
        assert_eq!(profile.web_data_dir(), old_web_data.to_str().unwrap());
        assert_eq!(
            profile.web_cache_dir(),
            cache.join(WEBKIT_DIR).to_str().unwrap()
        );
        assert_eq!(fs::read(old_web_data.join("existing")).unwrap(), b"example");
        for path in [
            profile.config_path().parent().unwrap().to_owned(),
            profile.data_dir().to_owned(),
            old_web_data,
            cache.join(WEBKIT_DIR),
        ] {
            assert_eq!(fs::metadata(path).unwrap().mode() & 0o777, 0o700);
        }
        assert!(matches!(
            Profile::open_at(&config, &data, &cache),
            Err(ProfileError::InUse(_))
        ));
        let other = root.path().join("other");
        assert!(Profile::open_at(&other, &other, &other).is_ok());
        // Sharing any one of the roots would still share persistent state.
        assert!(matches!(
            Profile::open_at(&config, &other, &other),
            Err(ProfileError::InUse(_))
        ));
        assert!(matches!(
            Profile::open_at(&other, &other, &cache),
            Err(ProfileError::InUse(_))
        ));
        assert!(Profile::open_at(&other, &other, &other).is_ok());
        drop(profile);
        assert!(Profile::open_at(&config, &data, &cache).is_ok());
    }

    #[test]
    fn rejects_relative_bases_symlinks_and_non_directories() {
        assert!(matches!(
            base(Some("relative".into()), "data"),
            Err(ProfileError::MissingBase("data"))
        ));
        assert!(base(None, "data").is_err());
        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let path = root.path().join(PROFILE_DIR);
        symlink(outside.path(), &path).unwrap();
        assert!(Profile::open_at(root.path(), root.path(), root.path()).is_err());
        assert!(!outside.path().join("profile.lock").exists());
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"existing").unwrap();
        assert!(Profile::open_at(root.path(), root.path(), root.path()).is_err());
        assert_eq!(fs::read(path).unwrap(), b"existing");
    }

    #[test]
    fn rejects_a_lock_symlink_without_changing_the_target() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join(PROFILE_DIR);
        fs::create_dir(&data).unwrap();
        let sentinel = root.path().join("sentinel");
        fs::write(&sentinel, b"unchanged").unwrap();
        symlink(&sentinel, data.join("profile.lock")).unwrap();
        assert!(Profile::open_at(root.path(), root.path(), root.path()).is_err());
        assert_eq!(fs::read(sentinel).unwrap(), b"unchanged");
    }

    #[test]
    fn webkit_path_requires_utf8() {
        use std::os::unix::ffi::OsStringExt;
        let invalid = PathBuf::from(std::ffi::OsString::from_vec(vec![0xff, b'x']));
        assert!(matches!(
            webkit_path(&invalid),
            Err(ProfileError::UnsupportedPath(_))
        ));
        assert!(webkit_path(Path::new("/tmp/bitter")).is_ok());
    }

    #[test]
    fn reports_create_dir_failures() {
        let root = tempfile::tempdir().unwrap();
        let base = root.path().join("readonly");
        fs::create_dir(&base).unwrap();
        let mut perms = fs::metadata(&base).unwrap().permissions();
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o500);
        fs::set_permissions(&base, perms.clone()).unwrap();
        let result = Profile::open_at(&base, root.path(), root.path());
        std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o700);
        fs::set_permissions(&base, perms).unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn open_uses_xdg_directories() {
        static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
        let _guard = LOCK.lock().unwrap();
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config");
        let data = root.path().join("data");
        let cache = root.path().join("cache");
        let old_config = std::env::var_os("XDG_CONFIG_HOME");
        let old_data = std::env::var_os("XDG_DATA_HOME");
        let old_cache = std::env::var_os("XDG_CACHE_HOME");
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", &config);
            std::env::set_var("XDG_DATA_HOME", &data);
            std::env::set_var("XDG_CACHE_HOME", &cache);
        }
        let profile = Profile::open().unwrap();
        assert_eq!(
            profile.config_path(),
            config.join("bitter-browser/config.toml")
        );
        unsafe {
            match old_config {
                Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
                None => std::env::remove_var("XDG_CONFIG_HOME"),
            }
            match old_data {
                Some(value) => std::env::set_var("XDG_DATA_HOME", value),
                None => std::env::remove_var("XDG_DATA_HOME"),
            }
            match old_cache {
                Some(value) => std::env::set_var("XDG_CACHE_HOME", value),
                None => std::env::remove_var("XDG_CACHE_HOME"),
            }
        }
        drop(profile);
    }

    #[test]
    fn rejects_a_lock_file_that_is_not_a_regular_file() {
        let root = tempfile::tempdir().unwrap();
        let data = root.path().join(PROFILE_DIR);
        fs::create_dir(&data).unwrap();
        let lock = data.join("profile.lock");
        let c_path = std::ffi::CString::new(lock.as_os_str().as_encoded_bytes()).unwrap();
        let rc = unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) };
        assert_eq!(rc, 0);
        assert!(Profile::open_at(root.path(), root.path(), root.path()).is_err());
    }
}
