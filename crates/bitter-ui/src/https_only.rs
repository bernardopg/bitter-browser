//! HTTPS-only mode: plain HTTP page loads are retried over HTTPS, and the
//! user is asked before a site that has no secure version is opened.

use std::net::IpAddr;

/// The HTTPS address to load instead of `uri`, when it must be upgraded.
/// Sites the user chose to open over HTTP keep their address.
pub fn upgrade(uri: &str, allowed_insecure: impl Fn(&str) -> bool) -> Option<String> {
    let mut url = url::Url::parse(uri).ok()?;
    if url.scheme() != "http" {
        return None;
    }
    let host = url.host_str()?.to_ascii_lowercase();
    if is_local(&host) || allowed_insecure(&host) {
        return None;
    }
    let port = url.port();
    url.set_scheme("https").ok()?;
    // An explicit port 80 cannot speak TLS; use the HTTPS default instead.
    if port == Some(80) {
        url.set_port(None).ok()?;
    }
    Some(url.into())
}

/// Whether `failed` (an HTTPS address) is the upgrade of `original`.
pub fn is_upgrade_of(original: &str, failed: &str) -> bool {
    let (Ok(original), Ok(failed)) = (url::Url::parse(original), url::Url::parse(failed)) else {
        return false;
    };
    original.scheme() == "http"
        && failed.scheme() == "https"
        && original.host_str().map(str::to_ascii_lowercase)
            == failed.host_str().map(str::to_ascii_lowercase)
}

/// Hosts that cannot get a public certificate: loopback, private networks
/// and names without a public suffix.
pub fn is_local(host: &str) -> bool {
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if let Ok(address) = host.parse::<IpAddr>() {
        return match address {
            IpAddr::V4(address) => {
                address.is_loopback()
                    || address.is_private()
                    || address.is_link_local()
                    || address.is_unspecified()
            }
            IpAddr::V6(address) => {
                address.is_loopback()
                    || address.is_unspecified()
                    // Unique local (fc00::/7) and link-local (fe80::/10).
                    || (address.segments()[0] & 0xfe00) == 0xfc00
                    || (address.segments()[0] & 0xffc0) == 0xfe80
            }
        };
    }
    let host = host.trim_end_matches('.');
    !host.contains('.')
        || [
            ".localhost",
            ".local",
            ".lan",
            ".home",
            ".internal",
            ".home.arpa",
            ".test",
        ]
        .iter()
        .any(|suffix| host.ends_with(suffix))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_http_sites_are_upgraded_keeping_path_and_custom_ports() {
        let never = |_: &str| false;
        assert_eq!(
            upgrade("http://Example.com/a?b=1#c", never).as_deref(),
            Some("https://example.com/a?b=1#c")
        );
        assert_eq!(
            upgrade("http://example.com:80/x", never).as_deref(),
            Some("https://example.com/x")
        );
        assert_eq!(
            upgrade("http://example.com:8080/x", never).as_deref(),
            Some("https://example.com:8080/x")
        );
        for unchanged in [
            "https://example.com/",
            "file:///tmp/a.html",
            "bitter://history",
            "about:blank",
            "not a url",
        ] {
            assert_eq!(upgrade(unchanged, never), None, "{unchanged}");
        }
        assert_eq!(
            upgrade("http://legacy.example.com/", |host| host
                == "legacy.example.com"),
            None
        );
    }

    #[test]
    fn local_and_private_hosts_stay_on_http() {
        for local in [
            "localhost",
            "router",
            "printer.local",
            "nas.lan",
            "app.localhost",
            "site.test",
            "127.0.0.1",
            "10.0.0.5",
            "192.168.1.1",
            "172.16.4.2",
            "169.254.1.1",
            "[::1]",
            "fd12::1",
            "fe80::1",
        ] {
            assert!(is_local(local), "{local}");
            assert_eq!(upgrade(&format!("http://{local}/"), |_| false), None);
        }
        for public in [
            "example.com",
            "8.8.8.8",
            "2001:4860::8888",
            "news.example.co.uk",
        ] {
            assert!(!is_local(public), "{public}");
        }
    }

    #[test]
    fn recognizes_the_failed_upgrade_of_a_page() {
        assert!(is_upgrade_of(
            "http://Example.com/a",
            "https://example.com/a"
        ));
        assert!(is_upgrade_of(
            "http://example.com/a",
            "https://example.com/other"
        ));
        assert!(!is_upgrade_of("http://example.com/", "https://other.com/"));
        assert!(!is_upgrade_of(
            "https://example.com/",
            "https://example.com/"
        ));
        assert!(!is_upgrade_of("not a url", "https://example.com/"));
        assert!(!is_upgrade_of("http://example.com/", "not a url"));
    }
}
