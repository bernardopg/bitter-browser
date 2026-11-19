//! File naming for downloads. Names come from untrusted servers, so every
//! destination stays inside the chosen directory and never replaces a file.

use std::path::{Path, PathBuf};

const FALLBACK_NAME: &str = "download";
const MAX_NAME_BYTES: usize = 200;

/// Turns a server-suggested name into a single, visible path component.
pub fn safe_file_name(suggested: &str) -> String {
    let base = suggested
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .chars()
        .map(|ch| match ch {
            ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            ch if ch.is_control() => '_',
            ch => ch,
        })
        .collect::<String>();
    let trimmed = base.trim_matches(|ch: char| ch.is_whitespace() || ch == '.');
    if trimmed.is_empty() || trimmed.chars().all(|ch| ch == '_') {
        return FALLBACK_NAME.into();
    }
    truncate_keeping_extension(trimmed, MAX_NAME_BYTES)
}

/// Returns `dir/name`, or `dir/stem (n).ext` for the first name that is
/// neither on disk nor `reserved` by a download still in progress.
pub fn unique_destination(dir: &Path, name: &str, reserved: impl Fn(&Path) -> bool) -> PathBuf {
    let name = safe_file_name(name);
    let free = |path: &Path| !reserved(path) && !exists(path);
    let candidate = dir.join(&name);
    if free(&candidate) {
        return candidate;
    }
    let (stem, extension) = split_extension(&name);
    (1..)
        .map(|index| dir.join(format!("{stem} ({index}){extension}")))
        .find(|path| free(path))
        .expect("an unused name exists")
}

fn exists(path: &Path) -> bool {
    // Dangling symlinks count as taken so a download never follows them.
    path.symlink_metadata().is_ok()
}

fn split_extension(name: &str) -> (&str, &str) {
    let compound = [".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst"]
        .into_iter()
        .find(|suffix| name.len() > suffix.len() && name.to_ascii_lowercase().ends_with(suffix));
    if let Some(suffix) = compound {
        return name.split_at(name.len() - suffix.len());
    }
    match name.rfind('.') {
        Some(index) if index > 0 => name.split_at(index),
        _ => (name, ""),
    }
}

fn truncate_keeping_extension(name: &str, limit: usize) -> String {
    if name.len() <= limit {
        return name.into();
    }
    let (stem, extension) = split_extension(name);
    let extension = if extension.len() <= 16 { extension } else { "" };
    let mut end = limit.saturating_sub(extension.len()).min(stem.len());
    while !stem.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{extension}", &stem[..end])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_names_become_one_visible_component() {
        for (input, expected) in [
            ("report.pdf", "report.pdf"),
            ("../../etc/passwd", "passwd"),
            ("..\\..\\boot.ini", "boot.ini"),
            ("/absolute/path.txt", "path.txt"),
            (".bashrc", "bashrc"),
            ("  notes.txt. ", "notes.txt"),
            ("bad\u{1}name\n.txt", "bad_name_.txt"),
            ("a:b*c?.txt", "a_b_c_.txt"),
            ("relatório café.odt", "relatório café.odt"),
            ("", "download"),
            ("..", "download"),
            ("///", "download"),
            ("\u{7}", "download"),
        ] {
            assert_eq!(safe_file_name(input), expected, "{input:?}");
        }
    }

    #[test]
    fn long_names_keep_their_extension_and_utf8() {
        let name = safe_file_name(&format!("{}.tar.gz", "é".repeat(300)));
        assert!(name.len() <= MAX_NAME_BYTES);
        assert!(name.ends_with(".tar.gz"));
        assert!(name.starts_with('é'));
    }

    #[test]
    fn destinations_never_reuse_existing_names() {
        let dir = tempfile::tempdir().unwrap();
        let first = unique_destination(dir.path(), "file.tar.gz", |_| false);
        assert_eq!(first, dir.path().join("file.tar.gz"));
        std::fs::write(&first, "").unwrap();
        let second = unique_destination(dir.path(), "file.tar.gz", |_| false);
        assert_eq!(second, dir.path().join("file (1).tar.gz"));
        std::fs::write(&second, "").unwrap();
        std::os::unix::fs::symlink(
            dir.path().join("missing"),
            dir.path().join("file (2).tar.gz"),
        )
        .unwrap();
        assert_eq!(
            unique_destination(dir.path(), "file.tar.gz", |_| false),
            dir.path().join("file (3).tar.gz")
        );
        assert_eq!(
            unique_destination(dir.path(), "../file.tar.gz", |_| false).parent(),
            Some(dir.path())
        );
        std::fs::write(dir.path().join("README"), "").unwrap();
        assert_eq!(
            unique_destination(dir.path(), "README", |_| false),
            dir.path().join("README (1)")
        );
        let reserved = dir.path().join("new.txt");
        assert_eq!(
            unique_destination(dir.path(), "new.txt", |path| path == reserved),
            dir.path().join("new (1).txt")
        );
    }
}
