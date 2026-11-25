//! Registers Bitter Browser as the desktop's default web browser.
//!
//! Desktop environments read the default browser from the freedesktop.org
//! `mimeapps.list` file (GNOME, Cinnamon, MATE, Budgie and others), and
//! some keep their own copy as well (`kdeglobals` on KDE Plasma, the exo
//! helpers on Xfce). GIO writes `mimeapps.list`; `xdg-settings`, when it is
//! installed, updates the desktop-specific settings too.

use gtk4::gio;
use gtk4::gio::prelude::*;
use std::path::Path;

pub const DESKTOP_ID: &str = "br.com.bebitterbebetter.browser.Bitter.desktop";

/// Content types a default web browser opens.
pub const HANDLED_TYPES: [&str; 4] = [
    "x-scheme-handler/http",
    "x-scheme-handler/https",
    "text/html",
    "application/xhtml+xml",
];

/// How the browser can become the default one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Support {
    /// The browser can register itself.
    Available,
    /// A sandbox (Flatpak) cannot change host settings; the desktop's own
    /// settings must be used.
    SystemSettings,
}

pub fn support() -> Support {
    support_for(Path::new("/.flatpak-info").exists())
}

fn support_for(sandboxed: bool) -> Support {
    if sandboxed {
        Support::SystemSettings
    } else {
        Support::Available
    }
}

/// What the Default browser row in Preferences shows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowState {
    /// Points to the desktop's settings (Flatpak).
    SystemSettings,
    /// This browser already opens web links.
    IsDefault,
    /// Offers the Make Default button.
    CanMakeDefault,
}

/// Decides the row. A sandbox is checked first: inside it GIO only sees the
/// application's own desktop file, so it would always report this browser
/// as the default.
pub fn row_state(support: Support, is_default: impl FnOnce() -> bool) -> RowState {
    match support {
        Support::SystemSettings => RowState::SystemSettings,
        Support::Available if is_default() => RowState::IsDefault,
        Support::Available => RowState::CanMakeDefault,
    }
}

/// Whether the desktop opens web links with this browser.
pub fn is_default() -> bool {
    gio::AppInfo::default_for_type(HANDLED_TYPES[1], true)
        .and_then(|app| app.id())
        .is_some_and(|id| is_own_id(&id))
}

fn is_own_id(id: &str) -> bool {
    id == DESKTOP_ID
}

/// Makes this browser the default for web links and HTML files.
pub fn make_default() -> Result<(), String> {
    let app = gio::AppInfo::all()
        .into_iter()
        .find(|app| app.id().is_some_and(|id| is_own_id(&id)))
        .ok_or_else(|| format!("{DESKTOP_ID} is not installed"))?;
    for content_type in HANDLED_TYPES {
        app.set_as_default_for_type(content_type)
            .map_err(|error| error.to_string())?;
    }
    // Updates the desktop-specific settings as well; mimeapps.list above
    // already covers desktops without them, so a failure is not an error.
    if let Ok(process) = gio::Subprocess::newv(
        &[
            "xdg-settings".as_ref(),
            "set".as_ref(),
            "default-web-browser".as_ref(),
            DESKTOP_ID.as_ref(),
        ],
        gio::SubprocessFlags::STDOUT_SILENCE | gio::SubprocessFlags::STDERR_SILENCE,
    ) {
        process.wait_async(gio::Cancellable::NONE, |_| {});
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handles_web_links_and_html_files() {
        for content_type in [
            "x-scheme-handler/http",
            "x-scheme-handler/https",
            "text/html",
        ] {
            assert!(HANDLED_TYPES.contains(&content_type), "{content_type}");
        }
    }

    #[test]
    fn desktop_file_declares_every_handled_type() {
        let desktop = include_str!("../data/br.com.bebitterbebetter.browser.Bitter.desktop");
        let mime = desktop
            .lines()
            .find_map(|line| line.strip_prefix("MimeType="))
            .expect("MimeType line");
        for content_type in HANDLED_TYPES {
            assert!(
                mime.split(';').any(|item| item == content_type),
                "{content_type}"
            );
        }
    }

    #[test]
    fn only_the_own_desktop_id_counts_as_default() {
        assert!(is_own_id(DESKTOP_ID));
        assert!(!is_own_id("firefox.desktop"));
        assert!(!is_own_id("com.bitterbrowser.Browser.desktop"));
    }

    #[test]
    fn a_sandbox_never_claims_to_be_the_default() {
        let state = row_state(Support::SystemSettings, || panic!("not asked in a sandbox"));
        assert_eq!(state, RowState::SystemSettings);
        assert_eq!(row_state(Support::Available, || true), RowState::IsDefault);
        assert_eq!(
            row_state(Support::Available, || false),
            RowState::CanMakeDefault
        );
    }

    #[test]
    fn a_sandbox_defers_to_the_system_settings() {
        assert_eq!(support_for(true), Support::SystemSettings);
        assert_eq!(support_for(false), Support::Available);
    }
}
