//! The News section of Settings: the feeds the user added, how long each
//! headline shows and how old one may be. Plain functions over
//! `NewsConfig`, so the rules are tested here; the editor sends the result
//! as one `set-news` edit, which undo covers.
//!
//! Adding a feed checks it first (`nexus_sources::news::check_feed`, off
//! the UI thread): its title and newest headline when it works, else why
//! not, in plain words.

use std::time::Duration;

use nexus_core::config::NewsConfig;
use nexus_sources::news::Problem;

/// How long each headline shows: the choices, in seconds (named in
/// app.slint).
pub const ROTATE: [u64; 3] = [8, 12, 20];
/// How old a headline may be: the choices, in hours.
pub const MAX_AGE: [u64; 3] = [24, 48, 7 * 24];

/// The `[news]` in use: the layout's, or the defaults when it has none.
pub fn current(news: Option<&NewsConfig>) -> NewsConfig {
    news.cloned().unwrap_or_default()
}

/// Which choice a duration is, if it is one of them.
pub fn rotate_index(c: &NewsConfig) -> Option<usize> {
    ROTATE.iter().position(|s| Duration::from_secs(*s) == c.rotate)
}

pub fn max_age_index(c: &NewsConfig) -> Option<usize> {
    MAX_AGE
        .iter()
        .position(|h| Duration::from_secs(h * 3600) == c.max_age)
}

/// A duration as words, for one that is not among the choices (set in a
/// layout file): "30 s", "15 min", "36 h", "3 days".
pub fn spoken(d: Duration) -> String {
    let s = d.as_secs();
    match s {
        0..=119 => format!("{s} s"),
        _ if s < 2 * 3600 => format!("{} min", s / 60),
        _ if s < 2 * 86_400 || s % 86_400 != 0 => format!("{} h", s / 3600),
        _ => format!("{} days", s / 86_400),
    }
}

/// The section with one setting changed: `key` is "rotate" or "max-age",
/// `index` a place in its choices. `None` for an unknown key or place.
pub fn with_choice(c: &NewsConfig, key: &str, index: usize) -> Option<NewsConfig> {
    let mut c = c.clone();
    match key {
        "rotate" => c.rotate = Duration::from_secs(*ROTATE.get(index)?),
        "max-age" => c.max_age = Duration::from_secs(MAX_AGE.get(index)? * 3600),
        _ => return None,
    }
    Some(c)
}

/// What the user typed, as the address to check: trimmed, and with
/// `https://` put in front when they left the scheme out ("example.org/rss").
pub fn address(typed: &str) -> String {
    let t = typed.trim();
    if t.is_empty() || t.contains("://") {
        t.to_string()
    } else {
        format!("https://{t}")
    }
}

/// The section with `url` added at the end, unless it is there already.
pub fn with_feed(c: &NewsConfig, url: &str) -> Option<NewsConfig> {
    if c.feeds.iter().any(|f| f.trim() == url) {
        return None;
    }
    let mut c = c.clone();
    c.feeds.push(url.to_string());
    Some(c)
}

pub fn without_feed(c: &NewsConfig, index: usize) -> Option<NewsConfig> {
    let mut c = c.clone();
    (index < c.feeds.len()).then(|| {
        c.feeds.remove(index);
        c
    })
}

/// The feed at `from` moved to `to`. On a tie between two copies of a
/// story the earlier feed's is shown, so the order is worth having.
pub fn moved_feed(c: &NewsConfig, from: usize, to: usize) -> Option<NewsConfig> {
    if from >= c.feeds.len() || to >= c.feeds.len() || from == to {
        return None;
    }
    let mut c = c.clone();
    let feed = c.feeds.remove(from);
    c.feeds.insert(to, feed);
    Some(c)
}

/// What is sent: no feeds and the default timings is no `[news]` at all.
pub fn to_send(c: NewsConfig) -> Option<NewsConfig> {
    (c != NewsConfig::default()).then_some(c)
}

/// Why a feed could not be added, in plain words, and whether to offer
/// "Add anyway" (when the site may only be down for now).
pub fn problem(p: &Problem) -> (String, bool) {
    match p {
        Problem::NotWebAddress => (
            "That is not a web address. A feed's address starts with https://".into(),
            false,
        ),
        Problem::NoAnswer => (
            "No answer from that site. Check the address, or that this computer is online."
                .into(),
            true,
        ),
        Problem::Redirects => (
            "That address keeps sending the request elsewhere, so no feed was found.".into(),
            false,
        ),
        Problem::NotFound => (
            "That site has nothing at this address. Check it for typing mistakes.".into(),
            false,
        ),
        Problem::Status(status) => (
            format!("That site answered with an error ({status}). It may work again later."),
            true,
        ),
        Problem::NotAFeed => (
            "That address is not a feed. On the site, look for a link named RSS, Atom or Feed, and paste that one.".into(),
            false,
        ),
        Problem::TooBig => (
            "That feed is too large to read (over 2 MB).".into(),
            false,
        ),
    }
}

/// A feed's name in the list: the title found when it was checked, else
/// its site's name ("news.example.org").
pub fn name(url: &str, title: Option<&str>) -> String {
    if let Some(t) = title.filter(|t| !t.trim().is_empty()) {
        return t.to_string();
    }
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let host = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = host.rsplit_once('@').map_or(host, |(_, h)| h);
    host.strip_prefix("www.").unwrap_or(host).to_string()
}

/// "No feeds yet", "1 feed", "3 feeds", for the news widget's editor.
pub fn count(feeds: &[String]) -> String {
    match feeds.iter().filter(|f| !f.trim().is_empty()).count() {
        0 => "No feeds yet".into(),
        1 => "1 feed".into(),
        n => format!("{n} feeds"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_are_plain_and_read_back() {
        let c = NewsConfig::default();
        assert_eq!(rotate_index(&c), Some(1), "12 s is the default");
        assert_eq!(max_age_index(&c), Some(1), "2 days is the default");
        let c = with_choice(&c, "rotate", 2).unwrap();
        let c = with_choice(&c, "max-age", 2).unwrap();
        assert_eq!(c.rotate, Duration::from_secs(20));
        assert_eq!(c.max_age, Duration::from_secs(7 * 86_400));
        assert_eq!((rotate_index(&c), max_age_index(&c)), (Some(2), Some(2)));
        assert_eq!(with_choice(&c, "rotate", 9), None);
        assert_eq!(with_choice(&c, "volume", 0), None);
        // One set in a file, said in words.
        assert_eq!(spoken(Duration::from_secs(30)), "30 s");
        assert_eq!(spoken(Duration::from_secs(900)), "15 min");
        assert_eq!(spoken(Duration::from_secs(36 * 3600)), "36 h");
        assert_eq!(spoken(Duration::from_secs(3 * 86_400)), "3 days");
    }

    #[test]
    fn feeds_are_added_once_removed_and_reordered() {
        let c = NewsConfig::default();
        assert_eq!(address("  news.example/rss "), "https://news.example/rss");
        assert_eq!(address("http://a.example/f"), "http://a.example/f");
        let c = with_feed(&c, "https://a.example/f").unwrap();
        let c = with_feed(&c, "https://b.example/f").unwrap();
        assert_eq!(with_feed(&c, "https://a.example/f"), None, "already there");
        let m = moved_feed(&c, 1, 0).unwrap();
        assert_eq!(m.feeds, ["https://b.example/f", "https://a.example/f"]);
        assert_eq!(moved_feed(&c, 0, 5), None);
        let r = without_feed(&m, 0).unwrap();
        assert_eq!(r.feeds, ["https://a.example/f"]);
        // The last one gone with the default timings: no section at all.
        assert_eq!(to_send(without_feed(&r, 0).unwrap()), None);
        assert!(to_send(r).is_some());
    }

    #[test]
    fn names_and_problems_are_plain_words() {
        assert_eq!(
            name("https://www.news.example/rss?k=1", None),
            "news.example"
        );
        assert_eq!(name("https://x.example/", Some("Harbour Gazette")), "Harbour Gazette");
        assert_eq!(count(&[]), "No feeds yet");
        assert_eq!(count(&["https://a".into(), "https://b".into()]), "2 feeds");
        for p in [
            Problem::NotWebAddress,
            Problem::NoAnswer,
            Problem::Redirects,
            Problem::NotFound,
            Problem::Status(500),
            Problem::NotAFeed,
            Problem::TooBig,
        ] {
            let (words, _) = problem(&p);
            assert!(words.starts_with("That") || words.starts_with("No answer"), "{words}");
            for jargon in ["ureq", "error:", "404", "parse"] {
                assert!(!words.contains(jargon), "{words}");
            }
        }
        assert!(problem(&Problem::NoAnswer).1, "a site that is down can be added anyway");
        assert!(!problem(&Problem::NotAFeed).1);
    }
}
