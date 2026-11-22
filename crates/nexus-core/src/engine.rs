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
//! The engine never reads a clock or touches a thread. Wall time (for the
//! clock widget) and monotonic time (for touch, animation and the alarm's
//! timers, in seconds from any origin) are passed in, so tests can drive it
//! to any moment.

use std::collections::HashMap;
use std::ops::Range;
use std::time::Duration;

use jiff::Zoned;

use crate::canvas::Canvas;
use crate::config::{Action, Config, SLIDER_VALUE, SliderControl, Widget};
use crate::fonts::FontChoice;
use crate::gesture::{Gesture, LONG_PRESS, MIN_TAP, Recognizer, should_commit};
use crate::layout::{
    bind_page, judge, reading_text, shows_clock, signals_used, slider, thresholds,
};
use crate::signal::{Signals, Unit, Value};
use crate::theme::Theme;
use crate::widget::{Renderer, Severity, WidgetView, Zone};

const WIDTH: f32 = crate::WIDTH as f32;

/// Page-snap spring, from the Go app: slightly under-damped (critical
/// damping would be 2·√400 = 40) for a crisp stop with no visible bounce.
const STIFFNESS: f32 = 400.0;
const DAMPING: f32 = 36.0;

/// How far a finger on a slider moves before the value follows it, in
/// pixels: the jitter of a finger resting on it never sets anything.
const SLIDER_SLOP: f32 = 4.0;

/// The alarm ends by itself once its reading has been back under the
/// threshold this many seconds.
pub const ALARM_CLEARS: f64 = 10.0;
/// After its alarm ends, a reading raises it again only once it has stayed
/// under the threshold this many seconds without a break, so a reading
/// hovering at the threshold does not raise it over and over.
pub const ALARM_REARMS: f64 = 30.0;
/// The alarm's OK key: this many pixels at the right end of the strip.
pub const ALARM_OK_WIDTH: u32 = 120;

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
}

/// The critical alarm, as the renderer needs it.
#[derive(Clone, Debug, PartialEq)]
pub struct Alarm {
    /// The reading it names: the most recent to go critical.
    pub signal: String,
    /// The label of the value widget that shows it.
    pub label: String,
    /// The reading now, formatted as its widget shows it.
    pub value: String,
    pub unit: String,
    /// How many readings it holds, this one included. OK acknowledges all.
    pub count: usize,
    /// The OK key's pixels on the strip: a tap there acknowledges.
    pub ok: Range<u32>,
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

/// What a finger rests on, drawn lit: a zone whose tap would act, or the
/// alarm's OK key.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Lit {
    Zone(Row, usize),
    AlarmOk,
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
    /// That widget's label, for the alarm.
    label: String,
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
}

impl Engine {
    pub fn new(config: Config) -> Self {
        let font = config.settings.font;
        let theme = Theme {
            accent: config.settings.accent,
            ..Theme::default()
        };
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
        Some(Alarm {
            signal: signal.clone(),
            label: watch.label.clone(),
            value,
            unit,
            count: self.alarm.len(),
            ok: crate::WIDTH - ALARM_OK_WIDTH..crate::WIDTH,
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
        self.widgets(row)[zone].hold()?;
        Some(Hold {
            zone: self.zone_span(row, zone),
            progress: ((self.now - start) / LONG_PRESS).clamp(0.0, 1.0) as f32,
        })
    }

    /// True while a page is springing into place and frames are due as fast
    /// as the device takes them.
    pub fn animating(&self) -> bool {
        matches!(self.motion, Motion::Settle { .. })
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
                // A change, not a first reading: the daemon starting up
                // is not news.
                let changed = self.signals.get(&name).is_some_and(|old| *old != value);
                let blank = matches!(&value, Value::Text(t) if t.trim().is_empty());
                self.signals.set(&name, value);
                self.dirty |= visible;
                self.watch(&name);
                if changed && !blank && !on_page {
                    self.take_over(&name);
                }
            }
            Input::Forget(name) => {
                let visible = self.visible(&name);
                self.dirty |= visible;
                self.signals.remove(&name);
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
                });
                if !same {
                    self.pressed = None;
                    self.dirty = true;
                }
            }
            Input::EndTakeover => self.uncover(),
            Input::Acknowledge => self.acknowledge(),
            Input::Redraw => self.dirty = true,
        }
    }

    /// The new frame, if the picture changed since the last one. `mono` is
    /// monotonic time in seconds, on the same clock as touch samples.
    pub fn frame(&mut self, now: &Zoned, mono: f64) -> Option<Canvas> {
        self.tick(mono);
        if self.motion != Motion::Still {
            self.step(mono);
        }
        if let Some(offset) = self.offset() {
            return Some(self.slide(offset, now));
        }
        if let Some(alarm) = self.alarm() {
            if !self.dirty {
                return None;
            }
            self.dirty = false;
            self.drawn_minute = None;
            let lit = self.pressed == Some(Lit::AlarmOk);
            return Some(self.renderer.render_alarm(
                &alarm.label,
                &alarm.value,
                &alarm.unit,
                ALARM_OK_WIDTH,
                lit,
            ));
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
                self.uncover();
            }
        }
        let minute = self
            .clock_shown()
            .then(|| now.timestamp().as_second().div_euclid(60));
        if !self.dirty && minute == self.drawn_minute {
            return None;
        }
        let canvas = match self.cover {
            None => self.render_row(Row::Page(self.page), now),
            Some(c) => {
                let over = Row::Takeover(c.index);
                let (left, width) = self.span(over);
                if width == crate::WIDTH {
                    self.render_row(over, now)
                } else {
                    let mut page = self.render_row(Row::Page(self.page), now);
                    page.copy_shifted(&self.render_row(over, now), left as i32);
                    page
                }
            }
        };
        self.dirty = false;
        self.drawn_minute = minute;
        Some(canvas)
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
        if self.clock_shown() {
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
            let ok = crate::WIDTH - ALARM_OK_WIDTH;
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
                if let Some(action) = self.widget_at(x).and_then(Widget::tap).cloned() {
                    self.act(action);
                }
            }
            Gesture::LongPress { x } => {
                if let Some(action) = self.widget_at(x).and_then(Widget::hold).cloned() {
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

    /// Where a row sits: its first pixel and its width.
    fn span(&self, row: Row) -> (u32, u32) {
        match row {
            Row::Page(_) => (0, crate::WIDTH),
            Row::Takeover(i) => self.config.takeovers[i].span(),
        }
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
    fn render_row(&mut self, row: Row, now: &Zoned) -> Canvas {
        let mut zones = bind_page(self.widgets(row), &self.signals, &self.config.settings, now);
        self.show_sliders(row, &mut zones);
        let pressed = match self.pressed {
            Some(Lit::Zone(r, zone)) if r == row => Some(zone),
            _ => None,
        };
        let width = self.span(row).1;
        let background = self.theme.palette.background;
        self.renderer.render_row(&zones, pressed, width, background)
    }

    /// Shows the takeover that `signal` sets off, if there is one. The same
    /// takeover again only starts its time over. A different one waits for
    /// no finger: it would swap the widgets out from under a touch. None
    /// shows while the alarm is up, nor afterwards for this change.
    fn take_over(&mut self, signal: &str) {
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
        self.cover = Some(Cover {
            index,
            until: None,
            lease: None,
        });
        if !same {
            self.pressed = None;
            self.dirty = true;
        }
    }

    /// How long a takeover stays: its lease when shown on request.
    fn cover_seconds(&self, c: &Cover) -> f64 {
        c.lease.unwrap_or(self.config.takeovers[c.index].seconds)
    }

    fn uncover(&mut self) {
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
        self.widgets(row)
            .iter()
            .position(|w| {
                right += w.width() as f32;
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
        let widgets = self.widgets(row);
        let left = self.span(row).0 + widgets[..zone].iter().map(Widget::width).sum::<u32>();
        left..left + widgets[zone].width()
    }

    /// The length of a slider's bar in pixels: the zone less its padding,
    /// so a drag from one end of the bar to the other is the full range.
    fn bar_length(&self, row: Row, zone: usize) -> f32 {
        let pad = Theme::default().grid.pad_left;
        (self.widgets(row)[zone].width() as f32 - 2.0 * pad).max(1.0)
    }

    /// What a slider shows at rest, 0–1; 0 when it shows nothing.
    fn slider_value(&self, row: Row, zone: usize) -> f32 {
        let last = self.last_set.get(&(row, zone)).copied();
        match slider(&self.widgets(row)[zone], &self.signals, None, last) {
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
        for (i, widget) in self.widgets(row).iter().enumerate() {
            if matches!(widget, Widget::Slider { .. }) {
                let held = self
                    .slide
                    .filter(|s| s.zone == i && s.row == row)
                    .map(|s| s.fraction);
                let last = self.last_set.get(&(row, i)).copied();
                zones[i].view = slider(widget, &self.signals, held, last);
            }
        }
    }

    /// What lights under x when a finger lands: a zone whose tap or long
    /// press does something, or the alarm's OK key.
    fn lit_at(&self, x: f32) -> Option<Lit> {
        if !self.alarm.is_empty() {
            return (x >= (crate::WIDTH - ALARM_OK_WIDTH) as f32).then_some(Lit::AlarmOk);
        }
        self.hit(x)
            .filter(|&(row, zone)| {
                let w = &self.widgets(row)[zone];
                w.tap().is_some() || w.hold().is_some()
            })
            .map(|(row, zone)| Lit::Zone(row, zone))
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
        let mut frame = Canvas::new(
            crate::WIDTH,
            crate::HEIGHT,
            Theme::default().palette.background,
        );
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
        frame
    }

    fn snapshot(&mut self, page: usize, now: &Zoned) -> &Canvas {
        if !self.snapshots.contains_key(&page) {
            let mut zones = bind_page(
                &self.config.pages[page].widgets,
                &self.signals,
                &self.config.settings,
                now,
            );
            self.show_sliders(Row::Page(page), &mut zones);
            let canvas = self.renderer.render_page(&zones, None);
            self.snapshots.insert(page, canvas);
        }
        &self.snapshots[&page]
    }

    fn go_to(&mut self, index: usize) {
        self.uncover();
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
            self.theme = Theme {
                accent: config.settings.accent,
                ..Theme::default()
            };
            self.renderer = Renderer::new(self.theme.clone(), self.font.load());
        }
        self.motion = Motion::Still;
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
/// widgets is watched at the lowest `crit`, named by that widget's label.
fn watches(config: &Config, old: &HashMap<String, Watch>) -> HashMap<String, Watch> {
    let mut out: HashMap<String, Watch> = HashMap::new();
    for page in &config.pages {
        for (widget, signal, _, crit) in thresholds(&page.widgets) {
            let (Some(crit), Widget::Value { label, .. }) = (crit, widget) else {
                continue;
            };
            if out.get(signal).is_some_and(|w| w.crit <= crit) {
                continue;
            }
            let mut watch = old.get(signal).cloned().unwrap_or(Watch {
                crit,
                label: String::new(),
                critical: false,
                below_since: None,
                armed: true,
            });
            watch.crit = crit;
            watch.label = label.clone();
            out.insert(signal.to_string(), watch);
        }
    }
    out
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
        e.handle(Input::ShowPage(2)); // "Load"
        let mut c = Config::default_layout();
        c.pages.remove(0); // "Load" is now index 1
        e.handle(Input::Config(Box::new(c)));
        assert_eq!(e.page(), 1);
        assert_eq!(e.config().pages[e.page()].name, "Load");
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
        let lit = Theme::default().palette.pressed;
        let lit = [lit.r, lit.g, lit.b];

        e.handle(Input::Touch {
            down: true,
            x: 150.0,
            at: 10.0,
        });
        let frame = e.frame(&now, 10.0).expect("the first sample redraws");
        assert_eq!(pixel(&frame, 150, 2), lit, "media zone lit");
        assert_ne!(pixel(&frame, 50, 2), lit, "clock zone not lit");

        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: 10.1,
        });
        let frame = e.frame(&now, 10.1).expect("lifting redraws");
        assert_ne!(pixel(&frame, 150, 2), lit);

        // No action under the finger: nothing lights, nothing redraws.
        e.handle(Input::Touch {
            down: true,
            x: 250.0,
            at: 11.0,
        });
        assert!(e.frame(&now, 11.0).is_none());
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
        let pad = Theme::default().grid.pad_left;
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
        let pad = Theme::default().grid.pad_left;
        let mid = 300.0 + 100.0; // the middle of the seek bar
        drag(&mut e, 300.0 + pad, mid, 0.2);
        assert_eq!(e.take_effects(), [Effect::Seek(100.0)]);

        // The run slider's bar is 124 px; with nothing set yet it starts at
        // 0, so 62 px to the right is 50.
        drag(&mut e, 520.0, 582.0, 0.1);
        let fx = e.take_effects();
        let Some(Effect::Run(argv)) = fx.last() else {
            panic!("{fx:?}")
        };
        assert_eq!(argv, &["ddcutil", "setvcp", "10", "50"]);
        // Held still, it sets nothing; the last value set is where the
        // next drag starts from.
        drag(&mut e, 600.0, 600.0, 0.1);
        assert!(e.take_effects().is_empty());
        drag(&mut e, 600.0, 569.0, 0.1);
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
    fn a_forgotten_signal_shows_no_data_and_returns_as_new() {
        let mut e = covered();
        let now = at("16:17:05");
        e.handle(title("First"));
        e.handle(Input::ShowPage(1)); // the media page
        e.frame(&now, 0.0);
        e.handle(Input::Forget("media.title".into()));
        assert!(e.signals().get("media.title").is_none());
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
        let pad = Theme::default().grid.pad_left;
        let half = (200.0 - 2.0 * pad) / 2.0; // half the volume bar: 92 px
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
        let mut zones = bind_page(
            &e.config.pages[0].widgets,
            &e.signals,
            &e.config.settings,
            &at("16:17:05"),
        );
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
        assert_eq!(
            e.next_wait(&now, 10.25),
            Some(Duration::from_millis(250)),
            "wakes for the threshold"
        );
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
                alarm.unit.as_str()
            ),
            ("CPU", "92", "°C")
        );
        assert_eq!((alarm.count, alarm.ok.clone()), (1, 520..640));
        let frame = e.frame(&now, 1.0).expect("drawn at once");
        let crit = Theme::default().palette.crit;
        assert_eq!(pixel(&frame, 300, 2), [crit.r, crit.g, crit.b], "filled");
        // A tap off the OK key does nothing; on it, acknowledges.
        trace(&mut e, &[(2.0, 100.0), (2.05, 100.0)], 2.1);
        assert!(e.alarm().is_some());
        trace(&mut e, &[(3.0, 600.0), (3.05, 600.0)], 3.1);
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
    fn the_alarm_clears_itself_ten_seconds_after_the_reading_drops() {
        let mut e = alarmed();
        let now = at("16:17:05");
        reading(&mut e, 0.0, "cpu.temp", 95.0);
        reading(&mut e, 5.0, "cpu.temp", 80.0);
        assert_eq!(e.next_wait(&now, 5.0), Some(Duration::from_secs(10)));
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
        // A second reading has its own threshold and label.
        reading(&mut e, 2.0, "mem.used", 97.0);
        assert_eq!(
            (alarm_label(&e).as_deref(), e.alarm().unwrap().count),
            (Some("Load"), 3)
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
}
