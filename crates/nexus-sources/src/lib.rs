//! Built-in sources: readings from this machine, the weather and news
//! headlines.
//!
//! A source publishes raw readings in base units (°C, percent, bytes per
//! second) under fixed signal names; how they look is up to the widgets.
//! A reading that cannot be taken is simply not published, so its widget
//! keeps showing "—" rather than a made-up number.
//!
//! Each source runs on its own thread and hands its readings to a callback,
//! so this crate knows nothing about the daemon.

use std::sync::Arc;
use std::thread;
use std::time::Duration;

use nexus_core::art::Art;
use nexus_core::config::AlbumArt;
use nexus_core::signal::Value;

mod cpu_sensor;
pub mod external;
mod gpu;
mod machine;
#[cfg(target_os = "linux")]
mod media;
#[cfg(target_os = "linux")]
mod media_art;
pub mod news;
#[cfg(target_os = "linux")]
mod volume;
mod weather;

pub use news::spawn as spawn_news;
pub use weather::spawn as spawn_weather;

/// Sent with every web request, so a service can tell who is asking
/// without anything else about the user.
pub(crate) const USER_AGENT: &str = concat!("nexus-open/", env!("CARGO_PKG_VERSION"));

/// Sends a command to the media player the media widget shows. Linux only
/// for now; elsewhere it does nothing.
pub fn media_command(command: nexus_core::config::MediaCommand) {
    #[cfg(target_os = "linux")]
    media::command(command);
    #[cfg(not(target_os = "linux"))]
    let _ = command;
}

/// Moves the track the media widget shows to `seconds` in. Linux only for
/// now.
pub fn media_seek(seconds: f64) {
    #[cfg(target_os = "linux")]
    media::seek(seconds);
    #[cfg(not(target_os = "linux"))]
    let _ = seconds;
}

/// Starts publishing `volume.level` and `volume.muted` for the default
/// audio output. Linux only for now (PulseAudio or PipeWire, via `pactl`).
pub fn spawn_volume(publish: impl FnMut(Vec<Reading>) -> bool + Send + 'static) {
    #[cfg(target_os = "linux")]
    volume::spawn(publish);
    #[cfg(not(target_os = "linux"))]
    drop(publish);
}

/// Sets the default output's volume, 0–100. Blocks until done.
pub fn set_volume(percent: u8) {
    #[cfg(target_os = "linux")]
    volume::set(percent);
    #[cfg(not(target_os = "linux"))]
    let _ = percent;
}

/// Starts publishing the `media.*` signals from the system's media players.
/// Linux only for now (MPRIS); elsewhere the media widget shows "Nothing
/// playing".
///
/// The track's artwork, where `album_art` allows it, is loaded off this
/// thread and the caller's, prepared once, and passed to `show_art` just
/// before `media.art` names it (`nexus_core::engine::Input::Art`).
pub fn spawn_media(
    publish: impl FnMut(Vec<Reading>) -> bool + Send + 'static,
    show_art: impl FnMut(Arc<Art>) -> bool + Send + 'static,
    album_art: AlbumArt,
) -> Media {
    #[cfg(target_os = "linux")]
    return Media {
        wake: media::spawn(publish, show_art, album_art),
    };
    #[cfg(not(target_os = "linux"))]
    {
        drop((publish, show_art, album_art));
        Media {}
    }
}

/// The running media source.
pub struct Media {
    #[cfg(target_os = "linux")]
    wake: Option<std::sync::mpsc::Sender<media::Wake>>,
}

impl Media {
    /// Where artwork may come from now; the track's is looked at again.
    pub fn set_album_art(&self, album_art: AlbumArt) {
        #[cfg(target_os = "linux")]
        if let Some(wake) = &self.wake {
            let _ = wake.send(media::Wake::Policy(album_art));
        }
        #[cfg(not(target_os = "linux"))]
        let _ = album_art;
    }
}

/// One reading: a signal name and its value.
pub type Reading = (&'static str, Value);

/// How often machine readings are taken. Graphs keep 60 samples, so they
/// span the last minute.
pub const MACHINE_INTERVAL: Duration = Duration::from_secs(1);

/// Samples CPU, memory, network and GPU every [`MACHINE_INTERVAL`] and
/// passes each batch to `publish`. Stops when `publish` returns false.
pub fn spawn_machine(mut publish: impl FnMut(Vec<Reading>) -> bool + Send + 'static) {
    thread::Builder::new()
        .name("machine-source".into())
        .spawn(move || {
            let mut machine = match machine::Machine::new() {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("nexusd: cannot read this machine's CPU and memory: {e}");
                    return;
                }
            };
            loop {
                thread::sleep(MACHINE_INTERVAL);
                if !publish(machine.sample()) {
                    return;
                }
            }
        })
        .expect("spawning a thread");
}
