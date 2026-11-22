//! Checking one feed address once, for the settings window: when the user
//! adds a feed, it is fetched a single time so the window can show its
//! title and newest headline, or say plainly why it cannot be read.
//!
//! The request is the one the source makes (same user agent, limits and
//! redirects), sent only to the address the user typed. It blocks for up to
//! the source's request timeout, so call it off the UI thread.

use std::time::Duration;

use super::Timings;
use super::fetch::{self, Failure, Fetched, Validators};

/// What a feed that works said.
#[derive(Clone, Debug, PartialEq)]
pub struct Checked {
    /// The feed's title, or its host name when it has none.
    pub title: String,
    /// Its newest headline, if it has any.
    pub newest: Option<String>,
}

/// Why a feed address cannot be read, by kind, so the window can say it
/// in plain words.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Problem {
    /// Not an `https://` or `http://` address.
    NotWebAddress,
    /// No answer: no such host, refused, offline or too slow.
    NoAnswer,
    /// The address redirects more often than is followed.
    Redirects,
    /// The site answered 404 or 410: nothing at that address.
    NotFound,
    /// The site answered with another error status.
    Status(u16),
    /// An answer, but not a feed (a web page, most often).
    NotAFeed,
    /// Larger than a feed is allowed to be.
    TooBig,
}

/// Fetches `url` once and reads it as a feed.
pub fn check(url: &str) -> Result<Checked, Problem> {
    check_within(url.trim(), Timings::REAL.timeout)
}

pub(crate) fn check_within(url: &str, timeout: Duration) -> Result<Checked, Problem> {
    let agent = fetch::agent(timeout);
    match fetch::fetch_or_fail(&agent, url, &Validators::default()) {
        Ok(Fetched::Fresh { feed, .. }) => {
            // Newest by date; undated entries count as oldest, and among
            // equals the feed's own order holds.
            let newest = feed
                .entries
                .iter()
                .enumerate()
                .max_by_key(|(i, e)| (e.published, std::cmp::Reverse(*i)))
                .map(|(_, e)| e.title.clone());
            Ok(Checked {
                title: feed.source,
                newest,
            })
        }
        // Nothing was sent to make a 304 likely; a server that answers one
        // anyway has not given a feed.
        Ok(Fetched::NotModified) => Err(Problem::NotAFeed),
        Err(Failure::Address(_)) => Err(Problem::NotWebAddress),
        Err(Failure::Net(ureq::Error::TooManyRedirects)) => Err(Problem::Redirects),
        Err(Failure::Net(_)) => Err(Problem::NoAnswer),
        Err(Failure::Status(404 | 410)) => Err(Problem::NotFound),
        Err(Failure::Status(status)) => Err(Problem::Status(status)),
        Err(Failure::TooBig) => Err(Problem::TooBig),
        Err(Failure::Parse(_)) => Err(Problem::NotAFeed),
    }
}
