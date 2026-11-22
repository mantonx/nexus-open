//! The engine: the single owner of everything the strip shows.
//!
//! Inputs arrive one at a time through `handle`: signal readings, touches,
//! page commands, config reloads. The engine only records what changed. The
//! caller then asks for a `frame`, which is `Some` only when the picture
//! would differ from the last one sent, and for `next_wait`, how long until
//! the picture changes on its own (an animation step, the clock's next
//! minute, a long press reaching its threshold, the alarm clearing).
//! Between those the daemon sleeps: there is no frame loop while nothing
//! changes. It calls `tick` whenever it wakes, so timers fire on time.
//!
//! A takeover is a row of widgets that covers the page, or one end of it,
//! for a few seconds when one of its signals changes. Touches are routed by
//! where they land: on the takeover to its widgets, beside a partial one to
//! the page, so drawing, taps and sliders work on either unchanged.
//!
//! The critical alarm covers the whole strip when a reading on any page
//! rises to its `crit`, above every takeover, until it is acknowledged
//! (its OK key, or a swipe) or the reading has been back under for
//! `ALARM_CLEARS` seconds.
//!
//! Frames are finished here: page markers along the top, takeovers fading
//! in and out, the strip dimmed when idle or at night (never a warning or
//! critical card), and the alarm's slow breathing. `animating` says when
//! frames are due at the display's pace: a page springing, a long press
//! filling, a takeover fading, a title scrolling.
//!
//! The engine never reads a clock or touches a thread. Wall time (for the
//! clock widget) and monotonic time (for touch, animation and the alarm's
//! timers, in seconds from any origin) are passed in, so tests can drive it
//! to any moment.

use std::collections::HashMap;
use std::ops::Range;
use std::time::Duration;

use jiff::Zoned;

use crate::canvas::Canvas;
use crate::config::{
    Action, Align, Config, MediaCommand, NewsCommand, SLIDER_VALUE, SliderControl, Widget,
};
use crate::fonts::FontChoice;
use crate::gesture::{Gesture, LONG_PRESS, MIN_TAP, Recognizer, should_commit};
use crate::icons;
use crate::layout::{
    Bind, WEATHER_STALE, bind_page, format_number, judge, media, news_live, reading_text,
    shows_clock, signals_used, slider, thresholds,
};
use crate::signal::{Signals, Unit, Value};
use crate::theme::{self, Theme};
use crate::widget::{
    AlarmView, Marker, Renderer, Severity, Touch, WidgetView, Zone, alarm_ok_span, media_keys,
};

const WIDTH: f32 = crate::WIDTH as f32;

/// Page-snap spring, from the Go app: slightly under-damped (critical
/// damping would be 2·√400 = 40) for a crisp stop with no visible bounce.
const STIFFNESS: f32 = 400.0;
const DAMPING: f32 = 36.0;

/// How far a finger on a slider moves before the value follows it, in
/// pixels: the jitter of a finger resting on it never sets anything.
const SLIDER_SLOP: f32 = 4.0;

/// A partial takeover's width moves to the nearest card edge of the page
/// under it when that is within this share of its width.
const TAKEOVER_SNAP: f32 = 0.4;

/// A snap never leaves a partial takeover narrower than this (or than the
/// width it was given, if that is less): a slider in it keeps a track
/// long enough to set (about 125 px at "100%").
const TAKEOVER_MIN: u32 = 320;

/// The alarm ends by itself once its reading has been back under the
/// threshold this many seconds.
pub const ALARM_CLEARS: f64 = 10.0;
/// After its alarm ends, a reading raises it again only once it has stayed
/// under the threshold this many seconds without a break, so a reading
/// hovering at the threshold does not raise it over and over.
pub const ALARM_REARMS: f64 = 30.0;
/// The alarm's OK key on the strip: the key drawn, and on to the strip's
/// right edge, so a finger that lands just past it still counts.
pub fn alarm_ok() -> Range<u32> {
    alarm_ok_span().start..crate::WIDTH
}

/// How often the alarm's breathing is redrawn, in seconds: slow enough
/// that the strip is not kept busy, smooth enough at a 2.4 s breath.
const BREATHE_STEP: f64 = 0.08;

/// Something that happened.
#[derive(Clone, Debug)]
pub enum Input {
    /// A source published a reading.
    Signal(String, Value),
    /// A signal's source is gone: its widgets show "—" until it is
    /// published again, which then counts as a first reading.
    Forget(String),
    /// A touch sample: finger down or not, panel x (0–639), and monotonic
    /// time in seconds.
    Touch { down: bool, x: f32, at: f64 },
    /// Show the page with this index. Out-of-range indexes are ignored.
    ShowPage(usize),
    /// Show the next page, or the previous one with -1. Stops at the ends.
    Step(i32),
    /// A new, already-validated config replaces the current one.
    Config(Box<Config>),
    /// Show the takeover with this name for `seconds` instead of its own
    /// time, as the settings window does while it is being edited. Sent
    /// again before it ends, it starts the time over. Unknown names are
    /// ignored, and so is any request while the alarm is up.
    ShowTakeover { name: String, seconds: f64 },
    /// Put the page back if a takeover covers it.
    EndTakeover,
    /// Acknowledge the alarm, as its OK key does: every reading critical
    /// now is acknowledged.
    Acknowledge,
    /// The device came back; it needs the whole picture again.
    Redraw,
}

/// Something for the caller to do. The engine never touches processes,
/// D-Bus or audio itself, which keeps it testable without any of them.
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    /// A tap or hold action other than a page change or a publish.
    Act(Action),
    /// Set the output volume, 0–100.
    Volume(u8),
    /// Move the shown track to this many seconds in.
    Seek(f64),
    /// Run a slider's command, `{value}` already filled in.
    Run(Vec<String>),
    /// Open a web address a source gave (the news article), in the
    /// default browser. It comes from a feed, so it is untrusted: the
    /// caller opens it only if it is a plain `http(s)` address, and never
    /// through a shell.
    OpenLink(String),
}

/// The critical alarm, as the renderer needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct Alarm {
    /// The reading it names: the most recent to go critical.
    pub signal: String,
    /// The label of the value widget that shows it.
    pub label: String,
    /// The reading now, formatted as its widget shows it: "92°".
    pub value: String,
    /// The critical limit it is over, formatted the same way: "90°".
    pub limit: String,
    /// The widget's icon.
    pub icon: char,
    /// It is a temperature: "too hot", and the fans are worth a look.
    pub hot: bool,
    /// The reading is back under its limit; the alarm stays until it has
    /// been for `ALARM_CLEARS` seconds, or OK.
    pub under: bool,
    /// How many readings it holds, this one included. OK acknowledges all.
    pub count: usize,
    /// The OK key's pixels on the strip: a tap there acknowledges.
    pub ok: Range<u32>,
}

impl Alarm {
    /// The alarm in words, as the strip shows it.
    pub fn view(&self) -> AlarmView {
        let label = if self.label.is_empty() {
            "A reading"
        } else {
            &self.label
        };
        let (headline, detail) = match (self.hot, self.under) {
            (true, false) => (
                format!("{label} is too hot"),
                format!("Over its {} limit · check the fans", self.limit),
            ),
            (false, false) => (
                format!("{label} is too high"),
                format!("Over its {} limit", self.limit),
            ),
            // The value shown is the reading now, so the words must not
            // say it is over.
            (true, true) => (
                format!("{label} was too hot"),
                format!("Back under its {} limit · check the fans", self.limit),
            ),
            (false, true) => (
                format!("{label} was too high"),
                format!("Back under its {} limit", self.limit),
            ),
        };
        AlarmView {
            icon: self.icon,
            headline,
            detail,
            value: self.value.clone(),
        }
    }
}

/// A long press on its way: the zone under the finger, whose hold action
/// fires when `progress` reaches 1.
#[derive(Clone, Debug, PartialEq)]
pub struct Hold {
    /// The zone's pixels on the strip.
    pub zone: Range<u32>,
    /// 0 when the finger lands, 1 at the threshold.
    pub progress: f32,
}

/// A row of widgets that can fill the strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Row {
    Page(usize),
    Takeover(usize),
}

/// What a finger rests on, drawn lit: a zone whose tap would act (or one
/// of its parts: a media widget's transport key), or the alarm's OK key.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Lit {
    Zone(Row, usize, Option<usize>),
    AlarmOk,
}

/// A takeover fading in or out, from monotonic time `start`.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Fade {
    index: usize,
    start: f64,
    out: bool,
    /// The takeover's `Cover::from`, kept while it fades out.
    from: Option<f32>,
}

impl Fade {
    fn seconds(&self) -> f64 {
        let ms = if self.out {
            theme::MS_TAKEOVER_OUT
        } else {
            theme::MS_TAKEOVER_IN
        };
        f64::from(ms) / 1000.0
    }

    /// How far it has come, 0–1, at `now`; eased out.
    fn progress(&self, now: f64) -> f32 {
        let t = ((now - self.start) / self.seconds()).clamp(0.0, 1.0) as f32;
        1.0 - (1.0 - t) * (1.0 - t)
    }

    fn done(&self, now: f64) -> bool {
        now - self.start >= self.seconds()
    }
}

/// A takeover on the strip.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cover {
    index: usize,
    /// Monotonic time it ends. `None` until the next frame, since a signal
    /// arrives without a time.
    until: Option<f64>,
    /// How long it lasts, when not its own `seconds`: shown on request.
    lease: Option<f64>,
    /// The volume before the change that brought it up, 0–1: its slider
    /// shows the change from there.
    from: Option<f32>,
}

/// A finger on a slider. The value moves by the finger's travel from what
/// it was when the finger landed: landing sets nothing.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Slide {
    row: Row,
    zone: usize,
    /// When the finger landed: until `MIN_TAP` has passed it might be a
    /// lift bounce, so nothing is set yet.
    start: f64,
    /// Where it landed.
    x0: f32,
    /// The slider's value when it landed, 0–1.
    from: f32,
    /// The value now, 0–1: `from` moved by the travel along the bar.
    fraction: f32,
    /// The finger has travelled past `SLIDER_SLOP`; until then the value
    /// stays where it was.
    moved: bool,
    /// The last whole percent sent, so a drag sends each value once.
    sent: Option<u8>,
}

/// How the page row is moving. `offset` is how far the current page is
/// shifted right in pixels; a neighbour fills the gap it leaves.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Motion {
    Still,
    /// Following the finger.
    Drag {
        offset: f32,
    },
    /// Springing to `target`: 0 (stay), or ±WIDTH (the neighbour).
    Settle {
        offset: f32,
        velocity: f32,
        target: f32,
    },
}

/// A reading with a critical threshold on one of the layout's pages.
#[derive(Clone, Debug, PartialEq)]
struct Watch {
    /// The lowest `crit` any widget gives it.
    crit: f64,
    /// That widget's label and icon, for the alarm.
    label: String,
    icon: char,
    /// The last reading was at or above `crit`.
    critical: bool,
    /// When the reading went under `crit`, or first came in under it.
    /// `None` while critical, and before any reading.
    below_since: Option<f64>,
    /// A rise to `crit` raises the alarm. Off once it has, until the
    /// reading stays under for `ALARM_REARMS`.
    armed: bool,
}

pub struct Engine {
    config: Config,
    signals: Signals,
    page: usize,
    theme: Theme,
    renderer: Renderer,
    font: FontChoice,
    gestures: Recognizer,
    motion: Motion,
    /// Pages rendered once at the start of a swipe and slid around until
    /// it ends, so a swipe never waits on a render.
    snapshots: HashMap<usize, Canvas>,
    /// Monotonic time of the last animation step.
    stepped_at: Option<f64>,
    /// The latest monotonic time seen, from `tick`, a frame or a touch.
    now: f64,
    /// The picture must be redrawn on the next `frame` call.
    dirty: bool,
    /// The minute last drawn, for pages with a clock.
    drawn_minute: Option<i64>,
    /// What a finger rests on, if a tap there would act. Drawn lit.
    pressed: Option<Lit>,
    /// What only the caller can carry out, collected by `take_effects`.
    effects: Vec<Effect>,
    /// A finger on a slider. While set, touches go to the slider and not
    /// to the gesture recognizer, so a slider's drag is never a swipe.
    slide: Option<Slide>,
    /// The last value of each `run` slider with no signal to show, by
    /// (row, zone).
    last_set: HashMap<(Row, usize), f32>,
    /// The takeover covering the page, if any.
    cover: Option<Cover>,
    /// Where the finger on the strip landed, until it lifts.
    touch_start: Option<f32>,
    /// The rest of this touch, up to the lift, does nothing: a swipe
    /// dismissed something, a hold fired, or the strip changed under it.
    ignoring: bool,
    /// Readings with a critical threshold, by signal.
    watches: HashMap<String, Watch>,
    /// The readings the alarm holds, oldest first; it shows the last.
    /// Empty while there is no alarm.
    alarm: Vec<String>,
    /// When the alarm was last drawn, for its breathing.
    breathed_at: Option<f64>,
    /// A takeover fading in or out.
    fade: Option<Fade>,
    /// When the track's title last changed: it scrolls once if it is too
    /// long to fit. Cleared once the scroll is over.
    marquee_at: Option<f64>,
    /// The last touch, or the first frame: idle dimming counts from here.
    active_at: Option<f64>,
    /// The brightness the last frame was drawn at.
    drawn_dim: f32,
    /// The last frame was drawn with the weather stale.
    drawn_stale: bool,
    /// The news article showing when the finger on the strip landed: what
    /// a `news = "open"` action opens.
    landing_link: Option<String>,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        let font = config.settings.font;
        let theme = Theme::new(config.settings.accent);
        Self {
            renderer: Renderer::new(theme.clone(), font.load()),
            theme,
            font,
            watches: watches(&config, &HashMap::new()),
            config,
            signals: Signals::new(),
            page: 0,
            gestures: Recognizer::new(),
            motion: Motion::Still,
            snapshots: HashMap::new(),
            stepped_at: None,
            now: 0.0,
            dirty: true,
            drawn_minute: None,
            pressed: None,
            effects: Vec::new(),
            slide: None,
            last_set: HashMap::new(),
            cover: None,
            touch_start: None,
            ignoring: false,
            alarm: Vec::new(),
            breathed_at: None,
            fade: None,
            marquee_at: None,
            active_at: None,
            drawn_dim: 1.0,
            drawn_stale: false,
            landing_link: None,
        }
    }

    /// What taps, holds and slides asked for since the last call, for the
    /// caller to carry out. Page changes and publishes are already done.
    pub fn take_effects(&mut self) -> Vec<Effect> {
        std::mem::take(&mut self.effects)
    }

    pub fn config(&self) -> &Config {
        &self.config
    }

    pub fn signals(&self) -> &Signals {
        &self.signals
    }

    /// The theme frames are drawn with: the default one with the user's
    /// accent colour, if they chose one.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Index of the page on the strip.
    pub fn page(&self) -> usize {
        self.page
    }

    /// How many pages the layout has, for the page markers.
    pub fn page_count(&self) -> usize {
        self.config.pages.len()
    }

    /// The worst a reading on page `index` stands against its thresholds,
    /// acknowledged or not: `Crit`, `Warn` or `Ok`. For the page markers.
    pub fn page_severity(&self, index: usize) -> Severity {
        self.config.pages.get(index).map_or(Severity::Ok, |p| {
            thresholds(&p.widgets)
                .map(|(_, signal, warn, crit)| judge(&self.signals, signal, warn, crit))
                .max()
                .unwrap_or_default()
        })
    }

    /// `page_severity` of every page, in order.
    pub fn page_severities(&self) -> Vec<Severity> {
        (0..self.page_count())
            .map(|i| self.page_severity(i))
            .collect()
    }

    /// The name of the takeover on the strip, if one is. While the alarm
    /// is up there is none.
    pub fn takeover(&self) -> Option<&str> {
        self.cover
            .map(|c| self.config.takeovers[c.index].name.as_str())
    }

    /// The alarm, while it is up.
    pub fn alarm(&self) -> Option<Alarm> {
        let signal = self.alarm.last()?;
        let watch = self.watches.get(signal)?;
        let (value, unit) = reading_text(&self.signals, signal, &self.config.settings);
        let base = match self.signals.get(signal) {
            Some(Value::Number(_, u)) => *u,
            _ => Unit::None,
        };
        let (limit, limit_unit) = format_number(watch.crit, base, self.config.settings.temperature);
        Some(Alarm {
            signal: signal.clone(),
            label: watch.label.clone(),
            value: value + &unit,
            limit: limit + &limit_unit,
            icon: watch.icon,
            hot: base == Unit::Celsius,
            under: !watch.critical,
            count: self.alarm.len(),
            ok: alarm_ok(),
        })
    }

    /// The long press under the finger, while one is on its way to firing
    /// an action. Measured at the latest time the engine has seen.
    pub fn hold(&self) -> Option<Hold> {
        if self.ignoring || !self.alarm.is_empty() || self.motion != Motion::Still {
            return None;
        }
        let (x, start) = self.gestures.holding()?;
        let (row, zone) = self.hit(x)?;
        // A transport key's long press is a slow tap, with nothing to fill.
        if self.part_at(row, zone, x).is_some_and(|(_, key)| key) {
            return None;
        }
        self.hold_of(&self.widgets(row)[zone])?;
        Some(Hold {
            zone: self.zone_span(row, zone),
            progress: ((self.now - start) / LONG_PRESS).clamp(0.0, 1.0) as f32,
        })
    }

    /// True while frames are due as fast as the device takes them: a page
    /// springing into place, a long press filling, a takeover fading in or
    /// out, a long title scrolling.
    pub fn animating(&self) -> bool {
        matches!(self.motion, Motion::Settle { .. })
            || self.hold().is_some()
            || self.fade.is_some()
            || (self.marquee_at.is_some() && self.alarm.is_empty())
    }

    /// The page markers as the strip shows them.
    pub fn markers(&self) -> Vec<Marker> {
        (0..self.page_count())
            .map(|i| Marker {
                current: i == self.page,
                severity: self.page_severity(i),
            })
            .collect()
    }

    /// How bright the strip is drawn, 0–1: dimmed at night (the layout's
    /// `night` hours) and after `IDLE_DIM_AFTER_S` with no touch, unless a
    /// reading on some page is at warning or worse, or something has come
    /// up over the page.
    pub fn brightness(&self, now: &Zoned) -> f32 {
        if !self.alarm.is_empty() || self.cover.is_some() {
            return 1.0;
        }
        let settings = &self.config.settings;
        let mut k: f32 = 1.0;
        if let Some(night) = &settings.night {
            let minute = i32::from(now.hour()) * 60 + i32::from(now.minute());
            if night.contains(minute as u32) {
                k = k.min(theme::NIGHT_DIM);
            }
        }
        let idle = self
            .active_at
            .is_some_and(|t| self.now - t >= f64::from(theme::IDLE_DIM_AFTER_S));
        let calm = self.page_severities().iter().all(|s| *s == Severity::Ok);
        if settings.idle_dim && idle && calm {
            k = k.min(theme::IDLE_DIM);
        }
        k
    }

    /// Moves the engine's clock to `mono` (monotonic seconds, the touch
    /// clock) and fires what is due by then: a long press held to its
    /// threshold, an alarm whose reading has been back under long enough.
    /// `frame` and touches do this too; the daemon calls it on every wake.
    pub fn tick(&mut self, mono: f64) {
        self.now = self.now.max(mono);
        self.expire_alarm();
        if let Some(gesture) = self.gestures.tick(self.now) {
            self.gesture(gesture);
        }
    }

    pub fn handle(&mut self, input: Input) {
        match input {
            Input::Signal(name, value) => {
                self.expire_alarm();
                let visible = self.visible(&name);
                let on_page =
                    signals_used(&self.config.pages[self.page].widgets).any(|s| s == name);
                // A change, not a first reading: the daemon starting up,
                // or a source coming back, is not news.
                let old = self.signals.get(&name).cloned();
                let back = self.signals.lost_at(&name).is_some();
                let changed = !back && old.as_ref().is_some_and(|old| *old != value);
                let blank = matches!(&value, Value::Text(t) if t.trim().is_empty());
                let new_title = name == "media.title" && !blank && old.as_ref() != Some(&value);
                self.signals.set_at(&name, value, self.now);
                self.dirty |= visible;
                self.watch(&name);
                if changed && !blank && !on_page {
                    self.take_over(&name, old.as_ref());
                }
                // A new title scrolls once, if it is too long to fit.
                if new_title && self.visible(&name) {
                    self.marquee_at = Some(self.now);
                }
            }
            Input::Forget(name) => {
                let visible = self.visible(&name);
                self.dirty |= visible;
                self.signals.forget(&name, self.now);
                // No data is not critical; it comes back as a first reading.
                if let Some(w) = self.watches.get_mut(&name) {
                    w.critical = false;
                    w.below_since = None;
                }
                if self.alarm.contains(&name) {
                    self.alarm.retain(|s| *s != name);
                    self.alarm_changed();
                }
            }
            Input::Touch { down, x, at } => self.touch(down, x, at),
            Input::ShowPage(i) => self.go_to(i),
            Input::Step(delta) => {
                let last = self.config.pages.len() - 1;
                let target = self.page.saturating_add_signed(delta as isize).min(last);
                self.go_to(target);
            }
            Input::Config(config) => self.reload(*config),
            Input::ShowTakeover { name, seconds } => {
                let Some(index) = self.config.takeovers.iter().position(|t| t.name == name) else {
                    return;
                };
                let busy =
                    self.gestures.pressed() || self.slide.is_some() || self.motion != Motion::Still;
                let same = self.cover.is_some_and(|c| c.index == index);
                if (busy && !same) || !self.alarm.is_empty() {
                    return;
                }
                self.cover = Some(Cover {
                    index,
                    until: None,
                    lease: Some(seconds),
                    from: None,
                });
                if !same {
                    self.fade_in(index);
                }
            }
            Input::EndTakeover => self.fade_out(),
            Input::Acknowledge => self.acknowledge(),
            Input::Redraw => self.dirty = true,
        }
    }

    /// The new frame, if the picture changed since the last one. `mono` is
    /// monotonic time in seconds, on the same clock as touch samples.
    pub fn frame(&mut self, now: &Zoned, mono: f64) -> Option<Canvas> {
        self.tick(mono);
        self.active_at.get_or_insert(self.now);
        if self.motion != Motion::Still {
            self.step(mono);
        }
        if let Some(offset) = self.offset() {
            return Some(self.slide(offset, now));
        }
        if let Some(alarm) = self.alarm() {
            let breathe_due = self.breathed_at.is_none_or(|t| mono - t >= BREATHE_STEP);
            if !self.dirty && !breathe_due {
                return None;
            }
            self.dirty = false;
            self.drawn_minute = None;
            self.breathed_at = Some(mono);
            let lit = self.pressed == Some(Lit::AlarmOk);
            return Some(
                self.renderer
                    .render_alarm(&alarm.view(), lit, breathe(mono)),
            );
        }
        if self.breathed_at.take().is_some() {
            self.dirty = true;
        }
        if let Some(cover) = self.cover {
            let seconds = self.cover_seconds(&cover);
            let until = *self
                .cover
                .as_mut()
                .expect("just matched")
                .until
                .get_or_insert(mono + seconds);
            // Never pulled from under a finger.
            if mono >= until && !self.gestures.pressed() && self.slide.is_none() {
                self.fade_out();
            }
        }
        if self.fade.is_some_and(|f| f.done(self.now)) {
            self.fade = None;
            self.dirty = true;
        }
        let stale = self.weather_stale();
        if stale != self.drawn_stale {
            self.drawn_stale = stale;
            self.dirty |= self.visible("weather.temp");
        }
        let minute = (self.clock_shown() || self.config.settings.night.is_some())
            .then(|| now.timestamp().as_second().div_euclid(60));
        let dim = self.brightness(now);
        if !self.dirty && !self.animating() && minute == self.drawn_minute && dim == self.drawn_dim
        {
            return None;
        }
        let canvas = self.compose(now, dim);
        self.dirty = false;
        self.drawn_minute = minute;
        self.drawn_dim = dim;
        Some(canvas)
    }

    /// The strip's picture now: the page, the takeover over it (fading in
    /// or out), dimmed as `dim` says, with the page markers on top.
    fn compose(&mut self, now: &Zoned, dim: f32) -> Canvas {
        let marquee = self.marquee_at.map(|t| (self.now - t) as f32);
        let mut moving = false;
        let mut canvas = match self.cover {
            Some(c) => {
                let over = Row::Takeover(c.index);
                let (left, width) = self.span(over);
                let top = self.render_row(over, now, marquee);
                moving |= self.renderer.moving();
                let fading = self.fade.filter(|f| !f.out && f.index == c.index);
                if width == crate::WIDTH && fading.is_none() {
                    top
                } else {
                    let mut page = self.render_row(Row::Page(self.page), now, marquee);
                    moving |= self.renderer.moving();
                    match fading {
                        Some(f) => {
                            let t = f.progress(self.now);
                            let rise = ((1.0 - t) * theme::TAKEOVER_RISE).round() as i32;
                            page.blend_from(&top, left as i32, rise, t);
                        }
                        None => page.copy_shifted(&top, left as i32),
                    }
                    page
                }
            }
            None => {
                let mut page = self.render_row(Row::Page(self.page), now, marquee);
                moving |= self.renderer.moving();
                let alerts = self.renderer.alerts().to_vec();
                if let Some(f) = self.fade.filter(|f| f.out) {
                    let row = Row::Takeover(f.index);
                    let left = self.span(row).0;
                    let top = self.render_row(row, now, None);
                    page.blend_from(&top, left as i32, 0, 1.0 - f.progress(self.now));
                }
                page.dim(dim, &alerts);
                page
            }
        };
        let markers = self.markers();
        self.renderer.draw_markers(&mut canvas, &markers, dim);
        if !moving {
            self.marquee_at = None;
        }
        canvas
    }

    /// The weather shown is old: no new reading for `WEATHER_STALE`.
    fn weather_stale(&self) -> bool {
        self.signals
            .heard_at("weather.temp")
            .is_some_and(|t| self.now - t > WEATHER_STALE)
    }

    /// How long until the picture changes, or a timer is due, without any
    /// input: no time at all while animating, otherwise the soonest of a
    /// takeover ending, a long press reaching its threshold, the alarm
    /// clearing and the next minute on a clock, or never. `mono` is as for
    /// `frame`.
    pub fn next_wait(&self, now: &Zoned, mono: f64) -> Option<Duration> {
        if self.animating() {
            return Some(Duration::ZERO);
        }
        let after = |at: f64| Duration::from_secs_f64((at - mono).max(0.0));
        let mut soonest: Option<Duration> = None;
        let mut sooner = |d: Duration| soonest = Some(soonest.map_or(d, |s: Duration| s.min(d)));
        if !self.alarm.is_empty() {
            let at = self.breathed_at.map_or(mono, |t| t + BREATHE_STEP);
            sooner(after(at));
        }
        if self.config.settings.idle_dim
            && let Some(t) = self.active_at
        {
            let idle_at = t + f64::from(theme::IDLE_DIM_AFTER_S);
            if idle_at > mono {
                sooner(after(idle_at));
            }
        }
        if let Some(t) = self.signals.heard_at("weather.temp")
            && t + WEATHER_STALE >= mono
            && self.visible("weather.temp")
        {
            sooner(after(t + WEATHER_STALE + 0.001));
        }
        if self.clock_shown() || self.config.settings.night.is_some() {
            let ms = now.timestamp().as_millisecond().rem_euclid(60_000);
            sooner(Duration::from_millis((60_000 - ms) as u64));
        }
        // While a finger is down the next touch sample is the wake-up, but
        // a still finger may send none before its long press is due.
        let touching = self.gestures.pressed() || self.slide.is_some();
        if let Some(c) = self.cover.filter(|_| !touching) {
            sooner(after(c.until.unwrap_or(mono + self.cover_seconds(&c))));
        }
        if self.hold().is_some()
            && let Some((_, start)) = self.gestures.holding()
        {
            sooner(after(start + LONG_PRESS));
        }
        for signal in &self.alarm {
            if let Some(since) = self.watches.get(signal).and_then(|w| w.below_since) {
                sooner(after(since + ALARM_CLEARS));
            }
        }
        soonest
    }

    fn touch(&mut self, down: bool, x: f32, at: f64) {
        self.now = self.now.max(at);
        self.active_at = Some(self.now);
        self.expire_alarm();
        if down && self.touch_start.is_none() {
            self.touch_start = Some(x);
        }
        // A touch on a takeover keeps it up for its full time again.
        if let Some(cover) = self.cover
            && self.started_on(Row::Takeover(cover.index))
        {
            let seconds = self.cover_seconds(&cover);
            self.cover.as_mut().expect("just matched").until = Some(at + seconds);
        }
        if self.ignoring {
            // Followed, so the recognizer knows where the touch is, but
            // nothing comes of it.
            self.gestures.update(down, x, at);
            if !down {
                self.ignoring = false;
                self.touch_start = None;
            }
            return;
        }
        let on_slider = self.slide.is_some()
            || (down
                && !self.gestures.pressed()
                && self.motion == Motion::Still
                && self.slider_at(x).is_some());
        if on_slider {
            self.slide_touch(down, x, at);
            if !down {
                self.touch_start = None;
            }
            return;
        }
        let landing = down && !self.gestures.pressed();
        if landing {
            // The article a tap opens is the one showing as the finger
            // lands: a headline that turns mid-tap must not open another.
            self.landing_link = match self.signals.live("news.link") {
                Some(Value::Text(t)) if news_live(&self.signals) => Some(t.clone()),
                _ => None,
            };
        }
        let gesture = self.gestures.update(down, x, at);
        // Light the zone as soon as the finger lands, so the strip
        // answers before the gesture is known; a swipe turns it off.
        let mut pressed = match () {
            _ if !self.gestures.pressed() => None,
            _ if landing && self.motion == Motion::Still => self.lit_at(x),
            _ => self.pressed,
        };
        if matches!(gesture, Some(Gesture::Drag { .. })) {
            pressed = None;
        }
        if pressed != self.pressed {
            self.pressed = pressed;
            self.dirty = true;
        }
        if let Some(gesture) = gesture {
            self.gesture(gesture);
        }
        if !down {
            self.ignoring = false;
            self.touch_start = None;
        }
    }

    fn gesture(&mut self, gesture: Gesture) {
        if self.ignoring {
            return;
        }
        // The alarm takes every touch: OK acknowledges it, a swipe puts it
        // away, and nothing else does anything.
        if !self.alarm.is_empty() {
            let ok = alarm_ok().start;
            match gesture {
                Gesture::Drag { .. } => {
                    self.acknowledge();
                    self.ignoring = true;
                }
                Gesture::Tap { x } | Gesture::LongPress { x } if x >= ok as f32 => {
                    self.acknowledge();
                }
                _ => {}
            }
            return;
        }
        // A swipe on a takeover puts the page back; it does not also move
        // the page underneath. One that starts beside a partial takeover
        // is the page's: it moves the page, and the takeover goes.
        if let Some(cover) = self.cover
            && matches!(gesture, Gesture::Drag { .. })
        {
            let on_it = self.started_on(Row::Takeover(cover.index));
            self.uncover();
            if on_it {
                self.ignoring = true;
                return;
            }
        }
        match gesture {
            Gesture::Drag { dx } => {
                // Grabbing a page mid-spring: let go of the spring.
                if let Motion::Settle { .. } = self.motion {
                    self.finish_settle();
                }
                let offset = if self.neighbour(dx).is_some() {
                    dx.clamp(-WIDTH, WIDTH)
                } else {
                    rubber_band(dx)
                };
                self.motion = Motion::Drag { offset };
            }
            Gesture::Release { dx, velocity } => {
                let Motion::Drag { offset } = self.motion else {
                    return;
                };
                let commit = self.neighbour(dx).is_some()
                    && should_commit(dx.abs() / WIDTH, velocity * dx.signum());
                self.motion = Motion::Settle {
                    offset,
                    velocity,
                    target: if commit { WIDTH.copysign(dx) } else { 0.0 },
                };
                self.stepped_at = None;
            }
            Gesture::Tap { x } => {
                if let Some((_, key)) = self.media_key_at(x) {
                    self.act(Action::Media(key));
                } else if let Some(action) = self.widget_at(x).and_then(|w| self.tap_of(w)).cloned()
                {
                    self.act(action);
                }
            }
            Gesture::LongPress { x } => {
                // A transport key held is a slow tap; the whole card as
                // play/pause keeps the widget's own long press.
                let action = match self.media_key_at(x) {
                    Some((true, key)) => Some(Action::Media(key)),
                    _ => self.widget_at(x).and_then(|w| self.hold_of(w)).cloned(),
                };
                if let Some(action) = action {
                    // It has fired: the zone goes dark, and lifting or
                    // moving the finger now does nothing more.
                    self.ignoring = self.gestures.pressed();
                    if self.pressed.is_some() {
                        self.pressed = None;
                        self.dirty = true;
                    }
                    self.act(action);
                }
            }
        }
    }

    /// The rows on the strip, the one on top first: a takeover, then the
    /// page beside a partial one. None under the alarm.
    fn layers(&self) -> impl Iterator<Item = Row> + use<> {
        let cover = self.cover.map(|c| Row::Takeover(c.index));
        let page = match cover {
            Some(row) if self.span(row).1 == crate::WIDTH => None,
            _ => Some(Row::Page(self.page)),
        };
        let alarm = !self.alarm.is_empty();
        [cover, page].into_iter().flatten().filter(move |_| !alarm)
    }

    fn widgets(&self, row: Row) -> &[Widget] {
        match row {
            Row::Page(i) => &self.config.pages[i].widgets,
            Row::Takeover(i) => &self.config.takeovers[i].widgets,
        }
    }

    /// Where a row sits: its first pixel and its width. A takeover over
    /// one end of the strip is snapped to the card edge of the page under
    /// it nearest its own width, so it never cuts a card in two.
    fn span(&self, row: Row) -> (u32, u32) {
        match row {
            Row::Page(_) => (0, crate::WIDTH),
            Row::Takeover(i) => {
                let t = &self.config.takeovers[i];
                let (left, width) = t.span();
                if width == crate::WIDTH {
                    return (left, width);
                }
                let edges = snap_edges(&self.config.pages[self.page].widgets, t.align);
                // Only so far: over a card much wider than the gap, the
                // given width stands (the card is cut, as asked). Never
                // narrower than the floor: a cramped slider is worse than
                // a card covered whole.
                let floor = width.min(TAKEOVER_MIN);
                let snapped = edges
                    .into_iter()
                    .filter(|e| *e >= floor)
                    .min_by_key(|e| e.abs_diff(width))
                    .filter(|e| e.abs_diff(width) as f32 <= width as f32 * TAKEOVER_SNAP)
                    .unwrap_or(width);
                match t.align {
                    Align::Left => (0, snapped),
                    _ => (crate::WIDTH - snapped, snapped),
                }
            }
        }
    }

    /// The widths of a row's zones: the layout's, except on a snapped
    /// takeover, whose zones are scaled to its width (the last takes what
    /// rounding leaves).
    fn widths(&self, row: Row) -> Vec<u32> {
        let widgets = self.widgets(row);
        let given: Vec<u32> = widgets.iter().map(Widget::width).collect();
        let total: u32 = given.iter().sum();
        let width = self.span(row).1;
        if !matches!(row, Row::Takeover(_)) || total == width || total == 0 {
            return given;
        }
        let mut out: Vec<u32> = given
            .iter()
            .map(|w| (u64::from(*w) * u64::from(width) / u64::from(total)) as u32)
            .collect();
        let used: u32 = out.iter().sum();
        if let Some(last) = out.last_mut() {
            *last += width - used;
        }
        out
    }

    /// Whether the touch on the strip landed on `row`'s part of it.
    fn started_on(&self, row: Row) -> bool {
        let (left, width) = self.span(row);
        self.touch_start
            .is_none_or(|x| x >= left as f32 && x < (left + width) as f32)
    }

    /// Whether a change to this signal changes the picture.
    fn visible(&self, name: &str) -> bool {
        if let Some(signal) = self.alarm.last() {
            return signal == name;
        }
        self.layers()
            .any(|row| signals_used(self.widgets(row)).any(|s| s == name))
    }

    /// Whether the strip shows the time, so it must redraw each minute.
    fn clock_shown(&self) -> bool {
        self.layers().any(|row| shows_clock(self.widgets(row)))
    }

    /// A row's zones, drawn, at its own width.
    fn render_row(&mut self, row: Row, now: &Zoned, marquee: Option<f32>) -> Canvas {
        let mut zones = self.bind(row, now, marquee);
        self.show_sliders(row, &mut zones);
        let touch = self.touch_on(row);
        let width = self.span(row).1;
        let raised = matches!(row, Row::Takeover(_)) && width < crate::WIDTH;
        self.renderer.render_row(&zones, touch, width, raised)
    }

    fn bind(&self, row: Row, now: &Zoned, marquee: Option<f32>) -> Vec<Zone> {
        let ctx = Bind {
            signals: &self.signals,
            settings: &self.config.settings,
            weather: self.config.weather.is_some(),
            news: self.config.news.as_ref().is_some_and(|n| n.has_feeds()),
            now,
            mono: self.now,
            marquee,
            music: self.config.music_action(),
        };
        let mut zones = bind_page(self.widgets(row), &ctx);
        for (zone, w) in zones.iter_mut().zip(self.widths(row)) {
            zone.width = w;
        }
        zones
    }

    /// The finger on `row`, as drawn: the zone lit, the part of it, and how
    /// far its long press has come.
    fn touch_on(&self, row: Row) -> Option<Touch> {
        let Some(Lit::Zone(r, zone, part)) = self.pressed else {
            return None;
        };
        if r != row {
            return None;
        }
        let span = self.zone_span(row, zone);
        let hold = self.hold().filter(|h| h.zone == span).map(|h| h.progress);
        Some(Touch { zone, part, hold })
    }

    /// The transport target under x in a media widget showing a track: its
    /// index, and whether it is drawn as a key.
    fn part_at(&self, row: Row, zone: usize, x: f32) -> Option<(usize, bool)> {
        let widget = &self.widgets(row)[zone];
        if !matches!(widget, Widget::Media { .. })
            || media(&self.signals, None) == WidgetView::Media(None)
        {
            return None;
        }
        let span = self.zone_span(row, zone);
        let local = x - span.start as f32;
        let keys = media_keys(span.end - span.start);
        keys.iter()
            .position(|k| k.span.contains(&local))
            .map(|i| (i, keys[i].key))
    }

    /// What a tap on `widget` does: its own `tap`, or, on a media widget
    /// with nothing playing, what opens the music player.
    fn tap_of<'a>(&'a self, widget: &'a Widget) -> Option<&'a Action> {
        const OPEN: Action = Action::News(NewsCommand::Open);
        match widget {
            Widget::Media { tap: None, .. } => self.config.music_action(),
            Widget::News { tap: None, .. } => news_live(&self.signals).then_some(&OPEN),
            w => w.tap(),
        }
    }

    /// What a long press on `widget` does: its own `hold`, or, on a news
    /// widget showing a headline, the next headline.
    fn hold_of<'a>(&'a self, widget: &'a Widget) -> Option<&'a Action> {
        const NEXT: Action = Action::News(NewsCommand::Next);
        match widget {
            Widget::News { hold: None, .. } => news_live(&self.signals).then_some(&NEXT),
            w => w.hold(),
        }
    }

    /// The transport command under x, and whether it is drawn as a key.
    fn media_key_at(&self, x: f32) -> Option<(bool, MediaCommand)> {
        let (row, zone) = self.hit(x)?;
        let (part, key) = self.part_at(row, zone, x)?;
        let span = self.zone_span(row, zone);
        Some((key, media_keys(span.end - span.start)[part].command))
    }

    /// Shows the takeover that `signal` sets off, if there is one. The same
    /// takeover again only starts its time over. A different one waits for
    /// no finger: it would swap the widgets out from under a touch. None
    /// shows while the alarm is up, nor afterwards for this change.
    fn take_over(&mut self, signal: &str, old: Option<&Value>) {
        if !self.alarm.is_empty() {
            return;
        }
        let Some(index) = self
            .config
            .takeovers
            .iter()
            .position(|t| t.on_change.iter().any(|s| s == signal))
        else {
            return;
        };
        let same = self.cover.is_some_and(|c| c.index == index);
        let busy = self.gestures.pressed() || self.slide.is_some() || self.motion != Motion::Still;
        if !same && busy {
            return;
        }
        // The level before this run of changes: a burst of volume steps
        // shows the change from where it started.
        let from = match (signal, old) {
            ("volume.level", Some(Value::Number(n, _))) => Some((*n / 100.0) as f32),
            _ => None,
        };
        let from = if same {
            self.cover.and_then(|c| c.from).or(from)
        } else {
            from
        };
        self.cover = Some(Cover {
            index,
            until: None,
            lease: None,
            from,
        });
        if !same {
            self.fade_in(index);
        }
    }

    /// A takeover comes up: it fades in over what is there.
    fn fade_in(&mut self, index: usize) {
        self.pressed = None;
        self.dirty = true;
        self.fade = Some(Fade {
            index,
            start: self.now,
            out: false,
            from: None,
        });
    }

    /// The takeover goes, fading out over the page. Touches go to the page
    /// at once.
    fn fade_out(&mut self) {
        if let Some(c) = self.cover {
            self.uncover();
            self.fade = Some(Fade {
                index: c.index,
                start: self.now,
                out: true,
                from: c.from,
            });
        }
    }

    /// How long a takeover stays: its lease when shown on request.
    fn cover_seconds(&self, c: &Cover) -> f64 {
        c.lease.unwrap_or(self.config.takeovers[c.index].seconds)
    }

    fn uncover(&mut self) {
        self.fade = None;
        if self.cover.take().is_some() {
            self.pressed = None;
            self.slide = None;
            self.dirty = true;
        }
    }

    /// Follows a new reading of a watched signal: a rise to its `crit`
    /// raises the alarm if it is armed, and a fall under it starts the
    /// clocks for clearing and re-arming. A reading that is not a number
    /// is no data and changes nothing.
    fn watch(&mut self, name: &str) {
        let now = self.now;
        let Some(w) = self.watches.get_mut(name) else {
            return;
        };
        let Some(Value::Number(n, _)) = self.signals.get(name) else {
            return;
        };
        let mut raise = false;
        if *n >= w.crit {
            if !w.critical {
                // A rise, or a first reading already critical.
                w.critical = true;
                if !w.armed && w.below_since.is_some_and(|t| now - t >= ALARM_REARMS) {
                    w.armed = true;
                }
                w.below_since = None;
                raise = std::mem::take(&mut w.armed);
            }
        } else if w.critical || w.below_since.is_none() {
            w.critical = false;
            w.below_since = Some(now);
        }
        if raise {
            self.raise(name);
        }
    }

    /// Puts the alarm up for `signal`, or brings it to the front. It takes
    /// the whole strip at once: a takeover goes, a moving page lands, and
    /// the touch under way does nothing more.
    fn raise(&mut self, signal: &str) {
        let touching = self.gestures.pressed() || self.slide.is_some();
        self.alarm.retain(|s| s != signal);
        self.alarm.push(signal.to_string());
        self.uncover();
        if let Motion::Settle { .. } = self.motion {
            self.finish_settle();
        }
        self.motion = Motion::Still;
        self.snapshots.clear();
        self.slide = None;
        self.pressed = None;
        self.ignoring |= touching;
        self.dirty = true;
    }

    /// Takes down every reading that has been back under its threshold for
    /// `ALARM_CLEARS`.
    fn expire_alarm(&mut self) {
        let (now, watches) = (self.now, &self.watches);
        let before = self.alarm.len();
        self.alarm.retain(|s| {
            !watches
                .get(s)
                .and_then(|w| w.below_since)
                .is_some_and(|t| now - t >= ALARM_CLEARS)
        });
        if self.alarm.len() != before {
            self.alarm_changed();
        }
    }

    fn acknowledge(&mut self) {
        if !self.alarm.is_empty() {
            self.alarm.clear();
            self.alarm_changed();
        }
    }

    /// The alarm changed or went: redraw, and let a finger on it do
    /// nothing to what is now under it.
    fn alarm_changed(&mut self) {
        self.dirty = true;
        self.pressed = None;
        self.ignoring |= self.gestures.pressed() || self.slide.is_some();
    }

    /// The zone under panel x, on whichever row is on top there.
    fn hit(&self, x: f32) -> Option<(Row, usize)> {
        let row = self.layers().find(|&row| {
            let (left, width) = self.span(row);
            x >= left as f32 && x < (left + width) as f32
        })?;
        let mut right = self.span(row).0 as f32;
        self.widths(row)
            .iter()
            .position(|w| {
                right += *w as f32;
                x < right
            })
            .map(|zone| (row, zone))
    }

    fn widget_at(&self, x: f32) -> Option<&Widget> {
        self.hit(x).map(|(row, zone)| &self.widgets(row)[zone])
    }

    /// The slider under x, if there is one.
    fn slider_at(&self, x: f32) -> Option<(Row, usize)> {
        self.hit(x)
            .filter(|&(row, zone)| matches!(self.widgets(row)[zone], Widget::Slider { .. }))
    }

    /// A zone's pixels on the strip.
    fn zone_span(&self, row: Row, zone: usize) -> Range<u32> {
        let widths = self.widths(row);
        let left = self.span(row).0 + widths[..zone].iter().sum::<u32>();
        left..left + widths[zone]
    }

    /// The length of a slider's bar in pixels: the zone less its padding,
    /// so a drag from one end of the bar to the other is the full range.
    fn bar_length(&self, row: Row, zone: usize) -> f32 {
        let pad = theme::CARD_INSET + theme::PAD;
        (self.widths(row)[zone] as f32 - 2.0 * pad).max(1.0)
    }

    /// What a slider shows at rest, 0–1; 0 when it shows nothing.
    fn slider_value(&self, row: Row, zone: usize) -> f32 {
        let last = self.last_set.get(&(row, zone)).copied();
        match slider(&self.widgets(row)[zone], &self.signals, None, last, None) {
            WidgetView::Slider {
                fraction: Some(f), ..
            } => f,
            _ => 0.0,
        }
    }

    /// A touch while on a slider. The value moves by how far the finger
    /// travels, from where it was when the finger landed; landing, or a
    /// finger resting still, sets nothing. Values are only set once the
    /// touch has outlasted a lift bounce. The lift sample reports x = 0,
    /// so the release keeps the last position.
    fn slide_touch(&mut self, down: bool, x: f32, at: f64) {
        let Some(mut slide) = self.slide.take().or_else(|| {
            let (row, zone) = self.slider_at(x)?;
            let from = self.slider_value(row, zone);
            Some(Slide {
                row,
                zone,
                start: at,
                x0: x,
                from,
                fraction: from,
                moved: false,
                sent: Some(whole_percent(from)),
            })
        }) else {
            return;
        };
        self.dirty = true;
        let settled = at - slide.start >= MIN_TAP;
        if down {
            let dx = x - slide.x0;
            slide.moved |= dx.abs() >= SLIDER_SLOP;
            if slide.moved {
                let length = self.bar_length(slide.row, slide.zone);
                slide.fraction = (slide.from + dx / length).clamp(0.0, 1.0);
            }
            if settled {
                self.slid(&mut slide, false);
            }
            self.slide = Some(slide);
        } else if settled {
            self.slid(&mut slide, true);
        }
    }

    /// Turns a slider position into effects: volume and commands as the
    /// value changes, a seek on release. Nothing until the finger moves.
    fn slid(&mut self, slide: &mut Slide, done: bool) {
        let Widget::Slider { controls, .. } = &self.widgets(slide.row)[slide.zone] else {
            return;
        };
        if !slide.moved {
            return;
        }
        let percent = whole_percent(slide.fraction);
        let changed = slide.sent != Some(percent);
        let effect = match controls {
            SliderControl::Volume if changed => Some(Effect::Volume(percent)),
            SliderControl::Run(argv) if changed => Some(Effect::Run(
                argv.iter()
                    .map(|a| a.replace(SLIDER_VALUE, &percent.to_string()))
                    .collect(),
            )),
            SliderControl::Seek if done => match self.signals.get("media.length") {
                Some(Value::Number(length, _)) if *length > 0.0 => {
                    Some(Effect::Seek(f64::from(slide.fraction) * length))
                }
                _ => None,
            },
            _ => None,
        };
        if matches!(controls, SliderControl::Run(_)) {
            self.last_set
                .insert((slide.row, slide.zone), slide.fraction);
        }
        if let Some(effect) = effect {
            slide.sent = Some(percent);
            self.effects.push(effect);
        }
    }

    /// Replaces slider views with ones that show the finger's value while
    /// a finger is on them (drawn held, with a thumb), and a `run`
    /// slider's last value at rest.
    fn show_sliders(&self, row: Row, zones: &mut [Zone]) {
        // A volume takeover shows the change that brought it up.
        let from = match row {
            Row::Takeover(t) => match (self.cover, self.fade) {
                (Some(c), _) if c.index == t => c.from,
                (None, Some(f)) if f.out && f.index == t => f.from,
                _ => None,
            },
            Row::Page(_) => None,
        };
        for (i, widget) in self.widgets(row).iter().enumerate() {
            if let Widget::Slider { controls, .. } = widget {
                let held = self
                    .slide
                    .filter(|s| s.zone == i && s.row == row)
                    .map(|s| s.fraction);
                let last = self.last_set.get(&(row, i)).copied();
                // Kept under a finger: the change is still the news.
                let from = from.filter(|_| *controls == SliderControl::Volume);
                zones[i].view = slider(widget, &self.signals, held, last, from);
            }
        }
    }

    /// What lights under x when a finger lands: a zone whose tap or long
    /// press does something, or the alarm's OK key.
    fn lit_at(&self, x: f32) -> Option<Lit> {
        if !self.alarm.is_empty() {
            return (x >= alarm_ok().start as f32).then_some(Lit::AlarmOk);
        }
        let (row, zone) = self.hit(x)?;
        if let Some((part, _)) = self.part_at(row, zone, x) {
            return Some(Lit::Zone(row, zone, Some(part)));
        }
        let w = &self.widgets(row)[zone];
        (self.tap_of(w).is_some() || self.hold_of(w).is_some())
            .then_some(Lit::Zone(row, zone, None))
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Page(target) => match target.as_str() {
                "next" => self.handle(Input::Step(1)),
                "prev" => self.handle(Input::Step(-1)),
                name => {
                    if let Some(i) = self.config.page_index(name) {
                        self.go_to(i);
                    }
                }
            },
            Action::Publish { signal, value } => {
                let value = match value.trim().parse::<f64>() {
                    Ok(n) => Value::Number(n, Unit::None),
                    Err(_) => Value::Text(value),
                };
                self.handle(Input::Signal(signal, value));
            }
            Action::News(NewsCommand::Open) => {
                if let Some(link) = self.landing_link.clone() {
                    self.effects.push(Effect::OpenLink(link));
                }
            }
            other => self.effects.push(Effect::Act(other)),
        }
    }

    /// The page a drag of `dx` reveals: the next one when dragging left.
    fn neighbour(&self, dx: f32) -> Option<usize> {
        if dx < 0.0 {
            Some(self.page + 1).filter(|&p| p < self.config.pages.len())
        } else if dx > 0.0 {
            self.page.checked_sub(1)
        } else {
            None
        }
    }

    fn offset(&self) -> Option<f32> {
        match self.motion {
            Motion::Still => None,
            Motion::Drag { offset } | Motion::Settle { offset, .. } => Some(offset),
        }
    }

    /// Advances the spring to `mono`.
    fn step(&mut self, mono: f64) {
        let Motion::Settle {
            mut offset,
            mut velocity,
            target,
        } = self.motion
        else {
            return;
        };
        // Clamp so a stall (a slow frame, a debugger) cannot fling the page.
        let dt = self.stepped_at.map_or(0.0, |t| (mono - t).clamp(0.0, 0.05)) as f32;
        self.stepped_at = Some(mono);
        // Semi-implicit Euler in 4 ms steps: stable and accurate at this
        // stiffness.
        let steps = (dt / 0.004).ceil().max(1.0) as usize;
        let h = dt / steps as f32;
        for _ in 0..steps {
            velocity += (-STIFFNESS * (offset - target) - DAMPING * velocity) * h;
            offset += velocity * h;
        }
        self.motion = Motion::Settle {
            offset: offset.clamp(-WIDTH, WIDTH),
            velocity,
            target,
        };
        if (offset - target).abs() < 0.5 && velocity.abs() < 20.0 {
            self.finish_settle();
        }
    }

    fn finish_settle(&mut self) {
        if let Motion::Settle { target, .. } = self.motion {
            if target < 0.0 {
                self.page += 1;
            } else if target > 0.0 {
                self.page -= 1;
            }
        }
        self.motion = Motion::Still;
        self.snapshots.clear();
        self.dirty = true;
    }

    /// The current page shifted by `offset`, with its neighbour beside it.
    fn slide(&mut self, offset: f32, now: &Zoned) -> Canvas {
        let shift = offset.round() as i32;
        let mut frame = Canvas::new(crate::WIDTH, crate::HEIGHT, theme::GROUND);
        frame.copy_shifted(self.snapshot(self.page, now), shift);
        // The offset has the drag's sign, so it picks the same neighbour.
        if let Some(next) = self.neighbour(offset) {
            let width = crate::WIDTH as i32;
            let at = if shift < 0 {
                shift + width
            } else {
                shift - width
            };
            frame.copy_shifted(self.snapshot(next, now), at);
        }
        let dim = self.brightness(now);
        let markers = self.markers();
        self.renderer.draw_markers(&mut frame, &markers, dim);
        frame
    }

    fn snapshot(&mut self, page: usize, now: &Zoned) -> &Canvas {
        if !self.snapshots.contains_key(&page) {
            let mut zones = self.bind(Row::Page(page), now, None);
            self.show_sliders(Row::Page(page), &mut zones);
            let mut canvas = self.renderer.render_page(&zones, None);
            let alerts = self.renderer.alerts().to_vec();
            canvas.dim(self.brightness(now), &alerts);
            self.snapshots.insert(page, canvas);
        }
        &self.snapshots[&page]
    }

    fn go_to(&mut self, index: usize) {
        self.uncover();
        self.marquee_at = None;
        self.motion = Motion::Still;
        self.pressed = None;
        self.slide = None;
        self.snapshots.clear();
        if index < self.config.pages.len() && index != self.page {
            self.page = index;
            self.dirty = true;
        }
    }

    /// Swaps in a new config without a visible jump: the same page stays on
    /// screen if it still exists by name.
    fn reload(&mut self, config: Config) {
        let current = &self.config.pages[self.page].name;
        self.page = config
            .page_index(current)
            .unwrap_or(self.page.min(config.pages.len() - 1));
        if config.settings.font != self.font || config.settings.accent != self.theme.accent {
            self.font = config.settings.font;
            self.theme = Theme::new(config.settings.accent);
            self.renderer = Renderer::new(self.theme.clone(), self.font.load());
        }
        self.motion = Motion::Still;
        self.fade = None;
        self.marquee_at = None;
        self.pressed = None;
        self.slide = None;
        // A takeover stays if it still exists by name, as an edit to it
        // from the settings window must not close it.
        let covering = self.cover.map(|c| &self.config.takeovers[c.index].name);
        let kept = covering.and_then(|name| config.takeovers.iter().position(|t| &t.name == name));
        self.cover = match (self.cover, kept) {
            (Some(c), Some(index)) => Some(Cover { index, ..c }),
            _ => None,
        };
        // Readings keep their alarm state; one no longer watched leaves
        // the alarm.
        self.watches = watches(&config, &self.watches);
        let watches = &self.watches;
        self.alarm.retain(|s| watches.contains_key(s));
        self.last_set.clear();
        self.snapshots.clear();
        self.dirty = true;
        self.config = config;
    }
}

/// The readings with a critical threshold on the layout's pages, by
/// signal, keeping the state of those in `old`. A signal on several
/// widgets is watched at the lowest `crit`, named by that widget's label;
/// a second reading by the label and its `second_label` ("GPU load").
/// The widths a takeover at `align` may snap to over a page with these
/// widgets: the distances from that end of the strip to each card edge,
/// leaving out none and all.
fn snap_edges(widgets: &[Widget], align: Align) -> Vec<u32> {
    let mut edges = Vec::new();
    let mut x = 0;
    for w in widgets {
        x += w.width();
        let e = match align {
            Align::Left => x,
            _ => crate::WIDTH.saturating_sub(x - w.width()),
        };
        if e > 0 && e < crate::WIDTH {
            edges.push(e);
        }
    }
    edges
}

fn watches(config: &Config, old: &HashMap<String, Watch>) -> HashMap<String, Watch> {
    let mut out: HashMap<String, Watch> = HashMap::new();
    for page in &config.pages {
        for (widget, signal, _, crit) in thresholds(&page.widgets) {
            let (
                Some(crit),
                Widget::Value {
                    label,
                    signal: first,
                    second_label,
                    ..
                },
            ) = (crit, widget)
            else {
                continue;
            };
            let is_second = signal != first;
            if out.get(signal).is_some_and(|w| w.crit <= crit) {
                continue;
            }
            let mut watch = old.get(signal).cloned().unwrap_or(Watch {
                crit,
                label: String::new(),
                icon: icons::ACTIVITY,
                critical: false,
                below_since: None,
                armed: true,
            });
            watch.crit = crit;
            watch.label = if is_second {
                let name = second_label
                    .clone()
                    .unwrap_or_else(|| signal.rsplit('.').next().unwrap_or(signal).to_string());
                format!("{label} {name}").trim().to_string()
            } else {
                label.clone()
            };
            // The widget's own icon stands for its first reading.
            watch.icon = widget
                .icon()
                .filter(|_| !is_second)
                .and_then(icons::by_name)
                .unwrap_or_else(|| icons::for_signal(signal));
            out.insert(signal.to_string(), watch);
        }
    }
    out
}

/// Where the alarm's red is in its slow breathing at `mono`, 0–1.
fn breathe(mono: f64) -> f32 {
    let period = f64::from(theme::MS_ALARM_BREATHE) / 1000.0;
    let phase = (mono / period).fract() as f32;
    0.5 - 0.5 * (phase * std::f32::consts::TAU).cos()
}

/// A slider's 0–1 as the whole percent it sets.
fn whole_percent(fraction: f32) -> u8 {
    (fraction.clamp(0.0, 1.0) * 100.0).round() as u8
}

/// Resistance past the first or last page: the page follows the finger less
/// and less, approaching a third of the width. The formula iOS uses.
fn rubber_band(dx: f32) -> f32 {
    const C: f32 = 0.55;
    let limit = WIDTH / 3.0;
    (1.0 - 1.0 / (dx.abs() * C / limit + 1.0)) * limit * dx.signum()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signal::Unit;

    fn at(s: &str) -> Zoned {
        format!("2026-06-09T{s}-04:00[America/New_York]")
            .parse()
            .unwrap()
    }

    fn temp(name: &str, c: f64) -> Input {
        Input::Signal(name.into(), Value::Number(c, Unit::Celsius))
    }

    #[test]
    fn draws_once_then_only_on_change() {
        let mut e = Engine::new(Config::default_layout());
        assert!(e.frame(&at("16:17:05"), 0.0).is_some(), "first frame");
        assert!(e.frame(&at("16:17:30"), 0.0).is_none(), "nothing changed");

        e.handle(temp("cpu.temp", 50.0));
        assert!(
            e.frame(&at("16:17:31"), 0.0).is_some(),
            "visible signal changed"
        );

        e.handle(temp("cpu.load", 20.0));
        assert!(
            e.frame(&at("16:17:32"), 0.0).is_none(),
            "cpu.load is not on page 1"
        );

        assert!(e.frame(&at("16:18:00"), 0.0).is_some(), "the minute turned");
    }

    #[test]
    fn hidden_signals_are_still_recorded() {
        let mut e = Engine::new(Config::default_layout());
        e.handle(temp("cpu.load", 20.0));
        e.handle(Input::ShowPage(2));
        assert_eq!(e.signals().history("cpu.load"), [20.0]);
        assert!(e.frame(&at("16:17:05"), 0.0).is_some());
    }

    #[test]
    fn waits_for_the_next_minute_only_with_a_clock() {
        let mut e = Engine::new(Config::default_layout());
        assert_eq!(
            e.next_wait(&at("16:17:05"), 0.0),
            Some(Duration::from_secs(55))
        );
        e.handle(Input::ShowPage(1)); // Performance: no clock
        assert_eq!(e.next_wait(&at("16:17:05"), 0.0), None);
    }

    #[test]
    fn stepping_stops_at_the_ends() {
        let mut e = Engine::new(Config::default_layout());
        let last = e.config().pages.len() - 1;
        e.handle(Input::Step(-1));
        assert_eq!(e.page(), 0);
        e.handle(Input::Step(10));
        assert_eq!(e.page(), last);
        e.handle(Input::ShowPage(99));
        assert_eq!(e.page(), last);
    }

    #[test]
    fn reload_keeps_the_page_by_name() {
        let mut e = Engine::new(Config::default_layout());
        e.handle(Input::ShowPage(2)); // "System"
        let mut c = Config::default_layout();
        c.pages.remove(0); // "System" is now index 1
        e.handle(Input::Config(Box::new(c)));
        assert_eq!(e.page(), 1);
        assert_eq!(e.config().pages[e.page()].name, "System");
    }

    #[test]
    fn reload_without_the_page_stays_in_range() {
        let mut e = Engine::new(Config::default_layout());
        e.handle(Input::ShowPage(3));
        let mut c = Config::default_layout();
        c.pages.truncate(1);
        e.handle(Input::Config(Box::new(c)));
        assert_eq!(e.page(), 0);
    }

    /// Drags from `from` to `to` over `secs` at 500 Hz, lifts, then runs
    /// the animation to rest at 60 fps. Returns the number of frames drawn.
    fn swipe(e: &mut Engine, from: f32, to: f32, secs: f64) -> usize {
        swipe_from(e, 10.0, from, to, secs)
    }

    /// `swipe`, starting at time `start`.
    fn swipe_from(e: &mut Engine, start: f64, from: f32, to: f32, secs: f64) -> usize {
        let samples = (secs * 500.0) as usize;
        let mut t = start;
        for i in 0..=samples {
            let x = from + (to - from) * i as f32 / samples as f32;
            e.handle(Input::Touch {
                down: true,
                x,
                at: t,
            });
            t += 0.002;
        }
        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: t,
        });
        let mut frames = 0;
        while e.animating() {
            t += 1.0 / 60.0;
            e.frame(&at("16:17:05"), t);
            frames += 1;
            assert!(frames < 120, "settles within 2 s");
        }
        frames
    }

    /// Page A: a clock that taps to page B (0–99), media with tap and hold
    /// (100–199), and a reading with no action (200–299). Page B: a clock.
    fn tappable() -> Engine {
        let config = Config::parse(
            r#"
            [[page]]
            name = "A"
            [[page.widget]]
            type = "clock"
            width = 100
            tap = { page = "B" }
            [[page.widget]]
            type = "media"
            width = 100
            tap = { media = "play-pause" }
            hold = { run = ["pavucontrol"] }
            [[page.widget]]
            type = "value"
            width = 100
            label = "CPU"
            signal = "cpu.temp"

            [[page]]
            name = "B"
            [[page.widget]]
            type = "clock"
            width = 640
            "#,
        )
        .unwrap();
        Engine::new(config)
    }

    /// Holds a still finger at `x` for `secs`, sampling at 500 Hz.
    fn press(e: &mut Engine, x: f32, secs: f64) {
        let mut t = 10.0;
        while t < 10.0 + secs {
            e.handle(Input::Touch {
                down: true,
                x,
                at: t,
            });
            t += 0.002;
        }
        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: t,
        });
    }

    fn pixel(canvas: &Canvas, x: u32, y: u32) -> [u8; 3] {
        let i = ((y * crate::WIDTH + x) * 4) as usize;
        let rgba = canvas.rgba();
        [rgba[i], rgba[i + 1], rgba[i + 2]]
    }

    #[test]
    fn a_finger_on_an_action_lights_its_zone_at_once() {
        let mut e = tappable();
        let now = at("16:17:05");
        e.frame(&now, 0.0);
        let lit = theme::CARD.mix(theme::KEY, 0.4);
        let lit = [lit.r, lit.g, lit.b];

        // The clock shows page B on a tap. (The idle media card beside it
        // is drawn as a key, which sinks rather than lights.)
        e.handle(Input::Touch {
            down: true,
            x: 50.0,
            at: 10.0,
        });
        let frame = e.frame(&now, 10.0).expect("the first sample redraws");
        assert_eq!(pixel(&frame, 50, 9), lit, "clock zone lit");
        assert_ne!(pixel(&frame, 150, 9), lit, "media zone not lit");

        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: 10.1,
        });
        let frame = e.frame(&now, 10.1).expect("lifting redraws");
        assert_ne!(pixel(&frame, 50, 9), lit);

        // No action under the finger: nothing lights, nothing redraws.
        e.handle(Input::Touch {
            down: true,
            x: 250.0,
            at: 11.0,
        });
        assert!(e.frame(&now, 11.0).is_none());
    }

    #[test]
    fn an_idle_media_card_opens_what_the_music_key_runs() {
        let text = "[[page]]\nname = \"A\"\n\
            [[page.widget]]\ntype = \"media\"\nwidth = 320\n\
            [[page.widget]]\ntype = \"key\"\nwidth = 320\nlabel = \"Music\"\n\
            tap = { run = [\"rhythmbox\"] }\n";
        let mut e = Engine::new(Config::parse(text).expect("valid"));
        press(&mut e, 150.0, 0.1);
        assert_eq!(
            e.take_effects(),
            [Effect::Act(Action::Run(vec!["rhythmbox".into()]))]
        );
    }

    #[test]
    fn taps_and_holds_act() {
        let mut e = tappable();
        press(&mut e, 150.0, 0.1);
        assert_eq!(
            e.take_effects(),
            [Effect::Act(Action::Media(
                crate::config::MediaCommand::PlayPause
            ))]
        );
        press(&mut e, 150.0, 0.7);
        assert_eq!(
            e.take_effects(),
            [Effect::Act(Action::Run(vec!["pavucontrol".into()]))]
        );
        assert!(e.take_effects().is_empty(), "taken once");

        press(&mut e, 250.0, 0.1); // no action there
        assert!(e.take_effects().is_empty());
        assert_eq!(e.page(), 0);
        press(&mut e, 50.0, 0.1);
        assert_eq!(e.page(), 1, "page actions are done by the engine");
    }

    #[test]
    fn a_swipe_from_a_lit_zone_turns_it_off_and_acts_on_nothing() {
        let mut e = tappable();
        swipe(&mut e, 150.0, 20.0, 0.1);
        assert_eq!(e.pressed, None);
        assert!(e.take_effects().is_empty());
    }

    /// Page A: a volume slider (0–199), a clock (200–299), a seek slider
    /// (300–499) and a run slider (500–639). Page B: a clock.
    fn sliders() -> Engine {
        let config = Config::parse(
            r#"
            [[page]]
            name = "A"
            [[page.widget]]
            type = "slider"
            width = 200
            label = "Volume"
            controls = "volume"
            [[page.widget]]
            type = "clock"
            width = 100
            [[page.widget]]
            type = "slider"
            width = 200
            label = "Position"
            controls = "seek"
            [[page.widget]]
            type = "slider"
            width = 140
            label = "Screen"
            controls = { run = ["ddcutil", "setvcp", "10", "{value}"] }

            [[page]]
            name = "B"
            [[page.widget]]
            type = "clock"
            width = 640
            "#,
        )
        .unwrap();
        Engine::new(config)
    }

    /// Drags from `from` to `to` over `secs` at the strip's ~100 Hz, lifts.
    fn drag(e: &mut Engine, from: f32, to: f32, secs: f64) {
        let samples = (secs * 100.0) as usize;
        let mut t = 50.0;
        for i in 0..=samples {
            let x = from + (to - from) * i as f32 / samples.max(1) as f32;
            e.handle(Input::Touch {
                down: true,
                x,
                at: t,
            });
            t += 0.01;
        }
        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: t,
        });
    }

    #[test]
    fn a_volume_drag_sends_each_level_once_and_never_swipes() {
        let mut e = sliders();
        let pad = theme::CARD_INSET + theme::PAD;
        drag(&mut e, pad, 200.0 - pad, 0.3);
        let levels: Vec<u8> = e
            .take_effects()
            .into_iter()
            .map(|fx| match fx {
                Effect::Volume(v) => v,
                other => panic!("{other:?}"),
            })
            .collect();
        assert_eq!(levels.last(), Some(&100));
        assert!(levels.windows(2).all(|w| w[0] < w[1]), "{levels:?}");
        assert!(
            levels[0] <= 15,
            "starts from the value it had (none: 0), not a jump: {levels:?}"
        );
        assert_eq!(e.page(), 0, "a drag on a slider is not a swipe");
        assert!(e.slide.is_none());
    }

    #[test]
    fn a_lift_bounce_on_a_slider_sets_nothing() {
        let mut e = sliders();
        e.handle(Input::Touch {
            down: true,
            x: 120.0,
            at: 9.0,
        });
        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: 9.01,
        });
        assert!(e.take_effects().is_empty());
    }

    #[test]
    fn seek_waits_for_release_and_run_fills_in_the_value() {
        let mut e = sliders();
        e.handle(Input::Signal(
            "media.length".into(),
            Value::Number(200.0, Unit::None),
        ));
        let pad = theme::CARD_INSET + theme::PAD;
        let mid = 300.0 + 100.0; // the middle of the seek bar
        drag(&mut e, 300.0 + pad, mid, 0.2);
        assert_eq!(e.take_effects(), [Effect::Seek(100.0)]);

        // The run slider's bar is 118 px; with nothing set yet it starts at
        // 0, so 59 px to the right is 50.
        drag(&mut e, 520.0, 579.0, 0.1);
        let fx = e.take_effects();
        let Some(Effect::Run(argv)) = fx.last() else {
            panic!("{fx:?}")
        };
        assert_eq!(argv, &["ddcutil", "setvcp", "10", "50"]);
        // Held still, it sets nothing; the last value set is where the
        // next drag starts from.
        drag(&mut e, 600.0, 600.0, 0.1);
        assert!(e.take_effects().is_empty());
        drag(&mut e, 600.0, 570.5, 0.1);
        let fx = e.take_effects();
        assert!(
            matches!(fx.last(), Some(Effect::Run(argv)) if argv[3] == "25"),
            "{fx:?}"
        );
    }

    #[test]
    fn a_swipe_starting_beside_a_slider_still_swipes() {
        let mut e = sliders();
        swipe(&mut e, 290.0, 60.0, 0.1); // lands on the clock, crosses the volume
        assert_eq!(e.page(), 1);
        assert!(e.take_effects().is_empty());
    }

    #[test]
    fn a_flick_left_moves_to_the_next_page() {
        let mut e = Engine::new(Config::default_layout());
        let frames = swipe(&mut e, 500.0, 300.0, 0.1);
        assert_eq!(e.page(), 1);
        assert!(frames > 5, "animated, not a jump: {frames} frames");
    }

    #[test]
    fn a_slow_short_drag_springs_back() {
        let mut e = Engine::new(Config::default_layout());
        swipe(&mut e, 500.0, 420.0, 1.0); // 12% of the width, slowly
        assert_eq!(e.page(), 0);
    }

    #[test]
    fn the_first_page_rubber_bands_and_stays() {
        let mut e = Engine::new(Config::default_layout());
        // Drag right by 300 px on the first page, then hold.
        for i in 0..=150 {
            let x = 100.0 + 2.0 * i as f32;
            e.handle(Input::Touch {
                down: true,
                x,
                at: 10.0 + i as f64 * 0.002,
            });
        }
        let Motion::Drag { offset } = e.motion else {
            panic!("dragging")
        };
        assert!(offset > 0.0 && offset < 120.0, "resists: {offset}");
        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: 10.4,
        });
        let mut t = 10.4;
        while e.animating() {
            t += 1.0 / 60.0;
            e.frame(&at("16:17:05"), t);
        }
        assert_eq!(e.page(), 0);
    }

    #[test]
    fn frames_during_a_drag_show_both_pages() {
        let mut e = Engine::new(Config::default_layout());
        let first = e.frame(&at("16:17:05"), 0.0).unwrap();
        for i in 0..=100 {
            let x = 600.0 - 3.0 * i as f32;
            e.handle(Input::Touch {
                down: true,
                x,
                at: 10.0 + i as f64 * 0.002,
            });
        }
        let mid = e.frame(&at("16:17:05"), 10.2).unwrap();
        assert_ne!(first.rgba(), mid.rgba());
    }

    /// Page A: a clock. Page B: media. Takeovers for the track (5 s, media
    /// and seek) and the volume (2 s, one slider).
    fn covered() -> Engine {
        let config = Config::parse(
            r#"
            [[page]]
            name = "A"
            [[page.widget]]
            type = "clock"
            width = 640

            [[page]]
            name = "B"
            [[page.widget]]
            type = "media"
            width = 640

            [[takeover]]
            name = "Now playing"
            on_change = ["media.title"]
            seconds = 5
            [[takeover.widget]]
            type = "media"
            width = 320
            [[takeover.widget]]
            type = "slider"
            width = 320
            label = "Position"
            controls = "seek"

            [[takeover]]
            name = "Volume"
            on_change = ["volume.level"]
            seconds = 2
            [[takeover.widget]]
            type = "slider"
            width = 640
            label = "Volume"
            controls = "volume"
            "#,
        )
        .unwrap();
        Engine::new(config)
    }

    fn title(t: &str) -> Input {
        Input::Signal("media.title".into(), Value::Text(t.into()))
    }

    #[test]
    fn a_change_covers_the_strip_until_its_time_is_up() {
        let mut e = covered();
        let now = at("16:17:05");
        e.handle(title("First"));
        assert_eq!(e.takeover(), None, "a first reading is not a change");
        e.frame(&now, 0.0);

        e.handle(title("Second"));
        assert_eq!(e.takeover(), Some("Now playing"));
        assert!(e.frame(&now, 1.0).is_some(), "drawn at once");
        assert_eq!(
            e.next_wait(&now, 1.0),
            Some(Duration::from_secs(5)),
            "wakes when it ends, sooner than the minute"
        );
        assert!(e.frame(&now, 5.9).is_none());
        assert!(e.frame(&now, 6.0).is_some(), "the page again");
        assert_eq!(e.takeover(), None);

        e.handle(title(""));
        assert_eq!(e.takeover(), None, "nothing playing is not news");
    }

    #[test]
    fn a_takeover_fades_in_and_out_and_frames_come_only_while_it_moves() {
        let mut e = covered();
        let now = at("16:17:05");
        e.handle(title("First"));
        let page = e.frame(&now, 0.0).expect("the page").rgba().to_vec();
        e.tick(10.0);
        e.handle(title("Second"));
        let mid = e.frame(&now, 10.1).expect("fading in").rgba().to_vec();
        assert_eq!(e.next_wait(&now, 10.1), Some(Duration::ZERO), "paced");
        let full = e.frame(&now, 10.5).expect("faded in").rgba().to_vec();
        assert!(mid != page && mid != full, "part way between the two");
        assert!(e.next_wait(&now, 10.5) > Some(Duration::from_secs(4)));
        assert!(e.frame(&now, 11.0).is_none(), "nothing moves: no frame");
        // At its time the takeover fades out over the page, from the frame
        // that finds it over.
        let mut t = 14.0;
        while e.takeover().is_some() {
            t += 0.05;
            e.frame(&now, t);
        }
        assert!(t < 15.5, "over after its five seconds: {t}");
        let out = e.frame(&now, t + 0.15).expect("fading out").rgba().to_vec();
        assert!(out != page && out != full);
        assert_eq!(e.frame(&now, 16.0).expect("gone").rgba(), &page[..]);
    }

    #[test]
    fn a_partial_takeover_snaps_to_the_card_edges_under_it() {
        let text = "[[page]]\nname = \"A\"\n\
            [[page.widget]]\ntype = \"clock\"\nwidth = 204\n\
            [[page.widget]]\ntype = \"clock\"\nwidth = 204\n\
            [[page.widget]]\ntype = \"clock\"\nwidth = 232\n\
            [[takeover]]\nname = \"V\"\non_change = [\"volume.level\"]\nseconds = 2\n\
            align = \"right\"\nwidth = 320\n\
            [[takeover.widget]]\ntype = \"slider\"\nwidth = 320\nlabel = \"V\"\ncontrols = \"volume\"\n";
        let mut e = Engine::new(Config::parse(text).expect("valid"));
        e.handle(Input::Signal(
            "volume.level".into(),
            Value::Number(40.0, Unit::Percent),
        ));
        e.handle(Input::Signal(
            "volume.level".into(),
            Value::Number(55.0, Unit::Percent),
        ));
        assert_eq!(e.takeover(), Some("V"));
        let row = Row::Takeover(0);
        // 232 is nearer 320, but would cramp the slider under the floor:
        // it ends at the next card edge out, covering that card whole.
        assert_eq!(e.span(row), (204, 436));
        assert_eq!(e.widths(row), [436]);
        assert_eq!(e.zone_span(row, 0), 204..640);
        assert_eq!(e.hit(300.0), Some((row, 0)));
        assert_eq!(e.hit(150.0), Some((Row::Page(0), 0)));
    }

    #[test]
    fn no_takeover_for_what_the_page_already_shows() {
        let mut e = covered();
        e.handle(Input::ShowPage(1)); // the media page
        e.handle(title("First"));
        e.handle(title("Second"));
        assert_eq!(e.takeover(), None);
    }

    #[test]
    fn a_touch_keeps_it_up_and_a_swipe_puts_the_page_back() {
        let mut e = covered();
        let now = at("16:17:05");
        e.handle(title("First"));
        e.handle(title("Second"));
        e.frame(&now, 0.0); // ends at 5 s
        press(&mut e, 100.0, 0.1); // at 10 s: now ends at 15 s
        e.frame(&now, 10.2);
        assert_eq!(e.takeover(), Some("Now playing"), "{:?}", e.cover);

        swipe(&mut e, 300.0, 100.0, 0.1);
        assert_eq!(e.takeover(), None);
        assert_eq!(e.page(), 0, "the swipe only dismissed it");
    }

    #[test]
    fn a_slider_on_a_takeover_works_and_restarts_its_time() {
        let mut e = covered();
        let level = |v| Input::Signal("volume.level".into(), Value::Number(v, Unit::Percent));
        e.handle(level(100.0));
        e.handle(level(90.0));
        assert_eq!(e.takeover(), Some("Volume"));
        e.frame(&at("16:17:05"), 0.0);
        // The bar is 624 px: 156 px to the left is a quarter, from 90 to 65.
        drag(&mut e, 300.0, 144.0, 0.2);
        let fx = e.take_effects();
        assert_eq!(fx.last(), Some(&Effect::Volume(65)), "{fx:?}");
        let Some(Cover {
            until: Some(until), ..
        }) = e.cover
        else {
            panic!("{:?}", e.cover)
        };
        assert!(until > 50.0 + 2.0, "the drag at 50 s restarted it: {until}");
        // The level the drag set comes back as a reading: still the same
        // takeover, not a new one.
        e.handle(level(65.0));
        assert_eq!(e.takeover(), Some("Volume"));
    }

    #[test]
    fn a_takeover_shown_on_request_lasts_its_lease() {
        let mut e = covered();
        let now = at("16:17:05");
        let show = || Input::ShowTakeover {
            name: "Volume".into(),
            seconds: 15.0,
        };
        e.handle(show());
        assert_eq!(e.takeover(), Some("Volume"));
        e.frame(&now, 0.0);
        e.frame(&now, 1.0); // its fade in is over
        assert!(e.frame(&now, 10.0).is_none(), "longer than its own 2 s");
        e.handle(show());
        e.frame(&now, 10.0);
        assert_eq!(e.takeover(), Some("Volume"));
        e.frame(&now, 24.0);
        assert_eq!(
            e.takeover(),
            Some("Volume"),
            "asked again: time starts over"
        );
        e.frame(&now, 25.0);
        assert_eq!(e.takeover(), None, "not renewed: it ends");

        e.handle(show());
        e.handle(Input::EndTakeover);
        assert_eq!(e.takeover(), None);
        e.handle(Input::ShowTakeover {
            name: "Nope".into(),
            seconds: 15.0,
        });
        assert_eq!(e.takeover(), None, "unknown names are ignored");
    }

    #[test]
    fn a_takeover_stays_through_a_reload_that_keeps_it() {
        let mut e = covered();
        e.handle(Input::ShowTakeover {
            name: "Volume".into(),
            seconds: 15.0,
        });
        let mut config = e.config().clone();
        config.takeovers[1].widgets[0] = config.takeovers[1].widgets[0].clone().with_width(600);
        e.handle(Input::Config(Box::new(config.clone())));
        assert_eq!(e.takeover(), Some("Volume"), "an edit to it keeps it");
        config.takeovers.remove(1);
        e.handle(Input::Config(Box::new(config)));
        assert_eq!(e.takeover(), None, "gone from the config: gone");
    }

    #[test]
    fn a_different_takeover_waits_for_the_finger_to_lift() {
        let mut e = covered();
        e.handle(title("First"));
        e.handle(Input::Touch {
            down: true,
            x: 100.0,
            at: 1.0,
        });
        e.handle(title("Second"));
        assert_eq!(e.takeover(), None);
    }

    #[test]
    fn a_forgotten_signal_is_shown_lost_and_returns_as_new() {
        let mut e = covered();
        let now = at("16:17:05");
        e.handle(title("First"));
        e.handle(Input::ShowPage(1)); // the media page
        e.frame(&now, 0.0);
        e.handle(Input::Forget("media.title".into()));
        assert!(e.signals().live("media.title").is_none());
        assert!(
            e.signals().lost_at("media.title").is_some(),
            "kept, as lost"
        );
        assert!(e.frame(&now, 0.1).is_some(), "redrawn without it");
        e.handle(Input::ShowPage(0));
        e.handle(title("Second"));
        assert_eq!(e.takeover(), None, "back again is a first reading");
    }

    // Relative sliders.

    fn volume(e: &mut Engine, v: f64) {
        e.handle(Input::Signal(
            "volume.level".into(),
            Value::Number(v, Unit::Percent),
        ));
    }

    /// Samples from `points` (time, x) with the finger down, then a lift
    /// at `lift`.
    fn trace(e: &mut Engine, points: &[(f64, f32)], lift: f64) {
        for &(at, x) in points {
            e.handle(Input::Touch { down: true, x, at });
        }
        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: lift,
        });
    }

    #[test]
    fn a_brush_on_a_slider_changes_nothing() {
        let mut e = sliders();
        volume(&mut e, 40.0);
        // A knuckle grazing it while typing: 20 ms, moving 10 px.
        trace(&mut e, &[(1.0, 120.0), (1.01, 125.0), (1.02, 130.0)], 1.02);
        assert!(e.take_effects().is_empty(), "a brush sets nothing");
        // A finger landing and resting, with a pixel of jitter.
        let rest: Vec<(f64, f32)> = (0..30)
            .map(|i| (2.0 + i as f64 * 0.01, 150.0 + (i % 2) as f32))
            .collect();
        trace(&mut e, &rest, 2.3);
        assert!(e.take_effects().is_empty(), "landing sets nothing");
        // A tap, on the volume and on the seek bar.
        e.handle(Input::Signal(
            "media.length".into(),
            Value::Number(200.0, Unit::None),
        ));
        press(&mut e, 30.0, 0.08);
        press(&mut e, 450.0, 0.08);
        assert!(e.take_effects().is_empty(), "a tap sets nothing");
        assert_eq!(e.page(), 0);
    }

    #[test]
    fn a_drag_moves_the_value_by_the_distance_from_where_it_was() {
        let mut e = sliders();
        let pad = theme::CARD_INSET + theme::PAD;
        let half = (200.0 - 2.0 * pad) / 2.0; // half the volume bar: 89 px
        volume(&mut e, 40.0);
        // From anywhere on the bar: half its length is half the range.
        drag(&mut e, 20.0, 20.0 + half, 0.3);
        assert_eq!(e.take_effects().last(), Some(&Effect::Volume(90)));
        volume(&mut e, 90.0);
        drag(&mut e, 180.0, 180.0 - half, 0.3);
        assert_eq!(e.take_effects().last(), Some(&Effect::Volume(40)));
        volume(&mut e, 40.0);
        // Clamped to the range, however far the finger goes.
        drag(&mut e, 100.0, 199.0, 0.3);
        drag(&mut e, 10.0, 199.0, 0.3);
        let fx = e.take_effects();
        assert_eq!(fx.last(), Some(&Effect::Volume(100)), "{fx:?}");
    }

    #[test]
    fn a_finger_on_a_slider_is_shown_held_at_its_value() {
        let mut e = sliders();
        volume(&mut e, 40.0);
        e.handle(Input::Touch {
            down: true,
            x: 150.0,
            at: 1.0,
        });
        let slide = e.slide.expect("the slider has the touch");
        assert!((slide.fraction - 0.4).abs() < 1e-6, "{slide:?}");
        let mut zones = e.bind(Row::Page(0), &at("16:17:05"), None);
        e.show_sliders(Row::Page(0), &mut zones);
        assert!(matches!(
            zones[0].view,
            WidgetView::Slider { active: true, fraction: Some(f), .. } if (f - 0.4).abs() < 1e-6
        ));
    }

    // Hold at the threshold.

    #[test]
    fn a_hold_fires_once_at_the_threshold_and_not_on_release() {
        let mut e = tappable();
        let run = || vec![Effect::Act(Action::Run(vec!["pavucontrol".into()]))];
        let mut t = 10.0;
        while t < 10.49 {
            e.handle(Input::Touch {
                down: true,
                x: 150.0,
                at: t,
            });
            t += 0.01;
        }
        assert!(e.take_effects().is_empty(), "not before the threshold");
        let hold = e.hold().expect("on its way");
        assert_eq!(hold.zone, 100..200);
        assert!((0.95..1.0).contains(&hold.progress), "{hold:?}");
        e.handle(Input::Touch {
            down: true,
            x: 150.0,
            at: 10.5,
        });
        assert_eq!(e.take_effects(), run(), "at the threshold, finger down");
        assert_eq!(e.hold(), None, "it has fired");
        trace(&mut e, &[(10.6, 150.0), (10.8, 150.0)], 11.0);
        assert!(e.take_effects().is_empty(), "the release does nothing more");
    }

    #[test]
    fn a_hold_fires_without_a_sample_at_the_threshold() {
        let mut e = tappable();
        let now = at("16:17:05");
        e.handle(Input::Touch {
            down: true,
            x: 150.0,
            at: 10.0,
        });
        assert_eq!(e.hold().map(|h| h.progress), Some(0.0));
        e.tick(10.25);
        assert_eq!(e.hold().map(|h| h.progress), Some(0.5), "half way");
        assert!(e.animating(), "the fill is drawn as it grows");
        assert_eq!(e.next_wait(&now, 10.25), Some(Duration::ZERO));
        e.tick(10.5);
        assert_eq!(
            e.take_effects(),
            [Effect::Act(Action::Run(vec!["pavucontrol".into()]))]
        );
    }

    #[test]
    fn lifting_before_the_threshold_is_a_tap() {
        let mut e = tappable();
        press(&mut e, 150.0, 0.45);
        assert_eq!(
            e.take_effects(),
            [Effect::Act(Action::Media(
                crate::config::MediaCommand::PlayPause
            ))]
        );
    }

    #[test]
    fn after_a_hold_fires_the_finger_cannot_swipe_but_elsewhere_it_can() {
        let mut e = tappable();
        let mut points: Vec<(f64, f32)> =
            (0..60).map(|i| (10.0 + i as f64 * 0.01, 150.0)).collect();
        points.extend((0..30).map(|i| (10.6 + i as f64 * 0.01, 150.0 - i as f32 * 5.0)));
        trace(&mut e, &points, 10.9);
        assert_eq!(e.take_effects().len(), 1, "the hold, once");
        assert_eq!(e.page(), 0, "the drag after it is nothing");
        // On the reading, with no hold action, resting first is no loss.
        let mut points: Vec<(f64, f32)> =
            (0..60).map(|i| (20.0 + i as f64 * 0.002, 250.0)).collect();
        points.extend((0..100).map(|i| (20.6 + i as f64 * 0.002, 250.0 - i as f32 * 2.5)));
        trace(&mut e, &points, 20.8);
        let mut t = 20.8;
        while e.animating() {
            t += 1.0 / 60.0;
            e.frame(&at("16:17:05"), t);
        }
        assert_eq!(e.page(), 1, "swiped");
    }

    // The critical alarm.

    /// Page A: CPU (crit 90) and a clock that taps to B. Page B: GPU (warn
    /// 75, crit 85) and a load with a second reading, mem.used (crit 95).
    /// A volume takeover.
    fn alarmed() -> Engine {
        let config = Config::parse(
            r#"
            [[page]]
            name = "A"
            [[page.widget]]
            type = "value"
            width = 200
            label = "CPU"
            signal = "cpu.temp"
            crit = 90
            [[page.widget]]
            type = "clock"
            width = 200
            tap = { page = "B" }

            [[page]]
            name = "B"
            [[page.widget]]
            type = "value"
            width = 200
            label = "GPU"
            signal = "gpu.temp"
            warn = 75
            crit = 85
            [[page.widget]]
            type = "value"
            width = 200
            label = "Load"
            signal = "cpu.load"
            second = "mem.used"
            second_crit = 95

            [[takeover]]
            name = "Volume"
            on_change = ["volume.level"]
            seconds = 2
            [[takeover.widget]]
            type = "slider"
            width = 640
            label = "Volume"
            controls = "volume"
            "#,
        )
        .unwrap();
        Engine::new(config)
    }

    fn reading(e: &mut Engine, at: f64, signal: &str, v: f64) {
        e.tick(at);
        e.handle(Input::Signal(
            signal.into(),
            Value::Number(v, Unit::Celsius),
        ));
    }

    fn alarm_label(e: &Engine) -> Option<String> {
        e.alarm().map(|a| a.label)
    }

    #[test]
    fn a_reading_going_critical_raises_the_alarm_until_ok() {
        let mut e = alarmed();
        let now = at("16:17:05");
        reading(&mut e, 0.0, "cpu.temp", 50.0);
        assert_eq!(e.alarm(), None);
        reading(&mut e, 1.0, "cpu.temp", 92.0);
        let alarm = e.alarm().expect("raised");
        assert_eq!(
            (
                alarm.label.as_str(),
                alarm.value.as_str(),
                alarm.limit.as_str()
            ),
            ("CPU", "92°", "90°")
        );
        assert_eq!((alarm.count, alarm.ok.clone()), (1, 546..640));
        let frame = e.frame(&now, 1.0).expect("drawn at once");
        let [r, g, _] = pixel(&frame, 300, 2);
        assert!(r > 160 && g < 70, "filled red, breathing: {r} {g}");
        // A tap off the OK key does nothing, even just left of it; on it,
        // acknowledges.
        trace(&mut e, &[(2.0, 100.0), (2.05, 100.0)], 2.1);
        assert!(e.alarm().is_some());
        trace(&mut e, &[(2.3, 540.0), (2.35, 540.0)], 2.4);
        assert!(e.alarm().is_some(), "540 is left of the key");
        trace(&mut e, &[(3.0, 550.0), (3.05, 550.0)], 3.1);
        assert_eq!(e.alarm(), None, "acknowledged");
        assert!(e.take_effects().is_empty());
        assert!(e.frame(&now, 3.1).is_some(), "the page again");
        reading(&mut e, 4.0, "cpu.temp", 96.0);
        assert_eq!(e.alarm(), None, "still critical is not a new rise");
    }

    #[test]
    fn a_first_reading_already_critical_raises_it_but_no_data_never_does() {
        let mut e = alarmed();
        e.handle(Input::Signal("cpu.temp".into(), Value::Text("n/a".into())));
        assert_eq!(e.alarm(), None, "text is no data");
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        assert_eq!(
            alarm_label(&e).as_deref(),
            Some("CPU"),
            "the user should know"
        );
        e.handle(Input::Forget("cpu.temp".into()));
        assert_eq!(e.alarm(), None, "its source gone: no data, no alarm");
    }

    #[test]
    fn the_alarm_says_when_the_reading_is_back_under() {
        let mut e = alarmed();
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        let words = |e: &Engine| e.alarm().map(|a| a.view().headline);
        assert_eq!(words(&e).as_deref(), Some("CPU is too hot"));
        reading(&mut e, 1.0, "cpu.temp", 74.0);
        let view = e.alarm().expect("still up").view();
        assert_eq!(
            (
                view.headline.as_str(),
                view.detail.as_str(),
                view.value.as_str()
            ),
            (
                "CPU was too hot",
                "Back under its 90° limit · check the fans",
                "74°"
            )
        );
        reading(&mut e, 2.0, "cpu.temp", 93.0);
        assert_eq!(words(&e).as_deref(), Some("CPU is too hot"), "over again");
    }

    #[test]
    fn the_alarm_clears_itself_ten_seconds_after_the_reading_drops() {
        let mut e = alarmed();
        let now = at("16:17:05");
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        reading(&mut e, 5.0, "cpu.temp", 80.0);
        // The red breathes, so it wakes often; it clears at 15 s.
        let wait = e.next_wait(&now, 5.0).expect("a wake-up");
        assert!(wait <= Duration::from_millis(80), "{wait:?}");
        e.tick(14.9);
        assert!(e.alarm().is_some(), "not yet");
        reading(&mut e, 14.9, "cpu.temp", 91.0);
        reading(&mut e, 15.0, "cpu.temp", 80.0);
        e.tick(24.9);
        assert!(e.alarm().is_some(), "back over restarts the ten seconds");
        e.tick(25.0);
        assert_eq!(e.alarm(), None, "cleared");
        assert!(e.frame(&now, 25.0).is_some(), "the page again");
    }

    #[test]
    fn it_rearms_after_thirty_seconds_under_and_not_before() {
        let mut e = alarmed();
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        e.handle(Input::Acknowledge);
        assert_eq!(e.alarm(), None);
        // Under for 29 s, then over: not re-armed.
        reading(&mut e, 1.0, "cpu.temp", 80.0);
        reading(&mut e, 30.0, "cpu.temp", 95.0);
        assert_eq!(e.alarm(), None, "29 s under is not enough");
        // 20 s under, a moment over, 20 s under: still not 30 s in a row.
        reading(&mut e, 31.0, "cpu.temp", 80.0);
        reading(&mut e, 51.0, "cpu.temp", 92.0);
        reading(&mut e, 52.0, "cpu.temp", 80.0);
        reading(&mut e, 72.0, "cpu.temp", 92.0);
        assert_eq!(e.alarm(), None, "the 30 s must be unbroken");
        // 30 s under, then over: raised again.
        reading(&mut e, 73.0, "cpu.temp", 80.0);
        reading(&mut e, 103.0, "cpu.temp", 92.0);
        assert_eq!(alarm_label(&e).as_deref(), Some("CPU"));
    }

    #[test]
    fn an_alarm_that_cleared_itself_rearms_the_same_way() {
        let mut e = alarmed();
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        reading(&mut e, 1.0, "cpu.temp", 80.0);
        e.tick(11.0);
        assert_eq!(e.alarm(), None, "cleared at 10 s");
        reading(&mut e, 20.0, "cpu.temp", 95.0);
        assert_eq!(e.alarm(), None, "19 s under");
        reading(&mut e, 21.0, "cpu.temp", 80.0);
        reading(&mut e, 51.0, "cpu.temp", 95.0);
        assert!(e.alarm().is_some(), "30 s under");
    }

    #[test]
    fn several_critical_readings_show_the_latest_and_ok_takes_them_all() {
        let mut e = alarmed();
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        reading(&mut e, 1.0, "gpu.temp", 88.0);
        let alarm = e.alarm().unwrap();
        assert_eq!(
            (alarm.label.as_str(), alarm.count),
            ("GPU", 2),
            "the most recent"
        );
        // A second reading has its own threshold, and is named by the
        // widget's label and its own (here none: the signal's last word).
        reading(&mut e, 2.0, "mem.used", 97.0);
        assert_eq!(
            (alarm_label(&e).as_deref(), e.alarm().unwrap().count),
            (Some("Load used"), 3)
        );
        trace(&mut e, &[(3.0, 630.0), (3.05, 630.0)], 3.1);
        assert_eq!(e.alarm(), None, "all acknowledged");
        reading(&mut e, 4.0, "gpu.temp", 90.0);
        reading(&mut e, 4.0, "cpu.temp", 99.0);
        assert_eq!(e.alarm(), None, "none raises again while still critical");
    }

    #[test]
    fn the_alarm_comes_before_takeovers() {
        let mut e = alarmed();
        volume(&mut e, 50.0);
        volume(&mut e, 60.0);
        assert_eq!(e.takeover(), Some("Volume"));
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        assert!(e.alarm().is_some());
        assert_eq!(e.takeover(), None, "the alarm took the strip");
        volume(&mut e, 70.0);
        assert_eq!(e.takeover(), None, "a volume change does not replace it");
        assert!(e.alarm().is_some());
        e.handle(Input::ShowTakeover {
            name: "Volume".into(),
            seconds: 10.0,
        });
        assert_eq!(e.takeover(), None, "nor a request");
        e.handle(Input::Acknowledge);
        assert_eq!((e.takeover(), e.page()), (None, 0), "the page resumes");
    }

    #[test]
    fn a_swipe_puts_the_alarm_away_without_moving_the_page() {
        let mut e = alarmed();
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        swipe(&mut e, 500.0, 200.0, 0.1);
        assert_eq!(e.alarm(), None);
        assert_eq!(e.page(), 0);
        assert!(e.take_effects().is_empty());
    }

    #[test]
    fn a_touch_under_way_when_the_alarm_rises_does_nothing_more() {
        let mut e = alarmed();
        e.handle(Input::Touch {
            down: true,
            x: 300.0, // the clock, which taps to page B
            at: 1.0,
        });
        reading(&mut e, 1.02, "cpu.temp", 95.0);
        trace(&mut e, &[(1.05, 300.0)], 1.1);
        assert_eq!(e.page(), 0, "the tap did not reach the page");
        assert!(e.alarm().is_some(), "nor the alarm");
    }

    #[test]
    fn pages_say_whether_they_hold_a_warning_or_a_critical_reading() {
        let mut e = alarmed();
        assert_eq!(e.page_count(), 2);
        assert_eq!(e.page_severities(), [Severity::Ok, Severity::Ok], "no data");
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        reading(&mut e, 0.0, "gpu.temp", 80.0);
        assert_eq!(e.page_severities(), [Severity::Crit, Severity::Warn]);
        e.handle(Input::Acknowledge);
        assert_eq!(
            e.page_severity(0),
            Severity::Crit,
            "acknowledged, still critical"
        );
        reading(&mut e, 1.0, "mem.used", 96.0);
        assert_eq!(
            e.page_severity(1),
            Severity::Crit,
            "a second reading counts"
        );
        reading(&mut e, 2.0, "cpu.temp", 60.0);
        assert_eq!(e.page_severity(0), Severity::Ok);
        assert_eq!(e.page_severity(9), Severity::Ok, "no such page");
    }

    #[test]
    fn a_reload_keeps_the_alarm_state_of_readings_still_watched() {
        let mut e = alarmed();
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        e.handle(Input::Config(Box::new(e.config().clone())));
        assert!(e.alarm().is_some(), "an edit elsewhere keeps it");
        e.handle(Input::Acknowledge);
        e.handle(Input::Config(Box::new(e.config().clone())));
        reading(&mut e, 1.0, "cpu.temp", 96.0);
        assert_eq!(e.alarm(), None, "and keeps it acknowledged");
    }

    // A takeover over part of the strip.

    /// Page A: a clock tapping to B (0–100) and media (100–640). Page B: a
    /// clock. A 200 px takeover at the right on `mode`, with a key.
    fn partial() -> Engine {
        let config = Config::parse(
            r#"
            [[page]]
            name = "A"
            [[page.widget]]
            type = "clock"
            width = 100
            tap = { page = "B" }
            [[page.widget]]
            type = "media"
            width = 540
            tap = { media = "play-pause" }

            [[page]]
            name = "B"
            [[page.widget]]
            type = "clock"
            width = 640

            [[takeover]]
            name = "Mode"
            on_change = ["mode"]
            seconds = 5
            align = "right"
            width = 200
            [[takeover.widget]]
            type = "key"
            width = 200
            icon = "moon"
            label = "Focus"
            tap = { run = ["focus"] }
            "#,
        )
        .unwrap();
        Engine::new(config)
    }

    fn mode(e: &mut Engine, m: &str) {
        e.handle(Input::Signal("mode".into(), Value::Text(m.into())));
    }

    #[test]
    fn a_partial_takeover_takes_touches_on_it_and_leaves_the_page_the_rest() {
        let mut e = partial();
        let now = at("16:17:05");
        mode(&mut e, "a");
        mode(&mut e, "b");
        assert_eq!(e.takeover(), Some("Mode"));
        e.frame(&now, 0.0); // ends at 5 s
        press(&mut e, 500.0, 0.1); // on it, at 10 s: now ends at 15 s
        assert_eq!(
            e.take_effects(),
            [Effect::Act(Action::Run(vec!["focus".into()]))]
        );
        press(&mut e, 300.0, 0.1); // the page's media, beside it
        assert_eq!(
            e.take_effects(),
            [Effect::Act(Action::Media(
                crate::config::MediaCommand::PlayPause
            ))]
        );
        assert_eq!(e.takeover(), Some("Mode"), "the page's touch left it up");
        let Some(Cover {
            until: Some(until), ..
        }) = e.cover
        else {
            panic!("{:?}", e.cover)
        };
        assert!(
            (until - 15.1).abs() < 0.01,
            "only its own touch restarted it: {until}"
        );
        press(&mut e, 50.0, 0.1);
        assert_eq!(e.page(), 1, "the page's tap still works");
    }

    #[test]
    fn a_partial_takeover_is_drawn_over_the_live_page() {
        let mut e = partial();
        let now = at("16:17:05");
        let page = e.frame(&now, 0.0).unwrap();
        mode(&mut e, "a");
        mode(&mut e, "b");
        let covered = e.frame(&now, 0.1).unwrap();
        let columns = |c: &Canvas, xs: Range<u32>| -> Vec<[u8; 3]> {
            xs.flat_map(|x| (0..crate::HEIGHT).map(move |y| (x, y)))
                .map(|(x, y)| pixel(c, x, y))
                .collect()
        };
        assert_eq!(
            columns(&page, 0..440),
            columns(&covered, 0..440),
            "page kept"
        );
        assert_ne!(columns(&page, 440..640), columns(&covered, 440..640));
        // The page stays live under it.
        e.handle(Input::Signal(
            "media.title".into(),
            Value::Text("Song".into()),
        ));
        e.handle(Input::Signal(
            "media.status".into(),
            Value::Text("playing".into()),
        ));
        assert!(e.frame(&now, 0.2).is_some(), "the page's change redraws");
    }

    #[test]
    fn a_swipe_on_a_partial_takeover_dismisses_it_and_beside_it_swipes_pages() {
        let mut e = partial();
        mode(&mut e, "a");
        mode(&mut e, "b");
        swipe(&mut e, 630.0, 450.0, 0.1);
        assert_eq!((e.takeover(), e.page()), (None, 0), "dismissed only");
        mode(&mut e, "c");
        assert_eq!(e.takeover(), Some("Mode"));
        swipe_from(&mut e, 20.0, 400.0, 100.0, 0.1);
        assert_eq!((e.takeover(), e.page()), (None, 1), "the page's swipe");
    }

    // Keys.

    #[test]
    fn a_key_taps_and_holds_like_any_widget_and_shows_nothing_live() {
        let config = Config::parse(
            r#"
            [[page]]
            name = "A"
            [[page.widget]]
            type = "key"
            width = 80
            icon = "play"
            tap = { media = "play-pause" }
            hold = { run = ["notify-send", "held"] }
            [[page.widget]]
            type = "key"
            width = 80
            label = "Mute"
            "#,
        )
        .unwrap();
        assert_eq!(signals_used(&config.pages[0].widgets).count(), 0);
        let mut e = Engine::new(config);
        assert!(e.frame(&at("16:17:05"), 0.0).is_some());
        press(&mut e, 40.0, 0.1);
        press(&mut e, 40.0, 0.7);
        press(&mut e, 120.0, 0.1); // no action
        assert_eq!(
            e.take_effects(),
            [
                Effect::Act(Action::Media(crate::config::MediaCommand::PlayPause)),
                Effect::Act(Action::Run(vec!["notify-send".into(), "held".into()])),
            ]
        );
    }

    // Dimming.

    #[test]
    fn the_strip_dims_when_idle_and_at_night_but_not_over_trouble() {
        let mut e = alarmed();
        let day = at("16:17:05");
        reading(&mut e, 0.0, "cpu.temp", 50.0);
        assert!(e.frame(&day, 0.0).is_some());
        let idle = f64::from(theme::IDLE_DIM_AFTER_S);
        assert_eq!(e.brightness(&day), 1.0);
        // Wakes in time to dim (the clock's minute may come first).
        assert!(e.next_wait(&day, 1.0) <= Some(Duration::from_secs_f64(idle - 1.0)));
        e.tick(idle + 1.0);
        assert_eq!(e.brightness(&day), theme::IDLE_DIM);
        // A touch wakes it.
        trace(&mut e, &[(idle + 2.0, 300.0)], idle + 2.05);
        assert_eq!(e.brightness(&day), 1.0);
        // A warning on any page keeps it bright.
        reading(&mut e, idle + 3.0, "gpu.temp", 80.0);
        e.tick(2.0 * idle + 10.0);
        assert_eq!(e.brightness(&day), 1.0, "a warning is showing");
        reading(&mut e, 2.0 * idle + 11.0, "gpu.temp", 60.0);
        assert_eq!(e.brightness(&day), theme::IDLE_DIM);

        // Night: dimmer still, whatever the touches; the alarm undims.
        let mut c = e.config.clone();
        c.settings.night = Config::parse(
            "[settings]\nnight = { from = \"22:30\", to = \"07:00\" }\n[[page]]\nname = \"A\"\n",
        )
        .unwrap()
        .settings
        .night;
        c.settings.idle_dim = false;
        e.handle(Input::Config(Box::new(c)));
        let late = at("23:40:00");
        assert_eq!(e.brightness(&day), 1.0, "idle_dim off");
        assert_eq!(e.brightness(&late), theme::NIGHT_DIM);
        reading(&mut e, 2.0 * idle + 20.0, "cpu.temp", 95.0);
        assert!(e.alarm().is_some());
        assert_eq!(e.brightness(&late), 1.0, "the alarm is never dimmed");
    }

    // Media transport keys.

    /// A media widget of each width on one page, from x = 0, playing.
    fn media_row(widths: &[u32]) -> Engine {
        let mut text = String::from("[[page]]\nname = \"A\"\n");
        for w in widths {
            text.push_str(&format!(
                "[[page.widget]]\ntype = \"media\"\nwidth = {w}\nhold = {{ media = \"next\" }}\n"
            ));
        }
        let mut e = Engine::new(Config::parse(&text).unwrap());
        for (name, v) in [
            ("media.title", "Weird Fishes"),
            ("media.artist", "Radiohead"),
            ("media.status", "Playing"),
        ] {
            e.handle(Input::Signal(name.into(), Value::Text(v.into())));
        }
        assert!(e.frame(&at("16:17:05"), 0.0).is_some());
        e
    }

    /// The middle of each drawn target of a media widget `width` wide that
    /// starts at `x0`.
    fn key_centres(x0: f32, width: u32) -> Vec<f32> {
        media_keys(width)
            .iter()
            .map(|k| x0 + (k.span.start + k.span.end) / 2.0)
            .collect()
    }

    fn media_act(c: MediaCommand) -> Effect {
        Effect::Act(Action::Media(c))
    }

    #[test]
    fn media_keys_are_where_they_are_drawn() {
        // Compact (128), medium (200), hero with one key (312).
        let mut e = media_row(&[128, 200, 312]);
        let compact = key_centres(0.0, 128);
        let hero = key_centres(328.0, 312);
        assert_eq!((compact.len(), hero.len()), (1, 1));
        press(&mut e, compact[0], 0.05);
        press(&mut e, 228.0, 0.05); // anywhere on the medium card
        press(&mut e, hero[0], 0.05);
        assert_eq!(
            e.take_effects(),
            vec![media_act(MediaCommand::PlayPause); 3]
        );
        // Beside a compact or hero key is the widget, which has no tap.
        press(&mut e, 120.0, 0.05);
        press(&mut e, 340.0, 0.05);
        assert!(e.take_effects().is_empty());
        // A long press on a key is a slow tap; on the medium card, whose
        // whole face is the key, it is the widget's own hold.
        press(&mut e, compact[0], 0.9);
        press(&mut e, 228.0, 0.9);
        assert_eq!(
            e.take_effects(),
            [
                media_act(MediaCommand::PlayPause),
                media_act(MediaCommand::Next),
            ]
        );
    }

    #[test]
    fn a_wide_media_widget_has_three_keys_in_order() {
        let mut e = media_row(&[428]);
        let keys = key_centres(0.0, 428);
        assert_eq!(keys.len(), 3);
        for x in &keys {
            press(&mut e, *x, 0.05);
        }
        assert_eq!(
            e.take_effects(),
            [
                media_act(MediaCommand::Previous),
                media_act(MediaCommand::PlayPause),
                media_act(MediaCommand::Next),
            ]
        );
        // Between two keys, and on the title, is the widget: it has no tap.
        let spans = media_keys(428);
        press(
            &mut e,
            (spans[0].span.end + spans[1].span.start) / 2.0,
            0.05,
        );
        press(&mut e, 40.0, 0.05);
        assert!(e.take_effects().is_empty());
    }

    #[test]
    fn nothing_playing_has_no_keys() {
        let config = Config::parse(
            "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"media\"\nwidth = 428\n",
        )
        .unwrap();
        let mut e = Engine::new(config);
        assert!(e.frame(&at("16:17:05"), 0.0).is_some());
        for x in key_centres(0.0, 428) {
            press(&mut e, x, 0.05);
        }
        assert!(e.take_effects().is_empty());
    }

    #[test]
    fn the_accent_is_on_the_theme_and_follows_a_reload() {
        let mut c = Config::default_layout();
        let mut e = Engine::new(c.clone());
        assert_eq!(e.theme().accent, None, "absent: the default");
        c.settings.accent = Some(crate::theme::Rgb::hex(0xff8800));
        e.handle(Input::Config(Box::new(c)));
        assert_eq!(e.theme().accent, Some(crate::theme::Rgb::hex(0xff8800)));
        assert_eq!(e.renderer.theme().accent, e.theme().accent, "drawn with it");
    }

    /// A page with a news card on the left (0–319) and one whose tap and
    /// hold are the layout's own on the right.
    fn newsy() -> Engine {
        let config = Config::parse(
            r#"
            [news]
            feeds = ["https://news.example/rss.xml"]
            [[page]]
            name = "A"
            [[page.widget]]
            type = "news"
            width = 320
            [[page.widget]]
            type = "news"
            width = 320
            tap = { page = "B" }
            hold = { news = "open" }
            [[page]]
            name = "B"
            [[page.widget]]
            type = "clock"
            width = 640
            "#,
        )
        .unwrap();
        Engine::new(config)
    }

    fn headline(e: &mut Engine, title: &str, link: &str) {
        for (name, value) in [
            ("news.title", Value::Text(title.into())),
            ("news.source", Value::Text("Gazette".into())),
            ("news.link", Value::Text(link.into())),
            ("news.age", Value::Number(60.0, Unit::None)),
            ("news.lost", Value::Number(0.0, Unit::None)),
        ] {
            e.handle(Input::Signal(name.into(), value));
        }
    }

    #[test]
    fn a_tap_opens_the_headline_showing_when_the_finger_landed() {
        let mut e = newsy();
        headline(&mut e, "First", "https://news.example/1");
        e.handle(Input::Touch {
            down: true,
            x: 100.0,
            at: 10.0,
        });
        // The source turns to the next story under the finger.
        headline(&mut e, "Second", "https://news.example/2");
        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: 10.08,
        });
        assert_eq!(
            e.take_effects(),
            [Effect::OpenLink("https://news.example/1".into())]
        );
    }

    #[test]
    fn a_hold_on_a_headline_shows_the_next() {
        let mut e = newsy();
        headline(&mut e, "First", "https://news.example/1");
        press(&mut e, 100.0, 0.6);
        assert_eq!(
            e.take_effects(),
            [Effect::Act(Action::News(NewsCommand::Next))]
        );
    }

    #[test]
    fn a_news_card_with_nothing_to_open_does_nothing() {
        let mut e = newsy();
        let now = at("16:17:05");
        // No headline yet: neither a tap nor a hold acts, nor lights it.
        press(&mut e, 100.0, 0.08);
        press(&mut e, 100.0, 0.6);
        assert!(e.take_effects().is_empty());
        e.handle(Input::Touch {
            down: true,
            x: 100.0,
            at: 20.0,
        });
        assert_eq!(e.touch_on(Row::Page(0)), None, "not lit");
        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: 20.05,
        });
        // Lost: every feed failing, the headline kept too old to open.
        headline(&mut e, "Old", "https://news.example/old");
        e.handle(Input::Signal(
            "news.lost".into(),
            Value::Number(1.0, Unit::None),
        ));
        press(&mut e, 100.0, 0.08);
        assert!(e.take_effects().is_empty());
        assert!(e.frame(&now, 30.0).is_some());
    }

    #[test]
    fn a_news_cards_own_tap_and_hold_win() {
        let mut e = newsy();
        headline(&mut e, "First", "https://news.example/1");
        press(&mut e, 400.0, 0.6);
        assert_eq!(
            e.take_effects(),
            [Effect::OpenLink("https://news.example/1".into())],
            "its hold opens the article"
        );
        press(&mut e, 400.0, 0.08);
        assert_eq!(e.page(), 1, "its tap shows page B");
    }

    #[test]
    fn a_new_headline_redraws_but_never_takes_over() {
        let mut e = newsy();
        let now = at("16:17:05");
        headline(&mut e, "First", "https://news.example/1");
        e.frame(&now, 1.0);
        headline(&mut e, "Second", "https://news.example/2");
        assert!(e.frame(&now, 2.0).is_some(), "the card shows it");
        assert!(!e.animating(), "at once: no scroll, no fade");
        assert_eq!(e.takeover(), None);
        let e = Config::parse(
            "[[page]]\nname = \"A\"\n[[takeover]]\nname = \"N\"\non_change = [\"news.title\"]\n\
             seconds = 3\n[[takeover.widget]]\ntype = \"news\"\nwidth = 640\n",
        )
        .unwrap_err();
        assert!(e.0.contains("news never takes over"), "{e}");
    }
}
