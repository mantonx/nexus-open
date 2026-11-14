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
//! The engine never reads a clock or touches a thread. Wall time (for the
//! clock widget) and monotonic time (for touch and animation, in seconds
//! from any origin) are passed in, so tests can drive it to any moment.

use std::collections::HashMap;
use std::time::Duration;

use jiff::Zoned;

use crate::canvas::Canvas;
use crate::config::Config;
use crate::fonts::FontChoice;
use crate::gesture::{Gesture, Recognizer, should_commit};
use crate::layout::{bind_page, shows_clock, signals_used};
use crate::signal::{Signals, Value};
use crate::theme::Theme;
use crate::widget::Renderer;

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
        }
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

    /// True while a page is springing into place and frames are due as fast
    /// as the device takes them.
    pub fn animating(&self) -> bool {
        matches!(self.motion, Motion::Settle { .. })
    }

    pub fn handle(&mut self, input: Input) {
        match input {
            Input::Signal(name, value) => {
                let visible = signals_used(&self.config.pages[self.page]).any(|s| s == name);
                self.signals.set(&name, value);
                self.dirty |= visible;
            }
            Input::Touch { down, x, at } => {
                if let Some(gesture) = self.gestures.update(down, x, at) {
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
        let page = &self.config.pages[self.page];
        let minute = shows_clock(page).then(|| now.timestamp().as_second().div_euclid(60));
        if !self.dirty && minute == self.drawn_minute {
            return None;
        }
        let zones = bind_page(page, &self.signals, &self.config.settings, now);
        self.dirty = false;
        self.drawn_minute = minute;
        Some(self.renderer.render_page(&zones))
    }

    /// How long until the picture changes without any input: no time at
    /// all while animating, the start of the next minute if the page shows
    /// a clock, otherwise never.
    pub fn next_wait(&self, now: &Zoned) -> Option<Duration> {
        if self.animating() {
            return Some(Duration::ZERO);
        }
        if !shows_clock(&self.config.pages[self.page]) {
            return None;
        }
        let ms = now.timestamp().as_millisecond().rem_euclid(60_000);
        Some(Duration::from_millis((60_000 - ms) as u64))
    }

    fn gesture(&mut self, gesture: Gesture) {
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
            // Taps and long presses get actions in Phase 3.
            Gesture::Tap { .. } | Gesture::LongPress { .. } => {}
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
            let zones = bind_page(
                &self.config.pages[page],
                &self.signals,
                &self.config.settings,
                now,
            );
            let canvas = self.renderer.render_page(&zones);
            self.snapshots.insert(page, canvas);
        }
        &self.snapshots[&page]
    }

    fn go_to(&mut self, index: usize) {
        self.motion = Motion::Still;
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
        assert_eq!(e.next_wait(&at("16:17:05")), Some(Duration::from_secs(55)));
        e.handle(Input::ShowPage(1)); // Performance: no clock
        assert_eq!(e.next_wait(&at("16:17:05")), None);
    }

    #[test]
    fn stepping_stops_at_the_ends() {
        let mut e = Engine::new(Config::default_layout());
        e.handle(Input::Step(-1));
        assert_eq!(e.page(), 0);
        e.handle(Input::Step(10));
        assert_eq!(e.page(), 3);
        e.handle(Input::ShowPage(99));
        assert_eq!(e.page(), 3);
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
}
