//! `nexusd run`: the daemon.
//!
//! One thread owns the engine and the device. Everything else (the touch
//! reader, the config watcher, sources) runs on its own thread and sends a
//! `Msg` down one channel. The loop sleeps on that channel until a message
//! arrives or the engine's next deadline passes, so an unchanging strip
//! costs no CPU.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use jiff::Zoned;
use nexus_core::config::{self, Config};
use nexus_core::demo;
use nexus_core::engine::{Engine, Input};
use nexus_device::protocol::{TouchCalibration, parse_touch};
use nexus_device::{DeviceError, Surface};
use notify::{EventKind, RecursiveMode, Watcher};

/// How often to look for the device while it is unplugged.
const RECONNECT_POLL: Duration = Duration::from_millis(250);
/// Frame pacing while a page animates: about 60 fps. A frame takes the
/// device 15–17 ms to accept, so this is close to the most it sustains.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
/// How often the demo source publishes.
const DEMO_INTERVAL: Duration = Duration::from_secs(1);

/// Everything the daemon thread reacts to.
enum Msg {
    Engine(Input),
    Device(nexus_device::Input),
    ConfigChanged,
}

impl From<nexus_device::Input> for Msg {
    fn from(input: nexus_device::Input) -> Self {
        Msg::Device(input)
    }
}

pub struct Options {
    pub config: Option<PathBuf>,
    pub demo: bool,
}

pub fn run(options: Options) -> Result<(), Box<dyn Error>> {
    let path = match options.config {
        Some(path) => path,
        None => default_config_path()?,
    };
    let (tx, rx) = mpsc::channel();
    let mut daemon = Daemon {
        engine: Engine::new(first_config(&path)?),
        path,
        device: None,
        next_attempt: Instant::now(),
        last_device_error: None,
        last_config_error: None,
        tx: tx.clone(),
        calibration: TouchCalibration::default(),
        origin: Instant::now(),
        last_frame: Instant::now(),
    };
    // Kept alive for as long as the daemon runs; dropping it stops watching.
    let _watcher = watch_config(&daemon.path, tx.clone())?;
    if options.demo {
        spawn_demo(tx);
    }
    daemon.run(&rx)
}

struct Daemon {
    engine: Engine,
    path: PathBuf,
    device: Option<Box<dyn Surface>>,
    /// When to next try to open the device, while it is missing.
    next_attempt: Instant,
    /// Logged once, not on every retry.
    last_device_error: Option<String>,
    last_config_error: Option<String>,
    tx: Sender<Msg>,
    calibration: TouchCalibration,
    /// Origin of the monotonic clock shared by touch samples and animation.
    origin: Instant,
    last_frame: Instant,
}

impl Daemon {
    fn run(&mut self, rx: &Receiver<Msg>) -> Result<(), Box<dyn Error>> {
        loop {
            if self.device.is_none() && Instant::now() >= self.next_attempt {
                self.connect();
            }
            let now = Zoned::now();
            self.show(&now);

            // Sleep until something happens or the picture is due to change.
            let mut wait = self.engine.next_wait(&now);
            if self.engine.animating() {
                wait = Some(FRAME_INTERVAL.saturating_sub(self.last_frame.elapsed()));
            }
            if self.device.is_none() {
                let retry = self.next_attempt.saturating_duration_since(Instant::now());
                wait = Some(wait.map_or(retry, |w| w.min(retry)));
            }
            let first = match wait {
                Some(w) => match rx.recv_timeout(w) {
                    Ok(msg) => Some(msg),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return Ok(()),
                },
                None => match rx.recv() {
                    Ok(msg) => Some(msg),
                    Err(_) => return Ok(()),
                },
            };

            // Handle everything already queued before drawing, so a burst of
            // updates becomes one frame and a burst of file events one reload.
            let mut reload = false;
            for msg in first.into_iter().chain(rx.try_iter()) {
                match msg {
                    Msg::Engine(input) => self.engine.handle(input),
                    Msg::Device(nexus_device::Input::Report(report, read_at)) => {
                        self.touch(&report, read_at);
                    }
                    Msg::Device(nexus_device::Input::Gone(e)) => self.lost(&e.to_string()),
                    Msg::ConfigChanged => reload = true,
                }
            }
            if reload {
                self.reload();
            }
        }
    }

    /// Passes a touch report to the engine, timed by when it was read.
    fn touch(&mut self, report: &[u8], read_at: Instant) {
        if let Some(t) = parse_touch(report) {
            self.engine.handle(Input::Touch {
                down: t.down,
                x: f32::from(self.calibration.to_panel_x(t.x)),
                at: read_at.duration_since(self.origin).as_secs_f64(),
            });
        }
    }

    fn connect(&mut self) {
        match open_device(&self.tx) {
            Ok(device) => {
                eprintln!("nexusd: device connected");
                self.device = Some(device);
                self.last_device_error = None;
                self.engine.handle(Input::Redraw);
            }
            Err(e) => {
                let message = e.to_string();
                if self.last_device_error.as_ref() != Some(&message) {
                    eprintln!("nexusd: {message}");
                    self.last_device_error = Some(message);
                }
                self.next_attempt = match e {
                    // Retrying cannot help on an OS without a backend.
                    DeviceError::Unsupported => Instant::now() + Duration::from_secs(86_400),
                    _ => Instant::now() + RECONNECT_POLL,
                };
            }
        }
    }

    /// Sends a frame if the picture changed. Without a device nothing is
    /// drawn, so the change is still pending when the device returns.
    fn show(&mut self, now: &Zoned) {
        let Some(device) = self.device.as_mut() else {
            return;
        };
        let mono = self.origin.elapsed().as_secs_f64();
        if let Some(frame) = self.engine.frame(now, mono) {
            self.last_frame = Instant::now();
            if let Err(e) = device.send_frame(frame.rgba()) {
                self.lost(&e.to_string());
            }
        }
    }

    fn lost(&mut self, why: &str) {
        if self.device.take().is_some() {
            eprintln!("nexusd: device lost ({why}); waiting for it to return");
        }
        self.next_attempt = Instant::now();
    }

    /// Re-reads the config. A broken file is reported once and the current
    /// layout stays on the strip.
    fn reload(&mut self) {
        match read_config(&self.path) {
            Ok(Some(config)) => {
                if self.last_config_error.take().is_some() {
                    eprintln!("nexusd: config fixed; applying it");
                }
                if &config != self.engine.config() {
                    self.engine.handle(Input::Config(Box::new(config)));
                }
            }
            Ok(None) => {} // Deleted or mid-save; keep what is shown.
            Err(e) => {
                let message = e.to_string();
                if self.last_config_error.as_ref() != Some(&message) {
                    eprintln!(
                        "nexusd: {}: {message}\nnexusd: keeping the previous layout",
                        self.path.display()
                    );
                    self.last_config_error = Some(message);
                }
            }
        }
    }
}

fn default_config_path() -> Result<PathBuf, Box<dyn Error>> {
    let dir = dirs::config_dir().ok_or("cannot find the user's config directory")?;
    Ok(dir.join("nexus-open").join("nexus.toml"))
}

/// `Ok(None)` if the file does not exist.
fn read_config(path: &Path) -> Result<Option<Config>, Box<dyn Error>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(Config::parse(&text)?)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The config to start with. A missing file is created from the default so
/// there is something to edit. A broken file falls back to the default
/// layout for this run, without overwriting the file.
fn first_config(path: &Path) -> Result<Config, Box<dyn Error>> {
    match read_config(path) {
        Ok(Some(config)) => Ok(config),
        Ok(None) => {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(path, config::DEFAULT)?;
            eprintln!("nexusd: wrote the default layout to {}", path.display());
            Ok(Config::default_layout())
        }
        Err(e) => {
            eprintln!(
                "nexusd: {}: {e}\nnexusd: showing the default layout until it is fixed",
                path.display()
            );
            Ok(Config::default_layout())
        }
    }
}

/// Watches the config's directory, not the file: editors often save by
/// writing a new file and renaming it over the old one, which a watch on
/// the old file would miss.
fn watch_config(path: &Path, tx: Sender<Msg>) -> notify::Result<impl Watcher + use<>> {
    let file = path.to_path_buf();
    let mut watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
        if let Ok(event) = event
            && !matches!(event.kind, EventKind::Access(_))
            && event.paths.iter().any(|p| p == &file)
        {
            let _ = tx.send(Msg::ConfigChanged);
        }
    })?;
    let dir = path.parent().unwrap_or(Path::new("."));
    watcher.watch(dir, RecursiveMode::NonRecursive)?;
    Ok(watcher)
}

fn spawn_demo(tx: Sender<Msg>) {
    let start = Instant::now();
    thread::Builder::new()
        .name("demo-source".into())
        .spawn(move || {
            loop {
                for (name, value) in demo::readings(start.elapsed().as_secs_f64()) {
                    let input = Input::Signal(name.to_string(), value);
                    if tx.send(Msg::Engine(input)).is_err() {
                        return;
                    }
                }
                thread::sleep(DEMO_INTERVAL);
            }
        })
        .expect("spawning a thread");
}

#[cfg(target_os = "linux")]
fn open_device(tx: &Sender<Msg>) -> Result<Box<dyn Surface>, DeviceError> {
    let device = nexus_device::hidraw::Nexus::open()?;
    device.spawn_reader(tx.clone())?;
    Ok(Box::new(device))
}

#[cfg(not(target_os = "linux"))]
fn open_device(_: &Sender<Msg>) -> Result<Box<dyn Surface>, DeviceError> {
    Err(DeviceError::Unsupported)
}
