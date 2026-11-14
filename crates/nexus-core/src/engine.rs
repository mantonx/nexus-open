//! The engine: the single owner of everything the strip shows.
//!
//! Inputs arrive one at a time through `handle`: signal readings, page
//! commands, config reloads. The engine only records what changed. The
//! caller then asks for a `frame`, which is `Some` only when the picture
//! would differ from the last one sent, and for the `next_deadline`, the
//! next moment the picture changes on its own (the clock's minute). Between
//! those, the daemon sleeps: there is no frame loop while nothing changes.
//!
//! The engine never reads the clock or touches a thread. Time is passed in,
//! so tests can drive it to any moment.

use jiff::{Timestamp, Zoned};

use crate::canvas::Canvas;
use crate::config::Config;
use crate::fonts::FontChoice;
use crate::layout::{bind_page, shows_clock, signals_used};
use crate::signal::{Signals, Value};
use crate::theme::Theme;
use crate::widget::Renderer;

/// Something that happened.
#[derive(Clone, Debug)]
pub enum Input {
    /// A source published a reading.
    Signal(String, Value),
    /// Show the page with this index. Out-of-range indexes are ignored.
    ShowPage(usize),
    /// Show the next page, or the previous one with -1. Stops at the ends.
    Step(i32),
    /// A new, already-validated config replaces the current one.
    Config(Box<Config>),
    /// The device came back; it needs the whole picture again.
    Redraw,
}

pub struct Engine {
    config: Config,
    signals: Signals,
    page: usize,
    renderer: Renderer,
    font: FontChoice,
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

    pub fn handle(&mut self, input: Input) {
        match input {
            Input::Signal(name, value) => {
                let visible = signals_used(&self.config.pages[self.page]).any(|s| s == name);
                self.signals.set(&name, value);
                self.dirty |= visible;
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

    /// The new frame, if the picture changed since the last one.
    pub fn frame(&mut self, now: &Zoned) -> Option<Canvas> {
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

    /// When the picture next changes without any input: the start of the
    /// next minute if the page shows a clock, otherwise never.
    pub fn next_deadline(&self, now: Timestamp) -> Option<Timestamp> {
        if !shows_clock(&self.config.pages[self.page]) {
            return None;
        }
        let next_minute = (now.as_second().div_euclid(60) + 1) * 60;
        Timestamp::from_second(next_minute).ok()
    }

    fn go_to(&mut self, index: usize) {
        if index < self.config.pages.len() && index != self.page {
            self.page = index;
            self.dirty = true;
        }
    }

    /// Swaps in a new config without a visible jump: the same page stays on
    /// screen if it still exists by name, and nothing is drawn if the
    /// visible result is identical.
    fn reload(&mut self, config: Config) {
        let current = &self.config.pages[self.page].name;
        self.page = config
            .page_index(current)
            .unwrap_or(self.page.min(config.pages.len() - 1));
        if config.settings.font != self.font {
            self.font = config.settings.font;
            self.renderer = Renderer::new(Theme::default(), self.font.load());
        }
        self.dirty = true;
        self.config = config;
    }
}

/// How long to wait from `now` until `deadline`; zero if it has passed.
pub fn until(deadline: Timestamp, now: Timestamp) -> std::time::Duration {
    // A negative duration fails to convert, which is exactly "already due".
    std::time::Duration::try_from(deadline.duration_since(now)).unwrap_or_default()
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
        assert!(e.frame(&at("16:17:05")).is_some(), "first frame");
        assert!(e.frame(&at("16:17:30")).is_none(), "nothing changed");

        e.handle(temp("cpu.temp", 50.0));
        assert!(e.frame(&at("16:17:31")).is_some(), "visible signal changed");

        e.handle(temp("cpu.load", 20.0));
        assert!(
            e.frame(&at("16:17:32")).is_none(),
            "cpu.load is not on page 1"
        );

        assert!(e.frame(&at("16:18:00")).is_some(), "the minute turned");
    }

    #[test]
    fn hidden_signals_are_still_recorded() {
        let mut e = Engine::new(Config::default_layout());
        e.handle(temp("cpu.load", 20.0));
        e.handle(Input::ShowPage(2));
        assert_eq!(e.signals().history("cpu.load"), [20.0]);
        assert!(e.frame(&at("16:17:05")).is_some());
    }

    #[test]
    fn deadline_is_next_minute_only_with_a_clock() {
        let mut e = Engine::new(Config::default_layout());
        let now = at("16:17:05").timestamp();
        assert_eq!(e.next_deadline(now), Some(at("16:18:00").timestamp()));
        e.handle(Input::ShowPage(1)); // Performance: no clock
        assert_eq!(e.next_deadline(now), None);
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
}
