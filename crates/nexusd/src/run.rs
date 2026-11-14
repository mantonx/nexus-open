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
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use jiff::Zoned;
use nexus_control::{PageTarget, Request, SignalEntry, Status};
use nexus_core::config::{self, Config};
use nexus_core::demo;
use nexus_core::engine::{Engine, Input};
use nexus_core::signal::Value;
use nexus_device::protocol::{TouchCalibration, parse_touch};
use nexus_device::{DeviceError, Surface};
use nexus_sources::Reading;
use notify::{EventKind, RecursiveMode, Watcher};
use serde_json::json;

use crate::control::{self, Frame, Reply};

/// How often to look for the device while it is unplugged.
const RECONNECT_POLL: Duration = Duration::from_millis(250);
/// Frame pacing while a page animates: about 60 fps. A frame takes the
/// device 15–17 ms to accept, so this is close to the most it sustains.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
/// How often the demo source publishes.
const DEMO_INTERVAL: Duration = Duration::from_secs(1);

/// Everything the daemon thread reacts to.
pub enum Msg {
    Engine(Input),
    Device(nexus_device::Input),
    ConfigChanged,
    /// A request from the control socket and where to send the answer.
    Control(Request, Sender<Reply>),
    /// A viewer (the settings window) wants every frame.
    Subscribe(SyncSender<Frame>),
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
    // Claim the socket first: it doubles as the guard against a second
    // daemon fighting over the device.
    let listener = nexus_control::listen().map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::channel();
    control::serve(listener, tx.clone());
    let (config, config_error) = first_config(&path)?;
    let mut daemon = Daemon {
        engine: Engine::new(config),
        path,
        device: None,
        next_attempt: Instant::now(),
        last_device_error: None,
        last_config_error: config_error,
        tx: tx.clone(),
        viewers: Vec::new(),
        current: None,
        calibration: TouchCalibration::default(),
        origin: Instant::now(),
        last_frame: Instant::now(),
        weather: None,
    };
    // Kept alive for as long as the daemon runs; dropping it stops watching.
    let _watcher = watch_config(&daemon.path, tx.clone())?;
    if options.demo {
        spawn_demo(publisher(tx));
    } else {
        nexus_sources::spawn_machine(publisher(tx.clone()));
        let weather = nexus_sources::spawn_weather(publisher(tx));
        let _ = weather.send(daemon.engine.config().weather.clone());
        daemon.weather = Some(weather);
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
    /// Settings windows receiving frames.
    viewers: Vec<SyncSender<Frame>>,
    /// The frame last produced, for a viewer that just connected.
    current: Option<Frame>,
    calibration: TouchCalibration,
    /// Origin of the monotonic clock shared by touch samples and animation.
    origin: Instant,
    last_frame: Instant,
    /// Tells the weather source where to look; `None` in demo mode.
    weather: Option<Sender<Option<config::Weather>>>,
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
                    Msg::Control(request, answer) => {
                        let _ = answer.send(self.control(request));
                    }
                    Msg::Subscribe(viewer) => {
                        match &self.current {
                            Some(frame) => {
                                let _ = viewer.try_send(frame.clone());
                            }
                            None => self.engine.handle(Input::Redraw),
                        }
                        self.viewers.push(viewer);
                    }
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
    ///
    /// With neither a device nor a viewer, nothing is drawn: the change
    /// stays pending until one appears.
    fn show(&mut self, now: &Zoned) {
        if self.device.is_none() && self.viewers.is_empty() {
            return;
        }
        let mono = self.origin.elapsed().as_secs_f64();
        let Some(frame) = self.engine.frame(now, mono) else {
            return;
        };
        self.last_frame = Instant::now();
        let frame: Frame = frame.rgba().into();
        control::broadcast(&mut self.viewers, &frame);
        self.current = Some(frame.clone());
        if let Some(device) = self.device.as_mut()
            && let Err(e) = device.send_frame(&frame)
        {
            self.lost(&e.to_string());
        }
    }

    /// Answers a control request.
    fn control(&mut self, request: Request) -> Reply {
        let pages = &self.engine.config().pages;
        match request {
            Request::Status => {
                let status = Status {
                    version: env!("CARGO_PKG_VERSION").to_string(),
                    pid: std::process::id(),
                    device_error: self.device.is_none().then(|| {
                        self.last_device_error
                            .clone()
                            .unwrap_or_else(|| "not connected yet".into())
                    }),
                    page: pages[self.engine.page()].name.clone(),
                    pages: pages.iter().map(|p| p.name.clone()).collect(),
                    config_path: self.path.display().to_string(),
                    config_error: self.last_config_error.clone(),
                };
                serde_json::to_value(status).map_err(|e| e.to_string())
            }
            Request::Signals => {
                let signals = self.engine.signals();
                let entries: Vec<SignalEntry> = signals
                    .names()
                    .into_iter()
                    .map(|name| SignalEntry {
                        name: name.to_string(),
                        value: match signals.get(name) {
                            Some(Value::Number(n, unit)) => format!("{n} ({unit:?})"),
                            Some(Value::Text(t)) => t.clone(),
                            None => String::new(),
                        },
                    })
                    .collect();
                serde_json::to_value(entries).map_err(|e| e.to_string())
            }
            Request::Page(target) => {
                let input = match target {
                    PageTarget::Next => Input::Step(1),
                    PageTarget::Prev => Input::Step(-1),
                    PageTarget::Index(i) if i < pages.len() => Input::ShowPage(i),
                    PageTarget::Index(i) => {
                        return Err(format!("no page {}; there are {}", i + 1, pages.len()));
                    }
                    PageTarget::Name(name) => match self.engine.config().page_index(&name) {
                        Some(i) => Input::ShowPage(i),
                        None => {
                            let names: Vec<&str> = pages.iter().map(|p| p.name.as_str()).collect();
                            return Err(format!(
                                "no page named \"{name}\"; pages are: {}",
                                names.join(", ")
                            ));
                        }
                    },
                };
                self.engine.handle(input);
                Ok(json!(null))
            }
            Request::Reload => {
                self.reload();
                match &self.last_config_error {
                    None => Ok(json!(null)),
                    Some(e) => Err(format!("{e}\nthe previous layout is still shown")),
                }
            }
            Request::Publish(name, value) => {
                self.engine.handle(Input::Signal(name, value));
                Ok(json!(null))
            }
            // Handled by the connection thread before it gets here.
            Request::Frames => Err("frames is handled by the connection".into()),
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
                if config.weather != self.engine.config().weather
                    && let Some(weather) = &self.weather
                {
                    let _ = weather.send(config.weather.clone());
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
    Ok(nexus_control::default_config_path().ok_or("cannot find the user's config directory")?)
}

/// `Ok(None)` if the file does not exist.
fn read_config(path: &Path) -> Result<Option<Config>, Box<dyn Error>> {
    match fs::read_to_string(path) {
        Ok(text) => Ok(Some(Config::parse(&text)?)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// The config to start with, and why the file was rejected if it was. A
/// missing file is created from the default so there is something to edit.
/// A broken file falls back to the default layout for this run, without
/// overwriting the file.
fn first_config(path: &Path) -> Result<(Config, Option<String>), Box<dyn Error>> {
    match read_config(path) {
        Ok(Some(config)) => Ok((config, None)),
        Ok(None) => {
            if let Some(dir) = path.parent() {
                fs::create_dir_all(dir)?;
            }
            fs::write(path, config::DEFAULT)?;
            eprintln!("nexusd: wrote the default layout to {}", path.display());
            Ok((Config::default_layout(), None))
        }
        Err(e) => {
            eprintln!(
                "nexusd: {}: {e}\nnexusd: showing the default layout until it is fixed",
                path.display()
            );
            Ok((Config::default_layout(), Some(e.to_string())))
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

/// Hands a source's readings to the daemon thread. Returns false once the
/// daemon has gone, which tells the source to stop.
fn publisher(tx: Sender<Msg>) -> impl FnMut(Vec<Reading>) -> bool + Send + 'static {
    move |readings| {
        readings.into_iter().all(|(name, value)| {
            let input = Input::Signal(name.to_string(), value);
            tx.send(Msg::Engine(input)).is_ok()
        })
    }
}

fn spawn_demo(mut publish: impl FnMut(Vec<Reading>) -> bool + Send + 'static) {
    let start = Instant::now();
    thread::Builder::new()
        .name("demo-source".into())
        .spawn(move || {
            while publish(demo::readings(start.elapsed().as_secs_f64())) {
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
