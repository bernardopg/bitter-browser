//! HTML for the internal pages: New Tab, History and Bookmarks.
//!
//! Every value from the profile is untrusted and escaped. Only web and file
//! addresses become links, and a strict Content Security Policy limits the
//! pages to their own inline style and filter script.

use libadwaita as adw;
use std::fmt::Write;

/// Color scheme and accent of the browser chrome, applied to the pages.
pub struct Look {
    pub dark: bool,
    pub accent: String,
}

impl Default for Look {
    fn default() -> Self {
        Self {
            dark: false,
            accent: "rgb(53 132 228)".into(),
        }
    }
}

impl Look {
    pub fn current() -> Self {
        let style = adw::StyleManager::default();
        let accent = style.accent_color_rgba();
        Self {
            dark: style.is_dark(),
            accent: format!(
                "rgb({} {} {})",
                (accent.red() * 255.0).round(),
                (accent.green() * 255.0).round(),
                (accent.blue() * 255.0).round()
            ),
        }
    }
}

pub enum ListKind {
    History,
    Bookmarks,
}

impl ListKind {
    fn title(&self) -> &'static str {
        match self {
            Self::History => "History",
            Self::Bookmarks => "Bookmarks",
        }
    }

    fn empty(&self) -> (&'static str, &'static str) {
        match self {
            Self::History => (
                "No history yet",
                "Pages you visit appear here, newest first.",
            ),
            Self::Bookmarks => (
                "No bookmarks yet",
                "Press Ctrl+D on any page to save it here.",
            ),
        }
    }
}

pub fn new_tab(look: &Look, frequent: &[(String, String)]) -> String {
    let mut tiles = String::new();
    for (url, title) in frequent.iter().filter(|(url, _)| linkable(url)).take(8) {
        let host = display_host(url);
        let label = if title.trim().is_empty() {
            &host
        } else {
            title
        };
        let _ = write!(
            tiles,
            r#"<a class="tile" href="{url}" title="{tooltip}">{avatar}<span class="tile-title">{label}</span><span class="tile-host">{host}</span></a>"#,
            url = escape_html(url),
            tooltip = escape_html(url),
            avatar = avatar(&host),
            label = escape_html(label),
            host = escape_html(&host),
        );
    }
    let frequent = if tiles.is_empty() {
        String::new()
    } else {
        format!(
            r#"<section class="frequent"><h2>Frequently visited</h2><div class="tiles">{tiles}</div></section>"#
        )
    };
    let body = format!(
        r#"<header class="hero">
  <div class="mark" aria-hidden="true">{MARK}</div>
  <h1>Where to next?</h1>
  <p class="lede">Type an address or a search in the bar above.</p>
  <ul class="hints" aria-label="Shortcuts">
    <li><kbd>Ctrl</kbd><kbd>L</kbd> Address bar</li>
    <li><kbd>Ctrl</kbd><kbd>K</kbd> Commands</li>
    <li><kbd>Ctrl</kbd><kbd>Shift</kbd><kbd>T</kbd> Reopen tab</li>
  </ul>
</header>
{frequent}"#
    );
    page(look, "New Tab", "page-newtab", &body, false)
}

pub fn list(look: &Look, kind: ListKind, rows: &[(String, String)]) -> String {
    let title = kind.title();
    if rows.is_empty() {
        let (heading, message) = kind.empty();
        let body = format!(
            r#"<h1>{title}</h1><section class="empty"><div class="empty-mark" aria-hidden="true">{MARK}</div><h2>{heading}</h2><p>{message}</p></section>"#
        );
        return page(look, title, "page-list", &body, false);
    }
    let mut items = String::new();
    for (url, label) in rows {
        let host = display_host(url);
        let label = if label.trim().is_empty() { url } else { label };
        let search = format!("{label} {url}").to_lowercase();
        let inner = format!(
            r#"{avatar}<span class="text"><span class="title">{label}</span><span class="url">{url}</span></span>"#,
            avatar = avatar(&host),
            label = escape_html(label),
            url = escape_html(url),
        );
        let remove = if matches!(kind, ListKind::History) {
            let encoded: String = url::form_urlencoded::byte_serialize(url.as_bytes()).collect();
            format!(
                r#"<a class="remove" href="bitter://history/delete?url={encoded}" aria-label="Remove from history" title="Remove from history">&#xd7;</a>"#,
                encoded = escape_html(&encoded),
            )
        } else {
            String::new()
        };
        if linkable(url) {
            let _ = write!(
                items,
                r#"<div class="row" data-search="{search}"><a class="entry" href="{href}">{inner}</a>{remove}</div>"#,
                href = escape_html(url),
                search = escape_html(&search),
            );
        } else {
            let _ = write!(
                items,
                r#"<div class="row inert" data-search="{search}"><div class="entry">{inner}</div>{remove}</div>"#,
                search = escape_html(&search),
            );
        }
    }
    let count = rows.len();
    let body = format!(
        r#"<div class="toolbar"><h1>{title}</h1><span class="count">{count}</span></div>
<input id="filter" class="filter" type="search" placeholder="Filter {lower}" aria-label="Filter {lower}" autocomplete="off" autofocus>
<div class="list" id="rows">{items}</div>
<p class="no-match" id="no-match" hidden>No matches</p>"#,
        lower = title.to_lowercase(),
    );
    page(look, title, "page-list", &body, true)
}

pub fn unavailable(look: &Look, title: &str, error: &str) -> String {
    let body = format!(
        r#"<h1>{title}</h1><section class="empty error"><h2>Could not load this list</h2><p>Check available disk space and profile permissions, then try again.</p><p class="detail">{error}</p></section>"#,
        title = escape_html(title),
        error = escape_html(error),
    );
    page(look, title, "page-list", &body, false)
}

pub fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn linkable(url: &str) -> bool {
    url::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https" | "file"))
}

/// The registrable part of a host, roughly: `docs.flatpak.org` -> `flatpak`.
fn site_name(host: &str) -> &str {
    let labels: Vec<&str> = host.split('.').filter(|label| !label.is_empty()).collect();
    match labels.as_slice() {
        [.., name, _tld] => name,
        [only] => only,
        [] => host,
    }
}

fn display_host(url: &str) -> String {
    url::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .map(|host| host.strip_prefix("www.").unwrap_or(&host).to_owned())
        .unwrap_or_else(|| url.to_owned())
}

/// A colored initial derived from the host, standing in for a site icon.
fn avatar(host: &str) -> String {
    let host = site_name(host);
    let initial = host
        .chars()
        .find(|ch| ch.is_alphanumeric())
        .map(|ch| ch.to_uppercase().collect::<String>())
        .unwrap_or_else(|| "•".into());
    let hue = host.bytes().fold(2166136261u32, |hash, byte| {
        (hash ^ byte as u32).wrapping_mul(16777619)
    }) % 360;
    format!(
        r#"<span class="avatar" style="--hue:{hue}" aria-hidden="true">{}</span>"#,
        escape_html(&initial)
    )
}

const MARK: &str = r##"<svg viewBox="0 0 64 64" width="64" height="64"><defs><linearGradient id="g" x1="0" y1="0" x2="1" y2="1"><stop offset="0" stop-color="var(--mark-a)"/><stop offset="1" stop-color="var(--mark-b)"/></linearGradient></defs><rect width="64" height="64" rx="18" fill="url(#g)"/><g fill="none" stroke="#fff" stroke-linecap="round" stroke-opacity=".95"><circle cx="32" cy="32" r="17" stroke-width="3.5"/><path d="M15 32h34M32 15c-8 8-8 26 0 34M32 15c8 8 8 26 0 34" stroke-width="3"/></g></svg>"##;

const FILTER_SCRIPT: &str = r##"<script>
const filter = document.getElementById("filter");
const rows = [...document.querySelectorAll("#rows .row")];
const none = document.getElementById("no-match");
filter.addEventListener("input", () => {
  const terms = filter.value.toLowerCase().trim().split(/\s+/).filter(Boolean);
  let shown = 0;
  for (const row of rows) {
    const match = terms.every((term) => row.dataset.search.includes(term));
    row.hidden = !match;
    shown += match;
  }
  none.hidden = shown > 0;
});
</script>"##;

fn page(look: &Look, title: &str, class: &str, body: &str, filter: bool) -> String {
    let accent = &look.accent;
    let theme = if look.dark { "dark" } else { "light" };
    let script = if filter { FILTER_SCRIPT } else { "" };
    format!(
        r#"<!doctype html>
<html class="{theme}" lang="en">
<head>
<meta charset="utf-8">
<meta http-equiv="Content-Security-Policy" content="default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'; img-src data:">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title}</title>
<style>{STYLE}:root {{ --accent: {accent}; }}</style>
</head>
<body class="{class}">
<main>
{body}
</main>
{script}
</body>
</html>"#,
        title = escape_html(title),
    )
}

const STYLE: &str = r#"
:root {
  color-scheme: light;
  --bg: #fafafb;
  --fg: #1e1e22;
  --muted: #64646e;
  --card: #ffffff;
  --card-hover: #f4f4f7;
  --line: rgb(0 0 6 / 9%);
  --shadow: 0 1px 2px rgb(0 0 6 / 6%), 0 4px 14px rgb(0 0 6 / 5%);
  --shadow-hover: 0 2px 4px rgb(0 0 6 / 8%), 0 10px 26px rgb(0 0 6 / 9%);
  --mark-a: #6d5dfc;
  --mark-b: #3b2f8f;
  font: 15px/1.5 "Inter", "Cantarell", system-ui, sans-serif;
  font-feature-settings: "cv11", "ss01";
  background: var(--bg);
  color: var(--fg);
  -webkit-font-smoothing: antialiased;
}
:root.dark {
  color-scheme: dark;
  --bg: #1d1d20;
  --fg: #f2f2f5;
  --muted: #a8a8b3;
  --card: #2a2a2e;
  --card-hover: #323237;
  --line: rgb(255 255 255 / 7%);
  --shadow: 0 1px 2px rgb(0 0 0 / 30%);
  --shadow-hover: 0 4px 18px rgb(0 0 0 / 40%);
  --mark-a: #8f82ff;
  --mark-b: #4a3fb0;
}
* { box-sizing: border-box; }
body { margin: 0; min-height: 100vh; }
main { max-width: 880px; margin: 0 auto; padding: clamp(28px, 6vw, 64px) clamp(20px, 5vw, 48px) 64px; }
h1 { margin: 0; font-size: 30px; font-weight: 700; letter-spacing: -0.02em; }
h2 { margin: 0 0 14px; font-size: 13px; font-weight: 650; letter-spacing: 0.06em; text-transform: uppercase; color: var(--muted); }
p { margin: 0; color: var(--muted); }
kbd {
  display: inline-block; min-width: 1.6em; padding: 1px 6px; margin-right: 3px;
  font: 600 11px/1.6 "Inter", system-ui, sans-serif; text-align: center;
  border-radius: 6px; background: var(--card); box-shadow: inset 0 -1px 0 var(--line), 0 0 0 1px var(--line);
}
a { color: inherit; text-decoration: none; }
a:focus-visible, input:focus-visible { outline: 2px solid var(--accent); outline-offset: 2px; }
.mark svg { display: block; filter: drop-shadow(0 8px 20px rgb(80 60 220 / 28%)); }
.page-newtab main { display: flex; flex-direction: column; align-items: center; padding-top: clamp(48px, 14vh, 140px); }
.hero { display: flex; flex-direction: column; align-items: center; gap: 12px; text-align: center; animation: rise .45s cubic-bezier(.2,.8,.2,1) both; }
.hero h1 { margin-top: 10px; font-size: 34px; }
.lede { font-size: 16px; }
.hints { display: flex; flex-wrap: wrap; justify-content: center; gap: 8px 18px; margin: 14px 0 0; padding: 0; list-style: none; color: var(--muted); font-size: 13px; }
.frequent { width: 100%; margin-top: 56px; animation: rise .5s .08s cubic-bezier(.2,.8,.2,1) both; }
.frequent h2 { text-align: center; }
.tiles { display: grid; grid-template-columns: repeat(auto-fill, minmax(150px, 1fr)); gap: 12px; }
.tile {
  display: grid; justify-items: center; gap: 4px; padding: 18px 12px 14px;
  border-radius: 16px; background: var(--card); box-shadow: var(--shadow);
  transition: transform .16s ease, box-shadow .16s ease, background-color .16s ease;
}
.tile:hover { transform: translateY(-2px); box-shadow: var(--shadow-hover); background: var(--card-hover); }
.tile .avatar { width: 40px; height: 40px; margin-bottom: 6px; font-size: 17px; border-radius: 12px; }
.tile-title, .tile-host { max-width: 100%; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.tile-title { font-size: 13px; font-weight: 600; }
.tile-host { font-size: 12px; color: var(--muted); }
.avatar {
  display: inline-grid; place-items: center; flex: none; width: 32px; height: 32px; border-radius: 10px;
  font-weight: 700; font-size: 14px; color: #fff;
  background: linear-gradient(135deg, hsl(var(--hue) 62% 58%), hsl(calc(var(--hue) + 28) 58% 44%));
}
.toolbar { display: flex; align-items: baseline; gap: 12px; margin-bottom: 20px; }
.count { font-size: 13px; font-weight: 600; padding: 1px 9px; border-radius: 999px; color: var(--muted); background: var(--card); box-shadow: 0 0 0 1px var(--line); }
.filter {
  width: 100%; margin-bottom: 18px; padding: 11px 16px; font: inherit; color: inherit;
  border: none; border-radius: 12px; background: var(--card); box-shadow: 0 0 0 1px var(--line);
  transition: box-shadow .16s ease;
}
.filter:focus { outline: none; box-shadow: 0 0 0 2px var(--accent); }
.list { display: grid; gap: 2px; padding: 6px; border-radius: 16px; background: var(--card); box-shadow: var(--shadow); animation: rise .35s cubic-bezier(.2,.8,.2,1) both; }
.row { display: flex; align-items: center; gap: 8px; padding: 10px 12px; border-radius: 11px; transition: background-color .14s ease; }
.row:hover { background: var(--card-hover); }
.row[hidden] { display: none; }
.row.inert { opacity: .7; }
.entry { display: flex; align-items: center; gap: 14px; flex: 1; min-width: 0; color: inherit; text-decoration: none; }
.remove {
  flex: none; display: inline-grid; place-items: center; width: 30px; height: 30px;
  border-radius: 8px; color: var(--muted); font-size: 18px; line-height: 1; text-decoration: none;
}
.remove:hover, .remove:focus-visible { background: color-mix(in srgb, var(--fg) 8%, transparent); color: var(--fg); }
.text { display: grid; min-width: 0; }
.title, .url { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.title { font-weight: 550; }
.url { font-size: 12.5px; color: var(--muted); }
.no-match { margin-top: 24px; text-align: center; }
.empty { display: grid; justify-items: center; gap: 8px; margin-top: 24px; padding: 56px 24px; text-align: center; border-radius: 18px; background: var(--card); box-shadow: var(--shadow); }
.empty h2 { margin: 10px 0 0; font-size: 18px; letter-spacing: 0; text-transform: none; color: var(--fg); }
.empty-mark { opacity: .85; }
.empty.error .detail { font-size: 12.5px; word-break: break-word; }
@keyframes rise { from { opacity: 0; transform: translateY(8px); } to { opacity: 1; transform: none; } }
@media (prefers-reduced-motion: reduce) { *, *::before, *::after { animation: none !important; transition: none !important; } }
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostile_profile_values_are_escaped_and_not_linked() {
        let rows = vec![
            (
                "https://example.test/?q=<script>".to_owned(),
                "<img src=x onerror=alert(1)>".to_owned(),
            ),
            ("javascript:alert(1)".to_owned(), "Click me".to_owned()),
            ("https://ok.test/\" onclick=\"x".to_owned(), String::new()),
        ];
        for html in [
            list(&Look::default(), ListKind::History, &rows),
            new_tab(&Look::default(), &rows),
        ] {
            assert!(!html.contains("<img src=x"));
            assert!(!html.contains("<script>alert"));
            assert!(!html.contains(r#"href="javascript:"#));
            assert!(!html.contains(r#"" onclick=""#));
            assert!(html.contains("Content-Security-Policy"));
        }
        let history = list(&Look::default(), ListKind::History, &rows);
        assert!(history.contains(r#"<div class="row inert""#));
        assert!(history.contains("&lt;img src=x onerror=alert(1)&gt;"));
    }

    #[test]
    fn empty_lists_explain_how_to_fill_them() {
        assert!(list(&Look::default(), ListKind::Bookmarks, &[]).contains("Press Ctrl+D"));
        assert!(list(&Look::default(), ListKind::History, &[]).contains("No history yet"));
        assert!(!new_tab(&Look::default(), &[]).contains("Frequently visited"));
    }

    #[test]
    fn avatars_use_the_host_initial_and_a_stable_hue() {
        assert_eq!(
            display_host("https://www.Rust-lang.org/learn"),
            "rust-lang.org"
        );
        assert_eq!(avatar("rust-lang.org"), avatar("rust-lang.org"));
        assert!(avatar("rust-lang.org").contains(">R<"));
        assert!(avatar("---").contains(">•<"));
        assert!(avatar("docs.flatpak.org").contains(">F<"));
        assert_eq!(avatar("docs.flatpak.org"), avatar("flatpak.org"));
        assert_eq!(site_name("localhost"), "localhost");
    }

    #[test]
    fn escape_html_escapes_every_special_character() {
        assert_eq!(
            escape_html("<a href=\"x\">&'</a>"),
            "&lt;a href=&quot;x&quot;&gt;&amp;&#39;&lt;/a&gt;"
        );
    }

    #[test]
    fn lists_use_inert_rows_for_non_web_addresses() {
        let rows = vec![
            ("file:///tmp/a.txt".to_owned(), "Local".to_owned()),
            ("javascript:alert(1)".to_owned(), "Bad".to_owned()),
        ];
        let html = list(&Look::default(), ListKind::History, &rows);
        assert!(html.contains(r#"<a class="entry" href="file:///tmp/a.txt">"#));
        assert!(html.contains(r#"<div class="row inert""#));
        assert!(html.contains(r#"<div class="entry">"#));
        assert!(!html.contains("href=\"javascript:"));
    }

    #[test]
    fn history_rows_include_remove_actions_but_bookmarks_do_not() {
        let rows = vec![(
            "https://example.test/?q=a&b=<x>".to_owned(),
            "Example".to_owned(),
        )];
        let history = list(&Look::default(), ListKind::History, &rows);
        assert!(history.contains(r#"class="remove""#));
        assert!(history.contains("bitter://history/delete?url="));
        assert!(history.contains("aria-label=\"Remove from history\""));
        assert!(!history.contains("href=\"https://example.test/?q=a&b=<x>\""));

        let bookmarks = list(&Look::default(), ListKind::Bookmarks, &rows);
        assert!(!bookmarks.contains("bitter://history/delete"));
        assert!(!bookmarks.contains(r#"class="remove""#));
    }

    #[test]
    fn new_tab_limits_frequent_sites_to_eight() {
        let mut frequent = Vec::new();
        for index in 0..10 {
            frequent.push((
                format!("https://site{index}.test/"),
                format!("Site {index}"),
            ));
        }
        let html = new_tab(&Look::default(), &frequent);
        assert!(html.contains("Site 7"));
        assert!(!html.contains("Site 8"));
    }

    #[test]
    fn display_hosts_and_site_names_come_from_parsed_urls() {
        assert_eq!(
            display_host("https://www.Example.test/path?x=1#frag"),
            "example.test"
        );
        assert_eq!(display_host("not a url"), "not a url");
        assert_eq!(site_name(""), "");
        assert_eq!(site_name("one"), "one");
        assert_eq!(site_name("a.b"), "a");
    }

    #[test]
    fn unavailable_page_escapes_user_visible_error() {
        let html = unavailable(&Look::default(), "History", "<broken &>");
        assert!(html.contains("History"));
        assert!(html.contains("Could not load this list"));
        assert!(html.contains("&lt;broken &amp;&gt;"));
        assert!(!html.contains("<broken &>"));
    }
}
