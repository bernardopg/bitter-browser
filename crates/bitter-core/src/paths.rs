use std::path::PathBuf;

pub fn config_dir() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("~/.config"));
    path.push("bitter-browser");
    std::fs::create_dir_all(&path).unwrap_or_default();
    path
}

pub fn data_dir() -> PathBuf {
    let mut path = dirs::data_dir().unwrap_or_else(|| PathBuf::from("~/.local/share"));
    path.push("bitter-browser");
    std::fs::create_dir_all(&path).unwrap_or_default();
    path
}

pub fn cache_dir() -> PathBuf {
    let mut path = dirs::cache_dir().unwrap_or_else(|| PathBuf::from("~/.cache"));
    path.push("bitter-browser");
    std::fs::create_dir_all(&path).unwrap_or_default();
    path
}
