//! The track's artwork, from the address the player gives in its metadata
//! (`mpris:artUrl`).
//!
//! - `file://`: read if it is a regular file (not a link, a pipe or a
//!   device) under `MAX_FILE`.
//! - `data:`: base64 image data of at most `MAX_DATA`.
//! - `https://`, `http://`: fetched, with `album_art = "all"` only, by a
//!   plain GET with the shared user agent, `TIMEOUT` and `MAX_FETCH`. It
//!   is the address the player itself fetched the picture from.
//! - Anything else is ignored.
//!
//! The image is untrusted: it is decoded by pure-Rust decoders (PNG, JPEG,
//! WebP only) with limits on its size and on the memory decoding takes,
//! on a thread of its own, never the render thread. It is prepared once
//! (`nexus_core::art::Art::prepare`) and kept in a small cache by address,
//! sized in bytes, so going back to a track costs nothing. Any failure is
//! the placeholder, and a line in the log once per track, with no query
//! string and no data.

use std::collections::VecDeque;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Duration;

use base64::Engine as _;
use nexus_core::art::Art;
use nexus_core::config::AlbumArt;

/// The largest artwork file read.
pub const MAX_FILE: u64 = 10 * 1024 * 1024;
/// The largest image inside a `data:` address, decoded.
pub const MAX_DATA: usize = 2 * 1024 * 1024;
/// The largest image fetched.
pub const MAX_FETCH: u64 = 5 * 1024 * 1024;
/// How long a fetch may take, all told.
pub const TIMEOUT: Duration = Duration::from_secs(5);
/// Redirects followed before giving up.
const MAX_REDIRECTS: u32 = 5;
/// The largest image decoded, on either side. Covers are 3000 px at most.
pub const MAX_SIDE: u32 = 4096;
/// The most memory decoding may take: a 4096×4096 RGBA image and change.
pub const MAX_DECODE: u64 = 80 * 1024 * 1024;
/// Prepared artwork kept, in bytes: about four tracks.
pub const CACHE_BYTES: usize = 4 * 28 * 1024;

/// Where an address points, if it is one artwork is read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    File,
    Data,
    Web,
}

impl Kind {
    pub fn of(url: &str) -> Option<Kind> {
        let scheme = url.split_once(':')?.0.to_ascii_lowercase();
        match scheme.as_str() {
            "file" => Some(Kind::File),
            "data" => Some(Kind::Data),
            "https" | "http" => Some(Kind::Web),
            _ => None,
        }
    }

    /// Whether `policy` lets artwork come from here.
    pub fn allowed(self, policy: AlbumArt) -> bool {
        match policy {
            AlbumArt::Off => false,
            AlbumArt::Local => self != Kind::Web,
            AlbumArt::All => true,
        }
    }
}

/// The address to read the track's artwork from, if `policy` allows it
/// and it is a kind artwork is read from.
pub fn wanted(url: Option<&str>, policy: AlbumArt) -> Option<&str> {
    let url = url?.trim();
    Kind::of(url).filter(|k| k.allowed(policy)).map(|_| url)
}

/// What the engine knows an address's artwork by (`media.art`): a short
/// hash, so the signal shows nothing of the address.
pub fn key(url: &str) -> String {
    let h = url.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("art-{h:016x}")
}

/// An address as the log shows it: no query or fragment, no user or key,
/// and for `data:` only its type.
pub fn shown(url: &str) -> String {
    match Kind::of(url) {
        Some(Kind::Data) => {
            let head = url.split([',', ';']).next().unwrap_or("data:");
            format!("{}…", head.chars().take(40).collect::<String>())
        }
        _ => crate::news::fetch::redacted(url)
            .chars()
            .take(200)
            .collect(),
    }
}

/// Reads, fetches and decodes `url`'s image and prepares it. Errors are
/// sentences for the log.
pub fn load(url: &str, policy: AlbumArt, agent: &ureq::Agent) -> Result<Art, String> {
    let kind = Kind::of(url)
        .filter(|k| k.allowed(policy))
        .ok_or("not an address artwork is read from")?;
    let bytes = match kind {
        Kind::File => read_file(url)?,
        Kind::Data => read_data(url)?,
        Kind::Web => fetch(agent, url)?,
    };
    let (width, height, rgba) = decode(&bytes)?;
    Art::prepare(key(url), width, height, &rgba).ok_or_else(|| "the image is empty".into())
}

/// The ureq agent artwork is fetched with.
pub fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .timeout_global(Some(TIMEOUT))
        .user_agent(crate::USER_AGENT)
        .max_redirects(MAX_REDIRECTS)
        .http_status_as_error(false)
        .build()
        .into()
}

fn read_file(url: &str) -> Result<Vec<u8>, String> {
    let rest = &url["file:".len()..];
    let path = rest
        .strip_prefix("//")
        .ok_or("a file address starts with file://")?;
    // file:///path, or file://localhost/path; no other host.
    let path = match path.split_once('/') {
        Some(("" | "localhost", p)) => format!("/{p}"),
        _ => return Err("the file is on another computer".into()),
    };
    let path = PathBuf::from(percent_decode(&path)?);
    // Only a regular file is read: a pipe or a device could block the
    // read forever or never end. The file is opened without blocking and
    // then checked, so what is checked is what is read, even if the path
    // changes in between. Links are followed: players keep their covers
    // behind them, and the daemon can read only what its user can.
    let file = open_nonblocking(&path).map_err(|e| format!("cannot read it: {e}"))?;
    let meta = file
        .metadata()
        .map_err(|e| format!("cannot read it: {e}"))?;
    if !meta.file_type().is_file() {
        return Err("it is not a regular file".into());
    }
    if meta.len() > MAX_FILE {
        return Err(format!("the file is larger than {} MB", MAX_FILE >> 20));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| format!("cannot read it: {e}"))?;
    if bytes.len() as u64 > MAX_FILE {
        return Err(format!("the file is larger than {} MB", MAX_FILE >> 20));
    }
    Ok(bytes)
}

/// Opens `path` for reading. On Unix with `O_NONBLOCK`, so opening a pipe
/// with no writer returns at once instead of waiting for one; the flag
/// does nothing to reads from a regular file.
fn open_nonblocking(path: &std::path::Path) -> std::io::Result<File> {
    let mut options = std::fs::OpenOptions::new();
    options.read(true);
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::fs::OpenOptionsExt;
        const O_NONBLOCK: i32 = 0o4000;
        options.custom_flags(O_NONBLOCK);
    }
    options.open(path)
}

/// "%20" and the like, to bytes; a path need not be UTF-8, but this one
/// must be to be named.
fn percent_decode(s: &str) -> Result<String, String> {
    let mut out = Vec::with_capacity(s.len());
    let mut bytes = s.bytes();
    while let Some(b) = bytes.next() {
        if b == b'%' {
            let hex = [bytes.next(), bytes.next()];
            let v = match hex {
                [Some(h), Some(l)] => std::str::from_utf8(&[h, l])
                    .ok()
                    .and_then(|t| u8::from_str_radix(t, 16).ok()),
                _ => None,
            };
            out.push(v.ok_or("the file address has a broken %-escape")?);
        } else {
            out.push(b);
        }
    }
    String::from_utf8(out).map_err(|_| "the file name is not UTF-8".into())
}

fn read_data(url: &str) -> Result<Vec<u8>, String> {
    let (head, data) = url.split_once(',').ok_or("the data address has no data")?;
    if !head.to_ascii_lowercase().ends_with(";base64") {
        return Err("the data address is not base64".into());
    }
    // Checked before decoding: four characters make three bytes.
    if data.len() > MAX_DATA.div_ceil(3) * 4 + 4 {
        return Err(format!("the image is larger than {} MB", MAX_DATA >> 20));
    }
    let cleaned: String = data.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(cleaned.as_bytes())
        .map_err(|_| "the data address is not valid base64")?;
    if bytes.len() > MAX_DATA {
        return Err(format!("the image is larger than {} MB", MAX_DATA >> 20));
    }
    Ok(bytes)
}

fn fetch(agent: &ureq::Agent, url: &str) -> Result<Vec<u8>, String> {
    let mut response = agent.get(url).call().map_err(|e| describe(&e))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(format!("the server answered {status}"));
    }
    response
        .body_mut()
        .with_config()
        .limit(MAX_FETCH)
        .read_to_vec()
        .map_err(|e| describe(&e))
}

fn describe(e: &ureq::Error) -> String {
    match e {
        ureq::Error::BodyExceedsLimit(_) => {
            format!("the image is larger than {} MB", MAX_FETCH >> 20)
        }
        ureq::Error::TooManyRedirects => {
            format!("the address redirects more than {MAX_REDIRECTS} times")
        }
        ureq::Error::Timeout(_) => "the image host did not answer in time".into(),
        ureq::Error::HostNotFound => "no such host (or this computer is offline)".into(),
        other => other.to_string(),
    }
}

/// PNG, JPEG or WebP to straight-alpha RGBA, within `MAX_SIDE` and
/// `MAX_DECODE`.
pub fn decode(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    use image::{ImageFormat, ImageReader, Limits};
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|e| e.to_string())?;
    match reader.format() {
        Some(ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP) => {}
        _ => return Err("the image is not a PNG, JPEG or WebP".into()),
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODE);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|e| format!("the image cannot be read: {e}"))?;
    let rgba = image.into_rgba8();
    Ok((rgba.width(), rgba.height(), rgba.into_raw()))
}

/// Prepared artwork by address, least recently used first.
#[derive(Default)]
pub struct Cache {
    entries: VecDeque<(String, Arc<Art>)>,
}

impl Cache {
    pub fn get(&mut self, url: &str) -> Option<Arc<Art>> {
        let i = self.entries.iter().position(|(u, _)| u == url)?;
        let entry = self.entries.remove(i)?;
        let art = Arc::clone(&entry.1);
        self.entries.push_back(entry);
        Some(art)
    }

    /// Keeps `art`, dropping the least recently used until the cache is
    /// back under `CACHE_BYTES` (the newest always stays).
    pub fn put(&mut self, url: &str, art: Arc<Art>) {
        self.entries.retain(|(u, _)| u != url);
        self.entries.push_back((url.to_string(), art));
        while self.bytes() > CACHE_BYTES && self.entries.len() > 1 {
            self.entries.pop_front();
        }
    }

    pub fn bytes(&self) -> usize {
        self.entries.iter().map(|(u, a)| u.len() + a.bytes()).sum()
    }

    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.len()
    }
}

/// The media source's side of artwork: which address the track's artwork
/// comes from under the policy, what the engine has been told
/// (`media.art`), and what is cached or on its way.
pub struct Artwork {
    policy: AlbumArt,
    /// The track's artwork address, when the policy allows it: shown, or
    /// being loaded.
    wanted: Option<String>,
    /// The `media.art` last published; `None` before the first.
    published: Option<String>,
    cache: Cache,
    loader: Sender<Job>,
}

/// What to hand on after a change: artwork for the engine, then the new
/// value of `media.art`. Either may be absent.
#[derive(Debug, Default, PartialEq)]
pub struct Update {
    pub art: Option<Arc<Art>>,
    pub key: Option<String>,
}

impl Artwork {
    pub fn new(policy: AlbumArt, loader: Sender<Job>) -> Self {
        Artwork {
            policy,
            wanted: None,
            published: None,
            cache: Cache::default(),
            loader,
        }
    }

    pub fn set_policy(&mut self, policy: AlbumArt) {
        self.policy = policy;
    }

    /// The track (or the policy) changed; `url` is the track's
    /// `mpris:artUrl`. Cached artwork is shown at once; otherwise the
    /// placeholder until the loader answers.
    pub fn track(&mut self, url: Option<&str>) -> Update {
        let wanted = wanted(url, self.policy).map(str::to_string);
        if wanted == self.wanted && self.published.is_some() {
            return Update::default();
        }
        self.wanted = wanted;
        let Some(url) = self.wanted.clone() else {
            return self.publish(None);
        };
        match self.cache.get(&url) {
            Some(art) => self.publish(Some(art)),
            None => {
                let _ = self.loader.send(Job {
                    url,
                    policy: self.policy,
                });
                self.publish(None)
            }
        }
    }

    /// The loader's answer for `url`: kept, and shown if it is still the
    /// track's.
    pub fn arrived(&mut self, url: &str, art: Option<Art>) -> Update {
        let Some(art) = art else {
            return Update::default();
        };
        let art = Arc::new(art);
        self.cache.put(url, Arc::clone(&art));
        if self.wanted.as_deref() == Some(url) {
            self.publish(Some(art))
        } else {
            Update::default()
        }
    }

    fn publish(&mut self, art: Option<Arc<Art>>) -> Update {
        let key = art
            .as_ref()
            .map_or_else(String::new, |a| a.key().to_string());
        let changed = self.published.as_ref() != Some(&key);
        self.published = Some(key.clone());
        Update {
            art,
            key: changed.then_some(key),
        }
    }
}

/// Loads artwork on its own thread, newest request first: older ones
/// still waiting are skipped, as the track has moved on. Each answer,
/// `(url, art)`, goes to `done`; the thread ends when `done` returns
/// false or every sender is gone.
pub fn spawn_loader(done: impl Fn(String, Option<Art>) -> bool + Send + 'static) -> Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    let _ = thread::Builder::new()
        .name("media-art".into())
        .spawn(move || run(&rx, &done));
    tx
}

/// An address to load, under the policy it was asked for with.
pub struct Job {
    pub url: String,
    pub policy: AlbumArt,
}

fn run(jobs: &Receiver<Job>, done: &impl Fn(String, Option<Art>) -> bool) {
    let agent = agent();
    while let Ok(first) = jobs.recv() {
        let job = jobs.try_iter().last().unwrap_or(first);
        let art = match load(&job.url, job.policy, &agent) {
            Ok(art) => Some(art),
            Err(why) => {
                eprintln!(
                    "nexusd: media: no artwork for this track ({why}): {}",
                    shown(&job.url)
                );
                None
            }
        };
        if !done(job.url, art) {
            return;
        }
    }
}

#[cfg(test)]
mod tests;
