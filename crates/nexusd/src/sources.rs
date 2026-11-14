//! Runs the config's external sources: programs that print signals as JSON
//! lines (docs/EXTERNAL_SOURCES.md). Each gets a supervisor thread that
//! starts it, publishes what it prints, logs what it says on stderr, and
//! starts it again if it exits, waiting longer each time it keeps failing.
//!
//! A source down for a minute (exited, failing to start, or restarting
//! without printing anything) has its signals forgotten, so the strip shows
//! "—" rather than a reading that has stopped being true. Removing a source,
//! or changing its program, forgets them at once.
//!
//! Programs run in the config folder, so `run = ["python3", "stars.py"]`
//! finds a script kept next to nexus.toml. Under systemd they run as their
//! own user unit, outside the daemon's sandbox, named after the source and
//! bound to the daemon's unit: if the daemon stops or crashes, systemd stops
//! them too. Stopping `systemd-run` itself would not; it leaves the unit
//! running (checked).

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use nexus_core::config::Source;
use nexus_core::engine::Input;
use nexus_sources::external::parse_line;

use crate::run::Msg;

/// Wait before the first restart; doubled after each quick failure.
const FIRST_RETRY: Duration = Duration::from_secs(2);
const MAX_RETRY: Duration = Duration::from_secs(60);
/// A run this long counts as healthy: the next failure waits FIRST_RETRY.
const HEALTHY: Duration = Duration::from_secs(60);
/// How long a source may be down before its signals are forgotten.
const STALE: Duration = Duration::from_secs(60);
/// Longest line read, stdout or stderr. Longer ones are dropped, so a
/// program printing without newlines cannot fill the daemon's memory.
const MAX_LINE: usize = 64 * 1024;
/// Bad lines logged per run; after that a run's mistakes are counted only.
const MAX_LOGGED: u32 = 3;

pub struct Sources {
    tx: Sender<Msg>,
    /// Where programs run: the config folder.
    dir: PathBuf,
    running: HashMap<String, Running>,
}

impl Sources {
    pub fn new(tx: Sender<Msg>, config: &Path) -> Self {
        let dir = config
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        Sources {
            tx,
            dir,
            running: HashMap::new(),
        }
    }

    /// Makes the running programs match the config: stops removed or
    /// changed ones, starts new ones, leaves the rest alone.
    pub fn apply(&mut self, sources: &[Source]) {
        let before = std::mem::take(&mut self.running);
        for (name, running) in before {
            if sources
                .iter()
                .any(|s| s.enabled && s.name == name && s.run == running.run)
            {
                self.running.insert(name, running);
            } else {
                // Stop it first, so it publishes nothing after the forgets.
                let published = Arc::clone(&running.published);
                drop(running);
                forget(&published, &self.tx);
            }
        }
        for source in sources.iter().filter(|s| s.enabled) {
            if !self.running.contains_key(&source.name) {
                let running = Running::start(source, &self.dir, self.tx.clone());
                self.running.insert(source.name.clone(), running);
            }
        }
    }
}

/// A supervised source. Dropping it stops the program.
struct Running {
    run: Vec<String>,
    unit: Option<String>,
    child: Arc<Mutex<Option<Child>>>,
    /// Every signal it has published and not had forgotten.
    published: Arc<Mutex<HashSet<String>>>,
    /// Never sent on: dropping it is what tells the supervisor to stop.
    _stop: Sender<()>,
}

impl Running {
    fn start(source: &Source, dir: &Path, tx: Sender<Msg>) -> Self {
        let (stop, stopped) = mpsc::channel();
        let unit = crate::actions::systemd_unit().map(|owner| Unit {
            name: format!("nexus-source-{}", source.name),
            owner,
        });
        let child = Arc::new(Mutex::new(None));
        let published = Arc::new(Mutex::new(HashSet::new()));
        let job = Job {
            published: Arc::clone(&published),
            name: source.name.clone(),
            argv: resolve(&source.run, dir),
            dir: dir.to_path_buf(),
            unit: unit.clone(),
            child: Arc::clone(&child),
            stopped,
            tx,
        };
        thread::Builder::new()
            .name(format!("source-{}", source.name))
            .spawn(move || job.supervise())
            .expect("spawning a thread");
        Running {
            run: source.run.clone(),
            unit: unit.map(|u| u.name),
            child,
            published,
            _stop: stop,
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        let (unit, child) = (self.unit.take(), Arc::clone(&self.child));
        // `systemctl stop` waits for the program to exit: not on the
        // daemon thread.
        let _ = thread::Builder::new()
            .name("source-stop".into())
            .spawn(move || end(unit.as_deref(), &child));
    }
}

/// The systemd unit a source runs as.
#[derive(Clone)]
struct Unit {
    name: String,
    /// The daemon's own unit, which the source's is bound to.
    owner: String,
}

struct Job {
    name: String,
    argv: Vec<String>,
    dir: PathBuf,
    unit: Option<Unit>,
    child: Arc<Mutex<Option<Child>>>,
    published: Arc<Mutex<HashSet<String>>>,
    stopped: Receiver<()>,
    tx: Sender<Msg>,
}

impl Job {
    fn supervise(self) {
        let unit = self.unit.as_ref().map(|u| u.name.as_str());
        let mut retry = FIRST_RETRY;
        // When it last stopped delivering readings, and whether its
        // signals were forgotten since.
        let mut down_since: Option<Instant> = None;
        let mut forgotten = false;
        loop {
            // A unit left over from a daemon that was killed outright.
            if let Some(unit) = unit {
                stop_unit(unit);
            }
            let started = Instant::now();
            let mut delivered = false;
            let why = match self.command().spawn() {
                Ok(mut child) => {
                    let (out, err) = (child.stdout.take(), child.stderr.take());
                    *lock(&self.child) = Some(child);
                    if self.is_stopped() {
                        end(unit, &self.child);
                        return;
                    }
                    if let Some(err) = err {
                        let name = self.name.clone();
                        let _ = thread::Builder::new()
                            .name(format!("source-{}-err", self.name))
                            .spawn(move || {
                                read_lines(err, |line| {
                                    eprintln!("nexusd: source {name}: {}", line.trim_end());
                                    true
                                });
                            });
                    }
                    if let Some(out) = out {
                        match self.publish(out) {
                            Some(any) => delivered = any,
                            None => return, // stopped, or the daemon is gone
                        }
                    }
                    let status = lock(&self.child).take().map(|mut c| c.wait());
                    match status {
                        Some(Ok(status)) => format!("exited ({status})"),
                        Some(Err(e)) => format!("exited ({e})"),
                        None => "was stopped".into(),
                    }
                }
                Err(e) => format!("cannot start {}: {e}", self.argv[0]),
            };
            if self.is_stopped() {
                return;
            }
            if started.elapsed() >= HEALTHY {
                retry = FIRST_RETRY;
            }
            // A run that printed readings was up until now; one that did
            // not leaves the source down since whenever it went down.
            if delivered || down_since.is_none() {
                down_since = Some(Instant::now());
                forgotten = false;
            }
            eprintln!(
                "nexusd: source {}: {why}; starting it again in {} s",
                self.name,
                retry.as_secs()
            );
            let restart_at = Instant::now() + retry;
            if let Some(since) = down_since
                && !forgotten
                && since + STALE < restart_at
            {
                if !self.sleep_until(since + STALE) {
                    return;
                }
                eprintln!(
                    "nexusd: source {}: down for {} s; its signals now show no data",
                    self.name,
                    STALE.as_secs()
                );
                forget(&self.published, &self.tx);
                forgotten = true;
            }
            if !self.sleep_until(restart_at) {
                return;
            }
            retry = (retry * 2).min(MAX_RETRY);
        }
    }

    fn command(&self) -> Command {
        let mut command = match &self.unit {
            Some(unit) => {
                let mut c = Command::new("systemd-run");
                c.args(["--user", "--pipe", "--quiet", "--collect"])
                    .arg(format!("--unit={}", unit.name))
                    .arg(format!("--working-directory={}", self.dir.display()))
                    .arg(format!("--property=BindsTo={}", unit.owner))
                    .arg(format!("--property=After={}", unit.owner))
                    .arg("--")
                    .args(&self.argv);
                c
            }
            None => {
                let mut c = Command::new(&self.argv[0]);
                c.args(&self.argv[1..]).current_dir(&self.dir);
                c
            }
        };
        command
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        command
    }

    /// Publishes each line's readings until the program closes its output.
    /// Whether any line had readings, or `None` once this source has been
    /// stopped or the daemon has gone.
    fn publish(&self, out: impl Read) -> Option<bool> {
        let mut mistakes = 0u32;
        let mut any = false;
        let open = read_lines(out, |line| match parse_line(line) {
            // Checked per line: a stopped source must not put back what
            // was just forgotten.
            Ok(_) if self.is_stopped() => false,
            Ok(readings) => {
                any |= !readings.is_empty();
                readings.into_iter().all(|(name, value)| {
                    lock(&self.published).insert(name.clone());
                    self.tx
                        .send(Msg::Engine(Input::Signal(name, value)))
                        .is_ok()
                })
            }
            Err(e) => {
                mistakes += 1;
                if mistakes <= MAX_LOGGED {
                    eprintln!("nexusd: source {}: line ignored: {e}", self.name);
                }
                if mistakes == MAX_LOGGED {
                    eprintln!(
                        "nexusd: source {}: further bad lines from this run are not logged",
                        self.name
                    );
                }
                true
            }
        });
        open.then_some(any)
    }

    /// Waits until `when`; false if stopped first.
    fn sleep_until(&self, when: Instant) -> bool {
        let wait = when.saturating_duration_since(Instant::now());
        matches!(
            self.stopped.recv_timeout(wait),
            Err(RecvTimeoutError::Timeout)
        )
    }

    fn is_stopped(&self) -> bool {
        matches!(self.stopped.try_recv(), Err(TryRecvError::Disconnected))
    }
}

/// Calls `f` with each line, dropping any longer than MAX_LINE. Stops when
/// the stream ends or `f` returns false, and returns false in that case.
fn read_lines(stream: impl Read, mut f: impl FnMut(&str) -> bool) -> bool {
    let mut reader = BufReader::new(stream);
    let mut buf = Vec::new();
    let mut too_long = false;
    loop {
        buf.clear();
        match (&mut reader)
            .take(MAX_LINE as u64)
            .read_until(b'\n', &mut buf)
        {
            Ok(0) | Err(_) => return true,
            Ok(_) => {}
        }
        let ended = buf.ends_with(b"\n");
        if too_long {
            // Inside an over-long line: skip to its end.
            too_long = !ended;
            continue;
        }
        if !ended && buf.len() == MAX_LINE {
            too_long = true;
            continue;
        }
        // A line, or the stream's last one if it lacks a newline.
        if !f(&String::from_utf8_lossy(&buf)) {
            return false;
        }
    }
}

/// Tells the engine a source's signals are gone.
fn forget(published: &Mutex<HashSet<String>>, tx: &Sender<Msg>) {
    for name in lock(published).drain() {
        let _ = tx.send(Msg::Engine(Input::Forget(name)));
    }
}

/// Stops a source's program, by unit when it has one.
fn end(unit: Option<&str>, child: &Mutex<Option<Child>>) {
    if let Some(unit) = unit {
        stop_unit(unit);
    }
    if let Some(mut child) = lock(child).take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn stop_unit(unit: &str) {
    let _ = Command::new("systemctl")
        .args(["--user", "stop", "--quiet", unit])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The program path made absolute against the config folder when it is
/// written as a relative path (`./disk.sh`), since Rust and systemd-run
/// disagree about which directory such a path means. Bare names (`python3`)
/// are left for PATH.
fn resolve(argv: &[String], dir: &Path) -> Vec<String> {
    let mut argv = argv.to_vec();
    if let Some(program) = argv.first_mut() {
        let path = Path::new(program.as_str());
        if path.is_relative() && path.components().count() > 1 {
            *program = dir.join(path).display().to_string();
        }
    }
    argv
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lines(input: &str) -> Vec<String> {
        let mut got = Vec::new();
        read_lines(input.as_bytes(), |l| {
            got.push(l.trim_end().to_string());
            true
        });
        got
    }

    #[test]
    fn reads_lines_and_drops_overlong_ones() {
        assert_eq!(lines("a\nb\nlast"), ["a", "b", "last"]);
        let long = "x".repeat(MAX_LINE * 2 + 5);
        assert_eq!(lines(&format!("a\n{long}\nb\n")), ["a", "b"]);
        let exact = "y".repeat(MAX_LINE - 1);
        assert_eq!(lines(&format!("{exact}\nb\n")), [exact.as_str(), "b"]);
        assert_eq!(
            lines(&format!("a\n{long}")),
            ["a"],
            "no tail of a long line"
        );
    }

    #[test]
    fn relative_programs_are_found_in_the_config_folder() {
        // A real absolute path on every platform.
        let dir = std::env::temp_dir();
        let argv = |a: &[&str]| a.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        let absolute = dir.join("tool").display().to_string();
        assert_eq!(
            resolve(&argv(&["./disk.sh", "x"]), &dir)[0],
            dir.join("./disk.sh").display().to_string()
        );
        assert_eq!(resolve(&argv(&["python3", "a.py"]), &dir)[0], "python3");
        assert_eq!(resolve(&argv(&[&absolute]), &dir)[0], absolute);
    }
}
