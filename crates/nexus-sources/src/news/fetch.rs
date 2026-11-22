//! Asking one feed for its latest version.
//!
//! The request is a plain GET with the shared user agent and, once a feed
//! has answered, its ETag and Last-Modified back so an unchanged feed
//! costs a 304 and no body. No cookies, no query added, nothing else sent.

use std::time::Duration;

use super::parse::{self, Parsed};

/// The largest feed read. Real feeds are well under 1 MB; anything bigger
/// is a mistake or an attack, and reading it would hold that much memory.
pub const MAX_BODY: u64 = 2 * 1024 * 1024;
/// Redirects followed before giving up. Feeds move, but rarely more than
/// once or twice (http → https, old host → new).
pub const MAX_REDIRECTS: u32 = 5;
/// Validators longer than this are not stored or sent back. Real ones are
/// a few dozen bytes; a huge one would be resent on every request.
const MAX_VALIDATOR: usize = 512;

/// What a feed said.
#[derive(Debug)]
pub enum Fetched {
    /// A new version, with the validators to send next time.
    Fresh {
        feed: Parsed,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    /// 304: what was read last time still holds.
    NotModified,
}

/// The validators from the last good answer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Validators {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}

pub fn agent(timeout: Duration) -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(timeout))
        .user_agent(crate::USER_AGENT)
        .max_redirects(MAX_REDIRECTS)
        // A 304 or a 500 is an answer to handle here, not an error to
        // describe; see `fetch`.
        .http_status_as_error(false)
        .build()
        .into()
}

/// Checks a feed address before anything is sent to it. Only web addresses
/// are fetched: a `file://` feed would read the user's own files, and
/// other schemes are not something ureq or a user means.
pub fn check_url(url: &str) -> Result<(), String> {
    if parse::is_web_link(url) {
        Ok(())
    } else {
        Err(format!(
            "\"{}\" is not a web address; a feed address starts with https:// or http://",
            url.chars().take(100).collect::<String>()
        ))
    }
}

/// Why a fetch failed, kept apart so the window can say it in its own
/// words; `Display` gives the sentence the log uses.
#[derive(Debug)]
pub enum Failure {
    /// Not an address that is fetched (see [`check_url`]).
    Address(String),
    /// No answer: no such host, refused, offline, timed out, or
    /// redirected too often.
    Net(ureq::Error),
    /// The server answered with this status, not 2xx or 304.
    Status(u16),
    /// The body was larger than [`MAX_BODY`].
    TooBig,
    /// The body is not a feed that can be read.
    Parse(String),
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Failure::Address(why) | Failure::Parse(why) => f.write_str(why),
            Failure::Net(e) => f.write_str(&describe(e)),
            Failure::Status(status) => write!(f, "the server answered {status}"),
            Failure::TooBig => write!(
                f,
                "the feed is larger than {} MB",
                MAX_BODY / 1024 / 1024
            ),
        }
    }
}

/// Fetches and reads one feed. Errors are sentences for the log.
pub fn fetch(agent: &ureq::Agent, url: &str, sent: &Validators) -> Result<Fetched, String> {
    fetch_or_fail(agent, url, sent).map_err(|e| e.to_string())
}

/// [`fetch`], with why it failed as a [`Failure`].
pub fn fetch_or_fail(
    agent: &ureq::Agent,
    url: &str,
    sent: &Validators,
) -> Result<Fetched, Failure> {
    check_url(url).map_err(Failure::Address)?;
    let mut request = agent.get(url);
    if let Some(etag) = &sent.etag {
        request = request.header("If-None-Match", etag);
    }
    if let Some(date) = &sent.last_modified {
        request = request.header("If-Modified-Since", date);
    }
    let mut response = request.call().map_err(Failure::Net)?;
    let status = response.status().as_u16();
    match status {
        304 => return Ok(Fetched::NotModified),
        200..=299 => {}
        _ => return Err(Failure::Status(status)),
    }
    let header = |name: &str| {
        response
            .headers()
            .get(name)
            .and_then(|v| v.to_str().ok())
            .filter(|v| !v.is_empty() && v.len() <= MAX_VALIDATOR)
            .map(str::to_string)
    };
    let (etag, last_modified) = (header("etag"), header("last-modified"));
    let body = response
        .body_mut()
        .with_config()
        .limit(MAX_BODY)
        .read_to_vec()
        .map_err(|e| match e {
            ureq::Error::BodyExceedsLimit(_) => Failure::TooBig,
            other => Failure::Net(other),
        })?;
    let feed = parse::parse(&body, url).map_err(Failure::Parse)?;
    Ok(Fetched::Fresh {
        feed,
        etag,
        last_modified,
    })
}

fn describe(e: &ureq::Error) -> String {
    match e {
        ureq::Error::BodyExceedsLimit(_) => {
            format!("the feed is larger than {} MB", MAX_BODY / 1024 / 1024)
        }
        ureq::Error::TooManyRedirects => {
            format!("the feed redirects more than {MAX_REDIRECTS} times")
        }
        ureq::Error::Timeout(_) => "the feed did not answer in time".into(),
        ureq::Error::HostNotFound => "no such host (or this computer is offline)".into(),
        other => other.to_string(),
    }
}

/// "https://user:key@example.org/feed?token=…" → "https://example.org/feed",
/// for the log: private feeds carry keys in the address, and a log is
/// something people paste into bug reports.
pub fn redacted(url: &str) -> String {
    let (scheme, rest) = url.split_once("://").unwrap_or(("", url));
    let rest = rest.split(['?', '#']).next().unwrap_or_default();
    let rest = match rest.split_once('/') {
        Some((authority, path)) => {
            let host = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
            format!("{host}/{path}")
        }
        None => rest.rsplit_once('@').map_or(rest, |(_, h)| h).to_string(),
    };
    if scheme.is_empty() {
        rest
    } else {
        format!("{scheme}://{rest}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_web_addresses_are_fetched() {
        assert!(check_url("https://example.org/feed").is_ok());
        assert!(check_url("http://example.org/feed").is_ok());
        for bad in [
            "ftp://example.org/feed",
            "file:///etc/passwd",
            "example.org/feed",
            "",
        ] {
            let e = check_url(bad).unwrap_err();
            assert!(e.contains("https://"), "{e}");
        }
        // Rejected before any request: the agent here could not connect.
        let agent = agent(Duration::from_millis(1));
        let e = fetch(&agent, "gopher://example.org", &Validators::default()).unwrap_err();
        assert!(e.contains("not a web address"), "{e}");
    }

    #[test]
    fn log_addresses_lose_their_secrets() {
        assert_eq!(
            redacted("https://me:key@example.org/feed?token=abc#x"),
            "https://example.org/feed"
        );
        assert_eq!(redacted("http://example.org"), "http://example.org");
        assert_eq!(redacted("https://u@example.org?k=1"), "https://example.org");
    }
}
