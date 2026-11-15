//! `nexusd run`: the daemon.
//!
//! One thread owns the engine and the device. Everything else (the touch
//! reader, the config watcher, sources) runs on its own thread and sends a
//! `Msg` down one channel. The loop sleeps on that channel until a message
//! arrives or the engine's next deadline passes, so an unchanging strip
//! costs no CPU.

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use jiff::Zoned;
use nexus_control::{Export, Layout, PageTarget, Request, SignalEntry, Status};
use nexus_core::config;
use nexus_core::demo;
use nexus_core::engine::{Engine, Input};
use nexus_core::signal::Value;
use nexus_device::protocol::{TouchCalibration, parse_touch};
use nexus_device::{DeviceError, Surface};
use nexus_sources::Reading;
use nexus_store::{Edit, Store, text};
use serde_json::json;

use crate::control::{self, Frame, Reply};

/// How often to look for the device while it is unplugged.
const RECONNECT_POLL: Duration = Duration::from_millis(250);
/// Frame pacing while a page animates: about 60 fps. A frame takes the
/// device 15–17 ms to accept, so this is close to the most it sustains.
const FRAME_INTERVAL: Duration = Duration::from_millis(16);
/// Days of daily store backups kept.
const BACKUPS: usize = 7;
/// How often the demo source publishes.
const DEMO_INTERVAL: Duration = Duration::from_secs(1);

/// Everything the daemon thread reacts to.
pub enum Msg {
    Engine(Input),
    Device(nexus_device::Input),
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
    pub db: Option<PathBuf>,
    pub demo: bool,
}

pub fn run(options: Options) -> Result<(), Box<dyn Error>> {
    let store_path = match options.db {
        Some(path) => path,
        None => {
            nexus_control::default_store_path().ok_or("cannot find the user's state directory")?
        }
    };
    // The config file of earlier versions, imported on first start. Sources
    // run in its folder, where their scripts are kept.
    let legacy = nexus_control::default_config_path();
    // Claim the socket first: it doubles as the guard against a second
    // daemon fighting over the device.
    let listener = nexus_control::listen().map_err(|e| e.to_string())?;
    let (tx, rx) = mpsc::channel();
    control::serve(listener, tx.clone());
    let (store, store_error) = open_store(&store_path, legacy.as_deref());
    let (config, _) = store.load(store.active_layout()?)?;
    let sources_dir = legacy.unwrap_or_else(|| store_path.clone());
    let sources = crate::sources::Sources::new(tx.clone(), &sources_dir);
    let mut daemon = Daemon {
        engine: Engine::new(config),
        store,
        store_path,
        store_error,
        device: None,
        next_attempt: Instant::now(),
        last_device_error: None,
        tx: tx.clone(),
        viewers: Vec::new(),
        current: None,
        calibration: TouchCalibration::default(),
        origin: Instant::now(),
        last_frame: Instant::now(),
        weather: None,
        effects: crate::actions::Effects::new(),
        sources,
    };
    daemon.sources.apply(&daemon.engine.config().sources);
    if options.demo {
        spawn_demo(publisher(tx));
    } else {
        nexus_sources::spawn_machine(publisher(tx.clone()));
        nexus_sources::spawn_media(publisher(tx.clone()));
        nexus_sources::spawn_volume(publisher(tx.clone()));
        let weather = nexus_sources::spawn_weather(publisher(tx));
        let _ = weather.send(daemon.engine.config().weather.clone());
        daemon.weather = Some(weather);
    }
    daemon.run(&rx)
}

struct Daemon {
    engine: Engine,
    /// Everything the strip is configured with. Only this thread writes it.
    store: Store,
    store_path: PathBuf,
    /// Why the store could not be opened; the default layout is shown from
    /// memory and edits are refused.
    store_error: Option<String>,
    device: Option<Box<dyn Surface>>,
    /// When to next try to open the device, while it is missing.
    next_attempt: Instant,
    /// Logged once, not on every retry.
    last_device_error: Option<String>,
    tx: Sender<Msg>,
    /// Settings windows receiving frames.
    viewers: Vec<SyncSender<Frame>>,
    /// The frame last produced, for a viewer that just connected.
    current: Option<Frame>,
    calibration: TouchCalibration,
    /// Origin of the monotonic clock shared by touch samples and animation.
    origin: Instant,
    last_frame: Instant,
    /// Carries out what taps, holds and slides ask for.
    effects: crate::actions::Effects,
    /// Tells the weather source where to look; `None` in demo mode.
    weather: Option<Sender<Option<config::Weather>>>,
    /// The config's external source programs.
    sources: crate::sources::Sources,
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
            let mut wait = self
                .engine
                .next_wait(&now, self.origin.elapsed().as_secs_f64());
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
            // updates becomes one frame.
            for msg in first.into_iter().chain(rx.try_iter()) {
                match msg {
                    Msg::Engine(input) => self.engine.handle(input),
                    Msg::Device(nexus_device::Input::Report(report, read_at)) => {
                        self.touch(&report, read_at);
                    }
                    Msg::Device(nexus_device::Input::Gone(e)) => self.lost(&e.to_string()),
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
            for effect in self.engine.take_effects() {
                self.effects.carry_out(effect);
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
                    layout: self
                        .store
                        .active_layout()
                        .and_then(|id| self.store.layout_name(id))
                        .unwrap_or_default(),
                    store_path: self.store_path.display().to_string(),
                    store_error: self.store_error.clone(),
                    takeover: self.engine.takeover().map(str::to_string),
                    revision: self.store.revision().unwrap_or_default(),
                };
                serde_json::to_value(status).map_err(|e| e.to_string())
            }
            Request::Signals => {
                let signals = self.engine.signals();
                let entries: Vec<SignalEntry> = signals
                    .names()
                    .into_iter()
                    .map(|name| {
                        let value = signals.get(name);
                        let (number, unit) = match value {
                            Some(Value::Number(n, unit)) => {
                                (Some(*n), Some(unit.name().to_string()))
                            }
                            _ => (None, None),
                        };
                        SignalEntry {
                            name: name.to_string(),
                            value: match value {
                                Some(Value::Number(n, unit)) => format!("{n} ({unit:?})"),
                                Some(Value::Text(t)) => t.clone(),
                                None => String::new(),
                            },
                            number,
                            unit,
                        }
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
            Request::Reload => self.reload().map(|()| json!(null)),
            Request::Edit(request) => {
                self.writable()?;
                let now = self.store.revision().map_err(|e| e.to_string())?;
                if let Some(base) = request.base
                    && base != now
                {
                    return Err(format!(
                        "the layout changed since this edit was started (revision {base}, now {now}); nothing was applied"
                    ));
                }
                self.backup();
                let revision = self.store.edit(&request.edit).map_err(|e| e.to_string())?;
                self.reload()?;
                Ok(json!({ "revision": revision }))
            }
            Request::Undo | Request::Redo => {
                self.writable()?;
                let undo = matches!(request, Request::Undo);
                let step = if undo {
                    self.store.undo()
                } else {
                    self.store.redo()
                };
                match step.map_err(|e| e.to_string())? {
                    Some(label) => {
                        self.reload()?;
                        let revision = self.store.revision().map_err(|e| e.to_string())?;
                        Ok(json!({ "label": label, "revision": revision }))
                    }
                    None if undo => Err("nothing to undo".into()),
                    None => Err("nothing to redo".into()),
                }
            }
            Request::Export(name) => {
                let store = &self.store;
                let id = match &name {
                    Some(name) => store
                        .layout_named(name)
                        .map_err(|e| e.to_string())?
                        .ok_or_else(|| format!("no layout called \"{name}\""))?,
                    None => store.active_layout().map_err(|e| e.to_string())?,
                };
                let (config, notes) = store.load(id).map_err(|e| e.to_string())?;
                let export = Export {
                    id,
                    layout: store.layout_name(id).map_err(|e| e.to_string())?,
                    text: text::write(&config, &notes),
                    revision: store.revision().map_err(|e| e.to_string())?,
                };
                serde_json::to_value(export).map_err(|e| e.to_string())
            }
            Request::Layout => {
                let store = &self.store;
                let id = store.active_layout().map_err(|e| e.to_string())?;
                let loaded = store.load(id).map_err(|e| e.to_string())?;
                let layout = Layout {
                    id,
                    name: store.layout_name(id).map_err(|e| e.to_string())?,
                    revision: store.revision().map_err(|e| e.to_string())?,
                    config: loaded.0,
                    notes: loaded.1,
                    ids: store.ids(id).map_err(|e| e.to_string())?,
                    undo: store.undo_label().map_err(|e| e.to_string())?,
                    redo: store.redo_label().map_err(|e| e.to_string())?,
                };
                serde_json::to_value(layout).map_err(|e| e.to_string())
            }
            Request::Import(import) => {
                self.writable()?;
                let (config, notes) = text::parse(&import.text).map_err(|e| e.to_string())?;
                self.backup();
                let id = self
                    .store
                    .import(&import.name, &config, &notes, false)
                    .map_err(|e| e.to_string())?;
                if import.activate {
                    self.store
                        .edit(&Edit::Activate { layout: id })
                        .map_err(|e| e.to_string())?;
                    self.reload()?;
                }
                Ok(json!({ "layout": id }))
            }
            Request::Publish(name, value) => {
                self.engine.handle(Input::Signal(name, value));
                Ok(json!(null))
            }
            Request::Touch { x, hold } => {
                // A still finger sampled every 10 ms, then lifted: long
                // enough to clear the brush guard, or the long-press time.
                let start = self.origin.elapsed().as_secs_f64();
                let length = if hold { 0.6 } else { 0.08 };
                let mut t = 0.0;
                while t < length {
                    self.engine.handle(Input::Touch {
                        down: true,
                        x,
                        at: start + t,
                    });
                    t += 0.01;
                }
                self.engine.handle(Input::Touch {
                    down: false,
                    x: 0.0,
                    at: start + length,
                });
                Ok(json!(null))
            }
            Request::Drag { from, to } => {
                // 30 samples 10 ms apart: the strip's own report rate.
                let start = self.origin.elapsed().as_secs_f64();
                for i in 0..=30 {
                    self.engine.handle(Input::Touch {
                        down: true,
                        x: from + (to - from) * i as f32 / 30.0,
                        at: start + f64::from(i) * 0.01,
                    });
                }
                self.engine.handle(Input::Touch {
                    down: false,
                    x: 0.0,
                    at: start + 0.31,
                });
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

    /// Shows the store's layout on the strip: the engine, the weather
    /// source and the external sources all follow it.
    fn reload(&mut self) -> Result<(), String> {
        let (config, _) = self
            .store
            .active_layout()
            .and_then(|id| self.store.load(id))
            .map_err(|e| e.to_string())?;
        if config.weather != self.engine.config().weather
            && let Some(weather) = &self.weather
        {
            let _ = weather.send(config.weather.clone());
        }
        self.sources.apply(&config.sources);
        if &config != self.engine.config() {
            self.engine.handle(Input::Config(Box::new(config)));
        }
        Ok(())
    }

    /// Edits need the real store, not the stand-in shown when it failed.
    fn writable(&self) -> Result<(), String> {
        match &self.store_error {
            None => Ok(()),
            Some(e) => Err(format!(
                "the store could not be opened ({e}), so the default layout is shown and changes cannot be saved; fix it and restart nexusd"
            )),
        }
    }

    /// A copy of the store before the first change of each day, in
    /// `backups/` beside it; the last BACKUPS days are kept. A failure is
    /// logged and does not block the change.
    fn backup(&self) {
        let Some(dir) = self.store_path.parent().map(|d| d.join("backups")) else {
            return;
        };
        let day = Zoned::now().date();
        let path = dir.join(format!("nexusd-{day}.db"));
        if path.exists() {
            return;
        }
        let made = fs::create_dir_all(&dir)
            .map_err(|e| e.to_string())
            .and_then(|()| self.store.backup(&path).map_err(|e| e.to_string()));
        if let Err(e) = made {
            eprintln!("nexusd: backup to {} failed: {e}", path.display());
            return;
        }
        let mut old: Vec<PathBuf> = fs::read_dir(&dir)
            .into_iter()
            .flatten()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("nexusd-") && n.ends_with(".db"))
            })
            .collect();
        old.sort();
        let excess = old.len().saturating_sub(BACKUPS);
        for path in &old[..excess] {
            let _ = fs::remove_file(path);
        }
    }
}

/// Opens the store, filling it on first start: from the earlier versions'
/// config file if there is one (which is then renamed, never deleted), else
/// from the bundled default. If it cannot be opened, the default layout is
/// shown from memory and the reason is returned.
fn open_store(path: &Path, legacy: Option<&Path>) -> (Store, Option<String>) {
    let opened = (|| -> Result<Store, Box<dyn Error>> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        let mut store = Store::open(path)?;
        if store.is_empty()? {
            seed(&mut store, legacy)?;
        }
        Ok(store)
    })();
    match opened {
        Ok(store) => (store, None),
        Err(e) => {
            eprintln!(
                "nexusd: {}: {e}\nnexusd: showing the default layout; changes cannot be saved until this is fixed",
                path.display()
            );
            let mut store = Store::open_in_memory().expect("an in-memory database opens");
            let (config, notes) =
                text::parse(config::DEFAULT).expect("the bundled default is valid");
            store
                .import("Default", &config, &notes, true)
                .expect("the bundled default imports");
            (store, Some(e.to_string()))
        }
    }
}

fn seed(store: &mut Store, legacy: Option<&Path>) -> Result<(), Box<dyn Error>> {
    if let Some(file) = legacy.filter(|f| f.exists()) {
        let read = fs::read_to_string(file)
            .map_err(|e| e.to_string())
            .and_then(|t| text::parse(&t).map_err(|e| e.to_string()));
        match read {
            Ok((config, notes)) => {
                // Older files carry copies of the bundled help as comments;
                // they are not the person's notes.
                let notes = notes.without_lines_in(config::DEFAULT);
                store.import("Main", &config, &notes, true)?;
                let kept = file.with_extension("toml.imported");
                match fs::rename(file, &kept) {
                    Ok(()) => eprintln!(
                        "nexusd: imported {} as the layout \"Main\"; the file is now {}",
                        file.display(),
                        kept.display()
                    ),
                    Err(e) => eprintln!(
                        "nexusd: imported {} as the layout \"Main\" but could not rename it ({e}); it is no longer read",
                        file.display()
                    ),
                }
                return Ok(());
            }
            Err(e) => eprintln!(
                "nexusd: {} could not be imported ({e}); starting from the default layout and leaving the file as it is",
                file.display()
            ),
        }
    }
    let (config, notes) = text::parse(config::DEFAULT)?;
    store.import("Default", &config, &notes, true)?;
    Ok(())
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
