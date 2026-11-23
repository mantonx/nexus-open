//! The source as a whole: rotation, configuration, and real HTTP against a
//! server on 127.0.0.1. Nothing here reaches the network.

use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use super::*;

fn fixture(name: &str) -> Vec<u8> {
    let path = format!("{}/src/testdata/news/{name}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read(&path).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn text<'a>(readings: &'a [Reading], name: &str) -> &'a str {
    match readings.iter().find(|(n, _)| *n == name) {
        Some((_, Value::Text(t))) => t,
        other => panic!("{name}: {other:?}"),
    }
}

fn number(readings: &[Reading], name: &str) -> f64 {
    match readings.iter().find(|(n, _)| *n == name) {
        Some((_, Value::Number(v, Unit::None))) => *v,
        other => panic!("{name}: {other:?}"),
    }
}

// ---- configuration ----

#[test]
fn config_reads_seconds_or_units_and_has_defaults() {
    let c: NewsConfig = toml::from_str("").unwrap();
    assert_eq!(c, NewsConfig::default());
    assert_eq!(c.rotate, Duration::from_secs(12));
    assert_eq!(c.max_age, Duration::from_secs(48 * 3600));

    let c: NewsConfig =
        toml::from_str("feeds = [\"https://example.org/feed\"]\nrotate = 20\nmax_age = \"36h\"")
            .unwrap();
    assert_eq!(c.feeds, ["https://example.org/feed"]);
    assert_eq!(c.rotate, Duration::from_secs(20));
    assert_eq!(c.max_age, Duration::from_secs(36 * 3600));

    for (text, secs) in [
        ("\"90s\"", 90.0),
        ("\"15m\"", 900.0),
        ("\"2d\"", 172_800.0),
        ("7.5", 7.5),
    ] {
        let c: NewsConfig = toml::from_str(&format!("rotate = {text}")).unwrap();
        assert_eq!(c.rotate.as_secs_f64(), secs, "{text}");
    }
    for bad in [
        "rotate = -1",
        "rotate = \"soon\"",
        "rotate = \"5 weeks\"",
        "colour = 1",
    ] {
        assert!(toml::from_str::<NewsConfig>(bad).is_err(), "{bad}");
    }

    // Written back the way a person would write it.
    let out = toml::to_string(&NewsConfig::default()).unwrap();
    assert!(
        out.contains("rotate = \"12s\"") && out.contains("max_age = \"48h\""),
        "{out}"
    );
    let back: NewsConfig = toml::from_str(&out).unwrap();
    assert_eq!(back, NewsConfig::default());
}

#[test]
fn backoff_doubles_up_to_the_cap() {
    let t = Timings::REAL;
    let minutes: Vec<u64> = (1..=8).map(|n| t.backoff(n).as_secs() / 60).collect();
    assert_eq!(minutes, [1, 2, 4, 8, 16, 30, 30, 30]);
    assert_eq!(t.backoff(u32::MAX), t.retry_max);
}

// ---- rotation, without HTTP ----

type Published = std::rc::Rc<std::cell::RefCell<Vec<Vec<Reading>>>>;

fn source() -> (Source<impl FnMut(Vec<Reading>) -> bool>, Published) {
    let published = Published::default();
    let log = published.clone();
    let (tx, _rx) = mpsc::channel();
    let source = Source::new(
        move |r| {
            log.borrow_mut().push(r);
            true
        },
        Timings::REAL,
        tx,
    );
    (source, published)
}

fn story(n: u64, wall: SystemTime) -> Item {
    Item {
        id: format!("id-{n}"),
        title: format!("Story {n}"),
        link: format!("https://e.example/{n}"),
        source: "E".into(),
        published: wall - Duration::from_secs(60 * n),
    }
}

/// A source whose one feed already holds `n` stories, as if fetched.
fn with_stories(
    n: u64,
) -> (
    Source<impl FnMut(Vec<Reading>) -> bool>,
    Published,
    Instant,
    SystemTime,
) {
    let (mut s, published) = source();
    let (now, wall) = (Instant::now(), SystemTime::now());
    s.config = Some(NewsConfig {
        feeds: vec!["https://e.example/feed".into()],
        ..NewsConfig::default()
    });
    let mut feed = Feed::new("https://e.example/feed".into(), now);
    // Out of order on purpose: the source sorts.
    feed.items = (0..n).rev().map(|i| story(i, wall)).collect();
    s.feeds.push(feed);
    (s, published, now, wall)
}

#[test]
fn rotates_newest_first_and_wraps() {
    let (mut s, published, now, wall) = with_stories(3);
    s.rebuild(now, wall);
    s.show(wall);
    for _ in 0..4 {
        s.advance(now, wall);
    }
    let titles: Vec<String> = published
        .borrow()
        .iter()
        .map(|r| text(r, "news.title").to_string())
        .collect();
    assert_eq!(
        titles,
        ["Story 0", "Story 1", "Story 2", "Story 0", "Story 1"]
    );
    let last = &published.borrow()[4];
    assert_eq!(text(last, "news.source"), "E");
    assert_eq!(text(last, "news.link"), "https://e.example/1");
    assert_eq!(number(last, "news.age"), 60.0);
    assert_eq!(number(last, "news.count"), 3.0);
    assert_eq!(s.next_turn, Some(now + DEFAULT_ROTATE));
}

#[test]
fn a_refresh_keeps_the_headline_showing() {
    let (mut s, published, now, wall) = with_stories(3);
    s.rebuild(now, wall);
    s.advance(now, wall); // Story 1
    // A newer story arrives at the top; Story 1 stays, and is next followed
    // by Story 2, not by whatever moved into its old place.
    s.feeds[0]
        .items
        .push(story(0, wall + Duration::from_secs(30)));
    s.feeds[0].items[3].id = "id-new".into();
    s.feeds[0].items[3].link = "https://e.example/new".into();
    s.feeds[0].items[3].title = "Newer".into();
    s.rebuild(now, wall + Duration::from_secs(30));
    s.show(wall);
    s.advance(now, wall + Duration::from_secs(30));
    let titles: Vec<String> = published
        .borrow()
        .iter()
        .map(|r| text(r, "news.title").to_string())
        .collect();
    assert_eq!(titles, ["Story 1", "Story 1", "Story 2"]);
}

#[test]
fn nothing_is_published_until_there_is_news_and_blanks_once_after() {
    let (mut s, published) = source();
    s.configure(Some(NewsConfig::default()), Instant::now());
    s.advance(Instant::now(), SystemTime::now());
    assert!(published.borrow().is_empty(), "no feeds, nothing published");

    let (mut s, published, now, wall) = with_stories(2);
    s.rebuild(now, wall);
    s.show(wall);
    s.configure(None, now);
    s.advance(now, wall);
    s.configure(None, now);
    let p = published.borrow();
    assert_eq!(p.len(), 2, "one headline, then one blank: {p:?}");
    assert_eq!(text(&p[1], "news.title"), "");
    assert_eq!(text(&p[1], "news.link"), "");
    assert_eq!(number(&p[1], "news.count"), 0.0);
}

#[test]
fn headlines_age_out_while_rotating() {
    let (mut s, published, now, wall) = with_stories(3);
    s.config.as_mut().unwrap().max_age = Duration::from_secs(150);
    s.rebuild(now, wall);
    s.show(wall);
    // Two minutes on, only Story 0 (now 2 min old) is under 150 s.
    s.advance(now, wall + Duration::from_secs(120));
    let p = published.borrow();
    assert_eq!(number(&p[0], "news.count"), 3.0);
    assert_eq!(text(&p[1], "news.title"), "Story 0");
    assert_eq!(number(&p[1], "news.count"), 1.0);
}

#[test]
fn with_every_feed_failing_the_last_headline_stays_lost() {
    let (mut s, published, now, wall) = with_stories(2);
    s.config.as_mut().unwrap().max_age = Duration::from_secs(150);
    s.rebuild(now, wall);
    s.show(wall);
    assert_eq!(number(&published.borrow()[0], "news.lost"), 0.0);
    // The feed starts failing; its headlines stay while they are young.
    s.feeds[0].failed("offline".into(), now, &Timings::REAL);
    s.advance(now, wall + Duration::from_secs(30));
    assert_eq!(text(&published.borrow()[1], "news.title"), "Story 1");
    assert_eq!(number(&published.borrow()[1], "news.lost"), 0.0);
    // Ten minutes on, all too old: the one shown last stays, lost.
    s.advance(now, wall + Duration::from_secs(600));
    {
        let p = published.borrow();
        let last = p.last().unwrap();
        assert_eq!(text(last, "news.title"), "Story 1");
        assert_eq!(text(last, "news.link"), "https://e.example/1");
        assert_eq!(number(last, "news.lost"), 1.0);
        assert_eq!(number(last, "news.age"), 660.0, "and keeps ageing");
        assert_eq!(number(last, "news.count"), 0.0);
    }
    // The feed comes back with nothing new: a blank, not lost.
    s.feeds[0].succeeded(now, &Timings::REAL);
    s.advance(now, wall + Duration::from_secs(600));
    let p = published.borrow();
    let last = p.last().unwrap();
    assert_eq!(text(last, "news.title"), "");
    assert_eq!(number(last, "news.lost"), 0.0);
}

#[test]
fn a_feed_that_is_fine_but_quiet_blanks_rather_than_lost() {
    let (mut s, published, now, wall) = with_stories(1);
    s.config.as_mut().unwrap().max_age = Duration::from_secs(150);
    s.rebuild(now, wall);
    s.show(wall);
    s.advance(now, wall + Duration::from_secs(600));
    let p = published.borrow();
    assert_eq!(text(p.last().unwrap(), "news.title"), "");
    assert_eq!(number(p.last().unwrap(), "news.lost"), 0.0);
}

#[test]
fn undated_entries_keep_their_first_seen_time() {
    let mut feed = Feed::new("https://nasty.example/".into(), Instant::now());
    let parsed = parse::parse(&fixture("nasty.xml"), "https://nasty.example/").unwrap();
    let first = SystemTime::UNIX_EPOCH + Duration::from_secs(1_795_166_000);
    feed.take(parsed.clone(), first);
    let undated = |f: &Feed| {
        f.items
            .iter()
            .find(|i| i.title == "No date at all")
            .unwrap()
            .published
    };
    assert_eq!(undated(&feed), first);
    feed.take(parsed, first + Duration::from_secs(3600));
    assert_eq!(undated(&feed), first, "not moved to the top again");
    assert!(feed.items.iter().all(|i| i.source == "Nasty & Co"));
}

#[test]
fn bad_addresses_are_never_fetched() {
    let (mut s, _published) = source();
    s.configure(
        Some(NewsConfig {
            feeds: vec![
                "file:///etc/passwd".into(),
                " ".into(),
                "https://e.example/a".into(),
                "https://e.example/a".into(),
            ],
            ..NewsConfig::default()
        }),
        Instant::now(),
    );
    assert_eq!(s.feeds.len(), 2, "blank and repeated addresses dropped");
    assert!(!s.feeds[0].usable);
    assert!(s.feeds[1].usable);
}

// ---- HTTP, against a local server ----

struct Reply {
    status: u16,
    headers: Vec<(&'static str, String)>,
    body: Vec<u8>,
}

impl Reply {
    fn new(status: u16, body: Vec<u8>) -> Reply {
        Reply {
            status,
            headers: Vec::new(),
            body,
        }
    }

    fn header(mut self, name: &'static str, value: &str) -> Reply {
        self.headers.push((name, value.into()));
        self
    }
}

/// A one-connection-at-a-time HTTP server on 127.0.0.1 that answers with
/// `replies` in order, then stops. Each request's head (request line and
/// headers, lowercased) and arrival time come out of the receiver.
fn serve(replies: Vec<Reply>) -> (String, Receiver<(Instant, String)>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/feed", listener.local_addr().unwrap());
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        for reply in replies {
            let Ok((stream, _)) = listener.accept() else {
                return;
            };
            let mut reader = BufReader::new(stream);
            let mut head = String::new();
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
                head.push_str(&line.to_ascii_lowercase());
            }
            let _ = tx.send((Instant::now(), head));
            let mut stream = reader.into_inner();
            let mut out = format!(
                "HTTP/1.1 {} X\r\nContent-Length: {}\r\nConnection: close\r\n",
                reply.status,
                reply.body.len()
            );
            for (name, value) in &reply.headers {
                out.push_str(&format!("{name}: {value}\r\n"));
            }
            out.push_str("\r\n");
            // The client may hang up early (a body over the limit).
            let _ = stream.write_all(out.as_bytes());
            let _ = stream.write_all(&reply.body);
        }
    });
    (url, rx)
}

const WAIT: Duration = Duration::from_secs(5);

#[test]
fn fetches_then_revalidates_then_backs_off_and_keeps_headlines() {
    let rss = fixture("rss2.xml");
    let (url, requests) = serve(vec![
        Reply::new(200, rss.clone())
            .header("ETag", "\"v1\"")
            .header("Last-Modified", "Fri, 20 Nov 2026 09:00:00 GMT"),
        Reply::new(304, Vec::new()),
        Reply::new(500, b"oops".to_vec()),
        Reply::new(500, b"oops".to_vec()),
        Reply::new(500, b"oops".to_vec()),
        Reply::new(200, fixture("atom.xml")).header("ETag", "\"v2\""),
    ]);
    let timings = Timings {
        refresh: Duration::from_millis(150),
        retry: Duration::from_millis(100),
        retry_max: Duration::from_millis(250),
        timeout: Duration::from_secs(2),
    };
    let (tx, published) = mpsc::channel();
    let news = spawn_with(move |r| tx.send(r).is_ok(), timings);
    assert!(news.configure(Some(NewsConfig {
        feeds: vec![url],
        // The fixtures are dated; keep them whatever today is.
        max_age: Duration::from_secs(100 * 365 * 86400),
        ..NewsConfig::default()
    })));

    let heads: Vec<(Instant, String)> = (0..6)
        .map(|n| {
            requests
                .recv_timeout(WAIT)
                .unwrap_or_else(|e| panic!("request {n}: {e}"))
        })
        .collect();
    let [first, revalidate, fail1, fail2, fail3, recovered] = &heads[..] else {
        unreachable!()
    };

    assert!(first.1.starts_with("get /feed http/1.1"), "{}", first.1);
    assert!(first.1.contains("user-agent: nexus-open/"), "{}", first.1);
    assert!(!first.1.contains("if-none-match"), "{}", first.1);
    assert!(
        revalidate.1.contains("if-none-match: \"v1\""),
        "{}",
        revalidate.1
    );
    assert!(
        revalidate
            .1
            .contains("if-modified-since: fri, 20 nov 2026 09:00:00 gmt")
    );
    assert!(!revalidate.1.contains("cookie"), "{}", revalidate.1);

    // Waits: refresh after a success, then 100 ms, 200 ms and the 250 ms cap.
    let gap = |a: &(Instant, String), b: &(Instant, String)| b.0 - a.0;
    assert!(gap(first, revalidate) >= timings.refresh);
    assert!(gap(revalidate, fail1) >= timings.refresh);
    assert!(gap(fail1, fail2) >= Duration::from_millis(100));
    assert!(gap(fail2, fail3) >= Duration::from_millis(200));
    assert!(gap(fail3, recovered) >= Duration::from_millis(250));

    // Every publish until the recovery shows the RSS headlines: the 304 and
    // the 500s kept them, and nothing was ever blanked.
    let mut seen = Vec::new();
    let deadline = Instant::now() + WAIT;
    while Instant::now() < deadline {
        let Ok(r) = published.recv_timeout(Duration::from_millis(100)) else {
            continue;
        };
        let title = text(&r, "news.title").to_string();
        assert_ne!(title, "", "never blanked");
        let done = title == "Desktop 50 beta is out";
        seen.push((title, number(&r, "news.count")));
        if done {
            break;
        }
    }
    let (last, before) = seen.split_last().expect("something was published");
    assert_eq!(last.0, "Desktop 50 beta is out", "{seen:?}");
    assert!(!before.is_empty());
    for (title, count) in before {
        assert_eq!(
            (title.as_str(), *count),
            ("Bridge reopens after repairs", 2.0)
        );
    }
}

#[test]
fn a_body_over_the_limit_is_refused() {
    let mut huge = b"<rss version=\"2.0\"><channel><title>Big</title>".to_vec();
    huge.resize(3 * 1024 * 1024, b' ');
    let (url, requests) = serve(vec![Reply::new(200, huge)]);
    let agent = fetch::agent(Duration::from_secs(5));
    let e = fetch::fetch(&agent, &url, &Validators::default()).unwrap_err();
    assert!(e.contains("larger than 2 MB"), "{e}");
    requests.recv_timeout(WAIT).unwrap();
}

#[test]
fn redirects_are_followed_up_to_five() {
    let moved = || Reply::new(301, Vec::new()).header("Location", "/moved");
    let (url, requests) = serve(vec![
        moved(),
        moved(),
        Reply::new(200, fixture("feed.json")),
    ]);
    let agent = fetch::agent(Duration::from_secs(5));
    match fetch::fetch(&agent, &url, &Validators::default()) {
        Ok(Fetched::Fresh { feed, .. }) => assert_eq!(feed.source, "Example JSON Daily"),
        other => panic!("{other:?}"),
    }
    assert!(
        requests
            .recv_timeout(WAIT)
            .unwrap()
            .1
            .starts_with("get /feed ")
    );
    assert!(
        requests
            .recv_timeout(WAIT)
            .unwrap()
            .1
            .starts_with("get /moved ")
    );

    let (url, _requests) = serve((0..7).map(|_| moved()).collect());
    let e = fetch::fetch(&agent, &url, &Validators::default()).unwrap_err();
    assert!(e.contains("more than 5"), "{e}");
}

// ---- checking one feed, for the window ----

#[test]
fn a_checked_feed_gives_its_title_and_newest_headline() {
    let (url, _requests) = serve(vec![Reply::new(200, fixture("rss2.xml"))]);
    assert_eq!(
        check::check_within(&url, WAIT),
        Ok(Checked {
            title: "Example World News".into(),
            newest: Some("Bridge reopens after repairs".into()),
        })
    );
    let (url, _requests) = serve(vec![Reply::new(200, fixture("feed.json"))]);
    let checked = check::check_within(&url, WAIT).unwrap();
    assert_eq!(checked.title, "Example JSON Daily");
    assert!(checked.newest.is_some());
}

#[test]
fn a_feed_that_cannot_be_read_says_why_by_kind() {
    let page = b"<!doctype html><html><head><title>Home</title></head><body>Hi</body></html>";
    let (url, _r) = serve(vec![Reply::new(200, page.to_vec())]);
    assert_eq!(check::check_within(&url, WAIT), Err(Problem::NotAFeed));
    let (url, _r) = serve(vec![Reply::new(404, Vec::new())]);
    assert_eq!(check::check_within(&url, WAIT), Err(Problem::NotFound));
    let (url, _r) = serve(vec![Reply::new(503, Vec::new())]);
    assert_eq!(check::check_within(&url, WAIT), Err(Problem::Status(503)));
    assert_eq!(
        check_feed("file:///etc/passwd"),
        Err(Problem::NotWebAddress)
    );
    // Nothing listens there: bound, then closed.
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    assert_eq!(
        check::check_within(&format!("http://127.0.0.1:{port}/feed"), WAIT),
        Err(Problem::NoAnswer)
    );
}
