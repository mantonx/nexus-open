//! "Run it from this window for now": when the service will not start,
//! the window runs the daemon itself, as its child, for this session.
//!
//! Never a second daemon against the strip: it is started only when
//! nothing answers on the control socket and the service is not running
//! (`systemctl --user is-active`), and the daemon itself refuses to run
//! while another one holds the socket, before it opens the strip. It stops
//! with the window: the window holds its standard input, and the daemon
//! (`--until-stdin-closes`) exits when that closes, however the window
//! ends; a window that quits normally also stops it at once.
//!
//! With `NEXUS_UI_SERVICE_STUB` set (the capture rig), the `is-active`
//! check goes through the stub and the daemon gets `--no-device`, so a
//! test never touches the strip.

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Mutex, PoisonError};
use std::thread;

use crate::status::{STUB_ENV, UNIT, installed, stubbed};

/// The child, and the pipe to its standard input: dropping the pipe is
/// what tells it to stop.
struct Running {
    child: Child,
    _stdin: ChildStdin,
}

static CHILD: Mutex<Option<Running>> = Mutex::new(None);
/// The child's last words on standard error, for the log and for why it
/// stopped.
static OUTPUT: Mutex<Vec<String>> = Mutex::new(Vec::new());
/// Lines of output kept.
const KEPT: usize = 200;

/// Where the daemon run from this window is.
#[derive(Clone, Debug, PartialEq)]
pub enum State {
    /// Never started this session.
    None,
    Running,
    /// It stopped by itself: why, in plain words.
    Stopped(String),
}

/// What runs the daemon from this window.
pub fn command(exe: &Path, testing: bool) -> Vec<String> {
    let mut argv = vec![
        exe.display().to_string(),
        "run".into(),
        "--until-stdin-closes".into(),
    ];
    if testing {
        argv.push("--no-device".into());
    }
    argv
}

/// What asks whether the service is running.
pub fn is_active_command() -> Vec<String> {
    ["systemctl", "--user", "is-active", UNIT]
        .map(String::from)
        .to_vec()
}

/// Whether `systemctl --user is-active` said the service is running or on
/// its way: then a second daemon must not be started beside it.
pub fn says_active(output: &str) -> bool {
    matches!(
        output.trim(),
        "active" | "activating" | "reloading" | "deactivating" | "refreshing"
    )
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Why running it here was refused: systemd says the service runs,
/// though it does not answer. The status says so in its own words.
pub const STILL_RUNNING: &str = "The service is still running, though it does not answer.";

/// Starts the daemon as this window's child, or says why not.
pub fn start() -> Result<(), String> {
    if state() == State::Running {
        return Ok(());
    }
    if nexus_control::Client::connect().is_ok() {
        return Err("The service answers now, so there is nothing to run here.".into());
    }
    let stub = std::env::var(STUB_ENV).ok();
    if installed() || stub.is_some() {
        let argv = stubbed(is_active_command(), stub.clone());
        if let Ok(out) = Command::new(&argv[0]).args(&argv[1..]).output()
            && says_active(&String::from_utf8_lossy(&out.stdout))
        {
            return Err(STILL_RUNNING.into());
        }
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let argv = command(&exe, stub.is_some_and(|s| !s.is_empty()));
    let mut child = Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("It could not be run from this window: {e}."))?;
    lock(&OUTPUT).clear();
    if let Some(err) = child.stderr.take() {
        let _ = thread::Builder::new()
            .name("here-log".into())
            .spawn(move || {
                for line in BufReader::new(err).lines().map_while(Result::ok) {
                    let mut out = lock(&OUTPUT);
                    out.push(line);
                    if out.len() > KEPT {
                        out.remove(0);
                    }
                }
            });
    }
    let Some(stdin) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Err("It could not be run from this window.".into());
    };
    *lock(&CHILD) = Some(Running {
        child,
        _stdin: stdin,
    });
    Ok(())
}

/// Whether the daemon run from this window is running, or why it stopped.
pub fn state() -> State {
    let mut slot = lock(&CHILD);
    let Some(running) = slot.as_mut() else {
        return if lock(&OUTPUT).is_empty() {
            State::None
        } else {
            State::Stopped(stopped_why(None))
        };
    };
    match running.child.try_wait() {
        Ok(None) => State::Running,
        Ok(Some(status)) => {
            *slot = None;
            let why = stopped_why(Some(status.to_string()));
            // Kept, so the state stays "stopped" and says why.
            let mut out = lock(&OUTPUT);
            if out.is_empty() {
                out.push(why.clone());
            }
            State::Stopped(why)
        }
        Err(e) => State::Stopped(format!("It stopped: {e}.")),
    }
}

/// Why it stopped: its last line on standard error, if it said anything.
fn stopped_why(status: Option<String>) -> String {
    let out = lock(&OUTPUT);
    match out.last() {
        Some(line) => format!("It stopped, saying: {}.", line.trim().trim_end_matches('.')),
        None => match status {
            Some(s) => format!("It stopped ({s})."),
            None => "It stopped.".into(),
        },
    }
}

/// What it wrote while it ran, for the log; empty if it never ran.
pub fn output() -> String {
    lock(&OUTPUT).join("\n")
}

/// Stops it now (the window is quitting).
pub fn stop() {
    if let Some(mut running) = lock(&CHILD).take() {
        let _ = running.child.kill();
        let _ = running.child.wait();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_runs_this_programs_daemon_tied_to_the_window() {
        let exe = Path::new("/usr/bin/nexusd");
        assert_eq!(
            command(exe, false),
            ["/usr/bin/nexusd", "run", "--until-stdin-closes"]
        );
        // In the rig it must never open the strip.
        assert_eq!(command(exe, true).last().unwrap(), "--no-device");
    }

    #[test]
    fn a_running_or_starting_service_is_never_doubled() {
        for active in ["active\n", "activating", "reloading"] {
            assert!(says_active(active), "{active}");
        }
        for not in ["inactive", "failed", "unknown", "stub: would run: x", ""] {
            assert!(!says_active(not), "{not}");
        }
        assert_eq!(
            is_active_command(),
            ["systemctl", "--user", "is-active", "nexusd.service"]
        );
    }
}
