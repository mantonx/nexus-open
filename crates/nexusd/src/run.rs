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
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender, SyncSender};
use std::thread;
use std::time::{Duration, Instant};

use jiff::Zoned;
use nexus_control::{
    ArtReply, EditRequest, Export, Layout, LayoutEntry, PageTarget, Request, SignalEntry, Status,
};
use nexus_core::art::Art;
use nexus_core::config;
use nexus_core::demo;
use nexus_core::engine::{Effect, Engine, Input};
use nexus_core::signal::Value;
use nexus_device::protocol::{TouchCalibration, parse_touch};
use nexus_device::{DeviceError, Surface};
use nexus_sources::Reading;
use nexus_store::{Edit, Store, text};
use serde_json::json;

use crate::control::{self, Frame, Reply};
use crate::notify::Notifier;

/// How often to look for the device while it is unplugged.
const RECONNECT_POLL: Duration = Duration::from_millis(250);
/// How often to look while another program has the strip.
const HELD_POLL: Duration = Duration::from_secs(1);
/// How long the device must stay unopenable before why is logged. After a
/// resume the strip comes back within about 3 s, and on the way its node
/// is briefly missing and then briefly without the seat's access, which
/// is not worth telling anyone to install a udev rule over.
const DEVICE_ERROR_GRACE: Duration = Duration::from_secs(10);
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
    /// Never open the strip; frames go to viewers only.
    pub no_device: bool,
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
        failing_since: None,
        logged_device_error: None,
        tx: tx.clone(),
        viewers: Vec::new(),
        current: None,
        calibration: TouchCalibration::default(),
        origin: Instant::now(),
        last_frame: Instant::now(),
        weather: None,
        news: None,
        media: None,
        effects: crate::actions::Effects::new(),
        sources,
        notifier: Notifier::from_env(),
    };
    if options.no_device {
        // Put the next attempt out of reach, as for an OS without a backend,
        // so the loop neither opens the strip nor wakes to retry.
        daemon.next_attempt = Instant::now() + Duration::from_secs(86_400 * 365);
        daemon.last_device_error = Some("the strip is not used: started with --no-device".into());
    }
    daemon.sources.apply(&daemon.engine.config().sources);
    daemon.tell_layouts();
    if options.demo {
        spawn_demo(publisher(tx));
    } else {
        nexus_sources::spawn_machine(publisher(tx.clone()));
        daemon.media = Some(nexus_sources::spawn_media(
            publisher(tx.clone()),
            art_sink(tx.clone()),
            daemon.engine.config().media.album_art,
        ));
        nexus_sources::spawn_volume(publisher(tx.clone()));
        let weather = nexus_sources::spawn_weather(publisher(tx.clone()));
        let _ = weather.send(daemon.engine.config().weather.clone());
        daemon.weather = Some(weather);
        // Fetches nothing until the layout names a feed.
        let news = nexus_sources::spawn_news(publisher(tx));
        news.configure(daemon.engine.config().news.clone());
        daemon.effects.set_news(news.clone());
        daemon.news = Some(news);
    }
    daemon.notifier.ready();
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
    /// Why the device cannot be opened right now, for `status`.
    last_device_error: Option<String>,
    /// When opening it started failing.
    failing_since: Option<Instant>,
    /// The reason last logged: each is logged once, not on every retry.
    logged_device_error: Option<String>,
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
    /// Tells the news source which feeds to read; `None` in demo mode.
    news: Option<nexus_sources::news::News>,
    /// Tells the media source where artwork may come from; `None` in demo
    /// mode.
    media: Option<nexus_sources::Media>,
    /// The config's external source programs.
    sources: crate::sources::Sources,
    /// systemd's readiness and watchdog. Pinged from this thread, since it
    /// is the one that owns the device: a hang here freezes the strip.
    notifier: Notifier,
}

impl Daemon {
    fn run(&mut self, rx: &Receiver<Msg>) -> Result<(), Box<dyn Error>> {
        loop {
            self.notifier.alive();
            if self.device.is_none() && Instant::now() >= self.next_attempt {
                self.connect();
            }
            // Timers fire on time even with no device or viewer to draw
            // for: a long press held still, the alarm clearing itself.
            self.engine.tick(self.origin.elapsed().as_secs_f64());
            self.carry_out();
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
            // An idle strip would otherwise sleep until a message comes.
            if let Some(max) = self.notifier.max_wait() {
                wait = Some(wait.map_or(max, |w| w.min(max)));
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

            // The engine's clock, for readings: they carry no time, and the
            // alarm's clearing and re-arming are timed from them.
            self.engine.tick(self.origin.elapsed().as_secs_f64());
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
            self.carry_out();
        }
    }

    /// Carries out what the engine asked for. A layout switch is done here,
    /// since this thread owns the store; the rest by `Effects`.
    fn carry_out(&mut self) {
        for effect in self.engine.take_effects() {
            match effect {
                Effect::SwitchLayout(name) => self.switch_layout(&name),
                other => self.effects.carry_out(other),
            }
        }
    }

    /// Puts a layout on the strip, chosen with the layout picker. It is
    /// the same edit the settings window sends (`Edit::Activate`), through
    /// the same request, so it is undoable as "Switch layout", the store's
    /// revision moves and the window follows, and the engine is told.
    fn switch_layout(&mut self, name: &str) {
        let id = match self.store.layout_named(name) {
            Ok(Some(id)) => id,
            Ok(None) => {
                eprintln!(
                    "nexusd: no layout called \"{name}\" to switch to; it may have just been renamed"
                );
                return;
            }
            Err(e) => {
                eprintln!("nexusd: could not switch to the layout \"{name}\": {e}");
                return;
            }
        };
        let edit = Request::Edit(Box::new(EditRequest {
            edit: Edit::Activate { layout: id },
            base: None,
        }));
        if let Err(e) = self.control(edit) {
            eprintln!("nexusd: could not switch to the layout \"{name}\": {e}");
        }
    }

    /// Tells the engine the store's layouts and which is on the strip, for
    /// the layout picker.
    fn tell_layouts(&mut self) {
        let layouts = match self.store.layouts() {
            Ok(layouts) => layouts,
            Err(e) => {
                eprintln!("nexusd: could not list the layouts: {e}");
                Vec::new()
            }
        };
        let active = self.store.active_layout().ok();
        let current = layouts.iter().position(|(id, _)| Some(*id) == active);
        let names = layouts.into_iter().map(|(_, name)| name).collect();
        self.engine.handle(Input::Layouts { names, current });
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
                self.failing_since = None;
                self.logged_device_error = None;
                self.engine.handle(Input::Redraw);
            }
            Err(e) => {
                let message = e.to_string();
                let now = Instant::now();
                let since = *self.failing_since.get_or_insert(now);
                if worth_logging(since, now, &message, self.logged_device_error.as_deref()) {
                    eprintln!("nexusd: {message}");
                    self.logged_device_error = Some(message.clone());
                }
                self.last_device_error = Some(message);
                self.next_attempt = match e {
                    // Retrying cannot help on an OS without a backend.
                    DeviceError::Unsupported => Instant::now() + Duration::from_secs(86_400),
                    // Taken by another program: only the user ends that, by
                    // quitting it and replugging, and each attempt walks
                    // /proc looking for the holder.
                    DeviceError::InUse(_) | DeviceError::Detached => Instant::now() + HELD_POLL,
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
                    alarm: self.engine.alarm().map(|a| a.label),
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
            Request::ShowTakeover { name, seconds } => {
                let known = self
                    .engine
                    .config()
                    .takeovers
                    .iter()
                    .any(|t| t.name == name);
                if !known {
                    return Err(format!("no takeover named \"{name}\""));
                }
                self.engine.handle(Input::ShowTakeover { name, seconds });
                Ok(json!(null))
            }
            Request::EndTakeover => {
                self.engine.handle(Input::EndTakeover);
                Ok(json!(null))
            }
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
                    layouts: store
                        .layouts()
                        .map_err(|e| e.to_string())?
                        .into_iter()
                        .map(|(id, name)| LayoutEntry { id, name })
                        .collect(),
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
                } else {
                    self.tell_layouts();
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
            Request::HoldDrag { from, to } => {
                // Still at `from` past the long press, a 0.4 s slide to
                // `to`, and a rest there so the picker catches up with the
                // finger before it lifts; 10 ms apart throughout.
                let start = self.origin.elapsed().as_secs_f64();
                let (hold, slide, rest): (u32, u32, u32) = (60, 40, 30);
                for i in 0..=hold + slide + rest {
                    let k = (i.saturating_sub(hold) as f32 / slide as f32).min(1.0);
                    self.engine.handle(Input::Touch {
                        down: true,
                        x: from + (to - from) * k,
                        at: start + f64::from(i) * 0.01,
                    });
                }
                self.engine.handle(Input::Touch {
                    down: false,
                    x: 0.0,
                    at: start + f64::from(hold + slide + rest + 1) * 0.01,
                });
                Ok(json!(null))
            }
            // Handled by the connection thread before it gets here.
            Request::Frames => Err("frames is handled by the connection".into()),
            Request::Art => {
                let art = self.engine.art().map(|a| ArtReply::new(a));
                serde_json::to_value(art).map_err(|e| e.to_string())
            }
        }
    }

    fn lost(&mut self, why: &str) {
        if self.device.take().is_some() {
            eprintln!("nexusd: device lost ({why}); waiting for it to return");
        }
        self.next_attempt = Instant::now();
    }

    /// Shows the store's layout on the strip: the engine, the weather and
    /// news sources and the external sources all follow it.
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
        if config.news != self.engine.config().news
            && let Some(news) = &self.news
        {
            news.configure(config.news.clone());
        }
        if config.media != self.engine.config().media
            && let Some(media) = &self.media
        {
            media.set_album_art(config.media.album_art);
        }
        self.sources.apply(&config.sources);
        if &config != self.engine.config() {
            self.engine.handle(Input::Config(Box::new(config)));
        }
        // Layouts may have been added, renamed, deleted or switched.
        self.tell_layouts();
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

/// Hands the media source's artwork to the daemon thread, as `publisher`
/// does readings.
fn art_sink(tx: Sender<Msg>) -> impl FnMut(Arc<Art>) -> bool + Send + 'static {
    move |art| tx.send(Msg::Engine(Input::Art(art))).is_ok()
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

/// Whether a device error is worth logging: it has lasted the grace
/// period, and it is not the reason already logged.
fn worth_logging(since: Instant, now: Instant, message: &str, logged: Option<&str>) -> bool {
    now.saturating_duration_since(since) >= DEVICE_ERROR_GRACE && logged != Some(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_device_error_is_logged_once_and_only_if_it_lasts() {
        let start = Instant::now();
        let soon = start + Duration::from_secs(3);
        let later = start + DEVICE_ERROR_GRACE;
        // The reconnect after a resume: never logged.
        assert!(!worth_logging(start, soon, "no permission", None));
        // A real problem: logged once it lasts, then not again.
        assert!(worth_logging(start, later, "no permission", None));
        assert!(!worth_logging(
            start,
            later,
            "no permission",
            Some("no permission")
        ));
        // A new reason is logged.
        assert!(worth_logging(
            start,
            later,
            "not found",
            Some("no permission")
        ));
    }
}
