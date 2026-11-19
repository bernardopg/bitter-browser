use bitter_core::config::{
    ConfigError, ConfigStore, Preferences, SaveOutcome, SearchEngine, Theme, ZOOM_PERCENTS,
};
use bitter_core::records::{Permission, SitePermission};
use gtk4::glib;
use gtk4::prelude::*;
use libadwaita as adw;
use libadwaita::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

pub type PreferenceState = Rc<RefCell<Result<ConfigStore, ConfigError>>>;

/// Rebuilds the permission list after a change.
type Refill = RefCell<Option<Box<dyn Fn()>>>;

/// Settings pages, by name.
pub const GENERAL: &str = "general";
pub const PRIVACY: &str = "privacy";
pub const PERMISSIONS: &str = "permissions";
pub const ADVANCED: &str = "advanced";

/// What to remove when clearing browsing data.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClearRequest {
    /// Unix seconds; `None` clears everything.
    pub since: Option<i64>,
    pub history: bool,
    pub site_data: bool,
    pub cache: bool,
    pub downloads: bool,
}

/// The browser state the settings show and change, besides the config file.
pub trait Hooks {
    /// Called after preferences were saved, to apply them.
    fn applied(&self, preferences: Preferences);
    fn site_permissions(&self) -> Vec<SitePermission>;
    /// Remembers an answer, or forgets it with `None`.
    fn set_permission(&self, origin: &str, permission: Permission, allowed: Option<bool>);
    fn reset_permissions(&self);
    fn unblocked_sites(&self) -> Vec<String>;
    fn set_unblocked(&self, host: &str, unblocked: bool);
    fn blocking_status(&self) -> String;
    fn update_filter_lists(&self);
    fn clear_data(&self, request: ClearRequest, done: Box<dyn FnOnce(Result<(), String>)>);
}

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

pub fn present(
    parent: &impl IsA<gtk4::Widget>,
    state: &PreferenceState,
    hooks: Rc<dyn Hooks>,
    page: &str,
) {
    let dialog = build_dialog(state, hooks);
    dialog.set_visible_page_name(page);
    dialog.present(Some(parent));
}

/// Banners on every page that report the last save problem.
#[derive(Clone)]
struct Feedback(Rc<Vec<adw::Banner>>);

impl Feedback {
    fn show(&self, title: &str) {
        for banner in self.0.iter() {
            banner.set_title(title);
            banner.set_revealed(true);
        }
    }

    fn hide(&self) {
        for banner in self.0.iter() {
            banner.set_revealed(false);
        }
    }
}

struct Context {
    dialog: glib::WeakRef<adw::PreferencesDialog>,
    state: PreferenceState,
    hooks: Rc<dyn Hooks>,
    feedback: Feedback,
    editable: bool,
}

fn build_dialog(state: &PreferenceState, hooks: Rc<dyn Hooks>) -> adw::PreferencesDialog {
    let dialog = adw::PreferencesDialog::builder()
        .title("Bitter Browser Preferences")
        .search_enabled(true)
        .content_width(700)
        .content_height(680)
        .build();
    let load_issue = state.borrow().as_ref().err().map(ToString::to_string);
    let pages = [
        (GENERAL, "General", "preferences-system-symbolic"),
        (PRIVACY, "Privacy", "preferences-system-privacy-symbolic"),
        (PERMISSIONS, "Permissions", "system-lock-screen-symbolic"),
        (ADVANCED, "Advanced", "applications-engineering-symbolic"),
    ]
    .map(|(name, title, icon)| {
        let page = adw::PreferencesPage::builder()
            .name(name)
            .title(title)
            .icon_name(icon)
            .build();
        dialog.add(&page);
        page
    });
    let mut banners = Vec::new();
    for page in &pages {
        let group = adw::PreferencesGroup::new();
        if let Some(error) = &load_issue {
            group.set_title("Settings could not be loaded");
            group.set_description(Some("Defaults are active."));
            group.add(
                &adw::ActionRow::builder()
                    .title("Configuration file")
                    .subtitle(format!(
                        "{error}\nCorrect the file or move it to a backup, then restart."
                    ))
                    .subtitle_selectable(true)
                    .use_markup(false)
                    .build(),
            );
        } else {
            let banner = adw::Banner::new("");
            banner.set_use_markup(false);
            group.add(&banner);
            banners.push(banner);
        }
        page.add(&group);
    }
    let context = Rc::new(Context {
        dialog: dialog.downgrade(),
        state: state.clone(),
        hooks,
        feedback: Feedback(Rc::new(banners)),
        editable: load_issue.is_none(),
    });
    let [general, privacy, permissions, advanced] = &pages;
    general_page(&context, general);
    privacy_page(&context, privacy);
    permissions_page(&context, permissions);
    advanced_page(&context, advanced);
    // Rows hold the context weakly; the dialog keeps it for its lifetime.
    dialog.connect_closed(move |_| {
        let _ = &context;
    });
    dialog
}

fn group(
    page: &adw::PreferencesPage,
    title: &str,
    description: Option<&str>,
) -> adw::PreferencesGroup {
    let group = adw::PreferencesGroup::builder().title(title).build();
    group.set_description(description);
    page.add(&group);
    group
}

fn switch(
    context: &Rc<Context>,
    group: &adw::PreferencesGroup,
    title: &str,
    subtitle: &str,
    get: fn(&Preferences) -> bool,
    set: fn(&mut Preferences, bool),
) -> adw::SwitchRow {
    let row = adw::SwitchRow::builder()
        .title(title)
        .subtitle(subtitle)
        .active(get(&current(&context.state)))
        .sensitive(context.editable)
        .build();
    group.add(&row);
    if context.editable {
        let weak = Rc::downgrade(context);
        row.connect_active_notify(move |row| {
            let Some(context) = weak.upgrade() else {
                return;
            };
            let previous = current(&context.state);
            if row.is_active() == get(&previous) {
                return;
            }
            let mut next = previous;
            set(&mut next, row.is_active());
            if !save(&context, next) {
                row.set_active(get(&previous));
            }
        });
    }
    row
}

fn choice<T: Copy + PartialEq + 'static>(
    context: &Rc<Context>,
    group: &adw::PreferencesGroup,
    title: &str,
    subtitle: &str,
    choices: Vec<(T, String)>,
    get: fn(&Preferences) -> T,
    set: fn(&mut Preferences, T),
) -> adw::ComboRow {
    let labels: Vec<&str> = choices.iter().map(|(_, label)| label.as_str()).collect();
    let selected = index_of(&choices, get(&current(&context.state)));
    let row = adw::ComboRow::builder()
        .title(title)
        .subtitle(subtitle)
        .model(&gtk4::StringList::new(&labels))
        .selected(selected)
        .sensitive(context.editable)
        .build();
    group.add(&row);
    if context.editable {
        let weak = Rc::downgrade(context);
        row.connect_selected_notify(move |row| {
            let Some(context) = weak.upgrade() else {
                return;
            };
            let Some(&(value, _)) = choices.get(row.selected() as usize) else {
                return;
            };
            let previous = current(&context.state);
            if value == get(&previous) {
                return;
            }
            let mut next = previous;
            set(&mut next, value);
            if !save(&context, next) {
                row.set_selected(index_of(&choices, get(&previous)));
            }
        });
    }
    row
}

fn index_of<T: PartialEq>(choices: &[(T, String)], value: T) -> u32 {
    choices
        .iter()
        .position(|(candidate, _)| *candidate == value)
        .unwrap_or_default() as u32
}

fn general_page(context: &Rc<Context>, page: &adw::PreferencesPage) {
    let startup = group(page, "Startup", None);
    switch(
        context,
        &startup,
        "Restore previous tabs",
        "Applies the next time you start Bitter Browser",
        |prefs| prefs.restore_session,
        |prefs, value| prefs.restore_session = value,
    );
    let search = group(page, "Search", None);
    choice(
        context,
        &search,
        "Address bar search",
        "Search provider for text entered in the address bar",
        SearchEngine::ALL
            .map(|engine| (engine, engine.label().to_owned()))
            .to_vec(),
        |prefs| prefs.search,
        |prefs, value| prefs.search = value,
    );
    let appearance = group(page, "Appearance", None);
    choice(
        context,
        &appearance,
        "Color scheme",
        "Changes the browser controls and dialogs",
        Theme::ALL
            .map(|theme| (theme, theme.label().to_owned()))
            .to_vec(),
        |prefs| prefs.theme,
        |prefs, value| prefs.theme = value,
    );
    choice(
        context,
        &appearance,
        "Default zoom",
        "For sites without their own zoom level",
        ZOOM_PERCENTS
            .map(|percent| (percent, format!("{percent}%")))
            .to_vec(),
        |prefs| prefs.default_zoom,
        |prefs, value| prefs.default_zoom = value,
    );
    let downloads = group(page, "Downloads", None);
    switch(
        context,
        &downloads,
        "Ask where to save each file",
        "Otherwise files go to your downloads folder",
        |prefs| prefs.ask_download_location,
        |prefs, value| prefs.ask_download_location = value,
    );
    let writing = group(page, "Writing", None);
    switch(
        context,
        &writing,
        "Check spelling",
        "Underline misspelled words in text fields, in your system languages",
        |prefs| prefs.spell_check,
        |prefs, value| prefs.spell_check = value,
    );
}

fn privacy_page(context: &Rc<Context>, page: &adw::PreferencesPage) {
    let blocking = group(
        page,
        "Content Blocking",
        Some("Uses the EasyList and EasyPrivacy filter lists, refreshed every few days."),
    );
    let block = switch(
        context,
        &blocking,
        "Block ads and trackers",
        "",
        |prefs| prefs.block_ads,
        |prefs, value| prefs.block_ads = value,
    );
    block.set_subtitle(&context.hooks.blocking_status());
    let weak_row = block.downgrade();
    let hooks = context.hooks.clone();
    glib::timeout_add_local(Duration::from_secs(1), move || {
        let Some(row) = weak_row.upgrade() else {
            return glib::ControlFlow::Break;
        };
        let status = hooks.blocking_status();
        if row.subtitle().as_deref() != Some(status.as_str()) {
            row.set_subtitle(&status);
        }
        glib::ControlFlow::Continue
    });
    let update = gtk4::Button::builder()
        .label("Update Now")
        .valign(gtk4::Align::Center)
        .build();
    let update_row = adw::ActionRow::builder()
        .title("Filter lists")
        .subtitle("Download the latest versions")
        .build();
    update_row.add_suffix(&update);
    update_row.set_activatable_widget(Some(&update));
    let hooks = context.hooks.clone();
    update.connect_clicked(move |_| hooks.update_filter_lists());
    blocking.add(&update_row);
    let exceptions = adw::ExpanderRow::builder()
        .title("Sites with blocking off")
        .subtitle("Turn blocking off for a site from its address bar icon")
        .build();
    blocking.add(&exceptions);
    fill_exceptions(context, &exceptions);

    let tracking = group(page, "Tracking", None);
    switch(
        context,
        &tracking,
        "Prevent cross-site tracking",
        "Limits cookies and storage of sites that follow you across the web",
        |prefs| prefs.prevent_tracking,
        |prefs, value| prefs.prevent_tracking = value,
    );
    switch(
        context,
        &tracking,
        "Block third-party cookies",
        "Some sign-in and payment pages embedded in other sites may stop working",
        |prefs| prefs.block_third_party_cookies,
        |prefs, value| prefs.block_third_party_cookies = value,
    );

    let security = group(page, "Security", None);
    switch(
        context,
        &security,
        "HTTPS-only mode",
        "Load sites over a secure connection and ask before opening insecure ones",
        |prefs| prefs.https_only,
        |prefs, value| prefs.https_only = value,
    );

    let data = group(page, "Browsing Data", None);
    let clear = adw::ButtonRow::builder()
        .title("Clear Browsing Data…")
        .start_icon_name("user-trash-symbolic")
        .build();
    let weak = Rc::downgrade(context);
    clear.connect_activated(move |_| {
        if let Some(context) = weak.upgrade()
            && let Some(dialog) = context.dialog.upgrade()
        {
            let weak_dialog = dialog.downgrade();
            present_clear_data(&dialog, context.hooks.clone(), move |message| {
                if let Some(dialog) = weak_dialog.upgrade() {
                    dialog.add_toast(adw::Toast::new(&message));
                }
            });
        }
    });
    data.add(&clear);
}

fn fill_exceptions(context: &Rc<Context>, expander: &adw::ExpanderRow) {
    let sites = context.hooks.unblocked_sites();
    expander.set_subtitle(&match sites.len() {
        0 => "None. Turn blocking off for a site from its address bar icon".to_owned(),
        1 => "1 site".to_owned(),
        count => format!("{count} sites"),
    });
    expander.set_enable_expansion(!sites.is_empty());
    for host in sites {
        let row = adw::ActionRow::builder().title(&host).build();
        let remove = gtk4::Button::builder()
            .icon_name("edit-delete-symbolic")
            .tooltip_text("Block ads on this site again")
            .valign(gtk4::Align::Center)
            .css_classes(["flat"])
            .build();
        row.add_suffix(&remove);
        expander.add_row(&row);
        let weak = Rc::downgrade(context);
        let weak_expander = expander.downgrade();
        let weak_row = row.downgrade();
        remove.connect_clicked(move |_| {
            if let (Some(context), Some(expander), Some(row)) =
                (weak.upgrade(), weak_expander.upgrade(), weak_row.upgrade())
            {
                context.hooks.set_unblocked(&host, false);
                expander.remove(&row);
                let remaining = context.hooks.unblocked_sites().len();
                expander.set_subtitle(&match remaining {
                    0 => "None".to_owned(),
                    1 => "1 site".to_owned(),
                    count => format!("{count} sites"),
                });
            }
        });
    }
}

fn permissions_page(context: &Rc<Context>, page: &adw::PreferencesPage) {
    let list = group(
        page,
        "Site Permissions",
        Some(
            "Answers you gave when sites asked for access. Sites ask again after you forget an answer.",
        ),
    );
    let reset = gtk4::Button::builder()
        .label("Forget All")
        .valign(gtk4::Align::Center)
        .css_classes(["destructive-action"])
        .build();
    list.set_header_suffix(Some(&reset));
    let rows: Rc<RefCell<Vec<gtk4::Widget>>> = Rc::default();
    let refill: Rc<Refill> = Rc::default();
    let fill = {
        let weak = Rc::downgrade(context);
        let list = list.downgrade();
        let rows = rows.clone();
        let refill = Rc::downgrade(&refill);
        let reset = reset.downgrade();
        move || {
            let (Some(context), Some(list), Some(reset)) =
                (weak.upgrade(), list.upgrade(), reset.upgrade())
            else {
                return;
            };
            for row in rows.borrow_mut().drain(..) {
                list.remove(&row);
            }
            let entries = context.hooks.site_permissions();
            reset.set_sensitive(!entries.is_empty());
            if entries.is_empty() {
                let empty = adw::ActionRow::builder()
                    .title("No site has asked for access yet")
                    .css_classes(["dim-label"])
                    .build();
                list.add(&empty);
                rows.borrow_mut().push(empty.upcast());
                return;
            }
            let mut origins: Vec<String> = Vec::new();
            for entry in &entries {
                if !origins.contains(&entry.origin) {
                    origins.push(entry.origin.clone());
                }
            }
            origins.sort();
            for origin in origins {
                let site = adw::ExpanderRow::builder()
                    .title(display_origin(&origin))
                    .subtitle(&origin)
                    .build();
                for entry in entries.iter().filter(|entry| entry.origin == origin) {
                    site.add_row(&permission_row(&context, entry, refill.clone()));
                }
                list.add(&site);
                rows.borrow_mut().push(site.upcast());
            }
        }
    };
    fill();
    refill.replace(Some(Box::new(fill)));
    let weak = Rc::downgrade(context);
    // The button owns the list builder, so it lives as long as the page.
    reset.connect_clicked(move |_| {
        if let Some(context) = weak.upgrade() {
            context.hooks.reset_permissions();
            if let Some(fill) = refill.borrow().as_ref() {
                fill();
            }
        }
    });
}

fn permission_row(
    context: &Rc<Context>,
    entry: &SitePermission,
    refill: std::rc::Weak<Refill>,
) -> adw::ComboRow {
    let row = adw::ComboRow::builder()
        .title(entry.permission.label())
        .model(&gtk4::StringList::new(&["Allow", "Block"]))
        .selected(if entry.allowed { 0 } else { 1 })
        .build();
    let forget = gtk4::Button::builder()
        .icon_name("edit-delete-symbolic")
        .tooltip_text("Forget, so the site asks again")
        .valign(gtk4::Align::Center)
        .css_classes(["flat"])
        .build();
    row.add_suffix(&forget);
    let origin = entry.origin.clone();
    let permission = entry.permission;
    let hooks = context.hooks.clone();
    row.connect_selected_notify(move |row| {
        hooks.set_permission(&origin, permission, Some(row.selected() == 0));
    });
    let origin = entry.origin.clone();
    let hooks = context.hooks.clone();
    forget.connect_clicked(move |_| {
        hooks.set_permission(&origin, permission, None);
        // Rebuild after the click handler returns; it removes this row.
        let refill = refill.clone();
        glib::idle_add_local_once(move || {
            if let Some(refill) = refill.upgrade()
                && let Some(fill) = refill.borrow().as_ref()
            {
                fill();
            }
        });
    });
    row
}

fn display_origin(origin: &str) -> String {
    url::Url::parse(origin)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| origin.to_owned())
}

fn advanced_page(context: &Rc<Context>, page: &adw::PreferencesPage) {
    let developer = group(page, "Developers", None);
    switch(
        context,
        &developer,
        "Developer tools",
        "Web Inspector with F12, Ctrl+Shift+I or Inspect Element",
        |prefs| prefs.developer_tools,
        |prefs, value| prefs.developer_tools = value,
    );
    page.add(
        &adw::PreferencesGroup::builder()
            .title("About")
            .description(format!(
                "Bitter Browser {} · WebKitGTK {}.{}.{}",
                env!("CARGO_PKG_VERSION"),
                webkit6::functions::major_version(),
                webkit6::functions::minor_version(),
                webkit6::functions::micro_version(),
            ))
            .build(),
    );
}

fn save(context: &Context, next: Preferences) -> bool {
    let outcome = {
        let mut state = context.state.borrow_mut();
        let Ok(store) = state.as_mut() else {
            return false;
        };
        store.save(next)
    };
    match outcome {
        Ok(SaveOutcome::Durable) => {
            apply_theme(next.theme);
            context.feedback.hide();
            context.hooks.applied(next);
            true
        }
        Ok(SaveOutcome::DirectorySyncFailed(error)) => {
            apply_theme(next.theme);
            context
                .feedback
                .show("Change applied. The filesystem could not confirm it will survive a crash.");
            tracing::warn!("Could not sync configuration directory: {error}");
            context.hooks.applied(next);
            true
        }
        Err(error) => {
            context
                .feedback
                .show("Could not save preferences. Your previous settings are still active.");
            if let Some(dialog) = context.dialog.upgrade() {
                show_error(&dialog, &error.to_string());
            }
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

/// Time ranges offered when clearing data, in seconds back from now.
const CLEAR_RANGES: [(&str, Option<i64>); 5] = [
    ("Last hour", Some(60 * 60)),
    ("Last 24 hours", Some(24 * 60 * 60)),
    ("Last 7 days", Some(7 * 24 * 60 * 60)),
    ("Last 4 weeks", Some(28 * 24 * 60 * 60)),
    ("All time", None),
];

/// Asks what to clear, clears it and reports the outcome to `done`.
pub fn present_clear_data(
    parent: &impl IsA<gtk4::Widget>,
    hooks: Rc<dyn Hooks>,
    done: impl Fn(String) + 'static,
) {
    let alert = adw::AlertDialog::builder()
        .heading("Clear Browsing Data")
        .body("Removes the selected data from this device.")
        .build();
    let content = gtk4::Box::builder()
        .orientation(gtk4::Orientation::Vertical)
        .spacing(6)
        .build();
    let range = gtk4::DropDown::from_strings(&CLEAR_RANGES.map(|(label, _)| label));
    range.set_selected(1);
    range.update_property(&[gtk4::accessible::Property::Label("Time range")]);
    content.append(&range);
    let check = |label: &str, active: bool| {
        let check = gtk4::CheckButton::builder()
            .label(label)
            .active(active)
            .build();
        content.append(&check);
        check
    };
    let history = check("Browsing history", true);
    let site_data = check("Cookies and site data (signs you out of these sites)", true);
    let cache = check("Cached images and files", true);
    let downloads = check("Download list (all finished downloads)", false);
    alert.set_extra_child(Some(&content));
    alert.add_response("cancel", "Cancel");
    alert.add_response("clear", "Clear Data");
    alert.set_response_appearance("clear", adw::ResponseAppearance::Destructive);
    alert.set_default_response(Some("cancel"));
    alert.set_close_response("cancel");
    let done: Rc<dyn Fn(String)> = Rc::new(done);
    alert.connect_response(Some("clear"), move |_, _| {
        let since = CLEAR_RANGES[range.selected() as usize]
            .1
            .map(|seconds| glib::real_time() / 1_000_000 - seconds);
        let request = ClearRequest {
            since,
            history: history.is_active(),
            site_data: site_data.is_active(),
            cache: cache.is_active(),
            downloads: downloads.is_active(),
        };
        if !(request.history || request.site_data || request.cache || request.downloads) {
            return;
        }
        let done = done.clone();
        hooks.clear_data(
            request,
            Box::new(move |result| {
                done(match result {
                    Ok(()) => "Browsing data cleared".to_owned(),
                    Err(error) => format!("Some data could not be cleared: {error}"),
                })
            }),
        );
    });
    alert.present(Some(parent));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

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

    #[derive(Default)]
    struct FakeHooks {
        applied: Cell<Option<Preferences>>,
        permissions: RefCell<Vec<SitePermission>>,
    }

    impl Hooks for FakeHooks {
        fn applied(&self, preferences: Preferences) {
            self.applied.set(Some(preferences));
        }
        fn site_permissions(&self) -> Vec<SitePermission> {
            self.permissions.borrow().clone()
        }
        fn set_permission(&self, origin: &str, permission: Permission, allowed: Option<bool>) {
            let mut permissions = self.permissions.borrow_mut();
            permissions.retain(|entry| !(entry.origin == origin && entry.permission == permission));
            if let Some(allowed) = allowed {
                permissions.push(SitePermission {
                    origin: origin.to_owned(),
                    permission,
                    allowed,
                });
            }
        }
        fn reset_permissions(&self) {
            self.permissions.borrow_mut().clear();
        }
        fn unblocked_sites(&self) -> Vec<String> {
            Vec::new()
        }
        fn set_unblocked(&self, _: &str, _: bool) {}
        fn blocking_status(&self) -> String {
            "Ready".into()
        }
        fn update_filter_lists(&self) {}
        fn clear_data(&self, _: ClearRequest, done: Box<dyn FnOnce(Result<(), String>)>) {
            done(Ok(()));
        }
    }

    fn switch_titled(dialog: &adw::PreferencesDialog, title: &str) -> adw::SwitchRow {
        children_of::<adw::SwitchRow>(dialog.upcast_ref())
            .into_iter()
            .find(|row| row.title() == title)
            .unwrap()
    }

    fn combo_titled(dialog: &adw::PreferencesDialog, title: &str) -> adw::ComboRow {
        children_of::<adw::ComboRow>(dialog.upcast_ref())
            .into_iter()
            .find(|row| row.title() == title)
            .unwrap()
    }

    #[gtk4::test]
    #[ignore = "requires a display; run with xvfb-run"]
    fn settings_widgets_save_restore_and_report_errors() {
        adw::init().unwrap();
        let profile = tempfile::tempdir().unwrap();
        let path = profile.path().join("config.toml");
        let state = Rc::new(RefCell::new(ConfigStore::open(path.clone())));
        let hooks = Rc::new(FakeHooks::default());
        hooks.set_permission("https://meet.example.test", Permission::Camera, Some(true));
        let window = adw::Window::new();
        window.present();
        let dialog = build_dialog(&state, hooks.clone());
        dialog.present(Some(&window));

        combo_titled(&dialog, "Address bar search").set_selected(1);
        combo_titled(&dialog, "Color scheme").set_selected(2);
        combo_titled(&dialog, "Default zoom").set_selected(7);
        assert_eq!(
            adw::StyleManager::default().color_scheme(),
            adw::ColorScheme::ForceDark
        );
        switch_titled(&dialog, "Restore previous tabs").set_active(false);
        switch_titled(&dialog, "Ask where to save each file").set_active(true);
        switch_titled(&dialog, "HTTPS-only mode").set_active(true);
        switch_titled(&dialog, "Block ads and trackers").set_active(false);
        switch_titled(&dialog, "Developer tools").set_active(false);
        let expected = Preferences {
            search: SearchEngine::Brave,
            theme: Theme::Dark,
            default_zoom: 125,
            restore_session: false,
            ask_download_location: true,
            https_only: true,
            block_ads: false,
            developer_tools: false,
            ..Preferences::default()
        };
        assert_eq!(ConfigStore::open(path.clone()).unwrap().current(), expected);
        assert_eq!(hooks.applied.get(), Some(expected));

        // The permission list shows the remembered answer and changes it.
        let camera = combo_titled(&dialog, "Camera");
        assert_eq!(camera.selected(), 0);
        camera.set_selected(1);
        assert!(
            !hooks.permissions.borrow()[0].allowed,
            "{:?}",
            hooks.permissions.borrow()
        );

        // Simulate an editor saving while the dialog is open.
        let edited = b"[appearance]\ntheme = 'light'";
        std::fs::write(&path, edited).unwrap();
        let theme = combo_titled(&dialog, "Color scheme");
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
        let dialog = build_dialog(&invalid, hooks);
        dialog.present(Some(&recovery));
        assert_eq!(current(&invalid), Preferences::default());
        let rows = children_of::<adw::ComboRow>(dialog.upcast_ref());
        assert!(
            rows.iter()
                .filter(|row| row.title() != "Camera")
                .all(|row| !row.is_sensitive())
        );
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
