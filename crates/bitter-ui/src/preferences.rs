use bitter_core::config::{
    ConfigError, ConfigStore, Preferences, SaveOutcome, SearchEngine, Theme,
};
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;

pub type PreferenceState = Rc<RefCell<Result<ConfigStore, ConfigError>>>;

pub fn current(state: &PreferenceState) -> Preferences {
    state
        .borrow()
        .as_ref()
        .map_or_else(|_| Preferences::default(), ConfigStore::current)
}

pub fn apply_theme(theme: Theme) {
    let scheme = match theme {
        Theme::System => adw::ColorScheme::Default,
        Theme::Light => adw::ColorScheme::ForceLight,
        Theme::Dark => adw::ColorScheme::ForceDark,
    };
    adw::StyleManager::default().set_color_scheme(scheme);
}

pub fn present(parent: &impl IsA<gtk4::Widget>, state: &PreferenceState) {
    build_dialog(state).present(Some(parent));
}

fn build_dialog(state: &PreferenceState) -> adw::PreferencesDialog {
    let dialog = adw::PreferencesDialog::builder()
        .title("Bitter Browser Preferences")
        .search_enabled(false)
        .content_width(560)
        .content_height(640)
        .build();
    let page = adw::PreferencesPage::new();
    dialog.add(&page);

    let status = adw::Banner::new("");
    status.set_use_markup(false);
    let load_issue = state.borrow().as_ref().err().map(ToString::to_string);
    let feedback = adw::PreferencesGroup::new();
    if let Some(error) = &load_issue {
        feedback.set_title("Settings could not be loaded");
        feedback.set_description(Some("Defaults are active."));
        let detail = adw::ActionRow::builder()
            .title("Configuration file")
            .subtitle(format!(
                "{error}\nCorrect the file or move it to a backup, then restart."
            ))
            .subtitle_selectable(true)
            .use_markup(false)
            .build();
        feedback.add(&detail);
    } else {
        feedback.add(&status);
    }
    page.add(&feedback);

    let active = current(state);
    let search_group = adw::PreferencesGroup::builder().title("Search").build();
    let search = adw::ComboRow::builder()
        .title("Address bar search")
        .subtitle("Search provider for text entered in the address bar")
        .model(&gtk4::StringList::new(
            &SearchEngine::ALL.map(SearchEngine::label),
        ))
        .selected(index_of(&SearchEngine::ALL, active.search))
        .sensitive(load_issue.is_none())
        .build();
    search_group.add(&search);
    page.add(&search_group);

    let startup = adw::PreferencesGroup::builder().title("Startup").build();
    let restore = adw::SwitchRow::builder()
        .title("Restore previous tabs")
        .subtitle("Applies the next time you start Bitter Browser")
        .active(active.restore_session)
        .sensitive(load_issue.is_none())
        .build();
    startup.add(&restore);
    page.add(&startup);

    let downloads = adw::PreferencesGroup::builder().title("Downloads").build();
    let ask_location = adw::SwitchRow::builder()
        .title("Ask where to save each file")
        .subtitle("Otherwise files go to your downloads folder")
        .active(active.ask_download_location)
        .sensitive(load_issue.is_none())
        .build();
    downloads.add(&ask_location);
    page.add(&downloads);

    let appearance = adw::PreferencesGroup::builder().title("Appearance").build();
    let theme = adw::ComboRow::builder()
        .title("Color scheme")
        .subtitle("Changes the browser controls and dialogs")
        .model(&gtk4::StringList::new(&Theme::ALL.map(Theme::label)))
        .selected(index_of(&Theme::ALL, active.theme))
        .sensitive(load_issue.is_none())
        .build();
    appearance.add(&theme);
    page.add(&appearance);

    let privacy = adw::PreferencesGroup::builder()
        .title("Privacy controls")
        .description("These browser controls are currently unavailable.")
        .build();
    for title in ["Ad and tracker blocking", "HTTPS-only mode"] {
        let row = adw::ActionRow::builder().title(title).build();
        let icon = gtk4::Image::from_icon_name("dialog-information-symbolic");
        icon.add_css_class("dim-label");
        row.add_prefix(&icon);
        privacy.add(&row);
    }
    page.add(&privacy);
    page.add(
        &adw::PreferencesGroup::builder()
            .title("About")
            .description(format!(
                "Bitter Browser {} · Experimental build",
                env!("CARGO_PKG_VERSION")
            ))
            .build(),
    );

    if load_issue.is_none() {
        connect_switch(
            &restore,
            &dialog,
            &status,
            state,
            |prefs| prefs.restore_session,
            |prefs, value| prefs.restore_session = value,
        );
        connect_switch(
            &ask_location,
            &dialog,
            &status,
            state,
            |prefs| prefs.ask_download_location,
            |prefs, value| prefs.ask_download_location = value,
        );
        connect_setting(
            &search,
            &dialog,
            &status,
            state,
            SearchEngine::ALL,
            |prefs| prefs.search,
            |prefs, value| prefs.search = value,
        );
        connect_setting(
            &theme,
            &dialog,
            &status,
            state,
            Theme::ALL,
            |prefs| prefs.theme,
            |prefs, value| prefs.theme = value,
        );
    }
    dialog
}

fn connect_switch(
    row: &adw::SwitchRow,
    dialog: &adw::PreferencesDialog,
    status: &adw::Banner,
    state: &PreferenceState,
    get: fn(&Preferences) -> bool,
    set: fn(&mut Preferences, bool),
) {
    let state = state.clone();
    let weak_dialog = dialog.downgrade();
    let weak_status = status.downgrade();
    row.connect_active_notify(move |row| {
        let (Some(dialog), Some(status)) = (weak_dialog.upgrade(), weak_status.upgrade()) else {
            return;
        };
        let previous = current(&state);
        if row.is_active() == get(&previous) {
            return;
        }
        let mut next = previous;
        set(&mut next, row.is_active());
        if !save(&state, &dialog, &status, next) {
            row.set_active(get(&previous));
        }
    });
}

fn index_of<T: PartialEq>(values: &[T], value: T) -> u32 {
    values
        .iter()
        .position(|candidate| *candidate == value)
        .expect("all supported preference values") as u32
}

fn connect_setting<T: Copy + Eq + 'static>(
    row: &adw::ComboRow,
    dialog: &adw::PreferencesDialog,
    status: &adw::Banner,
    state: &PreferenceState,
    choices: [T; 3],
    get: impl Fn(Preferences) -> T + 'static,
    set: impl Fn(&mut Preferences, T) + 'static,
) {
    let state = state.clone();
    let dialog = dialog.downgrade();
    let status = status.downgrade();
    row.connect_selected_notify(move |row| {
        let (Some(dialog), Some(status)) = (dialog.upgrade(), status.upgrade()) else {
            return;
        };
        let Some(&selected) = choices.get(row.selected() as usize) else {
            return;
        };
        let original = current(&state);
        if selected == get(original) {
            return;
        }
        let mut next = original;
        set(&mut next, selected);
        if !save(&state, &dialog, &status, next) {
            row.set_selected(index_of(&choices, get(original)));
        }
    });
}

fn save(
    state: &PreferenceState,
    dialog: &adw::PreferencesDialog,
    status: &adw::Banner,
    next: Preferences,
) -> bool {
    let outcome = {
        let mut state = state.borrow_mut();
        let Ok(store) = state.as_mut() else {
            return false;
        };
        store.save(next)
    };
    match outcome {
        Ok(SaveOutcome::Durable) => {
            apply_theme(next.theme);
            status.set_revealed(false);
            true
        }
        Ok(SaveOutcome::DirectorySyncFailed(error)) => {
            apply_theme(next.theme);
            status.set_title(
                "Change applied. The filesystem could not confirm it will survive a crash.",
            );
            status.set_revealed(true);
            tracing::warn!("Could not sync the configuration directory: {error}");
            true
        }
        Err(error) => {
            status
                .set_title("Could not save preferences. Your previous settings are still active.");
            status.set_revealed(true);
            show_error(dialog, &error.to_string());
            false
        }
    }
}

fn show_error(dialog: &adw::PreferencesDialog, error: &str) {
    let alert = adw::AlertDialog::builder()
        .heading("Preferences could not be saved")
        .body(error)
        .build();
    alert.add_response("close", "Close");
    alert.set_close_response("close");
    alert.set_default_response(Some("close"));
    alert.present(Some(dialog));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn children_of<T: IsA<gtk4::Widget> + glib::object::IsClass>(widget: &gtk4::Widget) -> Vec<T> {
        let mut matches = Vec::new();
        if let Ok(widget) = widget.clone().downcast::<T>() {
            matches.push(widget);
        }
        let mut child = widget.first_child();
        while let Some(widget) = child {
            child = widget.next_sibling();
            matches.extend(children_of(&widget));
        }
        matches
    }

    use gtk4::glib;

    #[gtk4::test]
    #[ignore = "requires a display; run with xvfb-run"]
    fn settings_widgets_save_restore_and_report_errors() {
        adw::init().unwrap();
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("config.toml");
        let state = Rc::new(RefCell::new(ConfigStore::open(path.clone())));
        let window = adw::Window::new();
        window.present();
        let dialog = build_dialog(&state);
        dialog.present(Some(&window));
        let rows = children_of::<adw::ComboRow>(dialog.upcast_ref());
        let search = rows
            .iter()
            .find(|row| row.title() == "Address bar search")
            .unwrap();
        let theme = rows
            .iter()
            .find(|row| row.title() == "Color scheme")
            .unwrap();

        search.set_selected(1);
        theme.set_selected(2);
        assert_eq!(
            adw::StyleManager::default().color_scheme(),
            adw::ColorScheme::ForceDark
        );
        let reopened = ConfigStore::open(path.clone()).unwrap();
        assert_eq!(
            reopened.current(),
            Preferences {
                search: SearchEngine::Brave,
                theme: Theme::Dark,
                ..Preferences::default()
            }
        );

        let switches = children_of::<adw::SwitchRow>(dialog.upcast_ref());
        assert_eq!(switches.len(), 2);
        switches[0].set_active(false);
        switches[1].set_active(true);
        let saved = ConfigStore::open(path.clone()).unwrap().current();
        assert!(!saved.restore_session && saved.ask_download_location);

        // Simulate an editor saving while the dialog is open.
        let edited = b"[appearance]\ntheme = 'light'";
        std::fs::write(&path, edited).unwrap();
        theme.set_selected(1);
        assert_eq!(theme.selected(), 2);
        assert_eq!(
            adw::StyleManager::default().color_scheme(),
            adw::ColorScheme::ForceDark
        );
        assert_eq!(std::fs::read(&path).unwrap(), edited);
        let banners = children_of::<adw::Banner>(dialog.upcast_ref());
        assert!(
            banners
                .iter()
                .any(|banner| banner.is_revealed() && banner.title().contains("Could not save"))
        );

        std::fs::write(&path, b"[broken configuration").unwrap();
        let invalid = Rc::new(RefCell::new(ConfigStore::open(path)));
        let recovery = adw::Window::new();
        recovery.present();
        let dialog = build_dialog(&invalid);
        dialog.present(Some(&recovery));
        assert_eq!(current(&invalid), Preferences::default());
        let rows = children_of::<adw::ComboRow>(dialog.upcast_ref());
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|row| !row.is_sensitive()));
        let groups = children_of::<adw::PreferencesGroup>(dialog.upcast_ref());
        assert!(
            groups
                .iter()
                .any(|group| group.title() == "Settings could not be loaded"
                    && group.description().as_deref() == Some("Defaults are active."))
        );
        apply_theme(Theme::System);
        recovery.close();
        window.close();
    }
}
