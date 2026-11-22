//! Carries out what taps, holds and slides ask for: the effects the engine
//! cannot do itself.
//!
//! Nothing here blocks the daemon thread: programs, URLs and media commands
//! are handed off, and slider values go to workers that only ever apply the
//! newest one. Failures are logged, since a touch has nowhere else to
//! report them.

use std::process::{Command, Stdio};
use std::sync::mpsc::{self, Sender};
use std::thread;

use nexus_core::config::Action;
use nexus_core::engine::Effect;

pub struct Effects {
    volume: Latest<u8>,
    slider: Latest<Vec<String>>,
}

impl Effects {
    pub fn new() -> Self {
        Effects {
            volume: Latest::spawn("set-volume", nexus_sources::set_volume),
            slider: Latest::spawn("slider-command", |argv: Vec<String>| {
                launch(&argv, true);
            }),
        }
    }

    pub fn carry_out(&self, effect: Effect) {
        match effect {
            Effect::Act(action) => act(action),
            Effect::Volume(percent) => self.volume.set(percent),
            Effect::Seek(seconds) => {
                let _ = thread::Builder::new()
                    .name("media-seek".into())
                    .spawn(move || nexus_sources::media_seek(seconds));
            }
            Effect::Run(argv) => self.slider.set(argv),
        }
    }
}

/// A worker that applies only the newest value. Values that arrive while a
/// job runs replace one another, so dragging a slider over a slow command
/// never builds a queue: the command catches up to where the finger is.
struct Latest<T> {
    tx: Sender<T>,
}

impl<T: Send + 'static> Latest<T> {
    fn spawn(name: &str, job: impl Fn(T) + Send + 'static) -> Self {
        let (tx, rx) = mpsc::channel::<T>();
        thread::Builder::new()
            .name(name.into())
            .spawn(move || {
                while let Ok(mut value) = rx.recv() {
                    while let Ok(newer) = rx.try_recv() {
                        value = newer;
                    }
                    job(value);
                }
            })
            .expect("spawning a thread");
        Latest { tx }
    }

    fn set(&self, value: T) {
        let _ = self.tx.send(value);
    }
}

fn act(action: Action) {
    match action {
        Action::Run(argv) => launch(&argv, false),
        Action::Open(target) => launch(&opener(&target), false),
        Action::Media(command) => {
            // A D-Bus call can stall on a hung player; keep it off the
            // daemon thread.
            let _ = thread::Builder::new()
                .name("media-command".into())
                .spawn(move || nexus_sources::media_command(command));
        }
        // Done by the engine before it gets here.
        Action::Page(_) | Action::Publish { .. } => {}
    }
}

/// Starts a program. With `wait`, returns once it has finished, which is
/// how a slider command paces itself; otherwise it runs on its own.
///
/// As a systemd service, nexusd runs sandboxed: a read-only system, no GPU,
/// a short device list. A program started directly would inherit all of
/// that, so a GUI app would fail. Under systemd it is started with
/// `systemd-run --user` instead, which asks the user's service manager to
/// run it as its own unit, outside this sandbox, as if from the launcher.
fn launch(argv: &[String], wait: bool) {
    let Some((program, args)) = argv.split_first() else {
        return;
    };
    let mut command = if systemd_unit().is_some() {
        let mut c = Command::new("systemd-run");
        c.args(["--user", "--collect", "--quiet"]);
        if wait {
            c.arg("--wait");
        }
        c.arg("--").arg(program).args(args);
        c
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    if wait {
        if let Err(e) = command.status() {
            eprintln!("nexusd: slider: cannot run {program}: {e}");
        }
        return;
    }
    match command.spawn() {
        // Reap it when it exits, so it does not linger as a zombie.
        Ok(mut child) => {
            let _ = thread::Builder::new()
                .name("launched".into())
                .spawn(move || child.wait());
        }
        Err(e) => eprintln!("nexusd: tap action: cannot start {program}: {e}"),
    }
}

/// The daemon's own systemd unit, when it runs as a service; `None` when
/// started by hand. Read from its cgroup, whose last part is the unit.
pub fn systemd_unit() -> Option<String> {
    if !cfg!(target_os = "linux") || std::env::var_os("INVOCATION_ID").is_none() {
        return None;
    }
    let cgroup = std::fs::read_to_string("/proc/self/cgroup").ok()?;
    cgroup
        .trim_end()
        .rsplit('/')
        .next()
        .filter(|unit| unit.ends_with(".service"))
        .map(str::to_string)
}

/// `~` or `~/…` at the start of a target is the home folder, so a layout
/// can open it without knowing whose it is.
fn home_relative(target: &str, home: Option<&str>) -> String {
    match (target.strip_prefix('~'), home) {
        (Some(rest), Some(home)) if rest.is_empty() || rest.starts_with('/') => {
            format!("{home}{rest}")
        }
        _ => target.to_string(),
    }
}

/// The command that opens a URL or file with the desktop's default app.
fn opener(target: &str) -> Vec<String> {
    let home = std::env::var("HOME").ok();
    let target = &home_relative(target, home.as_deref());
    let argv: &[&str] = if cfg!(target_os = "macos") {
        &["open"]
    } else if cfg!(target_os = "windows") {
        // `start` is a cmd built-in; the empty string is its window title.
        &["cmd", "/C", "start", ""]
    } else {
        &["xdg-open"]
    };
    argv.iter()
        .map(|s| s.to_string())
        .chain([target.to_string()])
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_leading_tilde_is_the_home_folder() {
        let home = Some("/home/u");
        assert_eq!(home_relative("~", home), "/home/u");
        assert_eq!(home_relative("~/Music", home), "/home/u/Music");
        assert_eq!(home_relative("~other", home), "~other");
        assert_eq!(home_relative("https://x.org/~a", home), "https://x.org/~a");
        assert_eq!(home_relative("~", None), "~");
    }
}
