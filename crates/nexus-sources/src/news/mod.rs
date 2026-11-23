//! News headlines from the feeds the user adds, and only those.
//!
//! Nothing is fetched until a feed is added, and nothing but a GET for
//! each feed is sent. Each feed is asked again every [`Timings::refresh`];
//! a feed that has not changed answers 304 and costs nothing. A feed that
//! fails is tried again after a minute, then two, four, up to half an hour,
//! and its last good headlines stay on the strip meanwhile: being offline
//! for a while should not empty the widget.
//!
//! The source keeps up to [`parse::MAX_KEPT`] headlines across all feeds,
//! newest first, and shows one at a time, moving to the next every
//! `rotate`. It publishes:
//!
//! - `news.title`, `news.source`, `news.link`: text;
//! - `news.age`: seconds since the headline was published, a plain number
//!   (there is no unit for time spans, so it is [`Unit::None`], as
//!   `media.position` is);
//! - `news.count`: how many headlines are kept;
//! - `news.lost`: 1 while every feed is failing and no headline is new
//!   enough to show, else 0.
//!
//! With no feeds, or none read yet, nothing is published; if headlines were
//! shown and then go (the last feed removed, everything too old), the text
//! signals are blanked and the numbers set to 0, as the media source does
//! when nothing is playing. The exception is a source that has lost every
//! feed: when its last headlines grow too old it keeps the one it showed
//! last, with `news.lost` 1, so the widget can show it greyed instead of
//! acting as if there were no news. Problems go to the log, once per feed
//! each time its state changes, never into the signals.

use std::sync::Arc;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use nexus_core::signal::{Unit, Value};

use crate::Reading;

mod check;
mod clean;
pub(crate) mod fetch;
mod parse;
#[cfg(test)]
mod tests;

pub use check::{Checked, Problem, check as check_feed};
pub use fetch::check_url;
use fetch::{Fetched, Validators};
use parse::Item;

/// How old a headline may be by default before it is dropped.
pub use nexus_core::config::NEWS_MAX_AGE as DEFAULT_MAX_AGE;
/// The shortest `rotate` honoured.
pub use nexus_core::config::NEWS_MIN_ROTATE as MIN_ROTATE;
/// How long each headline shows by default.
pub use nexus_core::config::NEWS_ROTATE as DEFAULT_ROTATE;
pub use nexus_core::config::NewsConfig;

/// How often feeds are asked, and how failures back off. Fixed in use;
/// tests shorten them so a local server can be exercised in milliseconds.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Timings {
    /// Between two successful fetches of one feed. Publishers rarely update
    /// more often, and many ask readers not to poll faster.
    pub refresh: Duration,
    /// The wait after a feed's first failure; it doubles with each one.
    pub retry: Duration,
    /// The longest wait between attempts at a failing feed.
    pub retry_max: Duration,
    /// For one whole request, redirects and body included.
    pub timeout: Duration,
}

impl Timings {
    pub const REAL: Timings = Timings {
        refresh: Duration::from_secs(15 * 60),
        retry: Duration::from_secs(60),
        retry_max: Duration::from_secs(30 * 60),
        timeout: Duration::from_secs(10),
    };

    /// The wait after the `failures`th failure in a row (counting from 1).
    fn backoff(&self, failures: u32) -> Duration {
        let doublings = failures.saturating_sub(1).min(20);
        self.retry
            .saturating_mul(1 << doublings)
            .min(self.retry_max)
    }
}

/// A handle on the news source. Dropping the last clone stops it.
#[derive(Clone)]
pub struct News {
    inner: Arc<Handle>,
}

struct Handle {
    tx: Sender<Msg>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        // The source's own fetch threads hold senders too, so the channel
        // would not close by itself.
        let _ = self.tx.send(Msg::Stop);
    }
}

impl News {
    /// Sends the `[news]` settings; `None` stops fetching and clears the
    /// headlines. Feeds new to the list are fetched straight away; feeds
    /// that stay keep their headlines and their schedule. Returns false if
    /// the source has stopped.
    pub fn configure(&self, config: Option<NewsConfig>) -> bool {
        self.inner.tx.send(Msg::Configure(config)).is_ok()
    }

    /// Shows the next headline now, and gives it a full `rotate` before the
    /// one after. Does nothing while there are no headlines.
    pub fn next(&self) -> bool {
        self.inner.tx.send(Msg::Next).is_ok()
    }
}

enum Msg {
    Configure(Option<NewsConfig>),
    Next,
    Fetched {
        url: String,
        result: Result<Fetched, String>,
    },
    Stop,
}

/// Starts the news source. It does nothing until [`News::configure`] gives
/// it feeds, passes readings to `publish`, and stops when that returns
/// false or every [`News`] handle is dropped.
pub fn spawn(publish: impl FnMut(Vec<Reading>) -> bool + Send + 'static) -> News {
    spawn_with(publish, Timings::REAL)
}

pub(crate) fn spawn_with(
    publish: impl FnMut(Vec<Reading>) -> bool + Send + 'static,
    timings: Timings,
) -> News {
    let (tx, rx) = mpsc::channel();
    let mut source = Source::new(publish, timings, tx.clone());
    thread::Builder::new()
        .name("news-source".into())
        .spawn(move || source.run(&rx))
        .expect("spawning a thread");
    News {
        inner: Arc::new(Handle { tx }),
    }
}

/// One feed the user added, and what is known of it.
struct Feed {
    url: String,
    /// False when the address is not one we fetch; it is never asked.
    usable: bool,
    sent: Validators,
    items: Vec<Item>,
    due: Instant,
    fetching: bool,
    failures: u32,
    /// The last problem logged, so the same one is not logged again.
    problem: Option<String>,
}

impl Feed {
    fn new(url: String, now: Instant) -> Feed {
        let usable = match check_url(&url) {
            Ok(()) => true,
            Err(e) => {
                eprintln!("nexusd: news: {e}");
                false
            }
        };
        Feed {
            url,
            usable,
            sent: Validators::default(),
            items: Vec::new(),
            due: now,
            fetching: false,
            failures: 0,
            problem: None,
        }
    }

    fn succeeded(&mut self, now: Instant, timings: &Timings) {
        if self.problem.take().is_some() {
            eprintln!(
                "nexusd: news: {}: working again",
                fetch::redacted(&self.url)
            );
        }
        self.failures = 0;
        self.due = now + timings.refresh;
    }

    fn failed(&mut self, problem: String, now: Instant, timings: &Timings) {
        self.failures += 1;
        self.due = now + timings.backoff(self.failures);
        if self.problem.as_ref() != Some(&problem) {
            let kept = if self.items.is_empty() {
                ""
            } else {
                "; keeping its last headlines"
            };
            eprintln!(
                "nexusd: news: {}: {problem}{kept}",
                fetch::redacted(&self.url)
            );
            self.problem = Some(problem);
        }
    }

    /// The feed's new entries as headlines. An entry without a date keeps
    /// the time it was first seen, so it does not jump back to the top on
    /// every fetch; a date in the future (a wrong clock, a scheduled post)
    /// is treated the same way, or it would sit first for days.
    fn take(&mut self, feed: parse::Parsed, wall: SystemTime) {
        let key = |id: &str, link: &str| if id.is_empty() { link } else { id }.to_string();
        let items = feed
            .entries
            .into_iter()
            .map(|e| {
                let k = key(&e.id, &e.link);
                let seen = self
                    .items
                    .iter()
                    .find(|i| key(&i.id, &i.link) == k)
                    .map(|i| i.published);
                let published = e.published.filter(|&p| p <= wall).or(seen).unwrap_or(wall);
                Item {
                    id: e.id,
                    title: e.title,
                    link: e.link,
                    source: feed.source.clone(),
                    published,
                }
            })
            .collect();
        self.items = items;
    }
}

struct Source<P> {
    publish: P,
    alive: bool,
    timings: Timings,
    agent: ureq::Agent,
    /// For fetch threads to report back on.
    results: Sender<Msg>,
    config: Option<NewsConfig>,
    feeds: Vec<Feed>,
    /// The headlines that may show, newest first.
    kept: Vec<Item>,
    /// Which of `kept` is showing.
    at: Option<usize>,
    next_turn: Option<Instant>,
    /// Whether a headline has been published since the last blank, so a
    /// blank is sent once when they go, and never before any came.
    shown: bool,
    /// The headline last published, kept on the strip as lost when every
    /// feed fails and nothing new enough is left.
    last: Option<Item>,
}

impl<P: FnMut(Vec<Reading>) -> bool> Source<P> {
    fn new(publish: P, timings: Timings, results: Sender<Msg>) -> Self {
        Source {
            publish,
            alive: true,
            timings,
            agent: fetch::agent(timings.timeout),
            results,
            config: None,
            feeds: Vec::new(),
            kept: Vec::new(),
            at: None,
            next_turn: None,
            shown: false,
            last: None,
        }
    }

    fn run(&mut self, rx: &mpsc::Receiver<Msg>) {
        while self.alive {
            let now = Instant::now();
            self.start_fetches(now);
            if self.next_turn.is_some_and(|t| t <= now) {
                self.advance(now, SystemTime::now());
            }
            let wake = self.next_wake();
            let message = match wake {
                None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                Some(at) => rx.recv_timeout(at.saturating_duration_since(Instant::now())),
            };
            match message {
                Ok(Msg::Configure(config)) => self.configure(config, Instant::now()),
                Ok(Msg::Next) => self.advance(Instant::now(), SystemTime::now()),
                Ok(Msg::Fetched { url, result }) => self.fetched(&url, result),
                Err(RecvTimeoutError::Timeout) => {}
                Ok(Msg::Stop) | Err(RecvTimeoutError::Disconnected) => return,
            }
        }
    }

    /// Starts a fetch for every feed that is due. Each runs on its own
    /// thread, so a slow feed holds up neither the others nor the rotation
    /// (a hold for "next" must answer at once).
    fn start_fetches(&mut self, now: Instant) {
        for feed in &mut self.feeds {
            if !feed.usable || feed.fetching || feed.due > now {
                continue;
            }
            feed.fetching = true;
            let agent = self.agent.clone();
            let results = self.results.clone();
            let url = feed.url.clone();
            let sent = feed.sent.clone();
            let started = thread::Builder::new()
                .name("news-fetch".into())
                .spawn(move || {
                    let result = fetch::fetch(&agent, &url, &sent);
                    let _ = results.send(Msg::Fetched { url, result });
                });
            if let Err(e) = started {
                feed.fetching = false;
                feed.failed(format!("cannot start a fetch: {e}"), now, &self.timings);
            }
        }
    }

    fn next_wake(&self) -> Option<Instant> {
        self.feeds
            .iter()
            .filter(|f| f.usable && !f.fetching)
            .map(|f| f.due)
            .chain(self.next_turn)
            .min()
    }

    fn configure(&mut self, config: Option<NewsConfig>, now: Instant) {
        if config == self.config {
            return;
        }
        let mut old = std::mem::take(&mut self.feeds);
        let mut urls: Vec<&str> = Vec::new();
        for url in config.iter().flat_map(|c| &c.feeds) {
            let url = url.trim();
            if !url.is_empty() && !urls.contains(&url) {
                urls.push(url);
            }
        }
        self.feeds = urls
            .into_iter()
            .map(|url| match old.iter().position(|f| f.url == url) {
                Some(i) => old.swap_remove(i),
                None => Feed::new(url.to_string(), now),
            })
            .collect();
        if config.is_none() {
            self.last = None;
        }
        self.config = config;
        self.rebuild(now, SystemTime::now());
        self.show(SystemTime::now());
    }

    fn fetched(&mut self, url: &str, result: Result<Fetched, String>) {
        let (now, wall) = (Instant::now(), SystemTime::now());
        // Gone from the config while it was being fetched.
        let Some(feed) = self.feeds.iter_mut().find(|f| f.url == url) else {
            return;
        };
        feed.fetching = false;
        match result {
            Ok(Fetched::Fresh {
                feed: parsed,
                etag,
                last_modified,
            }) => {
                feed.take(parsed, wall);
                feed.sent = Validators {
                    etag,
                    last_modified,
                };
                feed.succeeded(now, &self.timings);
            }
            Ok(Fetched::NotModified) => feed.succeeded(now, &self.timings),
            Err(problem) => feed.failed(problem, now, &self.timings),
        }
        self.rebuild(now, wall);
        self.show(wall);
    }

    fn rotate(&self) -> Duration {
        self.config
            .as_ref()
            .map_or(DEFAULT_ROTATE, |c| c.rotate)
            .max(MIN_ROTATE)
    }

    /// Recomputes the headlines from every feed's items, staying on the
    /// one showing if it is still there. If it went, the one that took its
    /// place shows for a full turn.
    fn rebuild(&mut self, now: Instant, wall: SystemTime) {
        let showing = self
            .at
            .and_then(|i| self.kept.get(i))
            .map(|i| i.link.clone());
        self.kept = match &self.config {
            Some(c) => parse::select(self.feeds.iter().flat_map(|f| &f.items), wall, c.max_age),
            None => Vec::new(),
        };
        if self.kept.is_empty() {
            self.at = None;
            self.next_turn = None;
            return;
        }
        let still = showing.and_then(|link| self.kept.iter().position(|i| i.link == link));
        self.at = Some(match still {
            Some(i) => i,
            None => {
                self.next_turn = Some(now + self.rotate());
                self.at.map_or(0, |i| i.min(self.kept.len() - 1))
            }
        });
    }

    /// Moves to the next headline (after dropping any that have grown too
    /// old), wrapping to the newest after the oldest.
    fn advance(&mut self, now: Instant, wall: SystemTime) {
        let was_showing = self.at.is_some();
        self.rebuild(now, wall);
        if let Some(at) = self.at
            && was_showing
        {
            self.at = Some((at + 1) % self.kept.len());
            self.next_turn = Some(now + self.rotate());
        }
        self.show(wall);
    }

    /// Every feed that can be fetched has failed its last attempt.
    fn failing(&self) -> bool {
        let mut usable = self.feeds.iter().filter(|f| f.usable).peekable();
        usable.peek().is_some() && usable.all(|f| f.failures > 0)
    }

    fn show(&mut self, wall: SystemTime) {
        let item = self.at.and_then(|i| self.kept.get(i)).cloned();
        // Nothing new enough, and no feed to bring anything newer: the
        // last headline stays, marked lost, rather than a blank that would
        // read as "no news".
        let lost = item.is_none() && self.shown && self.failing();
        let item = if lost { self.last.clone() } else { item };
        if item.is_none() && !self.shown {
            return;
        }
        self.shown = item.is_some();
        if !lost {
            self.last.clone_from(&item);
        }
        let readings = readings(item.as_ref(), self.kept.len(), lost, wall);
        self.alive &= (self.publish)(readings);
    }
}

fn readings(item: Option<&Item>, count: usize, lost: bool, wall: SystemTime) -> Vec<Reading> {
    let text = |f: fn(&Item) -> &str| Value::Text(item.map(f).unwrap_or_default().to_string());
    let age = item.map_or(0.0, |i| {
        wall.duration_since(i.published)
            .unwrap_or_default()
            .as_secs() as f64
    });
    vec![
        ("news.title", text(|i| &i.title)),
        ("news.source", text(|i| &i.source)),
        ("news.link", text(|i| &i.link)),
        ("news.age", Value::Number(age, Unit::None)),
        ("news.count", Value::Number(count as f64, Unit::None)),
        ("news.lost", Value::Number(f64::from(lost), Unit::None)),
    ]
}
