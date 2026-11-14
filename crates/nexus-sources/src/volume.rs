//! The default audio output's volume and mute, through `pactl`, which
//! talks to PulseAudio and to PipeWire's PulseAudio service alike.
//!
//! Event-driven: `pactl subscribe` reports every change to an output
//! ("sink") or to the default output ("server"); each wakes a reader that
//! asks for the current level once, however many events arrived together.

use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use nexus_core::signal::{Unit, Value};

use crate::Reading;

const SINK: &str = "@DEFAULT_SINK@";
/// Wait before restarting `pactl subscribe` if it exits (audio restarted).
const RESTART: Duration = Duration::from_secs(5);
/// How long a burst of change events is given to finish arriving.
const BURST: Duration = Duration::from_millis(15);

pub fn spawn(mut publish: impl FnMut(Vec<Reading>) -> bool + Send + 'static) {
    if read().is_none() {
        eprintln!("nexusd: volume: pactl is not available or no audio server answers");
        return;
    }
    let (wake, woken) = mpsc::channel::<()>();
    thread::Builder::new()
        .name("volume-listen".into())
        .spawn(move || listen(&wake))
        .expect("spawning a thread");
    thread::Builder::new()
        .name("volume-source".into())
        .spawn(move || {
            let mut last = None;
            loop {
                let now = read();
                if now.is_some() && now != last {
                    let (level, muted) = now.unwrap_or_default();
                    let readings = vec![
                        ("volume.level", Value::Number(level, Unit::Percent)),
                        (
                            "volume.muted",
                            Value::Number(f64::from(u8::from(muted)), Unit::None),
                        ),
                    ];
                    if !publish(readings) {
                        return;
                    }
                    last = now;
                }
                if woken.recv().is_err() {
                    return;
                }
                // A drag sends a burst of changes: let it arrive, then read
                // once for all of it.
                thread::sleep(BURST);
                while woken.try_recv().is_ok() {}
            }
        })
        .expect("spawning a thread");
}

/// Runs `pactl subscribe` for as long as the daemon lives, waking `wake`
/// on output changes.
fn listen(wake: &mpsc::Sender<()>) {
    loop {
        let child = Command::new("pactl")
            .arg("subscribe")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn();
        if let Ok(mut child) = child {
            if let Some(out) = child.stdout.take() {
                for line in BufReader::new(out).lines().map_while(Result::ok) {
                    let relevant = line.contains(" on sink #") || line.contains(" on server");
                    if relevant && wake.send(()).is_err() {
                        let _ = child.kill();
                        let _ = child.wait();
                        return;
                    }
                }
            }
            let _ = child.wait();
        }
        // The audio server went away; it comes back after a restart.
        thread::sleep(RESTART);
        if wake.send(()).is_err() {
            return;
        }
    }
}

/// The default output's volume (percent) and mute.
fn read() -> Option<(f64, bool)> {
    let volume = pactl(&["get-sink-volume", SINK])?;
    let mute = pactl(&["get-sink-mute", SINK])?;
    Some((parse_volume(&volume)?, mute.contains("yes")))
}

fn pactl(args: &[&str]) -> Option<String> {
    let out = Command::new("pactl")
        .args(args)
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}

/// The first channel's percentage: "Volume: front-left: 26214 /  40% / …".
fn parse_volume(text: &str) -> Option<f64> {
    text.split('/')
        .map(str::trim)
        .find_map(|part| part.strip_suffix('%')?.trim().parse().ok())
}

/// Sets the default output's volume. Blocks until `pactl` is done; the
/// caller runs it off the daemon thread, latest value only.
pub fn set(percent: u8) {
    let level = format!("{}%", percent.min(100));
    if pactl(&["set-sink-volume", SINK, &level]).is_none() {
        eprintln!("nexusd: volume: pactl could not set the volume to {level}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_first_channel() {
        let text = "Volume: front-left: 26214 /  40% / -23.88 dB,   front-right: 26214 /  40% / -23.88 dB\n        balance 0.00\n";
        assert_eq!(parse_volume(text), Some(40.0));
        assert_eq!(
            parse_volume("Volume: mono: 65536 / 100% / 0.00 dB"),
            Some(100.0)
        );
        assert_eq!(parse_volume("Mute: no"), None);
    }
}
