//! The control protocol between `nexusd` and its clients (`nexusctl`, the
//! settings window).
//!
//! Transport: a local socket. A Unix socket in the user's runtime directory
//! on Linux and macOS, a named pipe on Windows. Only the user's own
//! processes can reach it; there is no network listener and no token.
//!
//! Protocol: the client writes one request per line; the daemon answers
//! each with one line of JSON, either `{"ok":true,"data":…}` or
//! `{"ok":false,"error":"…"}`.
//!
//! ```text
//! status                        daemon, device and config state
//! signals                       every signal and its latest value
//! page next|prev|<n>|<name>     show a page (n counts from 1)
//! reload                        re-read the store now
//! publish <signal> <value>      set a signal: a number with an optional
//!                               unit (celsius, percent, bytes-per-second),
//!                               or any text
//! frames                        stream every frame (see below)
//! edit {"edit": {...}, "base": n}  one change (nexus_store::Edit); refused
//!                               if `base` is given and the revision moved
//! undo / redo                   take back or redo the last change
//! export [layout]               a layout, the settings and sources as TOML
//! layout                        the layout on the strip as data, with the
//!                               IDs that edits name things by
//! import {"name", "text", "activate"}  add a layout from TOML
//! art                           the track's artwork as the strip draws it
//!                               (`ArtReply`), or null; ask again when the
//!                               `media.art` signal changes
//! ```
//!
//! After `frames` and its reply, the connection carries only frames: each
//! is a 4-byte little-endian length followed by that many bytes of 640×48
//! RGBA. The current frame is sent first.

pub mod links;

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;

use base64::Engine as _;
use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{Listener, ListenerOptions, Name, Stream};
use nexus_core::art::Art;
use nexus_core::signal::{Unit, Value};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// One request line, parsed.
#[derive(Clone, Debug, PartialEq)]
pub enum Request {
    Status,
    Signals,
    Page(PageTarget),
    Reload,
    Publish(String, Value),
    /// A simulated touch at panel x (0–639): a tap, or a long press if
    /// `hold`. It goes through the same gesture path as a real finger.
    Touch {
        x: f32,
        hold: bool,
    },
    /// A simulated drag from one panel x to another over 0.3 s.
    Drag {
        from: f32,
        to: f32,
    },
    /// A simulated hold-then-slide: a still finger at `from` past the long
    /// press, then a slide to `to`, resting there before it lifts. On the
    /// page it switches layout as the layout picker does.
    HoldDrag {
        from: f32,
        to: f32,
    },
    Frames,
    Edit(Box<EditRequest>),
    Undo,
    Redo,
    /// A layout by name, or the one on the strip.
    Export(Option<String>),
    /// The layout on the strip, as data.
    Layout,
    Import(Box<ImportRequest>),
    /// Show a takeover for this many seconds (at most
    /// `TAKEOVER_LEASE_MAX`), as the settings window does while editing
    /// it; asked again before it ends, the time starts over.
    ShowTakeover {
        name: String,
        seconds: f64,
    },
    /// Put the page back if a takeover covers it.
    EndTakeover,
    /// The artwork the strip draws for the track.
    Art,
}

/// The longest a takeover is shown on request. A window that asked and
/// then went away leaves it up no longer than this.
pub const TAKEOVER_LEASE_MAX: f64 = 60.0;

/// The body of `edit`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditRequest {
    pub edit: nexus_store::Edit,
    /// The revision the edit was made against. When given and no longer
    /// current, the edit is refused rather than applied over a change the
    /// sender has not seen.
    #[serde(default)]
    pub base: Option<i64>,
}

/// The body of `import`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ImportRequest {
    pub name: String,
    /// The layout as TOML, as `export` writes it.
    pub text: String,
    /// Put it on the strip.
    #[serde(default)]
    pub activate: bool,
}

/// The answer to `layout`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layout {
    pub id: i64,
    pub name: String,
    /// Send it as an edit's `base` so the edit is refused if something
    /// else changed the layout first.
    pub revision: i64,
    pub config: nexus_core::config::Config,
    /// In the same order as `config`.
    pub ids: nexus_store::Ids,
    pub notes: nexus_store::Notes,
    /// What undo and redo would take back or make again, if anything.
    pub undo: Option<String>,
    pub redo: Option<String>,
    /// Every layout, this one among them, by name.
    #[serde(default)]
    pub layouts: Vec<LayoutEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LayoutEntry {
    pub id: i64,
    pub name: String,
}

/// The answer to `export`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Export {
    pub id: i64,
    pub layout: String,
    pub text: String,
    pub revision: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum PageTarget {
    Next,
    Prev,
    /// Counting from 0.
    Index(usize),
    Name(String),
}

impl Request {
    pub fn parse(line: &str) -> Result<Request, String> {
        let line = line.trim();
        let (word, rest) = line.split_once(' ').unwrap_or((line, ""));
        let rest = rest.trim();
        match word {
            "status" => Ok(Request::Status),
            "signals" => Ok(Request::Signals),
            "reload" => Ok(Request::Reload),
            "frames" => Ok(Request::Frames),
            "undo" => Ok(Request::Undo),
            "redo" => Ok(Request::Redo),
            "layout" => Ok(Request::Layout),
            "end-takeover" => Ok(Request::EndTakeover),
            "art" => Ok(Request::Art),
            "show-takeover" => {
                let (seconds, name) = rest.split_once(' ').unwrap_or((rest, ""));
                let seconds: f64 = seconds
                    .parse()
                    .ok()
                    .filter(|s: &f64| *s > 0.0 && *s <= TAKEOVER_LEASE_MAX)
                    .ok_or_else(|| {
                        format!(
                            "show-takeover needs seconds (up to {TAKEOVER_LEASE_MAX}) and a name, as in show-takeover 15 Volume"
                        )
                    })?;
                let name = name.trim();
                if name.is_empty() {
                    return Err("show-takeover needs a takeover name after the seconds".into());
                }
                Ok(Request::ShowTakeover {
                    name: name.into(),
                    seconds,
                })
            }
            "export" => Ok(Request::Export(
                (!rest.is_empty()).then(|| rest.to_string()),
            )),
            "edit" => serde_json::from_str(rest)
                .map(|e| Request::Edit(Box::new(e)))
                .map_err(|e| {
                    format!("edit needs JSON, as in edit {{\"edit\": {{\"op\": …}}}}: {e}")
                }),
            "import" => serde_json::from_str(rest)
                .map(|i| Request::Import(Box::new(i)))
                .map_err(|e| format!("import needs JSON with name and text: {e}")),
            "page" => Ok(Request::Page(match rest {
                "" => return Err("page needs next, prev, a number or a name".into()),
                "next" => PageTarget::Next,
                "prev" => PageTarget::Prev,
                _ => match rest.parse::<usize>() {
                    Ok(0) => return Err("pages are numbered from 1".into()),
                    Ok(n) => PageTarget::Index(n - 1),
                    Err(_) => PageTarget::Name(rest.to_string()),
                },
            })),
            "publish" => {
                let (name, value) = rest
                    .split_once(' ')
                    .ok_or("publish needs a signal name and a value")?;
                let value = value.trim();
                let (number, unit) = value.split_once(' ').unwrap_or((value, ""));
                // Not a unit name: the whole value is text.
                let unit = match unit.trim() {
                    "" => Some(Unit::None),
                    name => Unit::from_name(name),
                };
                let value = match (number.parse::<f64>(), unit) {
                    (Ok(n), Some(unit)) if n.is_finite() => Value::Number(n, unit),
                    _ => Value::Text(value.to_string()),
                };
                Ok(Request::Publish(name.to_string(), value))
            }
            "tap" | "hold" => {
                let x: f32 = rest
                    .parse()
                    .ok()
                    .filter(|x| (0.0..640.0).contains(x))
                    .ok_or(format!("{word} needs an x position from 0 to 639"))?;
                Ok(Request::Touch {
                    x,
                    hold: word == "hold",
                })
            }
            "drag" | "hold-drag" => {
                let xs: Vec<f32> = rest
                    .split_whitespace()
                    .filter_map(|x| x.parse().ok())
                    .filter(|x| (0.0..640.0).contains(x))
                    .collect();
                match xs[..] {
                    [from, to] if word == "drag" => Ok(Request::Drag { from, to }),
                    [from, to] => Ok(Request::HoldDrag { from, to }),
                    _ => Err(format!("{word} needs two x positions from 0 to 639")),
                }
            }
            "" => Err("empty request".into()),
            other => Err(format!(
                "unknown request \"{other}\"; expected status, signals, page, reload, publish, tap, hold, drag, hold-drag, frames, edit, undo, redo, export, import or art"
            )),
        }
    }
}

/// The answer to `status`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Status {
    pub version: String,
    pub pid: u32,
    /// `None` while connected; otherwise why there is no device.
    pub device_error: Option<String>,
    pub page: String,
    pub pages: Vec<String>,
    /// The layout on the strip.
    pub layout: String,
    pub store_path: String,
    /// Why the store could not be opened; the default layout is shown and
    /// edits are refused until it is fixed.
    pub store_error: Option<String>,
    /// The takeover covering the page, if one is.
    #[serde(default)]
    pub takeover: Option<String>,
    /// The label of the reading the critical alarm names, while it is up.
    #[serde(default)]
    pub alarm: Option<String>,
    /// The store's revision: when it moves, the layout or settings changed.
    #[serde(default)]
    pub revision: i64,
}

/// One entry of the answer to `signals`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalEntry {
    pub name: String,
    pub value: String,
    /// For a number: the number and its unit's name (`Unit::name`).
    #[serde(default)]
    pub number: Option<f64>,
    #[serde(default)]
    pub unit: Option<String>,
}

/// The answer to `art`: the track's artwork, prepared as the strip draws
/// it, so a window's previews (drawn by its own `Engine`) show the same
/// pixels. `null` when the strip shows the placeholder.
///
/// To use it: when the `media.art` signal's value changes, request `art`;
/// for a reply that is not null, give the window's engine
/// `Input::Art(Arc::new(reply.to_art()?))` alongside the readings. Its
/// key is `media.art`'s value, so the engine draws it exactly while the
/// readings name it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtReply {
    /// The `media.art` value that names it.
    pub key: String,
    pub squares: Vec<ArtSquare>,
}

/// One prepared square of artwork.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArtSquare {
    /// Pixels on a side; at most `nexus_core::art::MAX_SQUARE`.
    pub size: u32,
    /// `size`×`size` straight-alpha sRGB RGBA, row-major, base64.
    pub rgba: String,
}

impl ArtReply {
    pub fn new(art: &Art) -> ArtReply {
        let b64 = base64::engine::general_purpose::STANDARD;
        ArtReply {
            key: art.key().to_string(),
            squares: art
                .squares()
                .map(|(size, rgba)| ArtSquare {
                    size,
                    rgba: b64.encode(rgba),
                })
                .collect(),
        }
    }

    /// The artwork, ready for `Input::Art`; `None` if a square is not
    /// what it says.
    pub fn to_art(&self) -> Option<Art> {
        let b64 = base64::engine::general_purpose::STANDARD;
        let squares = self
            .squares
            .iter()
            .map(|s| Some((s.size, b64.decode(&s.rgba).ok()?)))
            .collect::<Option<Vec<_>>>()?;
        Art::from_squares(self.key.clone(), squares)
    }
}

/// Formats a reply line (without the newline).
pub fn reply(result: Result<serde_json::Value, String>) -> String {
    match result {
        Ok(data) => json!({ "ok": true, "data": data }).to_string(),
        Err(error) => json!({ "ok": false, "error": error }).to_string(),
    }
}

/// The config file of earlier versions: `nexus-open/nexus.toml` in the
/// user's config directory (`~/.config` on Linux). Imported into the store
/// on first start. External sources run in this folder.
pub fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("nexus-open").join("nexus.toml"))
}

/// The store. On Linux `~/.local/state/nexusd/nexusd.db`, a directory
/// systemd's `StateDirectory=nexusd` creates and opens to the sandboxed
/// service. Not `nexus-open`: when `~/.config/<name>` exists, systemd takes
/// the state directory to be an old-style one and links it there. Elsewhere
/// `nexus-open/nexusd.db` in the data directory.
pub fn default_store_path() -> Option<PathBuf> {
    match dirs::state_dir() {
        Some(state) => Some(state.join("nexusd").join("nexusd.db")),
        None => dirs::data_dir().map(|d| d.join("nexus-open").join("nexusd.db")),
    }
}

/// Where the socket lives: the per-user runtime directory (only the user
/// can enter it), falling back to the per-user temporary directory.
pub fn socket_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("nexus-open.sock")
}

fn name(path: &std::path::Path) -> io::Result<Name<'_>> {
    named(path, "nexus-open")
}

/// A socket's name: the file on Unix, a namespaced name on Windows.
fn named<'a>(path: &'a std::path::Path, windows: &'static str) -> io::Result<Name<'a>> {
    if cfg!(windows) {
        windows.to_ns_name::<interprocess::local_socket::GenericNamespaced>()
    } else {
        path.to_fs_name::<interprocess::local_socket::GenericFilePath>()
    }
}

/// The settings window's own socket, beside the daemon's: a second window
/// asks the first one to show itself instead of opening.
fn ui_socket_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("nexus-open-ui.sock")
}

/// Asks a window that is already open to show itself. True if one did.
pub fn ui_raise() -> bool {
    let path = ui_socket_path();
    let Ok(name) = named(&path, "nexus-open-ui") else {
        return false;
    };
    let Ok(mut stream) = Stream::connect(name) else {
        return false;
    };
    stream.write_all(b"show\n").is_ok()
}

/// Removes the settings window's socket file as the window exits.
pub fn ui_unlisten() {
    if cfg!(unix) {
        let _ = std::fs::remove_file(ui_socket_path());
    }
}

/// Opens the settings window's socket, replacing a stale one. Call after
/// `ui_raise` found no window.
pub fn ui_listen() -> io::Result<Listener> {
    let path = ui_socket_path();
    if cfg!(unix) {
        let _ = std::fs::remove_file(&path);
    }
    ListenerOptions::new()
        .name(named(&path, "nexus-open-ui")?)
        .create_sync()
}

/// Error from `listen` when another daemon already answers on the socket.
#[derive(Debug)]
pub struct AlreadyRunning;

impl std::fmt::Display for AlreadyRunning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("nexusd is already running; stop it first (nexusctl status shows its pid)")
    }
}

impl std::error::Error for AlreadyRunning {}

/// Opens the daemon's socket. Fails with `AlreadyRunning` if a daemon is
/// listening; a socket file left behind by a crashed daemon is replaced.
pub fn listen() -> Result<Listener, Box<dyn std::error::Error + Send + Sync>> {
    let path = socket_path();
    if Client::connect().is_ok() {
        return Err(Box::new(AlreadyRunning));
    }
    if cfg!(unix) {
        // Nothing answered, so any file here is stale.
        let _ = std::fs::remove_file(&path);
    }
    Ok(ListenerOptions::new().name(name(&path)?).create_sync()?)
}

/// A connection to the daemon.
pub struct Client {
    reader: BufReader<Stream>,
}

impl Client {
    pub fn connect() -> io::Result<Client> {
        let path = socket_path();
        let stream = Stream::connect(name(&path)?)?;
        Ok(Client {
            reader: BufReader::new(stream),
        })
    }

    /// Sends one request line and returns the reply's data, or the
    /// daemon's error message.
    pub fn request(&mut self, line: &str) -> io::Result<Result<serde_json::Value, String>> {
        let stream = self.reader.get_mut();
        stream.write_all(format!("{line}\n").as_bytes())?;
        stream.flush()?;
        let mut answer = String::new();
        if self.reader.read_line(&mut answer)? == 0 {
            return Err(io::ErrorKind::UnexpectedEof.into());
        }
        let reply: serde_json::Value = serde_json::from_str(&answer)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        Ok(if reply["ok"] == true {
            Ok(reply["data"].clone())
        } else {
            Err(reply["error"]
                .as_str()
                .unwrap_or("unknown error")
                .to_string())
        })
    }

    /// After a successful `frames` request: blocks for the next frame.
    pub fn next_frame(&mut self) -> io::Result<Vec<u8>> {
        let mut len = [0u8; 4];
        self.reader.read_exact(&mut len)?;
        let len = u32::from_le_bytes(len) as usize;
        if len > 16 << 20 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "frame too large",
            ));
        }
        let mut frame = vec![0; len];
        self.reader.read_exact(&mut frame)?;
        Ok(frame)
    }
}

/// Writes one frame in the `frames` stream format.
pub fn write_frame(out: &mut impl Write, rgba: &[u8]) -> io::Result<()> {
    out.write_all(&(rgba.len() as u32).to_le_bytes())?;
    out.write_all(rgba)?;
    out.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_touches() {
        assert_eq!(
            Request::parse("tap 150"),
            Ok(Request::Touch {
                x: 150.0,
                hold: false
            })
        );
        assert_eq!(
            Request::parse("hold 0"),
            Ok(Request::Touch { x: 0.0, hold: true })
        );
        assert_eq!(
            Request::parse("drag 10 300"),
            Ok(Request::Drag {
                from: 10.0,
                to: 300.0
            })
        );
        assert_eq!(
            Request::parse("hold-drag 300 450"),
            Ok(Request::HoldDrag {
                from: 300.0,
                to: 450.0
            })
        );
        for bad in [
            "tap",
            "tap 640",
            "hold -1",
            "tap left",
            "drag 10",
            "drag 10 700",
            "hold-drag 300",
        ] {
            assert!(Request::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_takeover_requests() {
        assert_eq!(
            Request::parse("show-takeover 15 Now playing"),
            Ok(Request::ShowTakeover {
                name: "Now playing".into(),
                seconds: 15.0
            })
        );
        assert_eq!(Request::parse("end-takeover"), Ok(Request::EndTakeover));
        assert!(Request::parse("show-takeover 600 Volume").is_err());
        assert!(Request::parse("show-takeover 15").is_err());
        assert!(Request::parse("show-takeover Volume").is_err());
    }

    #[test]
    fn parses_requests() {
        assert_eq!(Request::parse("status\n"), Ok(Request::Status));
        assert_eq!(Request::parse("layout"), Ok(Request::Layout));
        assert_eq!(
            Request::parse("page 2"),
            Ok(Request::Page(PageTarget::Index(1)))
        );
        assert_eq!(
            Request::parse("page Load"),
            Ok(Request::Page(PageTarget::Name("Load".into())))
        );
        assert_eq!(
            Request::parse("publish cpu.temp 52.5"),
            Ok(Request::Publish(
                "cpu.temp".into(),
                Value::Number(52.5, Unit::None)
            ))
        );
        assert_eq!(
            Request::parse("publish cpu.temp 52 celsius"),
            Ok(Request::Publish(
                "cpu.temp".into(),
                Value::Number(52.0, Unit::Celsius)
            ))
        );
        assert_eq!(
            Request::parse("publish media.title Blue in Green"),
            Ok(Request::Publish(
                "media.title".into(),
                Value::Text("Blue in Green".into())
            ))
        );
    }

    #[test]
    fn rejects_with_guidance() {
        assert!(Request::parse("page 0").unwrap_err().contains("from 1"));
        assert!(Request::parse("publish x").unwrap_err().contains("value"));
        assert!(
            Request::parse("frobnicate")
                .unwrap_err()
                .contains("expected")
        );
    }

    #[test]
    fn artwork_crosses_the_socket_intact() {
        assert_eq!(Request::parse("art"), Ok(Request::Art));
        let rgba = nexus_core::art::test_card(48, 40);
        let art = Art::prepare("art-0123", 48, 40, &rgba).unwrap();
        let reply = ArtReply::new(&art);
        let line = serde_json::to_string(&reply).unwrap();
        assert!(line.len() < 16_000, "small enough to send per track");
        let back: ArtReply = serde_json::from_str(&line).unwrap();
        let back = back.to_art().unwrap();
        assert_eq!(back.key(), "art-0123");
        assert!(back.squares().eq(art.squares()));
        let mut bad = reply;
        bad.squares[0].rgba = "AAAA".into();
        assert!(bad.to_art().is_none());
    }

    #[test]
    fn reply_shape() {
        assert_eq!(reply(Ok(json!(1))), r#"{"data":1,"ok":true}"#);
        assert_eq!(reply(Err("no".into())), r#"{"error":"no","ok":false}"#);
    }
}
