//! The engine: the single owner of everything the strip shows.
//!
//! Inputs arrive one at a time through `handle`: signal readings, touches,
//! page commands, config reloads. The engine only records what changed. The
//! caller then asks for a `frame`, which is `Some` only when the picture
//! would differ from the last one sent, and for `next_wait`, how long until
//! the picture changes on its own (an animation step, or the clock's next
//! minute). Between those the daemon sleeps: there is no frame loop while
//! nothing changes.
//!
//! A takeover is a row of widgets that covers the page for a few seconds
//! when one of its signals changes. While it is up, everything that asks
//! "which widgets are on the strip" gets its widgets instead of the page's,
//! so drawing, taps and sliders work on it unchanged.
//!
//! The engine never reads a clock or touches a thread. Wall time (for the
//! clock widget) and monotonic time (for touch and animation, in seconds
//! from any origin) are passed in, so tests can drive it to any moment.

use std::collections::HashMap;
use std::time::Duration;

use jiff::Zoned;

use crate::canvas::Canvas;
use crate::config::{Action, Config, SLIDER_VALUE, SliderControl, Widget};
use crate::fonts::FontChoice;
use crate::gesture::{Gesture, MIN_TAP, Recognizer, should_commit};
use crate::layout::{bind_page, shows_clock, signals_used, slider};
use crate::signal::{Signals, Unit, Value};
use crate::theme::Theme;
use crate::widget::{Renderer, Zone};

const WIDTH: f32 = crate::WIDTH as f32;

/// Page-snap spring, from the Go app: slightly under-damped (critical
/// damping would be 2·√400 = 40) for a crisp stop with no visible bounce.
const STIFFNESS: f32 = 400.0;
const DAMPING: f32 = 36.0;

/// Something that happened.
#[derive(Clone, Debug)]
pub enum Input {
    /// A source published a reading.
    Signal(String, Value),
    /// A touch sample: finger down or not, panel x (0–639), and monotonic
    /// time in seconds.
    Touch { down: bool, x: f32, at: f64 },
    /// Show the page with this index. Out-of-range indexes are ignored.
    ShowPage(usize),
    /// Show the next page, or the previous one with -1. Stops at the ends.
    Step(i32),
    /// A new, already-validated config replaces the current one.
    Config(Box<Config>),
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

/// A row of widgets that can fill the strip.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum Row {
    Page(usize),
    Takeover(usize),
}

/// A takeover on the strip.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Cover {
    index: usize,
    /// Monotonic time it ends. `None` until the next frame, since a signal
    /// arrives without a time.
    until: Option<f64>,
}

/// A finger on a slider.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Slide {
    row: Row,
    zone: usize,
    /// When the finger landed: until `MIN_TAP` has passed it might be a
    /// lift bounce, so nothing is set yet.
    start: f64,
    /// The finger's position along the bar, 0–1.
    fraction: f32,
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

pub struct Engine {
    config: Config,
    signals: Signals,
    page: usize,
    renderer: Renderer,
    font: FontChoice,
    gestures: Recognizer,
    motion: Motion,
    /// Pages rendered once at the start of a swipe and slid around until
    /// it ends, so a swipe never waits on a render.
    snapshots: HashMap<usize, Canvas>,
    /// Monotonic time of the last animation step.
    stepped_at: Option<f64>,
    /// The picture must be redrawn on the next `frame` call.
    dirty: bool,
    /// The minute last drawn, for pages with a clock.
    drawn_minute: Option<i64>,
    /// The zone a finger rests on, if a tap there would act. Drawn lit.
    pressed: Option<usize>,
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
    /// A swipe dismissed a takeover: the rest of it, up to the lift, does
    /// nothing.
    dismissing: bool,
}

impl Engine {
    pub fn new(config: Config) -> Self {
        let font = config.settings.font;
        Self {
            renderer: Renderer::new(Theme::default(), font.load()),
            font,
            config,
            signals: Signals::new(),
            page: 0,
            gestures: Recognizer::new(),
            motion: Motion::Still,
            snapshots: HashMap::new(),
            stepped_at: None,
            dirty: true,
            drawn_minute: None,
            pressed: None,
            effects: Vec::new(),
            slide: None,
            last_set: HashMap::new(),
            cover: None,
            dismissing: false,
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

    /// Index of the page on the strip.
    pub fn page(&self) -> usize {
        self.page
    }

    /// The name of the takeover on the strip, if one is.
    pub fn takeover(&self) -> Option<&str> {
        self.cover
            .map(|c| self.config.takeovers[c.index].name.as_str())
    }

    /// True while a page is springing into place and frames are due as fast
    /// as the device takes them.
    pub fn animating(&self) -> bool {
        matches!(self.motion, Motion::Settle { .. })
    }

    pub fn handle(&mut self, input: Input) {
        match input {
            Input::Signal(name, value) => {
                let visible = signals_used(self.shown()).any(|s| s == name);
                let on_page =
                    signals_used(&self.config.pages[self.page].widgets).any(|s| s == name);
                // A change, not a first reading: the daemon starting up
                // is not news.
                let changed = self.signals.get(&name).is_some_and(|old| *old != value);
                let blank = matches!(&value, Value::Text(t) if t.trim().is_empty());
                self.signals.set(&name, value);
                self.dirty |= visible;
                if changed && !blank && !on_page {
                    self.take_over(&name);
                }
            }
            Input::Touch { down, x, at } => {
                // Any touch keeps a takeover up for its full time again.
                if let Some(cover) = &mut self.cover {
                    cover.until = Some(at + self.config.takeovers[cover.index].seconds);
                }
                let on_slider = self.slide.is_some()
                    || (down
                        && !self.gestures.pressed()
                        && self.motion == Motion::Still
                        && self.slider_at(x).is_some());
                if on_slider {
                    self.slide_touch(down, x, at);
                    return;
                }
                let landing = down && !self.gestures.pressed();
                let gesture = self.gestures.update(down, x, at);
                // Light the zone as soon as the finger lands, so the strip
                // answers before the gesture is known; a swipe turns it off.
                let mut pressed = match () {
                    _ if !self.gestures.pressed() => None,
                    _ if landing && self.motion == Motion::Still => self.actionable_zone(x),
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
            }
            Input::ShowPage(i) => self.go_to(i),
            Input::Step(delta) => {
                let last = self.config.pages.len() - 1;
                let target = self.page.saturating_add_signed(delta as isize).min(last);
                self.go_to(target);
            }
            Input::Config(config) => self.reload(*config),
            Input::Redraw => self.dirty = true,
        }
    }

    /// The new frame, if the picture changed since the last one. `mono` is
    /// monotonic time in seconds, on the same clock as touch samples.
    pub fn frame(&mut self, now: &Zoned, mono: f64) -> Option<Canvas> {
        if self.motion != Motion::Still {
            self.step(mono);
        }
        if let Some(offset) = self.offset() {
            return Some(self.slide(offset, now));
        }
        if let Some(cover) = &mut self.cover {
            let until = *cover
                .until
                .get_or_insert(mono + self.config.takeovers[cover.index].seconds);
            // Never pulled from under a finger.
            if mono >= until && !self.gestures.pressed() && self.slide.is_none() {
                self.uncover();
            }
        }
        let widgets = self.shown();
        let minute = shows_clock(widgets).then(|| now.timestamp().as_second().div_euclid(60));
        if !self.dirty && minute == self.drawn_minute {
            return None;
        }
        let mut zones = bind_page(widgets, &self.signals, &self.config.settings, now);
        self.show_sliders(self.row(), &mut zones);
        self.dirty = false;
        self.drawn_minute = minute;
        Some(self.renderer.render_page(&zones, self.pressed))
    }

    /// How long until the picture changes without any input: no time at
    /// all while animating, otherwise the sooner of a takeover ending and
    /// the next minute on a clock, or never. `mono` is as for `frame`.
    pub fn next_wait(&self, now: &Zoned, mono: f64) -> Option<Duration> {
        if self.animating() {
            return Some(Duration::ZERO);
        }
        let minute = shows_clock(self.shown()).then(|| {
            let ms = now.timestamp().as_millisecond().rem_euclid(60_000);
            Duration::from_millis((60_000 - ms) as u64)
        });
        // While a finger is down the next touch sample is the wake-up.
        let touching = self.gestures.pressed() || self.slide.is_some();
        let ends = self.cover.filter(|_| !touching).map(|c| {
            let until = c
                .until
                .unwrap_or(mono + self.config.takeovers[c.index].seconds);
            Duration::from_secs_f64((until - mono).max(0.0))
        });
        match (minute, ends) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn gesture(&mut self, gesture: Gesture) {
        if self.dismissing {
            self.dismissing = !matches!(gesture, Gesture::Release { .. });
            return;
        }
        // A swipe on a takeover puts the page back; it does not also move
        // the page underneath.
        if self.cover.is_some() && matches!(gesture, Gesture::Drag { .. }) {
            self.uncover();
            self.dismissing = true;
            return;
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
                    self.act(action);
                }
            }
        }
    }

    /// What fills the strip when it is still: the takeover, or the page.
    fn row(&self) -> Row {
        match self.cover {
            Some(c) => Row::Takeover(c.index),
            None => Row::Page(self.page),
        }
    }

    fn widgets(&self, row: Row) -> &[Widget] {
        match row {
            Row::Page(i) => &self.config.pages[i].widgets,
            Row::Takeover(i) => &self.config.takeovers[i].widgets,
        }
    }

    /// The widgets on the strip.
    fn shown(&self) -> &[Widget] {
        self.widgets(self.row())
    }

    /// Shows the takeover that `signal` sets off, if there is one. The same
    /// takeover again only starts its time over. A different one waits for
    /// no finger: it would swap the widgets out from under a touch.
    fn take_over(&mut self, signal: &str) {
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
        self.cover = Some(Cover { index, until: None });
        if !same {
            self.pressed = None;
            self.dirty = true;
        }
    }

    fn uncover(&mut self) {
        if self.cover.take().is_some() {
            self.pressed = None;
            self.slide = None;
            self.dirty = true;
        }
    }

    /// Index of the widget under panel x on the strip.
    fn zone_at(&self, x: f32) -> Option<usize> {
        let mut right = 0.0;
        self.shown()
            .iter()
            .position(|w| {
                right += w.width() as f32;
                x < right
            })
            .filter(|_| x >= 0.0)
    }

    fn widget_at(&self, x: f32) -> Option<&Widget> {
        self.zone_at(x).map(|i| &self.shown()[i])
    }

    /// The slider under x, if there is one.
    fn slider_at(&self, x: f32) -> Option<usize> {
        self.zone_at(x)
            .filter(|&i| matches!(self.shown()[i], Widget::Slider { .. }))
    }

    /// Where x falls along a slider's bar, 0–1. The bar spans the zone less
    /// its padding, so the ends are reachable without leaving the zone.
    fn slider_fraction(&self, zone: usize, x: f32) -> f32 {
        let widgets = self.shown();
        let left: u32 = widgets[..zone].iter().map(Widget::width).sum();
        let pad = Theme::default().grid.pad_left;
        let span = (widgets[zone].width() as f32 - 2.0 * pad).max(1.0);
        ((x - left as f32 - pad) / span).clamp(0.0, 1.0)
    }

    /// A touch while on a slider. The bar follows the finger from the first
    /// sample; values are only set once the touch has outlasted a lift
    /// bounce. The lift sample reports x = 0, so the release keeps the last
    /// position.
    fn slide_touch(&mut self, down: bool, x: f32, at: f64) {
        let Some(mut slide) = self.slide.take().or_else(|| {
            let zone = self.slider_at(x)?;
            Some(Slide {
                row: self.row(),
                zone,
                start: at,
                fraction: self.slider_fraction(zone, x),
                sent: None,
            })
        }) else {
            return;
        };
        self.dirty = true;
        let settled = at - slide.start >= MIN_TAP;
        if down {
            slide.fraction = self.slider_fraction(slide.zone, x);
            if settled {
                self.slid(&mut slide, false);
            }
            self.slide = Some(slide);
        } else if settled {
            self.slid(&mut slide, true);
        }
    }

    /// Turns a slider position into effects: volume and commands as the
    /// value changes, a seek on release.
    fn slid(&mut self, slide: &mut Slide, done: bool) {
        let Widget::Slider { controls, .. } = &self.widgets(slide.row)[slide.zone] else {
            return;
        };
        let percent = (slide.fraction * 100.0).round() as u8;
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

    /// Replaces slider views with ones that show the finger while sliding,
    /// and a `run` slider's last value at rest.
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

    /// The zone under x, if a tap or a long press there does something.
    fn actionable_zone(&self, x: f32) -> Option<usize> {
        self.zone_at(x).filter(|&i| {
            let w = &self.shown()[i];
            w.tap().is_some() || w.hold().is_some()
        })
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
        self.dismissing = false;
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
        if config.settings.font != self.font {
            self.font = config.settings.font;
            self.renderer = Renderer::new(Theme::default(), self.font.load());
        }
        self.motion = Motion::Still;
        self.pressed = None;
        self.slide = None;
        self.cover = None;
        self.dismissing = false;
        self.last_set.clear();
        self.snapshots.clear();
        self.dirty = true;
        self.config = config;
    }
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
        let samples = (secs * 500.0) as usize;
        let mut t = 10.0;
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
            "starts near the finger's landing: {levels:?}"
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

        drag(&mut e, 600.0, 600.0, 0.1);
        let fx = e.take_effects();
        let Some(Effect::Run(argv)) = fx.last() else {
            panic!("{fx:?}")
        };
        let value: u8 = argv[3].parse().unwrap();
        assert_eq!(&argv[..3], ["ddcutil", "setvcp", "10"]);
        assert!((70..=90).contains(&value), "{value}");
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
        drag(&mut e, 300.0, 200.0, 0.2);
        let fx = e.take_effects();
        assert!(matches!(fx.last(), Some(Effect::Volume(30..=35))), "{fx:?}");
        // The level the drag set comes back as a reading: still the same
        // takeover, not a new one.
        e.handle(level(32.0));
        assert_eq!(e.takeover(), Some("Volume"));
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
}
