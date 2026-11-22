//! From a feed's bytes to headlines, and from many feeds' headlines to the
//! ones worth showing.
//!
//! feed-rs reads RSS 0.9x/2.0, RSS 1.0 (RDF), Atom 1.0 and JSON Feed. It
//! does not expand DTD entities, so a feed cannot make the parser fetch
//! anything or grow without bound.

use std::collections::HashSet;
use std::time::{Duration, SystemTime};

use super::clean;

/// Entries read from one feed. A feed with thousands (an archive) is cut
/// short: the strip only ever shows the newest few.
const MAX_PER_FEED: usize = 200;
/// The most headlines kept across all feeds.
pub const MAX_KEPT: usize = 50;
/// The longest link kept. Anything longer is not an article address.
const MAX_LINK: usize = 2048;

/// One feed, as read.
#[derive(Clone, Debug, PartialEq)]
pub struct Parsed {
    /// The feed's title, or its host name when it has none.
    pub source: String,
    pub entries: Vec<Entry>,
}

/// One entry, cleaned, with a title and a web link.
#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    /// The feed's own ID for it (guid, Atom id, JSON id); empty if none.
    pub id: String,
    pub title: String,
    pub link: String,
    /// When it was published, else when it was last updated.
    pub published: Option<SystemTime>,
}

/// A headline ready to show.
#[derive(Clone, Debug, PartialEq)]
pub struct Item {
    pub id: String,
    pub title: String,
    pub link: String,
    pub source: String,
    /// Published, updated or first seen, in that order of preference, and
    /// never later than when it was fetched.
    pub published: SystemTime,
}

/// Reads a feed. Entries with no title left after clean-up, or with no
/// http(s) link, are dropped here: they cannot be shown or opened.
pub fn parse(body: &[u8], url: &str) -> Result<Parsed, String> {
    let feed = feed_rs::parser::Builder::new()
        // feed-rs invents IDs for entries without one; an empty one tells
        // the dedupe step there was none to trust.
        .id_generator(|_, _, _| String::new())
        .build()
        .parse(body)
        .map_err(|e| format!("this is not a feed that can be read ({e})"))?;
    let source = feed
        .title
        .map(|t| clean::source(&t.content))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| host(url).to_string());
    let entries = feed
        .entries
        .into_iter()
        .take(MAX_PER_FEED)
        .filter_map(|e| {
            let title = clean::headline(&e.title?.content, &source);
            let link = pick_link(&e.links)?;
            (!title.is_empty()).then(|| Entry {
                id: clean::plain(&e.id),
                title,
                link,
                published: e.published.or(e.updated).map(SystemTime::from),
            })
        })
        .collect();
    Ok(Parsed { source, entries })
}

/// The article's address: the alternate link, else the first, if it is an
/// absolute http(s) URL. Relative links are not resolved; RSS requires
/// absolute ones and guessing a base could open the wrong page.
fn pick_link(links: &[feed_rs::model::Link]) -> Option<String> {
    let alternate = links
        .iter()
        .find(|l| l.rel.as_deref().is_none_or(|r| r == "alternate"));
    let link = alternate.or(links.first())?.href.trim();
    is_web_link(link).then(|| link.to_string())
}

/// An absolute http(s) URL with something after the scheme and nothing a
/// program receiving it could take as a second argument or a new line.
pub fn is_web_link(link: &str) -> bool {
    let lower = link.get(..8).unwrap_or(link).to_ascii_lowercase();
    let rest = if lower.starts_with("https://") {
        &link[8..]
    } else if lower.starts_with("http://") {
        &link[7..]
    } else {
        return false;
    };
    link.len() <= MAX_LINK
        && rest
            .chars()
            .next()
            .is_some_and(|c| c.is_alphanumeric() || c == '[')
        && !link.chars().any(|c| c.is_whitespace() || c.is_control())
}

/// "https://user@www.example.org:8080/feed?x" → "example.org".
pub fn host(url: &str) -> &str {
    let rest = url.split_once("://").map_or(url, |(_, r)| r);
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let host = match host.rsplit_once(':') {
        Some((h, port)) if !port.contains(']') => h,
        _ => host,
    };
    host.strip_prefix("www.").unwrap_or(host)
}

/// The headlines to show, newest first: at most [`MAX_KEPT`], none older
/// than `max_age`, and each story once. Feeds are given in the user's
/// order; on equal times the earlier feed's copy wins.
pub fn select<'a>(
    items: impl IntoIterator<Item = &'a Item>,
    now: SystemTime,
    max_age: Duration,
) -> Vec<Item> {
    let oldest = now.checked_sub(max_age).unwrap_or(SystemTime::UNIX_EPOCH);
    let mut fresh: Vec<&Item> = items
        .into_iter()
        .filter(|i| i.published >= oldest)
        .collect();
    // Stable, so ties keep feed order.
    fresh.sort_by_key(|i| std::cmp::Reverse(i.published));
    let (mut ids, mut links, mut titles) = (HashSet::new(), HashSet::new(), HashSet::new());
    let mut kept = Vec::new();
    for item in fresh {
        let link = link_key(&item.link);
        let title = title_key(&item.title);
        let seen = (!item.id.is_empty() && ids.contains(item.id.as_str()))
            || links.contains(&link)
            || (!title.is_empty() && titles.contains(&title));
        if seen {
            continue;
        }
        if !item.id.is_empty() {
            ids.insert(item.id.as_str());
        }
        links.insert(link);
        titles.insert(title);
        kept.push(item.clone());
        if kept.len() == MAX_KEPT {
            break;
        }
    }
    kept
}

/// The same article is often linked as http and https, or with and without
/// a trailing slash, by two feeds of one publisher.
fn link_key(link: &str) -> String {
    let rest = link.split_once("://").map_or(link, |(_, r)| r);
    rest.trim_end_matches('/').to_lowercase()
}

/// Titles that differ only in case, punctuation or spacing are one story:
/// wire copy reappears with a curly quote or a different dash.
fn title_key(title: &str) -> String {
    title
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: Duration = Duration::from_secs(3600);

    fn fixture(name: &str) -> Vec<u8> {
        let path = format!("{}/src/testdata/news/{name}", env!("CARGO_MANIFEST_DIR"));
        std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
    }

    fn read(name: &str) -> Parsed {
        parse(&fixture(name), "https://feeds.example.org/feed").unwrap()
    }

    fn at(rfc3339_secs: u64) -> SystemTime {
        SystemTime::UNIX_EPOCH + Duration::from_secs(rfc3339_secs)
    }

    // 2026-11-20T09:00:00Z, the time the fixtures are written around.
    const NOV_20_9AM: u64 = 1_795_165_200;

    #[test]
    fn reads_rss2() {
        let feed = read("rss2.xml");
        assert_eq!(feed.source, "Example World News");
        assert_eq!(feed.entries.len(), 2);
        let first = &feed.entries[0];
        assert_eq!(first.title, "Bridge reopens after repairs");
        assert_eq!(first.link, "https://news.example.org/bridge");
        assert_eq!(first.id, "urn:example:bridge");
        assert_eq!(first.published, Some(at(NOV_20_9AM)));
    }

    #[test]
    fn reads_rdf() {
        let feed = read("rdf.xml");
        assert_eq!(feed.source, "Example Tech (RDF)");
        assert_eq!(feed.entries[0].title, "Kernel 7.4 released");
        assert_eq!(feed.entries[0].link, "https://tech.example.org/kernel-7-4");
        assert_eq!(feed.entries[0].published, Some(at(NOV_20_9AM)));
    }

    #[test]
    fn reads_atom_and_prefers_published_then_updated() {
        let feed = read("atom.xml");
        assert_eq!(feed.source, "Example Linux Log");
        let [published, updated_only] = &feed.entries[..] else {
            panic!("{:?}", feed.entries);
        };
        assert_eq!(published.title, "Desktop 50 beta is out");
        assert_eq!(published.link, "https://linux.example.org/desktop-50");
        assert_eq!(published.published, Some(at(NOV_20_9AM)));
        assert_eq!(updated_only.published, Some(at(NOV_20_9AM - 3600)));
    }

    #[test]
    fn reads_json_feed() {
        let feed = read("feed.json");
        assert_eq!(feed.source, "Example JSON Daily");
        assert_eq!(feed.entries[0].title, "Rain expected all weekend");
        assert_eq!(feed.entries[0].link, "https://json.example.org/rain");
        assert_eq!(feed.entries[0].id, "rain-1");
    }

    #[test]
    fn cleans_hostile_entries() {
        let feed = read("nasty.xml");
        let titles: Vec<&str> = feed.entries.iter().map(|e| e.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Markets rise & fall",
                "“Quoted” headline — with entities",
                "Invoice gnp.exe opened",
                "No date at all",
            ],
            "the javascript: and relative links and the empty title are dropped"
        );
        assert!(feed.entries[3].published.is_none());
        assert!(feed.entries.iter().all(|e| is_web_link(&e.link)));
    }

    #[test]
    fn a_feed_without_a_title_is_named_by_its_host() {
        let body = br#"<rss version="2.0"><channel><item><title>T</title>
            <link>https://x.example/a</link></item></channel></rss>"#;
        let feed = parse(body, "https://www.example.com:8443/rss?k=1").unwrap();
        assert_eq!(feed.source, "example.com");
        assert!(parse(b"<html><body>Not a feed</body></html>", "https://x").is_err());
        assert!(parse(b"", "https://x").is_err());
    }

    #[test]
    fn web_links_only() {
        assert!(is_web_link("https://example.org/a"));
        assert!(is_web_link("HTTP://Example.org"));
        assert!(!is_web_link("javascript:alert(1)"));
        assert!(!is_web_link("file:///etc/passwd"));
        assert!(!is_web_link("https://"));
        assert!(!is_web_link("https://-oProxyCommand"));
        assert!(!is_web_link("https://example.org/a b"));
        assert!(!is_web_link("/relative"));
        assert!(!is_web_link(&format!(
            "https://e.org/{}",
            "a".repeat(MAX_LINK)
        )));
    }

    fn item(id: &str, title: &str, link: &str, source: &str, hours_ago: u64) -> Item {
        Item {
            id: id.into(),
            title: title.into(),
            link: link.into(),
            source: source.into(),
            published: at(NOV_20_9AM) - HOUR * hours_ago as u32,
        }
    }

    #[test]
    fn dedupes_by_id_then_link_then_title() {
        let now = at(NOV_20_9AM);
        let items = [
            item("a", "First", "https://one.example/1", "One", 1),
            item(
                "a",
                "Same id, other words",
                "https://two.example/9",
                "Two",
                1,
            ),
            item("", "Different words", "http://ONE.example/1/", "Two", 2),
            item("b", "“Shared” story!", "https://one.example/3", "One", 3),
            item("c", "shared story", "https://two.example/3", "Two", 3),
            item("", "Unique", "https://two.example/4", "Two", 4),
        ];
        let kept = select(&items, now, 48 * HOUR);
        let titles: Vec<&str> = kept.iter().map(|i| i.title.as_str()).collect();
        assert_eq!(titles, ["First", "“Shared” story!", "Unique"]);
    }

    #[test]
    fn the_cross_feed_fixtures_share_one_story() {
        let now = at(NOV_20_9AM + 600);
        let to_items = |p: Parsed| -> Vec<Item> {
            p.entries
                .into_iter()
                .map(|e| Item {
                    id: e.id,
                    title: e.title,
                    link: e.link,
                    source: p.source.clone(),
                    published: e.published.unwrap_or(now),
                })
                .collect()
        };
        let mut all = to_items(read("duplicate-a.xml"));
        all.extend(to_items(read("duplicate-b.xml")));
        let kept = select(&all, now, 48 * HOUR);
        let shared: Vec<&Item> = kept.iter().filter(|i| i.title.contains("Comet")).collect();
        assert_eq!(shared.len(), 1, "{kept:?}");
        assert_eq!(
            shared[0].source, "Wire A",
            "the first feed's copy wins a tie"
        );
        assert_eq!(kept.len(), 3);
    }

    #[test]
    fn drops_old_items_sorts_newest_first_and_caps() {
        let now = at(NOV_20_9AM);
        let items = [
            item("old", "Too old", "https://e.example/old", "E", 49),
            item("edge", "Just in", "https://e.example/edge", "E", 48),
            item("new", "Newest", "https://e.example/new", "E", 0),
        ];
        let kept = select(&items, now, 48 * HOUR);
        let ids: Vec<&str> = kept.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, ["new", "edge"]);
        assert!(select(&items, now, HOUR / 2).len() == 1);

        let many: Vec<Item> = (0..80)
            .map(|n| {
                item(
                    &n.to_string(),
                    &format!("Story {n}"),
                    &format!("https://e.example/{n}"),
                    "E",
                    n % 40,
                )
            })
            .collect();
        let kept = select(&many, now, 48 * HOUR);
        assert_eq!(kept.len(), MAX_KEPT);
        assert!(kept.windows(2).all(|w| w[0].published >= w[1].published));
    }

    #[test]
    fn host_names() {
        assert_eq!(host("https://www.example.org/feed"), "example.org");
        assert_eq!(
            host("http://user:pw@feeds.example.org:81"),
            "feeds.example.org"
        );
        assert_eq!(host("https://[::1]:8080/x"), "[::1]");
    }
}
