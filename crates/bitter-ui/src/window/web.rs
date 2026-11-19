//! Page features beyond loading: pop-up windows, permission requests, the
//! context menu, link previews, HTTPS-only mode, content blocking and the
//! engine settings that follow the preferences.

use super::*;

impl Browser {
    pub(super) fn connect_web_features(self: &Rc<Self>, tab: &Rc<Tab>, view: &TabView) {
        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_create(move |web, action| {
            let (browser, tab) = upgrade(&weak, &weak_tab)?;
            browser.open_popup(&tab, web, action)
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_close(move |_| {
            if let Some((browser, tab)) = upgrade(&weak, &weak_tab) {
                browser.close_from_page(&tab);
            }
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_permission_request(move |web, request| {
            match upgrade(&weak, &weak_tab) {
                Some((browser, tab)) => browser.permission_requested(&tab, web, request),
                None => request.deny(),
            }
            true
        });

        let weak = Rc::downgrade(self);
        view.web.connect_query_permission_state(move |_, query| {
            let Some(browser) = weak.upgrade() else {
                return false;
            };
            let permission = match query.name().as_deref() {
                Some("camera") => Permission::Camera,
                Some("microphone") => Permission::Microphone,
                Some("geolocation") => Permission::Location,
                Some("notifications") => Permission::Notifications,
                Some("clipboard-read") => Permission::Clipboard,
                _ => return false,
            };
            let Some(origin) = query
                .security_origin()
                .and_then(|origin| site_origin(&origin.to_str()))
            else {
                return false;
            };
            let state = match browser.sites.borrow().decision(&origin, permission) {
                Some(true) => webkit6::PermissionState::Granted,
                Some(false) => webkit6::PermissionState::Denied,
                None => webkit6::PermissionState::Prompt,
            };
            query.finish(state);
            true
        });

        let weak = Rc::downgrade(self);
        view.permissions.connect_answered(move |prompt, allowed| {
            if let Some(browser) = weak.upgrade()
                && let Request::Ask(remembered) = &prompt.kind
            {
                for permission in remembered {
                    browser.set_site_permission(&prompt.origin, *permission, Some(allowed));
                }
            }
        });

        let weak = Rc::downgrade(self);
        view.web.connect_context_menu(move |_, menu, hit| {
            if let Some(browser) = weak.upgrade()
                && let Some(actions) = browser.menu_actions.get()
            {
                let engine = preferences::current(&browser.preferences).search;
                context_menu::customize(menu, hit, actions, engine.label());
            }
            false
        });

        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(tab);
        view.web.connect_mouse_target_changed(move |_, hit, _| {
            if let Some((browser, tab)) = upgrade(&weak, &weak_tab)
                && let Some(view) = tab.view.get()
            {
                // A dialog covers the page; its links are out of reach.
                let covered = browser
                    .window
                    .upgrade()
                    .is_some_and(|window| window.visible_dialog().is_some());
                let link = (hit.context_is_link() && !covered)
                    .then(|| hit.link_uri())
                    .flatten();
                view.show_link(link.as_deref());
            }
        });
    }

    /// Opens a page's `window.open` request as a tab next to it.
    fn open_popup(
        self: &Rc<Self>,
        opener: &Rc<Tab>,
        web: &webkit6::WebView,
        action: &webkit6::NavigationAction,
    ) -> Option<gtk4::Widget> {
        let (workspace, index) = self.tab_position(&opener.id)?;
        if self.workspaces.borrow()[workspace.index()].tabs.len() >= MAX_TABS {
            self.toast("A page tried to open a tab, but this workspace has 100 tabs.");
            return None;
        }
        let uri = action
            .request()
            .and_then(|request| request.uri())
            .map(|uri| uri.to_string())
            .filter(|uri| !uri.is_empty())
            .unwrap_or_else(|| "about:blank".to_owned());
        let tab = Rc::new(Tab::new(SavedTab {
            uri: uri.clone(),
            title: uri.clone(),
        }));
        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(&tab);
        let view = tab.view.get_or_init(|| {
            TabView::new(Engine::Related(web), &self.web_settings, move || {
                if let Some((browser, tab)) = upgrade(&weak, &weak_tab)
                    && let Some(view) = tab.view.get()
                {
                    browser.retry(&tab, view);
                }
            })
        });
        view.opener.replace(Some(opener.id.clone()));
        *view.requested.borrow_mut() = uri.clone();
        self.connect_view(&tab, view);
        self.stack.add_child(&view.root);
        self.apply_filters(view, &uri);
        self.insert(workspace, tab.clone(), Some(index + 1));
        let weak = Rc::downgrade(self);
        let weak_tab = Rc::downgrade(&tab);
        view.web.connect_ready_to_show(move |_| {
            if let Some((browser, tab)) = upgrade(&weak, &weak_tab)
                && let Some((workspace, index)) = browser.tab_position(&tab.id)
            {
                browser.select(workspace, index);
            }
        });
        self.save_snapshot();
        Some(view.web.clone().upcast())
    }

    /// `window.close()`: closes the tab and returns to the page that opened it.
    fn close_from_page(self: &Rc<Self>, tab: &Rc<Tab>) {
        let opener = tab.view.get().and_then(|view| view.opener.borrow().clone());
        let was_active = self.is_active(tab);
        let id = tab.id.clone();
        let weak = Rc::downgrade(self);
        // The page is still running its own signal handler.
        glib::idle_add_local_once(move || {
            let Some(browser) = weak.upgrade() else {
                return;
            };
            browser.close(&id);
            if was_active
                && let Some(opener) = opener
                && let Some((workspace, index)) = browser.tab_position(&opener)
            {
                browser.select(workspace, index);
            }
        });
    }

    fn permission_requested(
        &self,
        tab: &Tab,
        web: &webkit6::WebView,
        request: &webkit6::PermissionRequest,
    ) {
        let Some(origin) = web.uri().and_then(|uri| site_origin(&uri)) else {
            request.deny();
            return;
        };
        if request.is::<webkit6::DeviceInfoPermissionRequest>() {
            // Device names are shared only with sites allowed to use them.
            let sites = self.sites.borrow();
            let allowed = [Permission::Camera, Permission::Microphone]
                .into_iter()
                .any(|permission| sites.decision(&origin, permission) == Some(true));
            if allowed {
                request.allow();
            } else {
                request.deny();
            }
            return;
        }
        let kind = permissions::classify(request);
        match &kind {
            Request::Automatic(true) => return request.allow(),
            Request::Automatic(false) => return request.deny(),
            Request::Ask(wanted) => {
                let sites = self.sites.borrow();
                let answers: Vec<_> = wanted
                    .iter()
                    .map(|permission| sites.decision(&origin, *permission))
                    .collect();
                if answers.contains(&Some(false)) {
                    return request.deny();
                }
                if answers.iter().all(|answer| *answer == Some(true)) {
                    return request.allow();
                }
            }
            Request::AskOnce(_) => {}
        }
        let Some(view) = tab.view.get() else {
            return request.deny();
        };
        view.permissions.ask(Prompt {
            request: request.clone(),
            origin,
            kind,
        });
        if !self.is_active(tab) {
            self.toast(&format!("{} is asking for permission", tab.title()));
        }
    }

    /// Remembers an answer, or forgets it with `None`.
    pub(super) fn set_site_permission(
        &self,
        origin: &str,
        permission: Permission,
        allowed: Option<bool>,
    ) {
        let result = {
            let mut sites = self.sites.borrow_mut();
            match allowed {
                Some(allowed) => sites.set(origin, permission, allowed),
                None => sites.forget(origin, permission),
            }
        };
        if let Err(error) = result {
            tracing::warn!("Could not save the site permission: {error}");
            self.toast("Could not save the permission. Check disk space and profile permissions.");
        }
        if permission == Permission::Notifications
            && let Some(context) = webkit6::WebContext::default()
        {
            self.sync_notification_permissions(&context);
        }
    }

    pub(super) fn reset_site_permissions(&self) {
        if let Err(error) = self.sites.borrow_mut().clear_permissions() {
            tracing::warn!("Could not reset site permissions: {error}");
            self.toast("Could not reset permissions. Check disk space and profile permissions.");
        }
        if let Some(context) = webkit6::WebContext::default() {
            self.sync_notification_permissions(&context);
        }
    }

    /// Gives WebKit the notification answers, so pages see them at once.
    pub(super) fn sync_notification_permissions(&self, context: &webkit6::WebContext) {
        let sites = self.sites.borrow();
        let origins = |allowed: bool| -> Vec<webkit6::SecurityOrigin> {
            sites
                .permissions()
                .iter()
                .filter(|entry| {
                    entry.permission == Permission::Notifications && entry.allowed == allowed
                })
                .map(|entry| webkit6::SecurityOrigin::for_uri(&entry.origin))
                .collect()
        };
        let allowed = origins(true);
        let denied = origins(false);
        context.initialize_notification_permissions(
            &allowed.iter().collect::<Vec<_>>(),
            &denied.iter().collect::<Vec<_>>(),
        );
    }

    /// Runs before a main-frame page loads or redirects. Returns true when
    /// the load was replaced (by its HTTPS version or a warning).
    pub(super) fn main_frame_started(&self, tab: &Tab, view: &TabView, uri: &str) -> bool {
        self.apply_filters(view, uri);
        if !preferences::current(&self.preferences).https_only {
            view.upgraded_from.replace(None);
            return false;
        }
        let secure = https_only::upgrade(uri, |host| self.insecure_allowed.borrow().contains(host));
        let Some(secure) = secure else {
            if !uri.starts_with("https:") {
                view.upgraded_from.replace(None);
            }
            return false;
        };
        // The HTTPS page sent us back to HTTP: there is no secure version.
        let original = view.upgraded_from.borrow().clone();
        if let Some(original) = original
            && https_only::is_upgrade_of(&original, &secure)
        {
            view.web.stop_loading();
            self.show_https_unavailable(tab, view, &original);
            return true;
        }
        view.upgraded_from.replace(Some(uri.to_owned()));
        let web = view.web.clone();
        glib::idle_add_local_once(move || {
            web.stop_loading();
            web.load_uri(&secure);
        });
        true
    }

    /// Handles a failed load of an HTTPS upgrade. Returns true if it was one.
    pub(super) fn secure_upgrade_failed(&self, tab: &Tab, view: &TabView, failed: &str) -> bool {
        let original = view.upgraded_from.borrow().clone();
        match original {
            Some(original) if https_only::is_upgrade_of(&original, failed) => {
                self.show_https_unavailable(tab, view, &original);
                true
            }
            _ => false,
        }
    }

    fn show_https_unavailable(&self, tab: &Tab, view: &TabView, original: &str) {
        view.upgraded_from.replace(None);
        let host = url::Url::parse(original)
            .ok()
            .and_then(|url| url.host_str().map(str::to_ascii_lowercase))
            .unwrap_or_default();
        *view.requested.borrow_mut() = original.to_owned();
        tab.update(original, original);
        view.show_problem(
            "Secure connection unavailable",
            &format!(
                "{host} does not support a secure connection. Others on your network could see or change what you send and receive on this site."
            ),
            "channel-insecure-symbolic",
        );
        let insecure = self.insecure_allowed.clone();
        let web = view.web.clone();
        let original = original.to_owned();
        view.set_alternative("Continue to HTTP Site", move || {
            insecure.borrow_mut().insert(host.clone());
            web.load_uri(&original);
        });
        self.display_active_address(tab);
        self.refresh_view(tab, view);
        self.save_snapshot();
    }

    /// Gives a page the filters for the site it is loading.
    pub(super) fn apply_filters(&self, view: &TabView, uri: &str) {
        let host = url::Url::parse(uri)
            .ok()
            .and_then(|url| url.host_str().map(str::to_ascii_lowercase));
        let blocked = host
            .as_deref()
            .is_none_or(|host| !self.sites.borrow().is_unblocked(host));
        let state = (self.blocker.revision(), blocked);
        if view.filter_state.replace(Some(state)) != Some(state) {
            self.blocker.apply(&view.content, blocked);
        }
    }

    pub(super) fn apply_filters_everywhere(&self) {
        for tab in self.loaded_tabs() {
            if let Some(view) = tab.view.get() {
                self.apply_filters(view, &tab.uri());
            }
        }
    }

    /// Tabs whose page has been created, in every workspace.
    fn loaded_tabs(&self) -> Vec<Rc<Tab>> {
        self.workspaces
            .borrow()
            .iter()
            .flat_map(|group| group.tabs.iter())
            .filter(|tab| tab.view.get().is_some())
            .cloned()
            .collect()
    }

    /// Turns content blocking off (`true`) or on for a site.
    pub(super) fn set_unblocked(&self, host: &str, unblocked: bool) {
        if let Err(error) = self.sites.borrow_mut().set_unblocked(host, unblocked) {
            tracing::warn!("Could not save the blocking exception: {error}");
            self.toast("Could not save the change. Check disk space and profile permissions.");
            return;
        }
        self.apply_filters_everywhere();
    }

    /// Applies preferences that change the engine while it runs.
    pub(super) fn apply_preferences(&self, preferences: Preferences) {
        self.web_settings
            .set_enable_developer_extras(preferences.developer_tools);
        if let Some(cookies) = self.network.cookie_manager() {
            cookies.set_accept_policy(if preferences.block_third_party_cookies {
                webkit6::CookieAcceptPolicy::NoThirdParty
            } else {
                webkit6::CookieAcceptPolicy::Always
            });
        }
        self.network.set_itp_enabled(preferences.prevent_tracking);
        if let Some(context) = webkit6::WebContext::default() {
            if preferences.spell_check {
                let names = glib::language_names();
                let languages = spell_languages(names.iter().map(|name| name.as_str()));
                context.set_spell_checking_languages(
                    &languages.iter().map(String::as_str).collect::<Vec<_>>(),
                );
            }
            context.set_spell_checking_enabled(preferences.spell_check);
        }
        self.blocker.set_enabled(preferences.block_ads);
        for tab in self.loaded_tabs() {
            if let Some(view) = tab.view.get() {
                view.web.set_zoom_level(self.saved_zoom(&tab.uri()));
            }
        }
    }

    /// Searches the active page's selected text in a new tab.
    pub(super) fn search_selection(self: &Rc<Self>) {
        let Some(tab) = self.active() else { return };
        let Some(view) = tab.view.get() else { return };
        let weak = Rc::downgrade(self);
        view.web.evaluate_javascript(
            "window.getSelection().toString()",
            None,
            None,
            gio::Cancellable::NONE,
            move |result| {
                let Some(browser) = weak.upgrade() else {
                    return;
                };
                let text = result.map(|value| value.to_str().to_string());
                let text = text.as_deref().map(str::trim).unwrap_or_default();
                if text.is_empty() {
                    return;
                }
                let query: String = text.chars().take(500).collect();
                let engine = preferences::current(&browser.preferences).search;
                browser.new_tab(browser.workspace.get(), &engine.search_url(&query), true);
            },
        );
    }
}

/// Spell checking dictionaries for the user's languages, such as `pt_BR`.
fn spell_languages<'a>(names: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut languages = Vec::new();
    for name in names {
        let base = name.split(['.', '@']).next().unwrap_or_default();
        let (language, region) = base.split_once('_').unwrap_or((base, ""));
        let valid = (2..=3).contains(&language.len())
            && language.bytes().all(|byte| byte.is_ascii_lowercase())
            && (region.is_empty()
                || (region.len() == 2 && region.bytes().all(|byte| byte.is_ascii_uppercase())));
        if valid && !languages.iter().any(|known: &String| known == base) {
            languages.push(base.to_owned());
        }
    }
    // A language with a region covers the same language without one.
    let regional: Vec<String> = languages
        .iter()
        .filter(|name| name.contains('_'))
        .map(|name| name.split('_').next().unwrap_or_default().to_owned())
        .collect();
    languages.retain(|name| name.contains('_') || !regional.contains(name));
    languages.truncate(3);
    if languages.is_empty() {
        languages.push("en_US".to_owned());
    }
    languages
}

#[cfg(test)]
mod tests {
    use super::spell_languages;

    #[test]
    fn spell_languages_follow_the_locale_without_duplicates() {
        assert_eq!(
            spell_languages(
                [
                    "pt_BR.UTF-8",
                    "pt_BR",
                    "pt.UTF-8",
                    "pt",
                    "en_US",
                    "C.UTF-8",
                    "C"
                ]
                .into_iter()
            ),
            ["pt_BR", "en_US"]
        );
        assert_eq!(spell_languages(["de"].into_iter()), ["de"]);
        assert_eq!(spell_languages(["C", "POSIX"].into_iter()), ["en_US"]);
        assert_eq!(
            spell_languages(["sr_RS@latin", "fr_FR", "es_ES", "it_IT"].into_iter()),
            ["sr_RS", "fr_FR", "es_ES"]
        );
    }
}
