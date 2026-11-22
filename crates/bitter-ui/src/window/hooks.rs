//! What the preferences dialog reads from and changes in the browser.

use super::*;
use crate::preferences::{ClearRequest, Hooks};
use bitter_core::records::SitePermission;
use webkit6::WebsiteDataTypes;

struct BrowserHooks(Weak<Browser>);

impl Browser {
    pub(super) fn hooks(self: &Rc<Self>) -> Rc<dyn Hooks> {
        Rc::new(BrowserHooks(Rc::downgrade(self)))
    }

    /// Removes the requested data. `done` gets the first error, if any.
    fn clear_data(&self, request: ClearRequest, done: Box<dyn FnOnce(Result<(), String>)>) {
        let mut error = None;
        // WebKitGTK can only delete all cookies, not those set in a time
        // range. For a range, remove the cookies of the sites visited in it,
        // which must be read before the history is cleared.
        let cookie_hosts = match (request.site_data, request.since) {
            (true, Some(since)) => match self.history.urls_since(since) {
                Ok(urls) => Some(visited_hosts(&urls)),
                Err(failure) => {
                    error = Some(failure.to_string());
                    Some(Vec::new())
                }
            },
            _ => None,
        };
        if request.history {
            if let Err(failure) = self.history.clear_since(request.since) {
                tracing::warn!("Could not clear history: {failure}");
                error.get_or_insert(failure.to_string());
            }
            // Closed tabs and the New Tab page would show cleared pages.
            self.closed.borrow_mut().clear();
            self.refresh_new_tab_pages();
        }
        if request.downloads {
            self.downloads.clear_finished();
        }
        let mut types = WebsiteDataTypes::empty();
        if request.site_data {
            types |= WebsiteDataTypes::LOCAL_STORAGE
                | WebsiteDataTypes::SESSION_STORAGE
                | WebsiteDataTypes::INDEXEDDB_DATABASES
                | WebsiteDataTypes::SERVICE_WORKER_REGISTRATIONS
                | WebsiteDataTypes::DOM_CACHE
                | WebsiteDataTypes::OFFLINE_APPLICATION_CACHE
                | WebsiteDataTypes::ITP
                | WebsiteDataTypes::DEVICE_ID_HASH_SALT;
            if cookie_hosts.is_none() {
                types |= WebsiteDataTypes::COOKIES;
            }
        }
        if request.cache {
            types |= WebsiteDataTypes::DISK_CACHE | WebsiteDataTypes::MEMORY_CACHE;
        }
        let Some(manager) = self.network.website_data_manager() else {
            done(Err("Website data is unavailable".into()));
            return;
        };
        let timespan = request.since.map_or(0, |since| {
            (glib::real_time() / 1_000_000 - since).max(1) * 1_000_000
        });
        let cookies_then = {
            let manager = manager.clone();
            move |error: Option<String>| match cookie_hosts {
                Some(hosts) => remove_cookies(&manager, hosts, error, done),
                None => done(error.map_or(Ok(()), Err)),
            }
        };
        if types.is_empty() {
            cookies_then(error);
            return;
        }
        // WebKit calls back on this thread; the guard only satisfies `Send`.
        let next = glib::thread_guard::ThreadGuard::new((cookies_then, error));
        manager.clear(
            types,
            glib::TimeSpan(timespan),
            gio::Cancellable::NONE,
            move |result| {
                let (next, error) = next.into_inner();
                next(error.or_else(|| result.err().map(|failure| failure.to_string())));
            },
        );
    }

    /// Reloads open New Tab pages so their frequent sites stay current.
    fn refresh_new_tab_pages(&self) {
        for group in self.workspaces.borrow().iter() {
            for tab in &group.tabs {
                if tab.uri() == HOME_URI
                    && let Some(view) = tab.view.get()
                {
                    self.navigate(tab, view, HOME_URI);
                }
            }
        }
    }
}

impl Hooks for BrowserHooks {
    fn applied(&self, preferences: Preferences) {
        if let Some(browser) = self.0.upgrade() {
            browser.apply_preferences(preferences);
        }
    }

    fn site_permissions(&self) -> Vec<SitePermission> {
        self.0
            .upgrade()
            .map(|browser| browser.sites.borrow().permissions().to_vec())
            .unwrap_or_default()
    }

    fn set_permission(&self, origin: &str, permission: Permission, allowed: Option<bool>) {
        if let Some(browser) = self.0.upgrade() {
            browser.set_site_permission(origin, permission, allowed);
        }
    }

    fn reset_permissions(&self) {
        if let Some(browser) = self.0.upgrade() {
            browser.reset_site_permissions();
        }
    }

    fn unblocked_sites(&self) -> Vec<String> {
        self.0
            .upgrade()
            .map(|browser| browser.sites.borrow().unblocked().to_vec())
            .unwrap_or_default()
    }

    fn set_unblocked(&self, host: &str, unblocked: bool) {
        if let Some(browser) = self.0.upgrade() {
            browser.set_unblocked(host, unblocked);
        }
    }

    fn blocking_status(&self) -> String {
        self.0
            .upgrade()
            .map(|browser| browser.blocker.status().describe())
            .unwrap_or_default()
    }

    fn update_filter_lists(&self) {
        if let Some(browser) = self.0.upgrade() {
            browser.blocker.update_now();
        }
    }

    fn clear_data(&self, request: ClearRequest, done: Box<dyn FnOnce(Result<(), String>)>) {
        match self.0.upgrade() {
            Some(browser) => browser.clear_data(request, done),
            None => done(Err("The browser window closed".into())),
        }
    }
}

/// Hosts of visited addresses, without duplicates.
fn visited_hosts(urls: &[String]) -> Vec<String> {
    let mut hosts: Vec<String> = urls
        .iter()
        .filter_map(|url| url::Url::parse(url).ok())
        .filter_map(|url| url.host_str().map(str::to_ascii_lowercase))
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts
}

/// Whether WebKit's cookie domain (`example.com`, or `.example.com`) covers
/// a visited host.
fn covers(domain: &str, host: &str) -> bool {
    let domain = domain.trim_start_matches('.').to_ascii_lowercase();
    !domain.is_empty() && (host == domain || host.ends_with(&format!(".{domain}")))
}

fn remove_cookies(
    manager: &webkit6::WebsiteDataManager,
    hosts: Vec<String>,
    error: Option<String>,
    done: Box<dyn FnOnce(Result<(), String>)>,
) {
    if hosts.is_empty() {
        done(error.map_or(Ok(()), Err));
        return;
    }
    let remover = manager.clone();
    manager.fetch(
        WebsiteDataTypes::COOKIES,
        gio::Cancellable::NONE,
        move |result| {
            let sites = match result {
                Ok(sites) => sites,
                Err(failure) => {
                    done(Err(error.unwrap_or_else(|| failure.to_string())));
                    return;
                }
            };
            let matching: Vec<_> = sites
                .iter()
                .filter(|site| {
                    site.name()
                        .is_some_and(|name| hosts.iter().any(|host| covers(&name, host)))
                })
                .collect();
            if matching.is_empty() {
                done(error.map_or(Ok(()), Err));
                return;
            }
            let finish = glib::thread_guard::ThreadGuard::new((done, error));
            remover.remove(
                WebsiteDataTypes::COOKIES,
                &matching,
                gio::Cancellable::NONE,
                move |result| {
                    let (done, error) = finish.into_inner();
                    done(match (error, result) {
                        (Some(error), _) => Err(error),
                        (None, Err(failure)) => Err(failure.to_string()),
                        (None, Ok(())) => Ok(()),
                    });
                },
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use super::{covers, visited_hosts};

    #[test]
    fn cookie_domains_cover_visited_hosts_and_their_parents_only() {
        let hosts = visited_hosts(&[
            "https://Mail.Example.com/inbox".into(),
            "https://mail.example.com/other".into(),
            "http://127.0.0.1:8080/".into(),
            "not a url".into(),
        ]);
        assert_eq!(hosts, ["127.0.0.1", "mail.example.com"]);
        assert!(covers("example.com", "mail.example.com"));
        assert!(covers(".example.com", "mail.example.com"));
        assert!(covers("mail.example.com", "mail.example.com"));
        assert!(covers("127.0.0.1", "127.0.0.1"));
        assert!(!covers("other.com", "mail.example.com"));
        assert!(!covers("ample.com", "mail.example.com"));
        assert!(!covers("", "mail.example.com"));
    }

    #[test]
    fn visited_hosts_normalizes_dedups_and_ignores_invalid_urls() {
        let hosts = visited_hosts(&[
            "https://EXAMPLE.com/page".into(),
            "http://example.com/other".into(),
            "https://sub.example.com/".into(),
            "ftp://files.example.com/".into(),
            "not a url".into(),
            "".into(),
            "about:blank".into(),
        ]);
        assert_eq!(
            hosts,
            ["example.com", "files.example.com", "sub.example.com"]
        );
        assert!(visited_hosts(&[]).is_empty());
    }

    #[test]
    fn covers_handles_case_boundaries_and_empty_values() {
        assert!(covers("Example.COM", "mail.example.com"));
        assert!(covers(".Example.com", "example.com"));
        assert!(!covers("example.com", "notexample.com"));
        assert!(!covers("example.com", "example.com.evil.test"));
        assert!(!covers(".", "example.com"));
        assert!(!covers("", ""));
    }
}
