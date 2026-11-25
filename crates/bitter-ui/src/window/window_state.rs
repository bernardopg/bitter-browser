//! Window size and maximized state, kept in GSettings when the schema is
//! installed. Uninstalled development builds simply use the defaults.

use gtk4::{gio, prelude::*};
use libadwaita as adw;

const SCHEMA_ID: &str = "br.com.bebitterbebetter.browser.Bitter";
const MIN_WIDTH: i32 = 640;
const MIN_HEIGHT: i32 = 480;

/// Applies the saved geometry to `window` and returns the settings to update.
pub fn load(window: &adw::ApplicationWindow) -> Option<gio::Settings> {
    let schema = gio::SettingsSchemaSource::default()?.lookup(SCHEMA_ID, true)?;
    let settings = gio::Settings::new_full(&schema, gio::SettingsBackend::NONE, None);
    let width = settings.int("window-width").max(MIN_WIDTH);
    let height = settings.int("window-height").max(MIN_HEIGHT);
    window.set_default_size(width, height);
    if settings.boolean("is-maximized") {
        window.maximize();
    }
    Some(settings)
}

pub fn save(window: &adw::ApplicationWindow, settings: &gio::Settings) {
    let maximized = window.is_maximized();
    let result = (|| {
        if !maximized && !window.is_fullscreen() {
            let (width, height) = window.default_size();
            if width > 0 && height > 0 {
                settings.set_int("window-width", width.max(MIN_WIDTH))?;
                settings.set_int("window-height", height.max(MIN_HEIGHT))?;
            }
        }
        settings.set_boolean("is-maximized", maximized)
    })();
    if let Err(error) = result {
        tracing::warn!("Could not save the window size: {error}");
    }
    gio::Settings::sync();
}
