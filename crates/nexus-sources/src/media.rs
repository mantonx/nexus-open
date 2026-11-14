//! What is playing, from MPRIS: the D-Bus interface every Linux media
//! player (Spotify, Firefox, Chromium, mpv, VLC, …) offers.
//!
//! Event-driven: a listener thread wakes the state thread whenever a player
//! appears, disappears, changes a property or seeks; the state thread then
//! reads every player afresh. Players do not announce their position as it
//! advances, so while a track plays the state thread moves the position on
//! by itself once a second, from the last position a player reported.

use std::collections::HashMap;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use nexus_core::signal::{Unit, Value};
use zbus::blocking::{Connection, MessageIterator, fdo::DBusProxy};
use zbus::zvariant::{OwnedValue, Value as Variant};
use zbus::{MatchRule, message::Type};

use crate::Reading;

const PREFIX: &str = "org.mpris.MediaPlayer2.";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const PATH: &str = "/org/mpris/MediaPlayer2";
/// `playerctld` re-broadcasts the last active player; reading it too would
/// count that player twice.
const MIRRORS: &[&str] = &["org.mpris.MediaPlayer2.playerctld"];
/// How often the position moves on while playing.
const TICK: Duration = Duration::from_secs(1);

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Track {
    pub player: String,
    pub title: String,
    pub artist: String,
    pub status: Status,
    /// Seconds.
    pub position: f64,
    /// Seconds; 0 when unknown.
    pub length: f64,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Status {
    Playing,
    Paused,
    #[default]
    Stopped,
}

impl Status {
    fn parse(s: &str) -> Status {
        match s {
            "Playing" => Status::Playing,
            "Paused" => Status::Paused,
            _ => Status::Stopped,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Status::Playing => "playing",
            Status::Paused => "paused",
            Status::Stopped => "stopped",
        }
    }
}

/// Starts watching media players. Does nothing if there is no session bus
/// (a headless machine); that is not an error worth more than a line.
pub fn spawn(mut publish: impl FnMut(Vec<Reading>) -> bool + Send + 'static) {
    let conn = match Connection::session() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nexusd: media: no session bus ({e}); media will not show");
            return;
        }
    };
    let (wake, woken) = mpsc::channel::<()>();
    if let Err(e) = listen(&conn, wake) {
        eprintln!("nexusd: media: cannot watch players ({e}); media will not show");
        return;
    }
    thread::Builder::new()
        .name("media-source".into())
        .spawn(move || {
            let mut current: Option<Track> = None;
            let mut read_at = Instant::now();
            let mut published = false;
            loop {
                let fresh = read_all(&conn);
                let next = choose(&fresh, current.as_ref().map(|t| t.player.as_str()));
                if !published || next != current {
                    read_at = Instant::now();
                    current = next;
                    published = true;
                    if !publish(readings(current.as_ref())) {
                        return;
                    }
                }
                // Sleep until a player changes, ticking the position meanwhile.
                loop {
                    let playing = current
                        .as_ref()
                        .is_some_and(|t| t.status == Status::Playing);
                    let event = if playing {
                        woken.recv_timeout(TICK)
                    } else {
                        woken.recv().map_err(|_| RecvTimeoutError::Disconnected)
                    };
                    match event {
                        Ok(()) => {
                            // A change often arrives as a burst; read once.
                            while woken.try_recv().is_ok() {}
                            break;
                        }
                        Err(RecvTimeoutError::Timeout) => {
                            if let Some(t) = &current {
                                let position = advanced(t, read_at.elapsed());
                                let reading = ("media.position", seconds(position));
                                if !publish(vec![reading]) {
                                    return;
                                }
                            }
                        }
                        Err(RecvTimeoutError::Disconnected) => return,
                    }
                }
            }
        })
        .expect("spawning a thread");
}

/// Wakes `wake` on every player event: a player starting or quitting, a
/// property change, a seek.
fn listen(conn: &Connection, wake: mpsc::Sender<()>) -> zbus::Result<()> {
    let bus = DBusProxy::new(conn)?;
    let rules = [
        MatchRule::builder()
            .msg_type(Type::Signal)
            .interface("org.freedesktop.DBus.Properties")?
            .member("PropertiesChanged")?
            .path(PATH)?
            .build(),
        MatchRule::builder()
            .msg_type(Type::Signal)
            .interface(PLAYER)?
            .member("Seeked")?
            .build(),
        MatchRule::builder()
            .msg_type(Type::Signal)
            .interface("org.freedesktop.DBus")?
            .member("NameOwnerChanged")?
            .arg0ns("org.mpris.MediaPlayer2")?
            .build(),
    ];
    for rule in rules {
        bus.add_match_rule(rule)?;
    }
    let messages = MessageIterator::from(conn);
    thread::Builder::new()
        .name("media-listen".into())
        .spawn(move || {
            for message in messages.flatten() {
                let header = message.header();
                if header.message_type() != Type::Signal {
                    continue;
                }
                let member = header.member().map(|m| m.as_str());
                if matches!(
                    member,
                    Some("PropertiesChanged" | "Seeked" | "NameOwnerChanged")
                ) && wake.send(()).is_err()
                {
                    return;
                }
            }
        })
        .map_err(|e| zbus::Error::Failure(e.to_string()))?;
    Ok(())
}

/// Every player's current track, in bus order.
fn read_all(conn: &Connection) -> Vec<Track> {
    let names = DBusProxy::new(conn)
        .ok()
        .and_then(|bus| bus.list_names().ok());
    let Some(names) = names else {
        return Vec::new();
    };
    names
        .iter()
        .map(|n| n.as_str())
        .filter(|n| n.starts_with(PREFIX) && !MIRRORS.contains(n))
        .filter_map(|name| read_player(conn, name))
        .collect()
}

fn read_player(conn: &Connection, name: &str) -> Option<Track> {
    let reply = conn
        .call_method(
            Some(name),
            PATH,
            Some("org.freedesktop.DBus.Properties"),
            "GetAll",
            &(PLAYER,),
        )
        .ok()?;
    let props: HashMap<String, OwnedValue> = reply.body().deserialize().ok()?;
    let get = |key: &str| props.get(key).map(|v| inner(v));
    let meta: HashMap<String, OwnedValue> = props
        .get("Metadata")
        .and_then(|v| v.try_clone().ok())
        .and_then(|v| v.try_into().ok())
        .unwrap_or_default();
    let meta_get = |key: &str| meta.get(key).map(|v| inner(v));
    Some(Track {
        player: name.to_string(),
        title: meta_get("xesam:title").and_then(text).unwrap_or_default(),
        artist: meta_get("xesam:artist").and_then(text).unwrap_or_default(),
        status: get("PlaybackStatus")
            .and_then(text)
            .map_or(Status::Stopped, |s| Status::parse(&s)),
        position: get("Position").and_then(micros).map_or(0.0, to_seconds),
        length: meta_get("mpris:length")
            .and_then(micros)
            .map_or(0.0, to_seconds),
    })
}

/// The track to show: a playing one, else a paused one (so pausing does not
/// blank the widget). Among equals the one already shown wins, so two
/// paused players do not flip back and forth.
pub fn choose(tracks: &[Track], shown: Option<&str>) -> Option<Track> {
    let rank = |t: &Track| match t.status {
        Status::Playing => 2,
        Status::Paused => 1,
        Status::Stopped => 0,
    };
    tracks
        .iter()
        .filter(|t| rank(t) > 0 && !t.title.trim().is_empty())
        .max_by_key(|t| (rank(t), Some(t.player.as_str()) == shown))
        .cloned()
}

/// Where a playing track is now, `elapsed` after its position was read.
fn advanced(track: &Track, elapsed: Duration) -> f64 {
    let p = track.position + elapsed.as_secs_f64();
    if track.length > 0.0 {
        p.min(track.length)
    } else {
        p
    }
}

fn readings(track: Option<&Track>) -> Vec<Reading> {
    let t = track.cloned().unwrap_or_default();
    vec![
        ("media.title", Value::Text(t.title)),
        ("media.artist", Value::Text(t.artist)),
        ("media.status", Value::Text(t.status.word().into())),
        ("media.position", seconds(t.position)),
        ("media.length", seconds(t.length)),
    ]
}

fn seconds(s: f64) -> Value {
    Value::Number(s, Unit::None)
}

fn to_seconds(micros: i64) -> f64 {
    micros.max(0) as f64 / 1e6
}

/// A value with any variant wrapping removed.
fn inner<'a>(v: &'a Variant<'a>) -> &'a Variant<'a> {
    match v {
        Variant::Value(boxed) => inner(boxed),
        other => other,
    }
}

/// A string, or the first of a list of strings (artists are a list).
fn text(v: &Variant<'_>) -> Option<String> {
    match inner(v) {
        Variant::Str(s) => Some(s.to_string()),
        Variant::Array(a) => a.iter().find_map(text),
        _ => None,
    }
}

/// Microseconds; players disagree on the integer type.
fn micros(v: &Variant<'_>) -> Option<i64> {
    match inner(v) {
        Variant::I64(n) => Some(*n),
        Variant::U64(n) => i64::try_from(*n).ok(),
        Variant::I32(n) => Some(i64::from(*n)),
        Variant::U32(n) => Some(i64::from(*n)),
        Variant::F64(n) => Some(*n as i64),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(player: &str, status: Status) -> Track {
        Track {
            player: player.into(),
            title: format!("{player} song"),
            status,
            ..Track::default()
        }
    }

    #[test]
    fn playing_beats_paused_and_shown_breaks_ties() {
        let a = track("a", Status::Paused);
        let b = track("b", Status::Playing);
        let c = track("c", Status::Paused);
        assert_eq!(choose(&[a.clone(), b.clone()], None), Some(b));
        assert_eq!(choose(&[a.clone(), c.clone()], Some("c")), Some(c));
        assert_eq!(choose(&[track("d", Status::Stopped)], None), None);
        let untitled = Track {
            title: " ".into(),
            ..a
        };
        assert_eq!(choose(&[untitled], None), None);
    }

    #[test]
    fn position_advances_but_stops_at_the_end() {
        let t = Track {
            position: 10.0,
            length: 11.0,
            ..Track::default()
        };
        assert_eq!(advanced(&t, Duration::from_millis(500)), 10.5);
        assert_eq!(advanced(&t, Duration::from_secs(5)), 11.0);
        let stream = Track { length: 0.0, ..t };
        assert_eq!(advanced(&stream, Duration::from_secs(5)), 15.0);
    }

    #[test]
    fn metadata_types_vary_by_player() {
        let artists = Variant::from(vec!["Massive Attack", "Liz Fraser"]);
        assert_eq!(text(&artists).as_deref(), Some("Massive Attack"));
        let wrapped = Variant::Value(Box::new(Variant::from("Teardrop")));
        assert_eq!(text(&wrapped).as_deref(), Some("Teardrop"));
        assert_eq!(micros(&Variant::U64(330_000_000)), Some(330_000_000));
        assert_eq!(micros(&Variant::I64(-5)).map(to_seconds), Some(0.0));
    }
}
