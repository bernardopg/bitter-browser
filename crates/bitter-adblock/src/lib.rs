//! Converts Adblock Plus filter lists into WebKit content blocker rules.
//!
//! WebKit compiles content blockers natively, so the browser only needs the
//! JSON rule format. The converter keeps the common, well-defined subset of the
//! filter syntax (domain anchors, wildcards, separators, resource types,
//! first/third-party and `domain=` options, element hiding) and drops filters
//! whose meaning WebKit cannot express, rather than approximating them in a
//! way that could block too much.

use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};

/// Bumped whenever the generated rules change, so compiled filters are rebuilt.
pub const CONVERTER_VERSION: u32 = 1;

/// WebKit refuses rule lists above this size.
pub const MAX_RULES: usize = 150_000;

/// Generic element hiding selectors are grouped into rules of this size.
const SELECTORS_PER_RULE: usize = 200;

/// A list compiled into the browser so blocking works before any download.
pub const BUILTIN_LIST: &str = include_str!("builtin.txt");

pub struct RemoteList {
    /// Also the identifier of the compiled filter.
    pub id: &'static str,
    pub name: &'static str,
    pub url: &'static str,
}

/// Maintained lists downloaded and refreshed by the browser.
pub const REMOTE_LISTS: [RemoteList; 2] = [
    RemoteList {
        id: "easylist",
        name: "EasyList",
        url: "https://easylist.to/easylist/easylist.txt",
    },
    RemoteList {
        id: "easyprivacy",
        name: "EasyPrivacy",
        url: "https://easylist.to/easylist/easyprivacy.txt",
    },
];

#[derive(Debug)]
pub struct Conversion {
    /// The rules as a WebKit content blocker JSON array.
    pub json: String,
    pub rules: usize,
    /// Filters skipped because WebKit cannot express them.
    pub skipped: usize,
}

/// Which parts of a list to convert.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scope {
    /// Network blocking and element hiding.
    Full,
    /// Network blocking only, for when a list's selectors fail to compile.
    NetworkOnly,
}

/// Checks that `text` looks like an Adblock Plus list before it replaces a
/// working copy.
pub fn looks_like_filter_list(text: &str) -> bool {
    text.trim_start().starts_with("[Adblock")
        && text.lines().filter(|line| !line.starts_with('!')).count() > 10
}

pub fn convert(list: &str, scope: Scope) -> Conversion {
    let mut blocks = Vec::new();
    let mut exceptions = Vec::new();
    let mut generic_selectors = BTreeSet::new();
    let mut specific: BTreeMap<(Vec<String>, Vec<String>), BTreeSet<String>> = BTreeMap::new();
    let mut hidden_exceptions: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut skipped = 0;

    for line in list.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('!') || line.starts_with('[') {
            continue;
        }
        if let Some((domains, kind, selector)) = split_cosmetic(line) {
            if scope == Scope::NetworkOnly {
                continue;
            }
            let Some(selector) = supported_selector(selector) else {
                skipped += 1;
                continue;
            };
            let Some((include, exclude)) = cosmetic_domains(domains) else {
                skipped += 1;
                continue;
            };
            match kind {
                Cosmetic::Hide if include.is_empty() && exclude.is_empty() => {
                    generic_selectors.insert(selector.to_owned());
                }
                Cosmetic::Hide if !include.is_empty() && !exclude.is_empty() => skipped += 1,
                Cosmetic::Hide => {
                    specific
                        .entry((include, exclude))
                        .or_default()
                        .insert(selector.to_owned());
                }
                // `#@#` with domains keeps a generic rule off those domains.
                Cosmetic::Allow if exclude.is_empty() => {
                    hidden_exceptions
                        .entry(selector.to_owned())
                        .or_default()
                        .extend(include);
                }
                Cosmetic::Allow => skipped += 1,
            }
            continue;
        }
        match network_rule(line) {
            Some(Network::Block(rule)) => blocks.push(rule),
            Some(Network::Allow(rule)) => exceptions.push(rule),
            None => skipped += 1,
        }
    }

    let mut hiding = Vec::new();
    let mut batch = Vec::new();
    for selector in generic_selectors {
        match hidden_exceptions.get(&selector) {
            // A generic selector disabled everywhere is dropped.
            Some(domains) if domains.is_empty() => {}
            Some(domains) => hiding.push(hide_rule(
                &selector,
                &[],
                &domains
                    .iter()
                    .map(|domain| format!("*{domain}"))
                    .collect::<Vec<_>>(),
            )),
            None => {
                batch.push(selector);
                if batch.len() == SELECTORS_PER_RULE {
                    hiding.push(hide_rule(&batch.join(", "), &[], &[]));
                    batch.clear();
                }
            }
        }
    }
    if !batch.is_empty() {
        hiding.push(hide_rule(&batch.join(", "), &[], &[]));
    }
    for ((include, exclude), selectors) in specific {
        let include: Vec<_> = include.iter().map(|domain| format!("*{domain}")).collect();
        let exclude: Vec<_> = exclude.iter().map(|domain| format!("*{domain}")).collect();
        let selectors: Vec<_> = selectors.into_iter().collect();
        for chunk in selectors.chunks(SELECTORS_PER_RULE) {
            hiding.push(hide_rule(&chunk.join(", "), &include, &exclude));
        }
    }

    // Exceptions must follow the rules they lift.
    let mut rules: Vec<Value> = Vec::new();
    for rule in blocks.into_iter().chain(hiding).chain(exceptions) {
        if rules.len() == MAX_RULES {
            skipped += 1;
            continue;
        }
        rules.push(rule);
    }
    Conversion {
        rules: rules.len(),
        json: Value::Array(rules).to_string(),
        skipped,
    }
}

#[derive(Clone, Copy)]
enum Cosmetic {
    Hide,
    Allow,
}

/// Splits `domains##selector` and `domains#@#selector`. Extended syntaxes
/// (`#?#`, `#$#`, `#%#`, `##+js`, `##^`) return `None` and are dropped by
/// the network parser as unsupported.
fn split_cosmetic(line: &str) -> Option<(&str, Cosmetic, &str)> {
    if let Some((domains, selector)) = line.split_once("#@#") {
        return Some((domains, Cosmetic::Allow, selector));
    }
    let (domains, selector) = line.split_once("##")?;
    if domains.contains('#') || domains.contains('/') {
        return None;
    }
    Some((domains, Cosmetic::Hide, selector))
}

fn supported_selector(selector: &str) -> Option<&str> {
    const UNSUPPORTED: [&str; 14] = [
        ":-abp-",
        ":has-text(",
        ":contains(",
        ":xpath(",
        ":matches-css",
        ":matches-attr(",
        ":matches-path(",
        ":upward(",
        ":remove(",
        ":style(",
        ":min-text-length(",
        ":watch-attr(",
        ":others(",
        ":nth-ancestor(",
    ];
    let selector = selector.trim();
    let usable = !selector.is_empty()
        && selector.is_ascii()
        && !selector.starts_with('+')
        && !selector.starts_with('^')
        && !selector.contains('{')
        && !selector.contains('}')
        && !selector.contains("\\")
        && !UNSUPPORTED.iter().any(|syntax| selector.contains(syntax));
    usable.then_some(selector)
}

/// `a.com,~b.a.com` into included and excluded domains.
fn cosmetic_domains(domains: &str) -> Option<(Vec<String>, Vec<String>)> {
    let mut include = Vec::new();
    let mut exclude = Vec::new();
    for domain in domains.split(',').filter(|domain| !domain.is_empty()) {
        let (list, domain) = match domain.strip_prefix('~') {
            Some(domain) => (&mut exclude, domain),
            None => (&mut include, domain),
        };
        list.push(valid_domain(domain)?);
    }
    include.sort();
    exclude.sort();
    Some((include, exclude))
}

fn valid_domain(domain: &str) -> Option<String> {
    let domain = domain.trim().to_ascii_lowercase();
    let valid = !domain.is_empty()
        && domain.len() <= 253
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && domain
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-.".contains(&byte));
    valid.then_some(domain)
}

fn hide_rule(selector: &str, include: &[String], exclude: &[String]) -> Value {
    let mut trigger = Map::new();
    trigger.insert("url-filter".into(), json!(".*"));
    if !include.is_empty() {
        trigger.insert("if-domain".into(), json!(include));
    }
    if !exclude.is_empty() {
        trigger.insert("unless-domain".into(), json!(exclude));
    }
    json!({
        "trigger": trigger,
        "action": {"type": "css-display-none", "selector": selector},
    })
}

enum Network {
    Block(Value),
    Allow(Value),
}

/// Resource types WebKit understands in every supported release.
const RESOURCE_TYPES: [&str; 8] = [
    "image",
    "style-sheet",
    "script",
    "font",
    "raw",
    "svg-document",
    "media",
    "popup",
];

fn network_rule(line: &str) -> Option<Network> {
    let (allow, body) = match line.strip_prefix("@@") {
        Some(body) => (true, body),
        None => (false, line),
    };
    if body.contains("##") || body.contains("#@#") || body.contains("#?#") || body.contains("#$#") {
        return None;
    }
    let (pattern, options) = match body.rsplit_once('$') {
        // A `$` inside a regular expression or path is part of the pattern.
        Some((pattern, options))
            if !options.contains('/') && !pattern.is_empty() && !options.is_empty() =>
        {
            (pattern, Some(options))
        }
        _ => (body, None),
    };
    let options = Options::parse(options.unwrap_or_default())?;

    // `@@||example.com^$document` turns blocking off on that site.
    if options.document {
        if !allow || !options.types.is_empty() {
            return None;
        }
        let domain = pattern
            .strip_prefix("||")?
            .trim_end_matches(['^', '|', '/']);
        let domain = valid_domain(domain)?;
        return Some(Network::Allow(json!({
            "trigger": {"url-filter": ".*", "if-domain": [format!("*{domain}")]},
            "action": {"type": "ignore-previous-rules"},
        })));
    }

    let url_filter = url_filter(pattern, options.match_case)?;
    let mut trigger = Map::new();
    trigger.insert("url-filter".into(), json!(url_filter));
    if options.match_case {
        trigger.insert("url-filter-is-case-sensitive".into(), json!(true));
    }
    if !options.types.is_empty() {
        trigger.insert("resource-type".into(), json!(options.types));
    }
    if options.child_frame_only {
        trigger.insert("load-context".into(), json!(["child-frame"]));
    }
    if let Some(third_party) = options.third_party {
        let load = if third_party {
            "third-party"
        } else {
            "first-party"
        };
        trigger.insert("load-type".into(), json!([load]));
    }
    if !options.include.is_empty() {
        trigger.insert("if-domain".into(), json!(options.include));
    }
    if !options.exclude.is_empty() {
        trigger.insert("unless-domain".into(), json!(options.exclude));
    }
    let action = if allow {
        "ignore-previous-rules"
    } else {
        "block"
    };
    let rule = json!({"trigger": trigger, "action": {"type": action}});
    Some(if allow {
        Network::Allow(rule)
    } else {
        Network::Block(rule)
    })
}

#[derive(Default)]
struct Options {
    types: Vec<&'static str>,
    child_frame_only: bool,
    third_party: Option<bool>,
    include: Vec<String>,
    exclude: Vec<String>,
    match_case: bool,
    document: bool,
}

impl Options {
    fn parse(text: &str) -> Option<Self> {
        let mut options = Self::default();
        let mut wanted: BTreeSet<&'static str> = BTreeSet::new();
        let mut unwanted: BTreeSet<&'static str> = BTreeSet::new();
        let mut subdocument = None;
        for option in text.split(',').filter(|option| !option.is_empty()) {
            let (negated, name) = match option.strip_prefix('~') {
                Some(name) => (true, name),
                None => (false, option),
            };
            let name = name.to_ascii_lowercase();
            if let Some(domains) = name.strip_prefix("domain=") {
                if negated {
                    return None;
                }
                for domain in domains.split('|').filter(|domain| !domain.is_empty()) {
                    match domain.strip_prefix('~') {
                        Some(domain) => options.exclude.push(format!("*{}", valid_domain(domain)?)),
                        None => options.include.push(format!("*{}", valid_domain(domain)?)),
                    }
                }
                continue;
            }
            let kind = match name.as_str() {
                "third-party" | "3p" => {
                    options.third_party = Some(!negated);
                    continue;
                }
                "first-party" | "1p" => {
                    options.third_party = Some(negated);
                    continue;
                }
                "match-case" => {
                    options.match_case = !negated;
                    continue;
                }
                "important" | "all" => continue,
                "document" | "doc" if !negated => {
                    options.document = true;
                    continue;
                }
                "subdocument" | "frame" => {
                    subdocument = Some(!negated);
                    continue;
                }
                "script" => "script",
                "image" => "image",
                "stylesheet" | "css" => "style-sheet",
                "font" => "font",
                "media" => "media",
                "popup" => "popup",
                "xmlhttprequest" | "xhr" | "websocket" | "ping" | "other" => "raw",
                // `object` covers plug-ins, which WebKit no longer loads.
                "object" | "object-subrequest" => {
                    if negated {
                        continue;
                    }
                    return None;
                }
                _ => return None,
            };
            if negated {
                unwanted.insert(kind);
            } else {
                wanted.insert(kind);
            }
        }
        if !options.include.is_empty() && !options.exclude.is_empty() {
            // WebKit cannot combine both lists in one trigger.
            return None;
        }
        if !wanted.is_empty() && !unwanted.is_empty() {
            return None;
        }
        if !unwanted.is_empty() {
            wanted = RESOURCE_TYPES
                .into_iter()
                .filter(|kind| !unwanted.contains(kind))
                .collect();
        }
        match subdocument {
            Some(true) if wanted.is_empty() => {
                options.child_frame_only = true;
                wanted.insert("document");
            }
            // Mixed with other types, frames are left to the other filters.
            Some(true) => {}
            Some(false) if wanted.is_empty() => {
                wanted = RESOURCE_TYPES.into_iter().collect();
            }
            _ => {}
        }
        options.types = wanted.into_iter().collect();
        Some(options)
    }
}

/// Translates a filter pattern into WebKit's restricted regular expressions.
fn url_filter(pattern: &str, match_case: bool) -> Option<String> {
    // Regular expression filters use syntax WebKit does not support.
    if pattern.len() > 1 && pattern.starts_with('/') && pattern.ends_with('/') {
        return None;
    }
    if !pattern.is_ascii() {
        return None;
    }
    let pattern = if match_case {
        pattern.to_owned()
    } else {
        pattern.to_ascii_lowercase()
    };
    let (mut filter, rest) = if let Some(rest) = pattern.strip_prefix("||") {
        (String::from("^[^:]+://+([^:/]+\\.)?"), rest)
    } else if let Some(rest) = pattern.strip_prefix('|') {
        (String::from("^"), rest)
    } else {
        (String::new(), pattern.as_str())
    };
    let (rest, end_anchor) = match rest.strip_suffix('|') {
        Some(rest) => (rest, true),
        None => (rest, false),
    };
    if filter.len() > 1 && rest.is_empty() {
        return None;
    }
    let mut chars = rest.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '*' => {
                while chars.peek() == Some(&'*') {
                    chars.next();
                }
                filter.push_str(".*");
            }
            // The separator: anything but a letter, digit or `_-.%`. WebKit
            // rejects disjunctions, so it cannot also match the end of the
            // address; URLs always continue after the host, so a separator
            // after a domain still matches.
            '^' => filter.push_str("[^a-zA-Z0-9_.%-]"),
            '.' | '+' | '?' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' | '|' | '$' => {
                filter.push('\\');
                filter.push(ch);
            }
            ch if ch.is_ascii_graphic() => filter.push(ch),
            _ => return None,
        }
    }
    if end_anchor {
        filter.push('$');
    }
    let filter = filter
        .strip_prefix(".*")
        .map_or(filter.clone(), str::to_owned);
    // A filter that matches every address would block whole sites.
    if filter.is_empty() || filter == ".*" || filter == "^" {
        return None;
    }
    Some(filter)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rules(list: &str) -> Vec<Value> {
        serde_json::from_str(&convert(list, Scope::Full).json).unwrap()
    }

    fn regex(filter: &str) -> regex_lite::Regex {
        regex_lite::Regex::new(filter).unwrap()
    }

    #[test]
    fn domain_anchors_match_the_domain_and_its_subdomains_only() {
        let rules = rules("||ads.example.com^\n");
        let filter = rules[0]["trigger"]["url-filter"].as_str().unwrap();
        let filter = regex(filter);
        for blocked in [
            "https://ads.example.com/banner.js",
            "http://cdn.ads.example.com:8080/x",
            "https://ads.example.com/",
        ] {
            assert!(filter.is_match(blocked), "{blocked}");
        }
        for allowed in [
            "https://notads.example.com/",
            "https://ads.example.com.evil.test/",
            "https://example.com/?u=ads.example.com",
        ] {
            assert!(!filter.is_match(allowed), "{allowed}");
        }
        assert_eq!(rules[0]["action"]["type"], "block");
    }

    #[test]
    fn wildcards_separators_anchors_and_case() {
        let rules =
            rules("|https://track.test/*/pixel.gif|\n/banner/*/ad_\n||x.test/Path$match-case\n");
        let anchored = regex(rules[0]["trigger"]["url-filter"].as_str().unwrap());
        assert!(anchored.is_match("https://track.test/a/b/pixel.gif"));
        assert!(!anchored.is_match("https://track.test/a/pixel.gif?x"));
        let plain = regex(rules[1]["trigger"]["url-filter"].as_str().unwrap());
        assert!(plain.is_match("https://site.test/banner/top/ad_1.png"));
        assert_eq!(rules[2]["trigger"]["url-filter-is-case-sensitive"], true);
        assert!(
            rules[2]["trigger"]["url-filter"]
                .as_str()
                .unwrap()
                .ends_with("/Path")
        );
    }

    #[test]
    fn options_map_to_resource_types_party_and_domains() {
        let rules = rules(
            "||cdn.test^$script,third-party,domain=news.test|blog.test\n\
             ||img.test^$~script,~third-party,domain=~shop.test\n\
             ||frames.test^$subdocument\n",
        );
        let trigger = &rules[0]["trigger"];
        assert_eq!(trigger["resource-type"], json!(["script"]));
        assert_eq!(trigger["load-type"], json!(["third-party"]));
        assert_eq!(trigger["if-domain"], json!(["*news.test", "*blog.test"]));
        let trigger = &rules[1]["trigger"];
        assert!(
            !trigger["resource-type"]
                .as_array()
                .unwrap()
                .contains(&json!("script"))
        );
        assert_eq!(trigger["load-type"], json!(["first-party"]));
        assert_eq!(trigger["unless-domain"], json!(["*shop.test"]));
        assert_eq!(rules[2]["trigger"]["load-context"], json!(["child-frame"]));
        assert_eq!(rules[2]["trigger"]["resource-type"], json!(["document"]));
    }

    #[test]
    fn element_hiding_is_grouped_and_scoped_to_domains() {
        let rules = rules(
            "##.ad-banner\n##.sponsored\nnews.test,blog.test##.promo\n\
             ~shop.test##.popup\nnews.test#@#.sponsored\n##.gone\n#@#.gone\n",
        );
        let selectors: Vec<_> = rules
            .iter()
            .map(|rule| {
                (
                    rule["action"]["selector"].as_str().unwrap().to_owned(),
                    rule["trigger"].clone(),
                )
            })
            .collect();
        assert!(
            selectors
                .iter()
                .any(|(selector, trigger)| selector == ".ad-banner"
                    && trigger.get("if-domain").is_none())
        );
        assert!(
            selectors
                .iter()
                .any(|(selector, trigger)| selector == ".sponsored"
                    && trigger["unless-domain"] == json!(["*news.test"]))
        );
        assert!(
            selectors
                .iter()
                .any(|(selector, trigger)| selector == ".promo"
                    && trigger["if-domain"] == json!(["*blog.test", "*news.test"]))
        );
        assert!(
            selectors
                .iter()
                .any(|(selector, trigger)| selector == ".popup"
                    && trigger["unless-domain"] == json!(["*shop.test"]))
        );
        assert!(
            !selectors
                .iter()
                .any(|(selector, _)| selector.contains(".gone"))
        );
    }

    #[test]
    fn exceptions_follow_blocks_and_document_exceptions_cover_the_site() {
        let rules = rules("@@||cdn.test/lib.js\n||cdn.test^\n@@||trusted.test^$document\n##.ad\n");
        let actions: Vec<_> = rules
            .iter()
            .map(|rule| rule["action"]["type"].as_str().unwrap())
            .collect();
        assert_eq!(
            actions,
            [
                "block",
                "css-display-none",
                "ignore-previous-rules",
                "ignore-previous-rules"
            ]
        );
        assert_eq!(rules[3]["trigger"]["if-domain"], json!(["*trusted.test"]));
    }

    #[test]
    fn unsupported_filters_are_skipped_not_approximated() {
        let list = "! comment\n[Adblock Plus 2.0]\n/ads?[0-9]+/\n||x.test^$csp=script-src 'none'\n\
                    ||y.test^$redirect=noop.js\nsite.test#?#div:-abp-has(.ad)\n\
                    site.test#$#abort-on-property-read x\n##div:has-text(Sponsored)\n\
                    ||z.test^$domain=a.test|~b.test\n*\n||\n||é.test^\n";
        let conversion = convert(list, Scope::Full);
        assert_eq!(conversion.rules, 0, "{}", conversion.json);
        assert_eq!(conversion.skipped, 10);
    }

    #[test]
    fn network_only_scope_drops_element_hiding() {
        let conversion = convert("||ads.test^\n##.ad\n", Scope::NetworkOnly);
        assert_eq!(conversion.rules, 1);
        assert!(!conversion.json.contains("css-display-none"));
    }

    #[test]
    fn builtin_list_converts_completely() {
        let conversion = convert(BUILTIN_LIST, Scope::Full);
        assert!(conversion.rules > 50);
        assert_eq!(conversion.skipped, 0);
        assert!(looks_like_filter_list(BUILTIN_LIST));
        assert!(!looks_like_filter_list("<html>not a list</html>"));
    }

    #[test]
    fn generated_filters_use_only_webkit_regex_features() {
        let conversion = convert(
            "||a.test^\n|https://b.test/x*y^z|\n/p/a+t(h)?/\n||c.test/$image\n",
            Scope::Full,
        );
        let rules: Vec<Value> = serde_json::from_str(&conversion.json).unwrap();
        for rule in rules {
            let filter = rule["trigger"]["url-filter"].as_str().unwrap();
            assert!(!filter.contains('|') || filter.contains("\\|"), "{filter}");
            assert!(!filter.contains('{'), "{filter}");
            regex(filter);
        }
    }

    #[test]
    fn looks_like_filter_lists_need_a_header_and_many_rules() {
        let mut list = String::from("[Adblock Plus 2.0]\n");
        for index in 0..10 {
            list.push_str(&format!("||rule{index}.test^\n"));
        }
        assert!(looks_like_filter_list(&list));
        // One rule fewer, or no header, is not a list.
        assert!(!looks_like_filter_list(&list.replacen(
            "||rule0.test^\n",
            "",
            1
        )));
        assert!(!looks_like_filter_list(
            &list.replace("[Adblock Plus 2.0]", "!hidden")
        ));
        // Comment lines do not count as rules.
        assert!(!looks_like_filter_list(
            "[Adblock Plus 2.0]\n! only the header and comments\n! another\n"
        ));
    }

    #[test]
    fn supported_selectors_reject_unsupported_syntax() {
        assert!(supported_selector("div.ad").is_some());
        assert!(supported_selector("a:hover").is_some());
        for bad in [
            "",
            "+div",
            "^div",
            "a{",
            "a}",
            "a\\b",
            "a:has-text(x)",
            "a:-abp-has(b)",
            "a:contains(b)",
            "a:xpath(//b)",
            "a:matches-css",
            "a:matches-attr(b)",
            "a:matches-path(b)",
            "a:upward(b)",
            "a:remove()",
            "a:style(b)",
            "a:min-text-length(5)",
            "a:watch-attr(b)",
            "a:others(b)",
            "a:nth-ancestor(2)",
            "é.com",
        ] {
            assert!(supported_selector(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn domains_are_normalized_and_length_checked() {
        assert_eq!(
            valid_domain("  EXAMPLE.test ").as_deref(),
            Some("example.test")
        );
        assert_eq!(valid_domain("a..b").as_deref(), Some("a..b"));
        assert_eq!(valid_domain(".a.test").as_deref(), None);
        assert_eq!(valid_domain("a.test.").as_deref(), None);
        assert_eq!(valid_domain("a b.test").as_deref(), None);
        assert_eq!(valid_domain("a\tb").as_deref(), None);
        assert_eq!(valid_domain("").as_deref(), None);
        let host253 = format!("{}.test", "a".repeat(248));
        assert_eq!(host253.len(), 253);
        assert_eq!(valid_domain(&host253).as_deref(), Some(host253.as_str()));
        assert!(valid_domain(&format!("{}.test", "a".repeat(249))).is_none());
    }

    #[test]
    fn url_filters_reject_webkit_unsupported_patterns() {
        assert_eq!(url_filter("/a/b/", false), None); // regular expression syntax
        assert_eq!(url_filter("a b", false), None); // not every character is graphic
        assert_eq!(url_filter("", false), None);
        assert_eq!(url_filter("|", false), None);
        assert_eq!(url_filter("*", false), None);
        assert_eq!(url_filter("||", false), None);
        assert_eq!(url_filter("a\\x", false), Some("a\\\\x".to_string())); // backslash escaped
        assert_eq!(url_filter("a.test", false), Some("a\\.test".to_string()));
        assert_eq!(
            url_filter("|http://example.com|", false),
            Some("^http://example\\.com$".to_string())
        );
        assert_eq!(
            url_filter("||ads.test^", false),
            Some("^[^:]+://+([^:/]+\\.)?ads\\.test[^a-zA-Z0-9_.%-]".to_string())
        );
    }

    #[test]
    fn document_exceptions_cover_sites_and_reject_other_types() {
        let rules = rules("@@||trusted.test^$document\n||a.test^\n");
        assert_eq!(rules[0]["action"]["type"], "block");
        assert_eq!(rules[1]["trigger"]["if-domain"], json!(["*trusted.test"]));
        assert_eq!(rules[1]["action"]["type"], "ignore-previous-rules");
        for line in [
            "||a.test^$document",
            "@@a.test$document",
            "@@||a.test^$document,script",
            "@@||a.test^$~document",
        ] {
            let conversion = convert(line, Scope::Full);
            assert_eq!(conversion.rules, 0, "{line}");
        }
    }

    #[test]
    fn options_merge_types_parties_and_domains() {
        let rules = rules(
            "||a.test^$~script,~stylesheet,domain=~shop.test\n\
             ||b.test^$xmlhttprequest,websocket,other\n",
        );
        assert_eq!(
            rules[0]["trigger"]["resource-type"],
            json!(["font", "image", "media", "popup", "raw", "svg-document"])
        );
        assert_eq!(rules[0]["trigger"]["unless-domain"], json!(["*shop.test"]));
        assert_eq!(rules[1]["trigger"]["resource-type"], json!(["raw"]));
        // Both party filters in one rule cannot be expressed.
        assert_eq!(convert("||a.test^$script,~image", Scope::Full).rules, 0);
    }

    #[test]
    fn options_cover_all_supported_types_and_modifiers() {
        let rules = rules(
            "||a.test^$first-party,font,media,popup,important,all\n\
             ||b.test^$1p\n\
             ||c.test^$~subdocument\n\
             ||d.test^$~object\n",
        );
        assert_eq!(rules[0]["trigger"]["load-type"], json!(["first-party"]));
        assert_eq!(
            rules[0]["trigger"]["resource-type"],
            json!(["font", "media", "popup"])
        );
        assert_eq!(rules[1]["trigger"]["load-type"], json!(["first-party"]));
        // ~subdocument with no other types means "everything but document child frames".
        let mut actual = rules[2]["trigger"]["resource-type"]
            .as_array()
            .unwrap()
            .clone();
        actual.sort_by_key(|value| value.as_str().unwrap().to_owned());
        let mut expected: Vec<_> = RESOURCE_TYPES.iter().map(|value| json!(value)).collect();
        expected.sort_by_key(|value| value.as_str().unwrap().to_owned());
        assert_eq!(actual, expected);
        assert!(
            rules[2]["trigger"]
                .get("load-context")
                .is_none_or(|value| value != &json!(["child-frame"]))
        );
        // A negated object type is ignored; a positive object type is unsupported.
        assert_eq!(rules[3]["trigger"].get("resource-type"), None);
        assert_eq!(convert("||e.test^$object", Scope::Full).rules, 0);
        assert_eq!(convert("||f.test^$object-subrequest", Scope::Full).rules, 0);
    }

    #[test]
    fn cosmetic_edge_cases_are_counted_or_dropped() {
        // Unsupported selector in a cosmetic rule is skipped.
        let conversion = convert("##+js(abort-on-property-read, x)\n", Scope::Full);
        assert_eq!(conversion.rules, 0);
        assert_eq!(conversion.skipped, 1);

        // A hiding exception with both include and exclude domains is unsupported.
        let conversion = convert("a.test,~b.test#@#.ad\n", Scope::Full);
        assert_eq!(conversion.rules, 0);
        assert_eq!(conversion.skipped, 1);

        // Domains containing '/' or '#' in the cosmetic domain part are not cosmetics.
        let conversion = convert("foo/bar##.ad\na#b##.ad\n", Scope::Full);
        assert_eq!(conversion.rules, 0);
        assert_eq!(conversion.skipped, 2);

        // A negated domain= option is rejected.
        assert_eq!(convert("||a.test^$~domain=x.test", Scope::Full).rules, 0);
        // Excluded domains in a domain= option are accepted.
        let rules = rules("||a.test^$domain=~x.test\n");
        assert_eq!(rules[0]["trigger"]["unless-domain"], json!(["*x.test"]));
    }

    #[test]
    fn large_batches_and_rule_limits_are_handled() {
        // SELECTORS_PER_RULE + 1 generic selectors force a batch flush.
        let mut list = String::new();
        for index in 0..(SELECTORS_PER_RULE + 1) {
            list.push_str(&format!("##.ad-{index}\n"));
        }
        let conversion = convert(&list, Scope::Full);
        let rules: Vec<Value> = serde_json::from_str(&conversion.json).unwrap();
        assert_eq!(conversion.skipped, 0);
        assert_eq!(rules.len(), 2);
        assert_eq!(rules[0]["action"]["type"], "css-display-none");

        // Rules beyond MAX_RULES are counted as skipped.
        let mut many = String::new();
        for index in 0..(MAX_RULES + 1) {
            many.push_str(&format!("||block{index}.test^\n"));
        }
        let conversion = convert(&many, Scope::Full);
        assert_eq!(conversion.rules, MAX_RULES);
        assert_eq!(conversion.skipped, 1);
    }

    #[test]
    fn url_filter_collapses_wildcards_and_rejects_unsupported_chars() {
        assert_eq!(url_filter("a**b", false), Some("a.*b".to_string()));
        // Non-graphic characters are rejected.
        assert_eq!(url_filter("a b", false), None);
        assert_eq!(url_filter("a\nb", false), None);
    }

    #[test]
    fn remaining_option_and_cosmetic_branches() {
        // Unsupported selector in a cosmetic rule increments skipped.
        let conversion = convert("##+js(abort-on-property-read, x)\n", Scope::Full);
        assert_eq!(conversion.skipped, 1);

        // document option is accepted.
        let doc_rules = rules("@@||trusted.test^$document\n@@||other.test^$doc\n");
        assert_eq!(doc_rules[0]["action"]["type"], "ignore-previous-rules");
        assert_eq!(doc_rules[1]["action"]["type"], "ignore-previous-rules");

        // subdocument mixed with other types is accepted (frames left to other filters).
        let mixed_rules = rules("||a.test^$script,subdocument\n");
        assert_eq!(
            mixed_rules[0]["trigger"]["resource-type"],
            json!(["script"])
        );
        assert!(mixed_rules[0]["trigger"].get("load-context").is_none());
    }

    #[test]
    fn cosmetic_batches_deduplicate_and_keep_domain_scopes_separate() {
        let mut list = String::new();
        for index in 0..=SELECTORS_PER_RULE {
            list.push_str(&format!("Example.TEST##.ad-{index:03}\n"));
        }
        list.push_str("example.test##.ad-000\n~shop.test##.ad-000\n");
        let conversion = convert(&list, Scope::Full);
        let rules: Vec<Value> = serde_json::from_str(&conversion.json).unwrap();
        assert_eq!(conversion.skipped, 0);
        assert_eq!(conversion.rules, 3);
        let scoped: Vec<_> = rules
            .iter()
            .filter(|rule| rule["trigger"]["if-domain"] == json!(["*example.test"]))
            .collect();
        assert_eq!(scoped.len(), 2);
        let selectors: Vec<_> = scoped
            .iter()
            .flat_map(|rule| rule["action"]["selector"].as_str().unwrap().split(", "))
            .collect();
        assert_eq!(selectors.len(), SELECTORS_PER_RULE + 1);
        assert_eq!(
            selectors
                .iter()
                .filter(|selector| **selector == ".ad-000")
                .count(),
            1
        );
        assert!(rules.iter().any(
            |rule| rule["trigger"]["unless-domain"] == json!(["*shop.test"])
                && rule["action"]["selector"] == ".ad-000"
        ));
    }

    #[test]
    fn rule_limit_counts_displaced_hiding_and_exceptions() {
        let mut list = String::from("@@||allowed.test^\n##.ad\n");
        for index in 0..MAX_RULES {
            list.push_str(&format!("||block{index}.test^\n"));
        }
        let conversion = convert(&list, Scope::Full);
        let rules: Vec<Value> = serde_json::from_str(&conversion.json).unwrap();
        assert_eq!(conversion.rules, MAX_RULES);
        assert_eq!(conversion.skipped, 2);
        assert!(rules.iter().all(|rule| rule["action"]["type"] == "block"));
    }

    #[test]
    fn literal_metacharacters_and_case_options_change_matching() {
        let rules = rules("|https://example.test/a+b?c|\n||EXAMPLE.test/Path$match-case\n");
        let literal = regex(rules[0]["trigger"]["url-filter"].as_str().unwrap());
        assert!(literal.is_match("https://example.test/a+b?c"));
        assert!(!literal.is_match("https://example.test/aaabc"));
        let case = regex(rules[1]["trigger"]["url-filter"].as_str().unwrap());
        assert!(case.is_match("https://EXAMPLE.test/Path"));
        assert!(!case.is_match("https://example.test/path"));
        assert_eq!(rules[1]["trigger"]["url-filter-is-case-sensitive"], true);
    }
}
