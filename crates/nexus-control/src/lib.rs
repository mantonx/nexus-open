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
//! reload                        re-read the config file now
//! publish <signal> <value>      set a signal: a number with an optional
//!                               unit (celsius, percent, bytes-per-second),
//!                               or any text
//! frames                        stream every frame (see below)
//! ```
//!
//! After `frames` and its reply, the connection carries only frames: each
//! is a 4-byte little-endian length followed by that many bytes of 640×48
//! RGBA. The current frame is sent first.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::path::PathBuf;

use interprocess::local_socket::prelude::*;
use interprocess::local_socket::{Listener, ListenerOptions, Name, Stream};
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
    Frames,
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
                let unit = match unit.trim() {
                    "" => Some(Unit::None),
                    "celsius" => Some(Unit::Celsius),
                    "percent" => Some(Unit::Percent),
                    "bytes-per-second" => Some(Unit::BytesPerSecond),
                    _ => None, // not a unit: the whole value is text
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
            "" => Err("empty request".into()),
            other => Err(format!(
                "unknown request \"{other}\"; expected status, signals, page, reload, publish, tap, hold or frames"
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
    pub config_path: String,
    /// Why the config file was rejected; the previous layout is in use.
    pub config_error: Option<String>,
}

/// One entry of the answer to `signals`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SignalEntry {
    pub name: String,
    pub value: String,
}

/// Formats a reply line (without the newline).
pub fn reply(result: Result<serde_json::Value, String>) -> String {
    match result {
        Ok(data) => json!({ "ok": true, "data": data }).to_string(),
        Err(error) => json!({ "ok": false, "error": error }).to_string(),
    }
}

/// The config file used when none is given: `nexus-open/nexus.toml` in the
/// user's config directory (`~/.config` on Linux).
pub fn default_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("nexus-open").join("nexus.toml"))
}

/// Where the socket lives: the per-user runtime directory (only the user
/// can enter it), falling back to the per-user temporary directory.
pub fn socket_path() -> PathBuf {
    dirs::runtime_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("nexus-open.sock")
}

fn name(path: &std::path::Path) -> io::Result<Name<'_>> {
    if cfg!(windows) {
        "nexus-open".to_ns_name::<interprocess::local_socket::GenericNamespaced>()
    } else {
        path.to_fs_name::<interprocess::local_socket::GenericFilePath>()
    }
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
        for bad in ["tap", "tap 640", "hold -1", "tap left"] {
            assert!(Request::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_requests() {
        assert_eq!(Request::parse("status\n"), Ok(Request::Status));
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
    fn reply_shape() {
        assert_eq!(reply(Ok(json!(1))), r#"{"data":1,"ok":true}"#);
        assert_eq!(reply(Err("no".into())), r#"{"error":"no","ok":false}"#);
    }
}
