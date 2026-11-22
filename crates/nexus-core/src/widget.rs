//! Widgets and the page renderer: the design of `DESIGN.md`, drawn.
//!
//! A page is a row of zones. Each zone has a width and a `WidgetView`: the
//! already-formatted data a widget shows. Turning live signals into views
//! (formatting, thresholds, scales) happens before this point (`layout`),
//! so drawing stays a pure function of view, touch, theme and fonts.
//!
//! Every zone holds one card, `theme::CARD_INSET` in from its edges, so
//! zones that add up to 640 leave the design's gutters between cards.
//! What a card shows depends on its size class (`class`): more width shows
//! more, or shows it bigger. Every size and colour comes from `theme`.

use std::ops::Range;

use tiny_skia::Path;

use crate::canvas::Canvas;
use crate::config::MediaCommand;
use crate::draw::{self, rrect_path};
use crate::icons;
use crate::text::{Font, TextEngine};
use crate::theme::*;
use crate::{HEIGHT, WIDTH};

/// The fonts a renderer draws with.
#[derive(Clone)]
pub struct Fonts {
    /// Words: titles, labels, key labels, dates. Rubik unless the user
    /// chose another face.
    pub text: Font,
    /// Numerals and their units, always: Rubik, with tabular figures.
    pub numerals: Font,
    /// Tried in order for characters the faces above lack, such as arrows
    /// or symbols in a song title.
    pub fallback: Vec<Font>,
    /// Icons for things: Phosphor Fill.
    pub icon: Font,
    /// Icons for actions on keys and for empty states: Phosphor Bold.
    pub icon_bold: Font,
}

/// How a value is judged against its thresholds.
/// Ordered from calm to critical, so the worst of several is their `max`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    #[default]
    Ok,
    Warn,
    Crit,
}

/// Whether a reading has something to show. The three ways of having
/// nothing never look alike: dashes on a normal card (no data), a dashed
/// outline with no card (not set up), a grey number with a broken link
/// (source lost).
#[derive(Clone, Debug, Default, PartialEq)]
pub enum Avail {
    #[default]
    Live,
    /// The source works but has given no reading yet.
    NoData,
    /// Nothing is chosen to show: no signal, no weather location.
    NotSetUp,
    /// The source stopped; the last value is kept, with the time it was
    /// last heard from ("10:41").
    Lost(String),
}

/// A reading's place on its fixed scale and the scale's limits, as
/// fractions 0–1 of the bar.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Bar {
    pub value: Option<f32>,
    pub warn: Option<f32>,
    pub crit: Option<f32>,
}

/// One reading, formatted.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    /// Its name: "CPU", "load".
    pub name: String,
    /// The numeral as shown: "115°", "62", "1.3".
    pub value: String,
    /// Drawn smaller after the numeral: "%", "kB/s"; or empty.
    pub unit: String,
    pub severity: Severity,
    pub avail: Avail,
    /// Its fixed-scale bar; `None` for a reading with no scale.
    pub bar: Option<Bar>,
}

/// A track as the media widget shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub title: String,
    /// Empty when the player gives none.
    pub artist: String,
    /// Seconds in, and the track's length; `None` when unknown.
    pub position: Option<f64>,
    pub length: Option<f64>,
    /// False while paused.
    pub playing: bool,
    /// Seconds since the title last started its scroll, while it may still
    /// be scrolling; `None` at rest.
    pub marquee: Option<f32>,
}

impl Track {
    /// 0–1 through the track, when its length is known.
    pub fn progress(&self) -> Option<f32> {
        let len = self.length.filter(|l| *l > 0.0)?;
        Some((self.position.unwrap_or(0.0) / len).clamp(0.0, 1.0) as f32)
    }
}

/// What a zone shows, already formatted.
#[derive(Clone, Debug, PartialEq)]
pub enum WidgetView {
    Clock {
        /// "10:53"
        time: String,
        /// "PM" on a 12-hour clock.
        meridiem: Option<String>,
        /// "Sunday", "Sun"
        weekday: String,
        weekday_short: String,
        /// "November 22", "Nov 22"
        date: String,
        date_short: String,
    },
    /// A reading with its bar, and optionally a second one beside it.
    Value {
        icon: char,
        reading: Reading,
        /// The limit in words on hero cards: "limit 167°".
        limit: Option<String>,
        second: Option<Reading>,
    },
    Weather {
        icon: char,
        /// "58°"
        temp: String,
        /// "Cloudy"
        condition: String,
        /// "61° / 49°"
        high_low: Option<String>,
        avail: Avail,
    },
    /// Download and upload rates, each on a logarithmic bar.
    Rate { down: Reading, up: Reading },
    /// The track playing, or `None` when nothing is.
    Media(Option<Track>),
    Slider {
        icon: char,
        label: String,
        /// "40", "Muted", "1:05 / 4:20", or "––" with nothing to show.
        value: String,
        /// "%" after a number.
        unit: String,
        /// 0–1; `None` when there is nothing to show.
        fraction: Option<f32>,
        /// A finger is on it: the thumb grows and glows.
        active: bool,
        muted: bool,
        /// The value before the change that brought the takeover up: shown
        /// as a mark, with the change from it.
        from: Option<f32>,
    },
    /// A control to press.
    Key {
        icon: Option<char>,
        label: Option<String>,
        sub: Option<String>,
        /// False when it does nothing yet: drawn as "set up".
        set_up: bool,
    },
}

impl WidgetView {
    /// How bad its readings are, for dimming and page markers.
    pub fn severity(&self) -> Severity {
        match self {
            WidgetView::Value {
                reading, second, ..
            } => second
                .as_ref()
                .map_or(reading.severity, |s| s.severity.max(reading.severity)),
            _ => Severity::Ok,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub width: u32,
    pub view: WidgetView,
}

/// A finger on a row: the zone under it, the part of the zone (a
/// transport key) if any, and how far a long press has come.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Touch {
    pub zone: usize,
    pub part: Option<usize>,
    /// 0–1 of the way to the long press firing, while one is on its way.
    pub hold: Option<f32>,
}

/// One page's marker.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    pub current: bool,
    pub severity: Severity,
}

/// The critical alarm, worded.
#[derive(Clone, Debug, PartialEq)]
pub struct AlarmView {
    /// The reading's icon.
    pub icon: char,
    /// "GPU is too hot"
    pub headline: String,
    /// "Over its 90° limit · check the fans"
    pub detail: String,
    /// "92°"
    pub value: String,
}

/// Size classes, by the widget's width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Class {
    Compact,
    Medium,
    Hero,
}

pub fn class(width: u32) -> Class {
    if width <= COMPACT_MAX {
        Class::Compact
    } else if width <= MEDIUM_MAX {
        Class::Medium
    } else {
        Class::Hero
    }
}

/// A touch target inside a media widget.
#[derive(Clone, Debug, PartialEq)]
pub struct MediaKey {
    /// Its pixels, from the zone's left edge.
    pub span: Range<f32>,
    pub command: MediaCommand,
    /// Drawn as a key (a long press is a slow tap), rather than the whole
    /// card standing for one (a long press is the widget's own).
    pub key: bool,
}

/// The touch targets of a media widget `width` wide that shows a track:
/// the keys it draws, or the whole card at medium width, which is the
/// play/pause key. Shared by drawing and the engine's hit test, so what is
/// drawn is what is hit.
pub fn media_keys(width: u32) -> Vec<MediaKey> {
    let x = CARD_INSET;
    let w = width as f32 - 2.0 * CARD_INSET;
    let key = |at: f32, kw: f32, command| MediaKey {
        span: at..at + kw,
        command,
        key: true,
    };
    let three = |x0: f32, kw: f32| {
        [
            MediaCommand::Previous,
            MediaCommand::PlayPause,
            MediaCommand::Next,
        ]
        .into_iter()
        .enumerate()
        .map(|(i, c)| key(x0 + i as f32 * (kw + KEY_GAP), kw, c))
        .collect()
    };
    if width >= MEDIA_SCENE_MIN {
        let keys_w = 3.0 * SCENE_KEY_W + 2.0 * KEY_GAP;
        return three(x + w - 4.0 - keys_w, SCENE_KEY_W);
    }
    match class(width) {
        Class::Compact => vec![key(x + PAD - 3.0, TARGET_MIN, MediaCommand::PlayPause)],
        Class::Medium => vec![MediaKey {
            span: x..x + w,
            command: MediaCommand::PlayPause,
            key: false,
        }],
        Class::Hero => {
            if width >= MEDIA_KEYS_MIN {
                let keys_w = 3.0 * TARGET_MIN + 2.0 * KEY_GAP;
                three(x + w - PAD + 3.0 - keys_w, TARGET_MIN)
            } else {
                let kx = x + w - PAD + 3.0 - TARGET_MIN;
                vec![key(kx, TARGET_MIN, MediaCommand::PlayPause)]
            }
        }
    }
}

/// Colours for a card at one severity.
#[derive(Clone, Copy)]
struct Ink {
    surface: Rgb,
    num: Rgb,
    label: Rgb,
    quiet: Rgb,
    icon: Rgb,
    fill: Rgb,
    track: Rgb,
}

/// The ink for one reading on a card of `card` severity. On a warning card
/// a reading that is itself fine keeps the calm colours, so only the one at
/// fault is amber: an amber numeral always means a warning. A critical card
/// is white throughout.
fn reading_ink(card: Severity, own: Severity) -> Ink {
    let k = ink(card);
    if card == Severity::Warn && own == Severity::Ok {
        Ink {
            surface: k.surface,
            track: k.track,
            ..ink(Severity::Ok)
        }
    } else {
        k
    }
}

fn ink(sev: Severity) -> Ink {
    match sev {
        Severity::Ok => Ink {
            surface: CARD,
            num: TEXT,
            label: TEXT2,
            quiet: TEXT3,
            icon: TEXT2,
            fill: FILL,
            track: TRACK,
        },
        Severity::Warn => Ink {
            surface: WARN_SURFACE,
            num: WARN,
            label: WARN.mix(TEXT, 0.55),
            quiet: WARN.mix(WARN_SURFACE, 0.35),
            icon: WARN,
            fill: WARN,
            track: WARN_SURFACE.mix(GROUND, 0.55),
        },
        Severity::Crit => Ink {
            surface: CRIT,
            num: CRIT_INK,
            label: CRIT_INK,
            quiet: CRIT_INK.mix(CRIT, 0.25),
            icon: CRIT_INK,
            fill: CRIT_INK,
            track: CRIT_DEEP,
        },
    }
}

/// A reading whose source is lost: greyed.
fn lost_ink() -> Ink {
    Ink {
        num: TEXT3,
        icon: TEXT3,
        fill: TEXT3.mix(CARD, 0.3),
        ..ink(Severity::Ok)
    }
}

/// Draws pages. Keep one per display: it caches rasterized glyphs.
pub struct Renderer {
    theme: Theme,
    fonts: Fonts,
    /// Words: the text face, then Rubik, then the fallbacks.
    words: Vec<Font>,
    /// Numerals: Rubik, then the fallbacks.
    nums: Vec<Font>,
    text: TextEngine,
    /// The columns of warning and critical cards in the last frame drawn.
    alerts: Vec<Range<u32>>,
    /// A title in the last frame drawn is part way through its scroll.
    moving: bool,
}

/// Which face a piece of text is set in.
#[derive(Clone, Copy, PartialEq)]
enum Face {
    Num,
    Word,
}

/// The state a card is drawn in, from the touch on its row.
#[derive(Clone, Copy, Default)]
struct Press {
    /// A finger is on it (or on `part`), and lifting would act.
    on: bool,
    part: Option<usize>,
    hold: Option<f32>,
}

impl Renderer {
    pub fn new(theme: Theme, fonts: Fonts) -> Self {
        let mut words = vec![fonts.text.clone(), fonts.numerals.clone()];
        words.extend(fonts.fallback.iter().cloned());
        let mut nums = vec![fonts.numerals.clone()];
        nums.extend(fonts.fallback.iter().cloned());
        Self {
            theme,
            fonts,
            words,
            nums,
            text: TextEngine::new(),
            alerts: Vec::new(),
            moving: false,
        }
    }

    /// The theme it draws with, the user's accent included.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// The columns of the warning and critical cards in the last row
    /// drawn, from its left edge: dimming leaves them bright.
    pub fn alerts(&self) -> &[Range<u32>] {
        &self.alerts
    }

    /// Whether a title in the last row drawn is still in its scroll, so
    /// more frames are due.
    pub fn moving(&self) -> bool {
        self.moving
    }

    /// One page of zones, left to right, on the whole strip, without its
    /// page markers (`draw_markers` adds them).
    pub fn render_page(&mut self, zones: &[Zone], touch: Option<Touch>) -> Canvas {
        self.render_row(zones, touch, WIDTH, false)
    }

    /// A row of zones in a new frame `width` pixels wide: a page is the
    /// whole strip, a takeover over one end of it less. `raised` draws its
    /// cards as on top of a page.
    pub fn render_row(
        &mut self,
        zones: &[Zone],
        touch: Option<Touch>,
        width: u32,
        raised: bool,
    ) -> Canvas {
        let mut canvas = Canvas::new(width, HEIGHT, GROUND);
        self.alerts.clear();
        self.moving = false;
        let mut x = 0u32;
        for (i, zone) in zones.iter().enumerate() {
            let press = match touch {
                Some(t) if t.zone == i => Press {
                    on: true,
                    part: t.part,
                    hold: t.hold,
                },
                _ => Press::default(),
            };
            if zone.view.severity() >= Severity::Warn {
                self.alerts.push(x..x + zone.width);
            }
            let mut pen = self.pen(&mut canvas);
            pen.raised = raised;
            pen.zone(x as f32, zone.width, &zone.view, press);
            self.moving |= pen.moving;
            x += zone.width;
        }
        canvas
    }

    /// The page markers along the top: one per page, the current one
    /// longer and lit, a page in trouble longer still and in its colour.
    /// `dim` is the brightness the frame is drawn at.
    pub fn draw_markers(&mut self, canvas: &mut Canvas, markers: &[Marker], dim: f32) {
        if markers.len() < 2 {
            return;
        }
        let w = |m: &Marker| {
            let base = if m.current {
                MARKER_W_CURRENT
            } else {
                MARKER_W
            };
            if m.severity == Severity::Ok {
                base
            } else {
                base + MARKER_W_ALERT
            }
        };
        let total: f32 = markers.iter().map(w).sum::<f32>()
            + MARKER_GAP * markers.len().saturating_sub(1) as f32;
        let mut x = ((WIDTH as f32 - total) / 2.0).round();
        for m in markers {
            let c = match (m.severity, m.current) {
                (Severity::Crit, _) => CRIT,
                (Severity::Warn, _) => WARN,
                (Severity::Ok, true) => TEXT,
                (Severity::Ok, false) => TEXT3.mix(GROUND, 0.45),
            };
            // Trouble is never dimmed; the rest is as bright as the frame.
            let (c, h) = if m.severity == Severity::Ok {
                (dimmed(c, dim), MARKER_H)
            } else {
                (c, MARKER_H_ALERT)
            };
            let mw = w(m);
            draw::rrect(canvas, x, MARKER_Y, mw, h, h / 2.0, c);
            x += mw + MARKER_GAP;
        }
        // Trouble on a page to one side: a pill at that end of the strip,
        // so it shows from the page in view without covering anything.
        let Some(here) = markers.iter().position(|m| m.current) else {
            return;
        };
        let worst = |ms: &[Marker]| ms.iter().map(|m| m.severity).max().unwrap_or_default();
        let sides = [
            (worst(&markers[..here]), 0.0),
            (worst(&markers[here + 1..]), WIDTH as f32 - EDGE_FLAG_W),
        ];
        for (sev, fx) in sides {
            let c = match sev {
                Severity::Ok => continue,
                Severity::Warn => WARN,
                Severity::Crit => CRIT,
            };
            let r = EDGE_FLAG_W / 2.0;
            draw::rrect(canvas, fx, EDGE_FLAG_Y, EDGE_FLAG_W, EDGE_FLAG_H, r, c);
        }
    }

    /// The critical alarm: the whole strip red, what is too hot and by how
    /// much in plain words, the reading large, and one white OK key.
    /// `breathe` (0–1) is where the red is in its slow breathing.
    pub fn render_alarm(&mut self, alarm: &AlarmView, ok_pressed: bool, breathe: f32) -> Canvas {
        let low = ALARM_BREATHE_LOW;
        let k = 1.0 - (1.0 - low) * breathe.clamp(0.0, 1.0);
        let red = CRIT.scaled(k);
        let mut canvas = Canvas::new(WIDTH, HEIGHT, red);
        self.alerts.clear();
        self.alerts.push(0..WIDTH);
        self.moving = false;
        let mut p = self.pen(&mut canvas);
        p.alarm(alarm, ok_pressed, red);
        canvas
    }

    fn pen<'a>(&'a mut self, canvas: &'a mut Canvas) -> Pen<'a> {
        Pen {
            c: canvas,
            text: &mut self.text,
            words: &self.words,
            nums: &self.nums,
            fill: &self.fonts.icon,
            bold: &self.fonts.icon_bold,
            a: self.theme.roles,
            raised: false,
            control: false,
            moving: false,
        }
    }
}

/// The alarm's OK key on the strip: its first column and its width.
pub fn alarm_ok_span() -> Range<u32> {
    let x = WIDTH as f32 - ALARM_OK_RIGHT - ALARM_OK_W;
    x as u32..(x + ALARM_OK_W) as u32
}

/// Everything one zone is drawn with.
struct Pen<'a> {
    c: &'a mut Canvas,
    text: &'a mut TextEngine,
    words: &'a [Font],
    nums: &'a [Font],
    fill: &'a Font,
    bold: &'a Font,
    a: Accent,
    /// Cards are on top of a page (a partial takeover).
    raised: bool,
    /// The card being drawn can be acted on (a key, a slider, media): only
    /// then is a raised card rimmed in the accent.
    control: bool,
    /// A title is mid-scroll.
    moving: bool,
}

/// Vertical centre of a numeral's figures for a style and baseline.
fn fig_mid(num: &TextStyle, base: f32) -> f32 {
    base - num.size * 0.35
}

fn fmt_time(s: f64) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// The dashes shown where a number has not arrived.
const DASHES: &str = "––";

impl Pen<'_> {
    // ------------------------------------------------------------ text

    fn measure(&mut self, f: Face, s: &TextStyle, t: &str) -> f32 {
        let fonts = match f {
            Face::Num => self.nums,
            Face::Word => self.words,
        };
        self.text.measure(fonts, s, t)
    }

    #[allow(clippy::too_many_arguments)]
    fn text(&mut self, f: Face, s: &TextStyle, t: &str, x: f32, base: f32, c: Rgb) -> f32 {
        let fonts = match f {
            Face::Num => self.nums,
            Face::Word => self.words,
        };
        self.text.draw(self.c, fonts, s, t, (x, base), c)
    }

    fn text_r(&mut self, f: Face, s: &TextStyle, t: &str, right: f32, base: f32, c: Rgb) -> f32 {
        let w = self.measure(f, s, t);
        self.text(f, s, t, right - w, base, c);
        w
    }

    fn text_c(&mut self, f: Face, s: &TextStyle, t: &str, cx: f32, base: f32, c: Rgb) -> f32 {
        let w = self.measure(f, s, t);
        self.text(f, s, t, (cx - w / 2.0).round(), base, c);
        w
    }

    /// `t` shortened with an ellipsis to fit `room`.
    fn fit(&mut self, f: Face, s: &TextStyle, t: &str, room: f32) -> String {
        if self.measure(f, s, t) <= room {
            return t.to_string();
        }
        let mut best = String::from("…");
        for (e, _) in t.char_indices().skip(1) {
            let cand = format!("{}…", t[..e].trim_end());
            if self.measure(f, s, &cand) <= room {
                best = cand;
            } else {
                break;
            }
        }
        best
    }

    /// A title that may not fit between `x` and `right`: at rest it fades
    /// out into the card; with `marquee` it scrolls once to its end, rests,
    /// and comes back (`DESIGN.md`, Long titles).
    #[allow(clippy::too_many_arguments)]
    fn title(
        &mut self,
        f: Face,
        s: &TextStyle,
        t: &str,
        x: f32,
        right: f32,
        base: f32,
        c: Rgb,
        surface: Rgb,
        marquee: Option<f32>,
    ) {
        let w = self.measure(f, s, t);
        if x + w <= right {
            self.text(f, s, t, x, base, c);
            return;
        }
        let over = x + w - right + FADE;
        let rest = MS_MARQUEE_REST as f32 / 1000.0;
        let scroll = over / MARQUEE_PX_PER_S;
        let back = 0.4;
        let (offset, alpha) = match marquee {
            Some(m) if m < rest => (0.0, 1.0),
            Some(m) if m < rest + scroll => ((m - rest) * MARQUEE_PX_PER_S, 1.0),
            Some(m) if m < 2.0 * rest + scroll => (over, 1.0),
            Some(m) if m < 2.0 * rest + scroll + back => {
                (over, 1.0 - (m - 2.0 * rest - scroll) / back)
            }
            _ => (0.0, 1.0),
        };
        self.moving |= marquee.is_some_and(|m| m < 2.0 * rest + scroll + back);
        let fonts = match f {
            Face::Num => self.nums,
            Face::Word => self.words,
        };
        let left_fade = if offset > 0.0 { FADE } else { 0.0 };
        self.text.draw_faded(
            self.c,
            fonts,
            s,
            t,
            (x - offset.round(), base),
            surface.mix(c, alpha),
            (x, right),
            (left_fade, FADE),
        );
        if alpha < 1.0 {
            // Coming back: the start of the title fades in where it rests.
            self.text.draw_faded(
                self.c,
                fonts,
                s,
                t,
                (x, base),
                surface.mix(c, 1.0 - alpha),
                (x, right),
                (0.0, FADE),
            );
        }
    }

    /// A numeral with a smaller unit after it on its baseline; returns the
    /// width.
    #[allow(clippy::too_many_arguments)]
    fn num_unit(
        &mut self,
        num: &TextStyle,
        t: &str,
        unit: &str,
        unit_st: &TextStyle,
        x: f32,
        base: f32,
        c: Rgb,
        uc: Rgb,
    ) -> f32 {
        let w = self.text(Face::Num, num, t, x, base, c);
        if unit.is_empty() {
            return w;
        }
        w + UNIT_GAP + self.text(Face::Num, unit_st, unit, x + w + UNIT_GAP, base, uc)
    }

    fn num_unit_w(&mut self, num: &TextStyle, t: &str, unit: &str, unit_st: &TextStyle) -> f32 {
        let w = self.measure(Face::Num, num, t);
        if unit.is_empty() {
            w
        } else {
            w + UNIT_GAP + self.measure(Face::Num, unit_st, unit)
        }
    }

    fn icon(&mut self, bold: bool, ch: char, size: f32, cx: f32, cy: f32, c: Rgb) -> f32 {
        let font = if bold { self.bold } else { self.fill };
        self.text.draw_icon(self.c, font, size, ch, cx, cy, c)
    }

    // ------------------------------------------------------------ shapes

    fn rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgb) {
        draw::rrect(self.c, x, y, w, h, r, c);
    }

    fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgb) {
        draw::circle(self.c, cx, cy, r, c);
    }

    fn card_path(x: f32, w: f32) -> Option<Path> {
        rrect_path(x, CARD_Y, w, CARD_H, CARD_R)
    }

    /// A card's surface, in its pressed or holding state if touched.
    fn card(&mut self, x: f32, w: f32, surface: Rgb, press: Press) {
        let surface = if self.raised && surface == CARD {
            RAISED
        } else {
            surface
        };
        if press.on && press.part.is_none() {
            self.rrect(x, CARD_Y, w, CARD_H, CARD_R, surface.mix(KEY, 0.4));
            self.hold_fill(x, w, surface.mix(self.a.base, HOLD_MIX * 0.6), press);
            draw::rrect_line(
                self.c,
                x,
                CARD_Y,
                w,
                CARD_H,
                CARD_R,
                self.a.base,
                PRESS_RIM,
                None,
            );
        } else {
            self.rrect(x, CARD_Y, w, CARD_H, CARD_R, surface);
        }
        if self.raised {
            // On top of the page: a rim, in the accent only on a control.
            let rim = if self.control {
                self.a.base.mix(surface, 0.35)
            } else {
                KEY_TOP
            };
            draw::rrect_line(self.c, x, CARD_Y, w, CARD_H, CARD_R, rim, PRESS_RIM, None);
        }
    }

    /// The long press on its way: the face fills from the left.
    fn hold_fill(&mut self, x: f32, w: f32, c: Rgb, press: Press) {
        let Some(p) = press.hold else { return };
        let fw = (w * p).max(CARD_R * 2.0);
        if let (Some(fill), Some(clip)) = (
            rrect_path(x, CARD_Y + KEY_LIFT, fw, CARD_H - KEY_LIFT, CARD_R),
            Self::card_path(x, w),
        ) {
            draw::fill_clipped(self.c, &fill, c, &clip);
        }
    }

    /// The "not set up" outline: dashed, in the accent, no fill.
    fn setup_card(&mut self, x: f32, w: f32) {
        draw::rrect_line(
            self.c,
            x,
            CARD_Y,
            w,
            CARD_H,
            CARD_R,
            self.a.base,
            DASH_W,
            Some(DASH),
        );
    }

    /// A raised key face; pressed, it sinks and lights.
    #[allow(clippy::too_many_arguments)]
    fn key_face(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, pressed: bool, primary: bool) {
        if pressed {
            let face = if primary {
                self.a.base.mix(self.a.ink, 0.25)
            } else {
                self.a.soft
            };
            self.rrect(x, y, w, h, r, face);
            draw::rrect_line(self.c, x, y, w, h, r, self.a.base, PRESS_RIM, None);
            return;
        }
        let (face, top) = if primary {
            (self.a.base, self.a.base.mix(TEXT, 0.45))
        } else {
            (KEY, KEY_TOP)
        };
        self.rrect(x, y, w, h, r, top);
        self.rrect(x, y + KEY_LIFT, w, h - KEY_LIFT, r, face);
    }

    /// The fixed-scale bar with its warning and critical zones notched.
    fn bar(&mut self, x: f32, y: f32, w: f32, h: f32, bar: &Bar, k: &Ink) {
        let r = h / 2.0;
        let Some(track) = rrect_path(x, y, w, h, r) else {
            return;
        };
        draw::fill(self.c, &track, k.track, 255);
        let crit_card = k.surface == CRIT;
        if !crit_card {
            if let Some(wv) = bar.warn {
                let x0 = x + w * wv;
                let x1 = bar.crit.map_or(x + w, |c| x + w * c);
                if let Some(p) = rrect_path(x0, y, x1 - x0, h, 0.0) {
                    draw::fill_clipped(self.c, &p, WARN_ZONE, &track);
                }
            }
            if let Some(cv) = bar.crit {
                let x0 = x + w * cv;
                if let Some(p) = rrect_path(x0, y, x + w - x0, h, 0.0) {
                    draw::fill_clipped(self.c, &p, CRIT_ZONE, &track);
                }
            }
        }
        if let Some(v) = bar.value {
            let fw = (w * v).max(h);
            if let Some(p) = rrect_path(x, y, fw, h, r) {
                draw::fill_clipped(self.c, &p, k.fill, &track);
            }
        }
        for (lim, c) in [(bar.warn, WARN), (bar.crit, CRIT)] {
            let Some(lv) = lim else { continue };
            let nx = (x + w * lv - NOTCH_W / 2.0).round();
            let under_fill = bar.value.is_some_and(|v| v > lv);
            let col = if crit_card && under_fill {
                // White on the white fill would vanish just when it counts.
                CRIT_DEEP
            } else if crit_card {
                CRIT_INK
            } else if k.surface == WARN_SURFACE && c == WARN {
                TEXT
            } else {
                c.mix(TEXT, 0.15)
            };
            self.rrect(
                nx,
                y - NOTCH_OUT,
                NOTCH_W,
                h + 2.0 * NOTCH_OUT,
                NOTCH_W / 2.0,
                col,
            );
        }
    }

    // ------------------------------------------------------------ widgets

    fn zone(&mut self, zx: f32, zw: u32, view: &WidgetView, press: Press) {
        let x = zx + CARD_INSET;
        let w = zw as f32 - 2.0 * CARD_INSET;
        let cls = class(zw);
        self.control = matches!(
            view,
            WidgetView::Slider { .. } | WidgetView::Key { .. } | WidgetView::Media(Some(_))
        );
        match view {
            WidgetView::Clock {
                time,
                meridiem,
                weekday,
                weekday_short,
                date,
                date_short,
            } => {
                self.card(x, w, CARD, press);
                let (day, d) = match cls {
                    Class::Hero => (weekday.as_str(), date.as_str()),
                    _ => (weekday_short.as_str(), date_short.as_str()),
                };
                self.clock(x, w, cls, time, meridiem.as_deref(), day, d);
            }
            WidgetView::Value {
                icon,
                reading,
                limit,
                second,
            } => self.value(
                x,
                w,
                cls,
                *icon,
                reading,
                limit.as_deref(),
                second.as_ref(),
                press,
            ),
            WidgetView::Weather {
                icon,
                temp,
                condition,
                high_low,
                avail,
            } => self.weather(
                x,
                w,
                cls,
                *icon,
                temp,
                condition,
                high_low.as_deref(),
                avail,
                press,
            ),
            WidgetView::Rate { down, up } => self.network(x, w, cls, down, up, press),
            WidgetView::Media(track) => self.media(zx, x, w, zw, track.as_ref(), press),
            WidgetView::Slider {
                icon,
                value,
                unit,
                fraction,
                active,
                muted,
                from,
                ..
            } => {
                let s = SliderLook {
                    icon: *icon,
                    value,
                    unit,
                    fraction: *fraction,
                    held: *active,
                    muted: *muted,
                };
                match from {
                    Some(from) => self.volume_change(x, w, &s, *from),
                    None => self.slider(x, w, cls, &s),
                }
            }
            WidgetView::Key {
                icon,
                label,
                sub,
                set_up,
            } => {
                if *set_up {
                    self.key(
                        x,
                        w,
                        zw,
                        *icon,
                        label.as_deref().unwrap_or(""),
                        sub.as_deref().unwrap_or(""),
                        press,
                    );
                } else {
                    self.setup_prompt(x, w, cls, icons::PLUS, "Add a key", "Tap to set up", "Key");
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn clock(
        &mut self,
        x: f32,
        w: f32,
        cls: Class,
        time: &str,
        meridiem: Option<&str>,
        day: &str,
        date: &str,
    ) {
        let num = match cls {
            Class::Compact => NUM_COMPACT,
            Class::Medium => NUM_MEDIUM,
            Class::Hero => NUM_HERO,
        };
        let unit_st = if cls == Class::Hero { UNIT_HERO } else { UNIT };
        let m = meridiem.unwrap_or("");
        let tw = self.num_unit_w(&num, time, m, &unit_st);
        if cls == Class::Compact {
            let (t, m) = if tw <= w - 2.0 * PAD {
                (tw, m)
            } else {
                (self.measure(Face::Num, &num, time), "")
            };
            let x0 = x + (w - t) / 2.0;
            self.num_unit(&num, time, m, &unit_st, x0, BASE_COMPACT + 1.0, TEXT, TEXT2);
            return;
        }
        let gap = if cls == Class::Hero { 14.0 } else { 8.0 };
        let (base, inset) = match cls {
            Class::Hero => (BASE_HERO, PAD + 4.0),
            _ => (BASE_MEDIUM, PAD),
        };
        let dw = self
            .measure(Face::Word, &TITLE, day)
            .max(self.measure(Face::Word, &BODY, date));
        // The date may run closer to the right edge than the padding: it is
        // words, and the time is what must not move.
        let show_date = tw + gap + dw <= w - inset / 2.0 - 2.0;
        let total = if show_date { tw + gap + dw } else { tw };
        let x0 = x + ((w - total) / 2.0).max(inset).min(w - total - 1.0).max(0.0);
        self.num_unit(&num, time, m, &unit_st, x0, base, TEXT, TEXT2);
        if show_date {
            let dx = x0 + tw + gap;
            self.text(Face::Word, &TITLE, day, dx, BASE_LINE1, TEXT);
            self.text(Face::Word, &BODY, date, dx, BASE_LINE2 - 1.0, TEXT2);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn value(
        &mut self,
        x: f32,
        w: f32,
        cls: Class,
        icon: char,
        r: &Reading,
        limit: Option<&str>,
        second: Option<&Reading>,
        press: Press,
    ) {
        if r.avail == Avail::NotSetUp {
            self.setup_prompt(
                x,
                w,
                cls,
                icons::PLUS,
                "Pick a sensor",
                "Tap to set up",
                "Sensor",
            );
            return;
        }
        // The card takes the worse of the two severities: area, not a dot.
        let sev = second.map_or(r.severity, |s| s.severity.max(r.severity));
        let k = ink(sev);
        let lost = matches!(r.avail, Avail::Lost(_));
        let kr = if lost {
            lost_ink()
        } else {
            reading_ink(sev, r.severity)
        };
        self.card(x, w, k.surface, press);
        let shown = |rd: &Reading| -> (String, Option<Bar>) {
            match rd.avail {
                Avail::Live => (rd.value.clone(), rd.bar),
                Avail::Lost(_) => (rd.value.clone(), rd.bar.map(|b| Bar { value: None, ..b })),
                _ => (DASHES.into(), rd.bar.map(|b| Bar { value: None, ..b })),
            }
        };
        let (txt, bar) = shown(r);
        let unit = if r.avail == Avail::NoData {
            ""
        } else {
            r.unit.as_str()
        };
        let num_c = if r.avail == Avail::NoData {
            TEXT3
        } else {
            kr.num
        };
        let icon_c = if lost { TEXT3 } else { kr.icon };
        let glyph = if lost { icons::LINK_BREAK } else { icon };
        match cls {
            Class::Compact => {
                // With no bar the numeral sits lower, in the middle.
                let base = if bar.is_some() {
                    BASE_COMPACT
                } else {
                    BASE_COMPACT + 3.0
                };
                let mut nx = x + PAD + ICON_COMPACT + ICON_GAP;
                let room = x + w - PAD - nx;
                let mut num = NUM_COMPACT;
                let mut nw = self.num_unit_w(&num, &txt, unit, &UNIT);
                if nw > room {
                    num = NUM_COMPACT_TIGHT;
                    nw = self.num_unit_w(&num, &txt, unit, &UNIT);
                }
                let show_icon = nw <= room;
                if show_icon {
                    let ix = x + PAD + ICON_COMPACT / 2.0;
                    self.icon(false, glyph, ICON_COMPACT, ix, fig_mid(&num, base), icon_c);
                    nx += ((room - nw) / 2.0).max(0.0);
                } else {
                    nx = x + ((w - nw) / 2.0).max(PAD / 2.0);
                }
                let txt = self.fit(Face::Num, &num, &txt, w - PAD);
                self.num_unit(&num, &txt, unit, &UNIT, nx, base, num_c, kr.label);
                if let Some(b) = bar {
                    self.bar(x + PAD, BAR_Y_COMPACT, w - 2.0 * PAD, BAR_H_BANDED, &b, &kr);
                }
            }
            Class::Medium => {
                let base = if bar.is_some() {
                    BASE_COMPACT
                } else {
                    BASE_MEDIUM - 2.0
                };
                let ix = x + PAD + ICON_MEDIUM / 2.0;
                self.icon(
                    false,
                    glyph,
                    ICON_MEDIUM,
                    ix,
                    fig_mid(&NUM_BANDED, base),
                    icon_c,
                );
                let nx = x + PAD + ICON_MEDIUM + ICON_GAP;
                let right = x + w - PAD;
                let txt = self.fit(Face::Num, &NUM_BANDED, &txt, right - nx);
                let nw = self.num_unit(&NUM_BANDED, &txt, unit, &UNIT, nx, base, num_c, kr.label);
                if let Some(s) = second {
                    let ks = reading_ink(sev, s.severity);
                    let (stxt, sbar) = shown(s);
                    let live = s.avail == Avail::Live;
                    let su = if live { s.unit.as_str() } else { "" };
                    let sc = if live { ks.num } else { TEXT3 };
                    let sw = self.num_unit_w(&NUM_SECOND, &stxt, su, &UNIT);
                    let sx = right - sw;
                    if sx > nx + nw + 6.0 {
                        self.num_unit(&NUM_SECOND, &stxt, su, &UNIT, sx, base, sc, ks.label);
                    }
                    let split = (sx - 10.0).max(nx + nw + 6.0);
                    if let Some(b) = bar {
                        let bw = split - 8.0 - (x + PAD);
                        self.bar(x + PAD, BAR_Y_COMPACT, bw, BAR_H_BANDED, &b, &kr);
                    }
                    if let Some(b) = sbar {
                        self.bar(split, BAR_Y_COMPACT, right - split, BAR_H_BANDED, &b, &ks);
                    }
                } else {
                    // The name, or why there is no number, top right.
                    let words = match &r.avail {
                        Avail::NoData => "No reading".to_string(),
                        Avail::Lost(t) => format!("Lost {t}"),
                        _ => r.name.clone(),
                    };
                    let lc = if r.avail == Avail::Live {
                        kr.label
                    } else {
                        TEXT3
                    };
                    let lw = self.measure(Face::Word, &LABEL, &words);
                    if nx + nw + 8.0 + lw <= right {
                        self.text_r(Face::Word, &LABEL, &words, right, 20.0, lc);
                    }
                    if let Some(b) = bar {
                        self.bar(x + PAD, BAR_Y_COMPACT, w - 2.0 * PAD, BAR_H_BANDED, &b, &kr);
                    }
                }
            }
            Class::Hero => {
                // The numeral a step under the hero scale, so the bar can
                // run the whole card beneath it: more width, a longer bar.
                // A second reading sits at the right, over the bar's end,
                // named; it takes no share of the band.
                let num = NUM_HERO_BANDED;
                let base = BASE_COMPACT;
                let ix = x + PAD + 2.0 + ICON_HERO / 2.0;
                self.icon(false, glyph, ICON_HERO, ix, fig_mid(&num, base), icon_c);
                let nx = x + PAD + 2.0 + ICON_HERO + ICON_GAP + 2.0;
                let right = x + w - PAD;
                let txt = self.fit(Face::Num, &num, &txt, right - nx);
                let nw = self.num_unit(&num, &txt, unit, &UNIT, nx, base, num_c, kr.label);
                if let Some(b) = bar {
                    self.bar(x + PAD, BAR_Y_COMPACT, w - 2.0 * PAD, BAR_H_BANDED, &b, &kr);
                }
                let cx = nx + nw + 14.0;
                let mut col_right = right;
                if let Some(s) = second {
                    let ks = reading_ink(sev, s.severity);
                    let (stxt, _) = shown(s);
                    let live = s.avail == Avail::Live;
                    let su = if live { s.unit.as_str() } else { "" };
                    let sc = if live { ks.num } else { TEXT3 };
                    // Its name before its number, on the numeral's baseline.
                    let sw = self.num_unit_w(&NUM_SECOND, &stxt, su, &UNIT);
                    let nm = self.measure(Face::Word, &LABEL, &s.name) + 6.0;
                    let named = right - sw - nm >= cx + 30.0;
                    let colw = if named { sw + nm } else { sw };
                    if right - colw < cx {
                        return;
                    }
                    let sx = right - sw;
                    self.num_unit(&NUM_SECOND, &stxt, su, &UNIT, sx, base, sc, ks.label);
                    if named {
                        self.text(Face::Word, &LABEL, &s.name, sx - nm, base, ks.label);
                    }
                    col_right = right - colw - 14.0;
                }
                // The name, or why there is no number, over the limit.
                let words = match &r.avail {
                    Avail::NoData => "No reading yet".to_string(),
                    Avail::Lost(t) => format!("Lost at {t}"),
                    _ => r.name.clone(),
                };
                let lc = if r.avail == Avail::Live {
                    kr.label
                } else {
                    TEXT3
                };
                // The top line is the name's alone; the limit shares the
                // numeral's baseline with a second reading, if any.
                if right - cx < 30.0 {
                    return;
                }
                let words = self.fit(Face::Word, &LABEL, &words, right - cx);
                self.text(Face::Word, &LABEL, &words, cx, BASE_HERO_NAME, lc);
                if let (Some(lim), Avail::Live) = (limit, &r.avail) {
                    let limw = self.measure(Face::Word, &LABEL, lim);
                    if cx + limw <= col_right {
                        self.text(Face::Word, &LABEL, lim, cx, base + 1.0, kr.quiet);
                    }
                }
            }
        }
    }

    fn network(&mut self, x: f32, w: f32, cls: Class, down: &Reading, up: &Reading, press: Press) {
        if down.avail == Avail::NotSetUp {
            self.setup_prompt(
                x,
                w,
                cls,
                icons::PLUS,
                "Pick a network",
                "Tap to set up",
                "Network",
            );
            return;
        }
        let k = ink(Severity::Ok);
        self.card(x, w, k.surface, press);
        let nodata = down.avail != Avail::Live;
        let txt = |r: &Reading| {
            if nodata {
                DASHES.to_string()
            } else {
                r.value.clone()
            }
        };
        let bar = |r: &Reading| {
            r.bar.map(|b| Bar {
                value: if nodata { None } else { b.value },
                ..b
            })
        };
        let num_c = if nodata { TEXT3 } else { TEXT };
        let unit = if nodata { "" } else { down.unit.as_str() };
        match cls {
            Class::Compact => {
                let ix = x + PAD + 8.0;
                self.icon(
                    true,
                    icons::ARROW_DOWN,
                    ICON_COMPACT,
                    ix,
                    fig_mid(&NUM_COMPACT, BASE_COMPACT),
                    TEXT2,
                );
                let nx = x + PAD + 20.0;
                let mut num = NUM_COMPACT;
                if self.num_unit_w(&num, &txt(down), unit, &LABEL) > x + w - PAD - nx {
                    num = NUM_COMPACT_TIGHT;
                }
                self.num_unit(
                    &num,
                    &txt(down),
                    unit,
                    &LABEL,
                    nx,
                    BASE_COMPACT,
                    num_c,
                    TEXT2,
                );
                if let Some(b) = bar(down) {
                    self.bar(x + PAD, BAR_Y_COMPACT, w - 2.0 * PAD, BAR_H_BANDED, &b, &k);
                }
            }
            Class::Medium | Class::Hero => {
                // Down is the primary reading; up sits beside it, each over
                // its own bar.
                let hero = cls == Class::Hero;
                let (num, isz) = if hero {
                    (NUM_BANDED, ICON_MEDIUM)
                } else {
                    (NUM_COMPACT, ICON_COMPACT)
                };
                let base = BASE_COMPACT;
                let right = x + w - PAD;
                let nx = x + PAD + isz + 1.0;
                let up_unit = if nodata || (!hero && up.unit == down.unit) {
                    ""
                } else {
                    up.unit.as_str()
                };
                let nw = self.num_unit_w(&num, &txt(down), unit, &UNIT);
                let sw = self.num_unit_w(&NUM_SECOND, &txt(up), up_unit, &UNIT);
                let ux = right - sw;
                if ux - ICON_SMALL - 4.0 <= nx + nw {
                    // Up in another unit than down, at medium: no room for
                    // both side by side, so two rows, each reading with its
                    // own unit and its own bar beside it.
                    self.rate_rows(
                        x,
                        right,
                        &[(down, icons::ARROW_DOWN), (up, icons::ARROW_UP)],
                    );
                    return;
                }
                let ix = x + PAD + isz / 2.0 - 2.0;
                self.icon(true, icons::ARROW_DOWN, isz, ix, fig_mid(&num, base), TEXT2);
                self.num_unit(&num, &txt(down), unit, &UNIT, nx, base, num_c, TEXT2);
                // With no data, the words top right say so for both; up's
                // own dashes under them would crowd them.
                if !nodata {
                    self.num_unit(
                        &NUM_SECOND,
                        &txt(up),
                        up_unit,
                        &UNIT,
                        ux,
                        base,
                        num_c,
                        TEXT2,
                    );
                    self.icon(
                        true,
                        icons::ARROW_UP,
                        ICON_SMALL,
                        ux - 3.0 - ICON_SMALL / 2.0,
                        fig_mid(&NUM_SECOND, base),
                        TEXT2,
                    );
                }
                let split = (ux - ICON_SMALL - 6.0).max(nx + nw + 8.0);
                if let Some(b) = bar(down) {
                    self.bar(
                        x + PAD,
                        BAR_Y_COMPACT,
                        split - 10.0 - (x + PAD),
                        BAR_H_BANDED,
                        &b,
                        &k,
                    );
                }
                if let Some(b) = bar(up) {
                    self.bar(split, BAR_Y_COMPACT, right - split, BAR_H_BANDED, &b, &k);
                }
                if nodata {
                    let words = match &down.avail {
                        Avail::Lost(t) => format!("Lost {t}"),
                        _ => "No reading".into(),
                    };
                    self.text_r(Face::Word, &LABEL, &words, right, 20.0, TEXT3);
                }
            }
        }
    }

    /// Rates as rows: an arrow, the number with its unit, and its bar to
    /// the right, one row per reading (a medium card whose two rates are
    /// in different units).
    fn rate_rows(&mut self, x: f32, right: f32, rows: &[(&Reading, char)]) {
        let k = ink(Severity::Ok);
        let bases = [22.0, 41.0];
        let mut widths = Vec::new();
        for (r, _) in rows {
            widths.push(self.num_unit_w(&TITLE, &r.value, &r.unit, &UNIT));
        }
        let tx = x + PAD + ICON_SMALL + 3.0;
        let bx = tx + widths.iter().cloned().fold(0.0, f32::max) + 10.0;
        for (((r, glyph), base), _) in rows.iter().zip(bases).zip(&widths) {
            let mid = fig_mid(&TITLE, base);
            self.icon(
                true,
                *glyph,
                ICON_SMALL,
                x + PAD + ICON_SMALL / 2.0 - 1.0,
                mid,
                TEXT2,
            );
            self.num_unit(&TITLE, &r.value, &r.unit, &UNIT, tx, base, TEXT, TEXT2);
            if let Some(b) = r.bar
                && right - bx > 20.0
            {
                let h = BAR_H_BANDED - 1.0;
                self.bar(bx, (mid - h / 2.0).round(), right - bx, h, &b, &k);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn weather(
        &mut self,
        x: f32,
        w: f32,
        cls: Class,
        icon: char,
        temp: &str,
        cond: &str,
        high_low: Option<&str>,
        avail: &Avail,
        press: Press,
    ) {
        if *avail == Avail::NotSetUp {
            self.setup_prompt(
                x,
                w,
                cls,
                icons::MAP_PIN_PLUS,
                "Weather",
                "Add your town",
                "Weather",
            );
            return;
        }
        self.card(x, w, CARD, press);
        let (t, tc, ic, glyph, words): (&str, Rgb, Rgb, char, String) = match avail {
            Avail::NoData => (
                DASHES,
                TEXT3,
                TEXT3,
                icons::CLOUD_SLASH,
                "No forecast yet".into(),
            ),
            Avail::Lost(at) => (temp, TEXT3, TEXT3, icons::CLOUD_SLASH, format!("From {at}")),
            _ => (temp, TEXT, TEXT2, icon, cond.to_string()),
        };
        let live = *avail == Avail::Live;
        match cls {
            Class::Compact => {
                let tw = self.measure(Face::Num, &NUM_MEDIUM, t);
                let total = ICON_MEDIUM + ICON_GAP + tw;
                let x0 = x + ((w - total) / 2.0).max(PAD / 2.0);
                self.icon(
                    false,
                    glyph,
                    ICON_MEDIUM,
                    x0 + ICON_MEDIUM / 2.0,
                    fig_mid(&NUM_MEDIUM, BASE_MEDIUM),
                    ic,
                );
                self.text(
                    Face::Num,
                    &NUM_MEDIUM,
                    t,
                    x0 + ICON_MEDIUM + ICON_GAP,
                    BASE_MEDIUM,
                    tc,
                );
            }
            Class::Medium => {
                let x0 = x + PAD;
                self.icon(
                    false,
                    glyph,
                    ICON_MEDIUM,
                    x0 + ICON_MEDIUM / 2.0,
                    fig_mid(&NUM_MEDIUM, BASE_MEDIUM),
                    ic,
                );
                let nx = x0 + ICON_MEDIUM + ICON_GAP;
                let nw = self.text(Face::Num, &NUM_MEDIUM, t, nx, BASE_MEDIUM, tc);
                let dx = nx + nw + 8.0;
                let room = x + w - PAD - dx;
                // Two short lines; words chosen to fit, never cut.
                let (l1, l2) = match avail {
                    Avail::NoData => ("No data".to_string(), String::new()),
                    Avail::Lost(at) => ("From".to_string(), at.clone()),
                    _ => (words, high_low.unwrap_or("").to_string()),
                };
                let c1 = if live { TEXT } else { TEXT3 };
                let l1 = self.fit(Face::Word, &BODY, &l1, room);
                // "61° / 49°" closes up to "61°/49°" before it would be cut,
                // and is left out before that.
                let l2 = if self.measure(Face::Num, &BODY, &l2) <= room {
                    l2
                } else {
                    let tight = l2.replace(" / ", "/");
                    if self.measure(Face::Num, &BODY, &tight) <= room {
                        tight
                    } else {
                        String::new()
                    }
                };
                if l2.is_empty() {
                    self.text(Face::Word, &BODY, &l1, dx, 32.0, c1);
                } else {
                    self.text(Face::Word, &BODY, &l1, dx, BASE_LINE1, c1);
                    let l2 = self.fit(Face::Num, &BODY, &l2, room);
                    self.text(Face::Num, &BODY, &l2, dx, BASE_LINE2 - 1.0, TEXT2);
                }
            }
            Class::Hero => {
                let x0 = x + PAD + 4.0;
                self.icon(
                    false,
                    glyph,
                    ICON_HERO,
                    x0 + ICON_HERO / 2.0,
                    fig_mid(&NUM_HERO, BASE_HERO),
                    ic,
                );
                let nx = x0 + ICON_HERO + ICON_GAP + 2.0;
                let nw = self.text(Face::Num, &NUM_HERO, t, nx, BASE_HERO, tc);
                let dx = nx + nw + 14.0;
                let room = x + w - PAD - dx;
                let line1 = self.fit(Face::Word, &TITLE, &words, room);
                let c1 = if live { TEXT } else { TEXT3 };
                let l2 = if live {
                    high_low.unwrap_or("").to_string()
                } else {
                    String::new()
                };
                if l2.is_empty() {
                    self.text(Face::Word, &TITLE, &line1, dx, 32.0, c1);
                } else {
                    self.text(Face::Word, &TITLE, &line1, dx, BASE_LINE1, c1);
                    let l2 = self.fit(Face::Num, &BODY, &l2, room);
                    self.text(Face::Num, &BODY, &l2, dx, BASE_LINE2 - 1.0, TEXT2);
                }
            }
        }
    }

    fn media(&mut self, zx: f32, x: f32, w: f32, zw: u32, track: Option<&Track>, press: Press) {
        let Some(t) = track else {
            self.card(x, w, CARD, press);
            self.media_nothing(x, w, class(zw));
            return;
        };
        let keys = media_keys(zw);
        // A finger on the whole-card key lights the card.
        let card_press = Press {
            on: press.on && (press.part.is_none() || keys.iter().all(|k| !k.key)),
            part: None,
            hold: press.hold,
        };
        self.card(x, w, CARD, card_press);
        let pressed = |i: usize| press.on && press.part == Some(i);
        let glyph = if t.playing { icons::PAUSE } else { icons::PLAY };
        let tc = if t.playing { TEXT } else { TEXT2 };
        let line2 = if t.playing {
            t.artist.as_str()
        } else {
            "Paused"
        };
        let art_y = CARD_Y + (CARD_H - ART) / 2.0;
        if zw >= MEDIA_SCENE_MIN {
            self.media_scene(zx, x, w, t, &keys, press);
            return;
        }
        match class(zw) {
            Class::Compact => {
                let k = &keys[0];
                let (kx, kw) = (zx + k.span.start, k.span.end - k.span.start);
                self.key_face(kx, KEY_Y, kw, KEY_H, KEY_R, pressed(0), true);
                let dy = if pressed(0) { 1.0 } else { 0.0 };
                let cy = KEY_Y + KEY_H / 2.0 + dy;
                self.icon(false, glyph, ICON_KEY, kx + kw / 2.0, cy, self.a.ink);
                let tx = kx + kw + 8.0;
                if let Some(pos) = t.position {
                    let el = fmt_time(pos);
                    self.text(Face::Num, &TITLE, &el, tx, BASE_LINE1 + 1.0, tc);
                }
                if let Some(f) = t.progress() {
                    self.progress(tx, 34.0, x + w - PAD - tx, f, t.playing);
                }
            }
            Class::Medium => {
                let ax = x + PAD - 3.0;
                draw::art(self.c, ax, art_y, ART, ART_R, &t.title, !t.playing);
                // The card is a play/pause key: a small accent badge says so.
                let (bx, by) = (ax + ART - 4.0, art_y + ART - 4.0);
                let surface = if card_press.on {
                    CARD.mix(KEY, 0.4)
                } else {
                    CARD
                };
                self.circle(bx, by, 8.5, surface);
                self.circle(bx, by, 7.0, self.a.base);
                self.icon(false, glyph, ICON_BADGE, bx, by, self.a.ink);
                let tx = ax + ART + 10.0;
                let right = x + w - PAD;
                self.title(
                    Face::Word,
                    &TITLE,
                    &t.title,
                    tx,
                    right,
                    BASE_LINE1,
                    tc,
                    CARD,
                    t.marquee,
                );
                self.title(
                    Face::Word,
                    &BODY,
                    line2,
                    tx,
                    right,
                    BASE_LINE2 - 1.0,
                    TEXT2,
                    CARD,
                    None,
                );
            }
            Class::Hero => {
                let ax = x + PAD - 3.0;
                draw::art(self.c, ax, art_y, ART, ART_R, &t.title, !t.playing);
                self.transport(zx, &keys, t.playing, press, ICON_KEY - 2.0);
                let kx0 = zx + keys[0].span.start;
                let tx = ax + ART + 10.0;
                let right = kx0 - 10.0;
                self.title(
                    Face::Word,
                    &TITLE,
                    &t.title,
                    tx,
                    right,
                    BASE_LINE1,
                    tc,
                    CARD,
                    t.marquee,
                );
                let mut times = match (t.position, t.length.filter(|l| *l > 0.0)) {
                    (Some(p), Some(l)) => format!("{} / {}", fmt_time(p), fmt_time(l)),
                    (Some(p), None) => fmt_time(p),
                    _ => String::new(),
                };
                // The length goes before the artist would be cut: the
                // elapsed time is what changes.
                let aw = self.measure(Face::Word, &BODY, line2);
                let mut tw = self.measure(Face::Num, &LABEL, &times);
                if tx + aw + 10.0 + tw > right
                    && let Some(p) = t.position
                {
                    times = fmt_time(p);
                    tw = self.measure(Face::Num, &LABEL, &times);
                }
                let artist_right = if times.is_empty() {
                    right
                } else {
                    right - tw - 10.0
                };
                self.title(
                    Face::Word,
                    &BODY,
                    line2,
                    tx,
                    artist_right,
                    BASE_LINE2 - 1.0,
                    TEXT2,
                    CARD,
                    None,
                );
                if !times.is_empty() {
                    self.text_r(Face::Num, &LABEL, &times, right, BASE_LINE2 - 1.0, TEXT3);
                }
            }
        }
    }

    /// Transport keys: play/pause filled with the accent, prev and next
    /// raised with accent glyphs.
    fn transport(&mut self, zx: f32, keys: &[MediaKey], playing: bool, press: Press, size: f32) {
        for (i, k) in keys.iter().enumerate() {
            let (kx, kw) = (zx + k.span.start, k.span.end - k.span.start);
            let p = press.on && press.part == Some(i);
            let primary = k.command == MediaCommand::PlayPause;
            self.key_face(kx, KEY_Y, kw, KEY_H, KEY_R, p, primary);
            let glyph = match k.command {
                MediaCommand::Previous => icons::SKIP_BACK,
                MediaCommand::Next => icons::SKIP_FWD,
                _ if playing => icons::PAUSE,
                _ => icons::PLAY,
            };
            let c = if primary {
                self.a.ink
            } else if p {
                TEXT
            } else {
                self.a.text
            };
            let dy = if p { 1.0 } else { 0.0 };
            self.icon(
                false,
                glyph,
                size,
                kx + kw / 2.0,
                KEY_Y + KEY_H / 2.0 + dy,
                c,
            );
        }
    }

    /// The media scene (a media widget as wide as the strip, as the
    /// now-playing takeover is): bigger artwork and keys, the full title
    /// large, the artist and the times in the text ramp.
    fn media_scene(&mut self, zx: f32, x: f32, w: f32, t: &Track, keys: &[MediaKey], press: Press) {
        let s = ART_SCENE;
        let ay = CARD_Y + (CARD_H - s) / 2.0;
        draw::art(self.c, x + 4.0, ay, s, ART_R + 1.0, &t.title, !t.playing);
        self.transport(zx, keys, t.playing, press, ICON_KEY);
        let kx0 = zx + keys[0].span.start;
        let tx = x + 4.0 + s + 12.0;
        let right = kx0 - 14.0;
        let _ = w;
        // The artist stays when paused: the key and the dimmed art say so.
        let artist = t.artist.as_str();
        let aw = if artist.is_empty() {
            0.0
        } else {
            let a = self.fit(Face::Word, &BODY, artist, (right - tx) / 2.0);
            // Information, so the text ramp: the accent is for what a
            // finger can act on.
            self.text(Face::Word, &BODY, &a, tx, 20.0, TEXT2)
        };
        let times = match (t.position, t.length.filter(|l| *l > 0.0)) {
            (Some(p), Some(l)) => format!("{} / {}", fmt_time(p), fmt_time(l)),
            (Some(p), None) => fmt_time(p),
            _ => String::new(),
        };
        let tw = if times.is_empty() {
            0.0
        } else {
            self.text_r(Face::Num, &LABEL, &times, right, 20.0, TEXT2)
        };
        let px0 = tx + aw + if aw > 0.0 { 12.0 } else { 0.0 };
        let px1 = right - tw - 12.0;
        if let Some(f) = t.progress()
            && px1 - px0 > 30.0
        {
            self.progress(px0, 14.0, px1 - px0, f, t.playing);
        }
        let tc = if t.playing { TEXT } else { TEXT2 };
        self.title(
            Face::Word,
            &TITLE_BIG,
            &t.title,
            tx,
            right,
            43.0,
            tc,
            CARD,
            t.marquee,
        );
    }

    /// A thin progress line with no thumb: information, not a control.
    fn progress(&mut self, x: f32, y: f32, w: f32, f: f32, playing: bool) {
        if w <= PROGRESS_H {
            return;
        }
        let h = PROGRESS_H;
        self.rrect(x, y, w, h, h / 2.0, TRACK);
        // Information, not a control: cream, quieter when paused.
        let c = if playing { TEXT2 } else { TEXT3 };
        self.rrect(x, y, (w * f).max(h), h, h / 2.0, c);
    }

    fn media_nothing(&mut self, x: f32, w: f32, cls: Class) {
        // An empty sleeve: a dashed rounded square in a quiet neutral.
        // Narrower cards give the sleeve's room to the words, at 18 px.
        let (s, gap) = if cls == Class::Hero {
            (31.0, 9.0)
        } else {
            (27.0, 7.0)
        };
        let sx = x + PAD - 3.0;
        let sy = CARD_Y + (CARD_H - s) / 2.0;
        draw::rrect_line(self.c, sx, sy, s, s, ART_R, TEXT3, 1.2, Some((3.0, 3.0)));
        self.icon(
            true,
            icons::MUSIC,
            ICON_SMALL,
            sx + s / 2.0,
            sy + s / 2.0,
            TEXT3,
        );
        let tx = sx + s + gap;
        let room = x + w - PAD - tx;
        match cls {
            Class::Compact => {
                self.text(Face::Word, &LABEL, "Nothing", tx, BASE_LINE1 - 1.0, TEXT2);
                self.text(Face::Word, &LABEL, "playing", tx, BASE_LINE2 - 3.0, TEXT2);
            }
            _ => {
                let st1 = if cls == Class::Hero { TITLE } else { BODY };
                let l1 = self.fit(Face::Word, &st1, "Nothing playing", room);
                self.text(Face::Word, &st1, &l1, tx, BASE_LINE1, TEXT2);
                let hint = if cls == Class::Hero {
                    "Play some music"
                } else {
                    "Play something"
                };
                let hint = self.fit(Face::Word, &BODY, hint, room);
                self.text(Face::Word, &BODY, &hint, tx, BASE_LINE2 - 1.0, TEXT3);
            }
        }
    }

    fn slider(&mut self, x: f32, w: f32, cls: Class, s: &SliderLook) {
        self.card(x, w, CARD, Press::default());
        let value = s.fraction.map_or(0.0, |f| f * 100.0);
        let ic = if s.muted { TEXT3 } else { TEXT2 };
        // The level is information even while dragged: the held thumb is
        // what shows the finger, in the accent.
        let num_c = if s.muted || s.fraction.is_none() {
            TEXT3
        } else {
            TEXT
        };
        // A number with a unit, or words ("Muted", "1:05 / 4:20").
        let numeric = !s.unit.is_empty();
        match cls {
            Class::Compact => {
                let ix = x + PAD + 9.0;
                self.icon(false, s.icon, ICON_SMALL + 1.0, ix, 18.0, ic);
                let nx = x + PAD + 22.0;
                if numeric {
                    self.num_unit(&TITLE, s.value, s.unit, &LABEL, nx, 25.0, num_c, TEXT2);
                } else {
                    let v = self.fit(Face::Num, &BODY, s.value, x + w - PAD - nx);
                    self.text(Face::Num, &BODY, &v, nx, 24.0, TEXT2);
                }
                self.slider_track(
                    x + PAD + 2.0,
                    37.0,
                    w - 2.0 * PAD - 4.0,
                    value,
                    s,
                    None,
                    false,
                );
            }
            Class::Medium => {
                let base = 37.0;
                let ix = x + PAD + ICON_COMPACT / 2.0;
                self.icon(
                    false,
                    s.icon,
                    ICON_COMPACT,
                    ix,
                    fig_mid(&NUM_COMPACT, base),
                    ic,
                );
                let nx = x + PAD + ICON_COMPACT + ICON_GAP - 1.0;
                let nw = if numeric {
                    self.num_unit(
                        &NUM_COMPACT,
                        s.value,
                        s.unit,
                        &LABEL,
                        nx,
                        base,
                        num_c,
                        TEXT2,
                    )
                } else {
                    let v = self.fit(Face::Num, &BODY, s.value, (w - 2.0 * PAD) / 2.0);
                    self.text(Face::Num, &BODY, &v, nx, 32.0, TEXT2)
                };
                let min = self.measure(Face::Num, &NUM_COMPACT, "00");
                let tx = nx + nw.max(min) + 12.0;
                let tw = x + w - PAD - 4.0 - tx;
                self.slider_track(tx, 26.5, tw, value, s, None, false);
            }
            Class::Hero => {
                let ix = x + PAD + 2.0 + ICON_HERO / 2.0;
                self.icon(
                    false,
                    s.icon,
                    ICON_HERO,
                    ix,
                    fig_mid(&NUM_HERO, BASE_HERO),
                    ic,
                );
                let nx = x + PAD + 2.0 + ICON_HERO + ICON_GAP + 2.0;
                let nw = if numeric {
                    self.num_unit(
                        &NUM_HERO, s.value, s.unit, &UNIT_HERO, nx, BASE_HERO, num_c, TEXT2,
                    )
                } else if s.muted {
                    // The level stays, grey; "Muted" goes under the track.
                    let v = format!("{}", value.round() as i32);
                    self.num_unit(&NUM_HERO, &v, "%", &UNIT_HERO, nx, BASE_HERO, TEXT3, TEXT3)
                } else {
                    let v = self.fit(Face::Num, &TITLE, s.value, (w - 2.0 * PAD) / 2.0);
                    self.text(Face::Num, &TITLE, &v, nx, 33.0, TEXT2)
                };
                let tx = nx + nw + 18.0;
                let tw = x + w - PAD - 6.0 - tx;
                self.slider_track(tx, 24.0, tw, value, s, None, !s.muted);
                if s.muted {
                    self.text_c(Face::Word, &LABEL, "Muted", tx + tw / 2.0, 44.0, TEXT2);
                }
            }
        }
    }

    /// A slider track with its thumb. `from` marks where the value was
    /// before a change (the volume takeover's mark); `ticks` adds a scale.
    #[allow(clippy::too_many_arguments)]
    fn slider_track(
        &mut self,
        x: f32,
        cy: f32,
        w: f32,
        value: f32,
        s: &SliderLook,
        from: Option<f32>,
        ticks: bool,
    ) {
        if w < 2.0 * THUMB_R {
            return;
        }
        let h = SLIDER_TRACK_H;
        let y = cy - h / 2.0;
        if ticks {
            for i in 0..=4 {
                let tx = (x + w * i as f32 / 4.0).round() - 1.0;
                let th = if i % 2 == 0 { 6.0 } else { 4.0 };
                self.rrect(tx, y + h + 6.0, 2.0, th, 1.0, TICK);
            }
        }
        self.rrect(x, y, w, h, h / 2.0, TRACK);
        if s.fraction.is_none() {
            return;
        }
        let f = (value / 100.0).clamp(0.0, 1.0);
        let tx = x + w * f;
        let surface = if self.raised { RAISED } else { CARD };
        if let Some(fr) = from {
            // The change just made is bright; what was there before calm.
            let fx = x + w * fr.clamp(0.0, 1.0);
            let (lo, hi) = (fx.min(tx), fx.max(tx));
            self.rrect(
                x,
                y,
                (tx - x).max(h),
                h,
                h / 2.0,
                self.a.fill.mix(surface, 0.5),
            );
            self.rrect(lo - h / 2.0, y, hi - lo + h, h, h / 2.0, self.a.base);
            self.rrect((fx - 1.0).round(), y - 4.0, 2.0, h + 8.0, 1.0, TEXT2);
        } else {
            let fill = if s.muted {
                TEXT3.mix(CARD, 0.35)
            } else {
                self.a.fill
            };
            self.rrect(x, y, (tx - x).max(h), h, h / 2.0, fill);
        }
        if s.held {
            draw::glow(self.c, tx, cy, THUMB_HALO, self.a.base, 110);
            self.circle(tx, cy, THUMB_R_HELD + 1.5, surface);
            self.circle(tx, cy, THUMB_R_HELD, self.a.base);
            self.circle(tx, cy, 3.0, self.a.ink);
        } else {
            // A cream disc with an accent ring: something to take hold of.
            let ring = if s.muted { TEXT3 } else { self.a.base };
            self.circle(tx, cy, THUMB_R + 1.5, surface);
            self.circle(tx, cy, THUMB_R, ring);
            self.circle(tx, cy, THUMB_R - 2.5, TEXT);
        }
    }

    /// A slider over part of a page after a change (the volume takeover):
    /// the new level large, the change beside it in the text ramp, and a track whose
    /// part from the old level to the new is bright.
    fn volume_change(&mut self, x: f32, w: f32, s: &SliderLook, from: f32) {
        self.card(x, w, CARD, Press::default());
        let to = s.fraction.unwrap_or(0.0);
        let ix = x + PAD + 2.0 + ICON_MEDIUM / 2.0;
        let ic = if s.muted { TEXT3 } else { TEXT2 };
        self.icon(
            false,
            s.icon,
            ICON_MEDIUM,
            ix,
            fig_mid(&NUM_MEDIUM, BASE_MEDIUM),
            ic,
        );
        let nx = x + PAD + 2.0 + ICON_MEDIUM + ICON_GAP;
        let nw = if s.unit.is_empty() {
            self.text(Face::Num, &TITLE, s.value, nx, 32.0, TEXT2)
        } else {
            self.num_unit(
                &NUM_MEDIUM,
                s.value,
                s.unit,
                &UNIT,
                nx,
                BASE_MEDIUM,
                TEXT,
                TEXT2,
            )
        };
        let d = ((to - from) * 100.0).round() as i32;
        let delta = match d {
            0 => String::new(),
            d if d > 0 => format!("+{d}"),
            d => format!("−{}", -d),
        };
        let dx = nx + nw + 8.0;
        let dw = if delta.is_empty() {
            0.0
        } else {
            // The change is information: the text ramp, not the accent.
            self.text(Face::Num, &LABEL, &delta, dx, 22.0, TEXT2)
        };
        let tx = dx + dw.max(22.0) + 12.0;
        let tw = x + w - PAD - 8.0 - tx;
        self.slider_track(tx, 26.5, tw, to * 100.0, s, Some(from), false);
    }

    #[allow(clippy::too_many_arguments)]
    fn key(
        &mut self,
        x: f32,
        w: f32,
        zw: u32,
        icon: Option<char>,
        label: &str,
        sub: &str,
        press: Press,
    ) {
        let pressed = press.on;
        // The whole card is the key: raised face, lit top edge.
        self.key_face(x, CARD_Y, w, CARD_H, CARD_R, pressed, false);
        self.hold_fill(x, w, KEY.mix(self.a.base, HOLD_MIX), press);
        let dy = if pressed { 1.0 } else { 0.0 };
        let ic = if pressed { TEXT } else { self.a.text };
        let cy = CARD_Y + CARD_H / 2.0 + 0.5 + dy;
        let show_label = zw >= KEY_LABEL_MIN && !label.is_empty();
        let isz_for = |cls: Class| {
            if cls == Class::Compact {
                ICON_KEY
            } else {
                ICON_MEDIUM - 2.0
            }
        };
        match (class(zw), icon) {
            (Class::Hero, _) if !sub.is_empty() && !label.is_empty() => {
                let lw = self.measure(Face::Word, &TITLE, label).max(self.measure(
                    Face::Word,
                    &BODY,
                    sub,
                ));
                let iw = if icon.is_some() {
                    ICON_HERO + 12.0
                } else {
                    0.0
                };
                let total = iw + lw;
                let x0 = x + ((w - total) / 2.0).max(PAD);
                if let Some(i) = icon {
                    self.icon(true, i, ICON_HERO - 2.0, x0 + ICON_HERO / 2.0, cy, ic);
                }
                let tx = x0 + iw;
                let room = x + w - PAD - tx;
                let l = self.fit(Face::Word, &TITLE, label, room);
                self.text(Face::Word, &TITLE, &l, tx, BASE_LINE1 + dy, TEXT);
                let s = self.fit(Face::Word, &BODY, sub, room);
                self.text(Face::Word, &BODY, &s, tx, BASE_LINE2 - 1.0 + dy, TEXT2);
            }
            (cls, Some(i)) if show_label => {
                let isz = isz_for(cls);
                let lw = self.measure(Face::Word, &KEY_LABEL, label);
                let room = w - 2.0 * PAD - isz - 7.0;
                let label = self.fit(Face::Word, &KEY_LABEL, label, room);
                let lw = lw.min(room);
                let total = isz + 7.0 + lw;
                let x0 = x + ((w - total) / 2.0).max(PAD);
                self.icon(true, i, isz, x0 + isz / 2.0, cy, ic);
                self.text(
                    Face::Word,
                    &KEY_LABEL,
                    &label,
                    x0 + isz + 7.0,
                    cy + 6.5,
                    TEXT,
                );
            }
            (_, Some(i)) => {
                self.icon(true, i, ICON_MEDIUM, x + w / 2.0, cy, ic);
            }
            (_, None) => {
                let l = self.fit(Face::Word, &KEY_LABEL, label, w - 2.0 * PAD);
                self.text_c(Face::Word, &KEY_LABEL, &l, x + w / 2.0, cy + 6.5, TEXT);
            }
        }
    }

    /// "Not set up": a dashed accent outline and plain words.
    #[allow(clippy::too_many_arguments)]
    fn setup_prompt(
        &mut self,
        x: f32,
        w: f32,
        cls: Class,
        icon: char,
        title: &str,
        hint: &str,
        short: &str,
    ) {
        self.setup_card(x, w);
        let cy = CARD_Y + CARD_H / 2.0 + 0.5;
        if cls == Class::Compact {
            // Two short lines: what it is, and what to do.
            let lw = self.measure(Face::Word, &BODY, short).max(self.measure(
                Face::Word,
                &BODY,
                "Set up",
            ));
            let total = ICON_COMPACT + 7.0 + lw;
            let x0 = x + ((w - total) / 2.0).max(PAD - 2.0);
            self.icon(
                false,
                icon,
                ICON_COMPACT,
                x0 + ICON_COMPACT / 2.0,
                cy,
                self.a.base,
            );
            let tx = x0 + ICON_COMPACT + 7.0;
            let room = x + w - PAD / 2.0 - tx;
            let short = self.fit(Face::Word, &BODY, short, room);
            self.text(Face::Word, &BODY, &short, tx, BASE_LINE1 - 1.0, TEXT);
            self.text(
                Face::Word,
                &BODY,
                "Set up",
                tx,
                BASE_LINE2 - 2.0,
                self.a.text,
            );
        } else {
            let x0 = x + PAD + 2.0;
            self.icon(
                false,
                icon,
                ICON_MEDIUM,
                x0 + ICON_MEDIUM / 2.0,
                cy,
                self.a.base,
            );
            let tx = x0 + ICON_MEDIUM + 9.0;
            let room = x + w - PAD - tx;
            let t = self.fit(Face::Word, &TITLE, title, room);
            self.text(Face::Word, &TITLE, &t, tx, BASE_LINE1, TEXT);
            let h = self.fit(Face::Word, &BODY, hint, room);
            self.text(Face::Word, &BODY, &h, tx, BASE_LINE2 - 1.0, self.a.text);
        }
    }

    fn alarm(&mut self, al: &AlarmView, ok_pressed: bool, red: Rgb) {
        let x0 = EDGE + 10.0;
        self.icon(
            false,
            icons::WARNING,
            ICON_HERO,
            x0 + ICON_HERO / 2.0,
            24.0,
            CRIT_INK,
        );
        let tx = x0 + ICON_HERO + 12.0;
        // OK key: a white key on the red, the one thing to press.
        let ok = alarm_ok_span();
        let (kx, kw, kh) = (ok.start as f32, ALARM_OK_W, ALARM_OK_H);
        let ky = (HEIGHT as f32 - kh) / 2.0;
        let vw = self.measure(Face::Num, &NUM_HERO, &al.value);
        let vx = kx - 22.0 - vw;
        let room = vx - 16.0 - tx;
        self.icon(false, al.icon, ICON_SMALL + 1.0, tx + 9.0, 15.0, CRIT_INK);
        let head = self.fit(Face::Word, &TITLE, &al.headline, room - 24.0);
        self.text(Face::Word, &TITLE, &head, tx + 24.0, 22.0, CRIT_INK);
        let detail = self.fit(Face::Word, &BODY, &al.detail, room);
        self.text(
            Face::Word,
            &BODY,
            &detail,
            tx,
            42.0,
            CRIT_INK.mix(red, 0.18),
        );
        if ok_pressed {
            self.rrect(
                kx,
                ky + 1.0,
                kw,
                kh - 1.0,
                KEY_R + 1.0,
                CRIT_INK.mix(red, 0.3),
            );
        } else {
            self.rrect(kx, ky, kw, kh, KEY_R + 1.0, CRIT_INK.mix(red, 0.35));
            self.rrect(kx, ky + 1.0, kw, kh - 1.0, KEY_R + 1.0, CRIT_INK);
        }
        let dy = if ok_pressed { 1.0 } else { 0.0 };
        self.text_c(
            Face::Word,
            &OK_LABEL,
            "OK",
            kx + kw / 2.0,
            ky + kh / 2.0 + 8.0 + dy,
            red,
        );
        self.text(Face::Num, &NUM_HERO, &al.value, vx, BASE_HERO, CRIT_INK);
    }
}

/// What a slider is drawn from.
struct SliderLook<'a> {
    icon: char,
    value: &'a str,
    unit: &'a str,
    fraction: Option<f32>,
    held: bool,
    muted: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classes_by_width() {
        assert_eq!(class(128), Class::Compact);
        assert_eq!(class(149), Class::Compact);
        assert_eq!(class(150), Class::Medium);
        assert_eq!(class(299), Class::Medium);
        assert_eq!(class(300), Class::Hero);
    }

    #[test]
    fn media_keys_are_wide_enough_to_hit_and_inside_the_card() {
        for width in [120, 200, 320, 428, 640] {
            for k in media_keys(width) {
                assert!(k.span.end - k.span.start >= TARGET_MIN, "{width}: {k:?}");
                assert!(k.span.start >= CARD_INSET, "{width}: {k:?}");
                assert!(k.span.end <= width as f32 - CARD_INSET, "{width}: {k:?}");
            }
        }
        assert_eq!(media_keys(320).len(), 1);
        assert_eq!(media_keys(428).len(), 3);
        assert!(!media_keys(200)[0].key, "medium: the whole card");
    }
}
