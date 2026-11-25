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
use std::sync::Arc;

use tiny_skia::Path;

use crate::art::Art;
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
    /// The value and unit at their widest for this signal ("100°", "100"
    /// and "%"), known before the first reading: what a card that must
    /// not rearrange as the value changes is laid out for.
    pub widest: (String, String),
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
    /// The player's artwork, once it has arrived; until then, and when
    /// there is none, the generated placeholder is drawn.
    pub art: Option<Arc<Art>>,
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
        /// The second reading's icon, which names it where its name does
        /// not fit (medium cards).
        second_icon: char,
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
    /// Nothing playing, and a tap opens the music player (the widget's
    /// own `tap`, or `Config::music_action`): drawn as a key.
    OpenMusic,
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
    /// One news headline. Live, the whole card is a key (a tap opens the
    /// article); the words stay in the text ramp, since they are
    /// information.
    News {
        icon: char,
        /// The headline, whole: cut to fit when drawn.
        headline: String,
        /// The feed's name: "BBC News".
        source: String,
        /// "12 min", "now".
        age: String,
        avail: Avail,
    },
    /// The weather to come (`ForecastView`).
    Forecast(Box<ForecastView>),
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
/// "Harbour Gazette · 12 min", or whichever of the two there is.
fn news_from_text(source: &str, age: &str) -> String {
    match (source.is_empty(), age.is_empty()) {
        (false, false) => format!("{source} · {age}"),
        (false, true) => source.to_string(),
        _ => age.to_string(),
    }
}

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
    /// page markers (`draw_markers` adds them). A page with no widgets
    /// says so quietly, so it is not taken for a fault.
    pub fn render_page(&mut self, zones: &[Zone], touch: Option<Touch>) -> Canvas {
        let mut canvas = self.render_row(zones, touch, WIDTH, false);
        if zones.is_empty() {
            self.pen(&mut canvas).empty_page();
        }
        canvas
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
            if m.current && m.severity != Severity::Ok {
                // "You are here" stays cream; the trouble on this page is
                // a segment after it, so the two are never told apart by
                // length alone.
                let cw = MARKER_W_CURRENT;
                draw::rrect(canvas, x, MARKER_Y, cw, h, h / 2.0, TEXT);
                let ax = x + cw + MARKER_SPLIT;
                draw::rrect(canvas, ax, MARKER_Y, mw - cw - MARKER_SPLIT, h, h / 2.0, c);
            } else {
                draw::rrect(canvas, x, MARKER_Y, mw, h, h / 2.0, c);
            }
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

/// The value to lay a card out for, so it does not rearrange as the
/// reading changes: its digits as 8s (tabular, all one width), and at
/// least `min` of them. A value with no digits ("––") is `min` 8s.
fn wide_probe(t: &str, min: usize) -> String {
    let digits = t.chars().filter(char::is_ascii_digit).count();
    let mut out: String = if digits == 0 {
        String::new()
    } else {
        t.chars()
            .map(|c| if c.is_ascii_digit() { '8' } else { c })
            .collect()
    };
    for _ in digits..min {
        out.insert(0, '8');
    }
    out
}

/// The gap before a medium card's second reading, and the room its icon
/// takes before its number.
const SECOND_GAP: f32 = 5.0;
const SECOND_ICON_W: f32 = ICON_SMALL + 2.0;

/// Sizes for a medium value card with a second reading, chosen from the
/// card's width and the readings' units alone.
struct SecondPlan {
    /// The first reading's numeral and icon.
    num: TextStyle,
    icon: f32,
    /// The second reading's numeral, and its widest width with its unit.
    second: TextStyle,
    second_w: f32,
    /// The widest of both, and the second's icon, fit.
    fits: bool,
}

/// A compact value card's numeral and icon sizes, from the largest down,
/// tried until the reading's widest value fits beside its icon: the
/// numeral gives a step first, then the icon, then the numeral again.
const COMPACT_STEPS: [(TextStyle, f32); 6] = [
    (NUM_COMPACT, ICON_COMPACT),
    (NUM_COMPACT_TIGHT, ICON_COMPACT),
    (NUM_COMPACT_TIGHT, ICON_SMALL),
    (NUM_SECOND, ICON_SMALL),
    (NUM_SECOND_TIGHT, ICON_SMALL),
    (NUM_SECOND_SMALL, ICON_SMALL),
];

/// Every unit a rate is shown in (`layout::format_rate`), for laying a
/// rate card out for the widest.
const RATE_UNITS: [&str; 5] = ["B/s", "kB/s", "MB/s", "GB/s", "TB/s"];

/// The room a rate's arrow takes before its number, gap included.
fn rate_arrow_w(icon: f32) -> f32 {
    icon - 2.0
}

/// How a compact rate card is laid out (`Pen::rate_plan`).
enum RatePlan {
    /// "↓ 12 kB/s" on one line; `width` is the widest number and unit.
    Line {
        num: TextStyle,
        icon: f32,
        width: f32,
    },
    /// The arrow and unit above (`top` wide), the number under them.
    Stacked { top: f32, num: TextStyle },
    /// Not even that fits: the arrow alone.
    Arrow,
}

/// How a compact value card names its reading and how big its numeral is,
/// chosen from the card's width and the reading's widest value alone.
struct CompactPlan {
    /// The numeral's size; `None` when not even the smallest step holds
    /// the widest value (a card narrower than any reading needs).
    num: Option<TextStyle>,
    /// The width the numeral is laid out for: its widest.
    num_w: f32,
    /// The icon's size, beside the numeral.
    icon: f32,
    /// What names the reading above the numeral, when the icon does not
    /// fit beside it.
    above: Option<Above>,
}

/// What names a narrow compact reading above its numeral.
enum Above {
    /// Its name ("CPU"), in place of the icon.
    Name(String),
    /// Its icon, when not even the name's first word fits.
    Icon,
}

fn fmt_time(s: f64) -> String {
    let s = s.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// The ways to show a time, longest first, for `Pen::first_fit`: "9:26 /
/// 43:12" in the first style, then "9:26" in each style. Each is laid out
/// for its widest (the elapsed time as long as the total can be), so a
/// form is chosen once per track, not as the seconds tick.
fn time_forms(
    elapsed: &str,
    total: Option<&str>,
    with_total: bool,
    styles: &[TextStyle],
) -> Vec<(String, String, TextStyle)> {
    let probe = wide_probe(total.unwrap_or(elapsed), 0);
    let mut forms = Vec::new();
    if let (Some(t), true, Some(s)) = (total, with_total, styles.first()) {
        forms.push((format!("{elapsed} / {t}"), format!("{probe} / {t}"), *s));
    }
    for s in styles {
        forms.push((elapsed.to_string(), probe.clone(), *s));
    }
    forms
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

    /// `t`, or as much of it as fits in `room` with an ellipsis: cut after
    /// a whole word when that keeps most of what a cut between letters
    /// would ("Weird Fishes…" over "Weird Fishes / Ar…"), else between
    /// letters. Never a letter cut through, and never a word with a figure
    /// in it: numbers and times are shown whole or not at all (`first_fit`
    /// chooses among their shorter forms).
    fn fit(&mut self, f: Face, s: &TextStyle, t: &str, room: f32) -> String {
        if self.measure(f, s, t) <= room {
            return t.to_string();
        }
        let trim = |p: &str| {
            p.trim_end_matches([' ', '/', '-', '–', '·', ',', ':', '&'])
                .to_string()
        };
        let mut glyph = 0;
        for (e, _) in t.char_indices().skip(1) {
            if self.measure(f, s, &format!("{}…", trim(&t[..e]))) <= room {
                glyph = e;
            } else {
                break;
            }
        }
        // A number is never cut: a cut through a word with a figure in it
        // ("43:12", "194°") goes before that word instead.
        let start = t[..glyph].rfind(' ').map_or(0, |i| i + 1);
        let end = t[glyph..].find(' ').map_or(t.len(), |i| glyph + i);
        if glyph < end && t[start..end].chars().any(|c| c.is_ascii_digit()) {
            glyph = start;
        }
        let kept = trim(&t[..glyph]);
        let word = t[..glyph]
            .rfind(' ')
            .map(|i| trim(&t[..i]))
            .filter(|w| !w.is_empty() && w.chars().count() * 5 >= kept.chars().count() * 3);
        let cut = word.unwrap_or(kept);
        if cut.is_empty() {
            "…".to_string()
        } else {
            format!("{cut}…")
        }
    }

    /// The first of `forms` (each a text and its style, longest first)
    /// whose `widest` fits `room`, drawn as its text: how a number or a
    /// time is shortened, since it is never cut. `None` when none fits.
    fn first_fit(
        &mut self,
        f: Face,
        forms: &[(String, String, TextStyle)],
        room: f32,
    ) -> Option<(String, TextStyle, f32)> {
        forms.iter().find_map(|(text, widest, s)| {
            let w = self.measure(f, s, widest).max(self.measure(f, s, text));
            (w <= room).then(|| (text.clone(), *s, w))
        })
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
        // At rest the title ends in an ellipsis after a whole word, as every
        // other text that does not fit; only a moving title fades at the
        // edge it slides under.
        if !marquee.is_some_and(|m| m < 2.0 * rest + scroll + back) {
            let fitted = self.fit(f, s, t, right - x);
            self.text(f, s, &fitted, x, base, c);
            return;
        }
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

    /// The sizes for a medium value card of inner width `w` with a second
    /// reading: the largest that hold the widest values of both (three
    /// digits) and the second's icon. The second steps down first (26,
    /// 22), then the first with it (32 and 22, 32 and 20: a 200 px card
    /// inside a 204 px zone), then the first again (28 and 20: a 200 px
    /// zone).
    fn second_plan(
        &mut self,
        w: f32,
        first: &(String, String),
        second: &(String, String),
    ) -> SecondPlan {
        let room = w - 2.0 * PAD;
        let (p1, unit) = (wide_probe(&first.0, 3), first.1.as_str());
        let (p2, sunit) = (wide_probe(&second.0, 3), second.1.as_str());
        let steps = [
            (NUM_BANDED, ICON_MEDIUM, NUM_SECOND),
            (NUM_BANDED, ICON_MEDIUM, NUM_SECOND_TIGHT),
            (NUM_COMPACT, ICON_COMPACT, NUM_SECOND_TIGHT),
            (NUM_COMPACT, ICON_COMPACT, NUM_SECOND_SMALL),
            (NUM_COMPACT_TIGHT, ICON_COMPACT, NUM_SECOND_SMALL),
        ];
        let mut plan = None;
        for (num, icon, second) in steps {
            let w1 = self.num_unit_w(&num, &p1, unit, &UNIT);
            let w2 = self.num_unit_w(&second, &p2, sunit, &UNIT);
            let need = icon + ICON_GAP + w1 + SECOND_GAP + SECOND_ICON_W + w2;
            let fits = need <= room;
            plan = Some(SecondPlan {
                num,
                icon,
                second,
                second_w: w2,
                fits,
            });
            if fits {
                break;
            }
        }
        plan.expect("steps is not empty")
    }

    /// The sizes for a compact value card of width `w`. A value card always
    /// says what it reads: the icon beside the numeral at the largest step
    /// of `COMPACT_STEPS` that holds the reading's widest value ("212°",
    /// "100%"), never the value now, so nothing moves or goes as it
    /// changes. Narrower than every step, the name ("CPU") goes above a
    /// `NUM_SECOND_SMALL` numeral instead (the icon, if not even the
    /// name's first word fits); narrower still, the icon alone.
    fn compact_plan(&mut self, w: f32, r: &Reading, txt: &str, unit: &str) -> CompactPlan {
        let room = w - 2.0 * PAD;
        let probe = wide_probe(&r.widest.0, 3);
        let wide_unit = if r.widest.1.is_empty() {
            unit
        } else {
            r.widest.1.as_str()
        };
        // The value now as well, in case it is wider than its scale says
        // (a text reading): a numeral is never cut.
        let widest = |pen: &mut Self, num: &TextStyle| {
            let a = pen.num_unit_w(num, &probe, wide_unit, &UNIT);
            a.max(pen.num_unit_w(num, txt, unit, &UNIT))
        };
        for (num, icon) in COMPACT_STEPS {
            let nw = widest(self, &num);
            if icon + ICON_GAP + nw <= room {
                return CompactPlan {
                    num: Some(num),
                    num_w: nw,
                    icon,
                    above: None,
                };
            }
        }
        let num = NUM_SECOND_SMALL;
        let nw = widest(self, &num);
        let first = r.name.split(' ').next().unwrap_or_default().to_string();
        let name = [r.name.clone(), first]
            .into_iter()
            .find(|n| !n.is_empty() && self.measure(Face::Word, &LABEL, n) <= room);
        let fits = nw <= room;
        CompactPlan {
            num: fits.then_some(num),
            num_w: nw,
            icon: ICON_SMALL,
            // A card that cannot show a number still says what it would
            // show, with its icon.
            above: fits.then(|| name.map_or(Above::Icon, Above::Name)),
        }
    }

    /// How a compact rate card fits "↓ 12 kB/s" in a card `w` wide: laid
    /// out for the widest rate on its scale, three figures in its longest
    /// unit ("888 MB/s"), never the rate now. One line at the largest of
    /// `COMPACT_STEPS` that holds it; else stacked, the arrow and unit
    /// above a `NUM_SECOND_SMALL` number; else the arrow alone.
    fn rate_plan(&mut self, w: f32, txt: &str, unit: &str) -> RatePlan {
        let room = w - 2.0 * PAD;
        let units = RATE_UNITS.iter().copied().chain([unit]);
        let widest = |pen: &mut Self, num: &TextStyle| {
            let mut best = pen.num_unit_w(num, txt, unit, &LABEL);
            for u in units.clone() {
                best = best.max(pen.num_unit_w(num, "888", u, &LABEL));
            }
            best
        };
        for (num, icon) in COMPACT_STEPS {
            let width = widest(self, &num);
            if rate_arrow_w(icon) + width <= room {
                return RatePlan::Line { num, icon, width };
            }
        }
        let num = NUM_SECOND_SMALL;
        let mut unit_w = 0.0f32;
        for u in units.clone() {
            unit_w = unit_w.max(self.measure(Face::Num, &LABEL, u));
        }
        let top = rate_arrow_w(ICON_SMALL) + unit_w;
        let nw = self
            .measure(Face::Num, &num, "888")
            .max(self.measure(Face::Num, &num, txt));
        if top <= room && nw <= room {
            RatePlan::Stacked { top, num }
        } else {
            RatePlan::Arrow
        }
    }

    /// A reading's icon; a lost one carries a broken-link badge at its
    /// lower right, cut out of the icon by a ring of the card.
    fn reading_icon(&mut self, lost: bool, ch: char, size: f32, cx: f32, cy: f32, c: Rgb) {
        self.icon(false, ch, size, cx, cy, c);
        if lost {
            // Big enough to read as a broken link at 60 cm: 14 px on a
            // medium icon, 11 on the second reading's.
            let bs = (size * 0.55).max(ICON_BADGE + 1.0);
            let (bx, by) = (cx + size * 0.36, cy + size * 0.3);
            self.circle(bx, by, bs * 0.62 + 1.0, CARD);
            self.icon(true, icons::LINK_BREAK, bs, bx, by, TEXT2);
        }
    }

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
            WidgetView::Slider { .. }
                | WidgetView::Key { .. }
                | WidgetView::Media(Some(_))
                | WidgetView::OpenMusic
                | WidgetView::News {
                    avail: Avail::Live,
                    ..
                }
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
                second_icon,
            } => self.value(
                x,
                w,
                cls,
                *icon,
                reading,
                limit.as_deref(),
                second.as_ref().map(|s| (*second_icon, s)),
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
            WidgetView::OpenMusic => self.media_open(x, w, cls, press),
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
            WidgetView::News {
                icon,
                headline,
                source,
                age,
                avail,
            } => self.news(x, w, cls, *icon, headline, source, age, avail, press),
            WidgetView::Forecast(f) => self.forecast(x, w, cls, f, press),
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
        second: Option<(char, &Reading)>,
        press: Press,
    ) {
        let (second_icon, second) = (second.map(|s| s.0), second.map(|s| s.1));
        // A lost card says so all over: a second reading on it is drawn lost
        // too, never a bright number beside a grey one.
        let relost = match (second, &r.avail) {
            (Some(s), Avail::Lost(_)) if s.avail == Avail::Live => Some(Reading {
                avail: r.avail.clone(),
                bar: s.bar.map(|b| Bar { value: None, ..b }),
                ..s.clone()
            }),
            _ => None,
        };
        let second = relost.as_ref().or(second);
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
        // A lost reading keeps its own icon, its name at compact and
        // medium; a broken-link badge on it says the source is gone.
        let glyph = icon;
        match cls {
            Class::Compact => {
                // With no bar the numeral sits lower, in the middle.
                let base = if bar.is_some() {
                    BASE_COMPACT
                } else {
                    BASE_COMPACT + 3.0
                };
                let lc = if r.avail == Avail::Live {
                    kr.label
                } else {
                    TEXT3
                };
                let plan = self.compact_plan(w, r, &txt, unit);
                let cx = x + w / 2.0;
                match (plan.num, &plan.above) {
                    // The icon names it, beside the numeral.
                    (Some(num), None) => {
                        let ix = x + PAD + plan.icon / 2.0;
                        let iy = fig_mid(&num, base);
                        self.reading_icon(lost, glyph, plan.icon, ix, iy, icon_c);
                        let nx = x + PAD + plan.icon + ICON_GAP;
                        let room = x + w - PAD - nx;
                        let nx = nx + ((room - plan.num_w) / 2.0).max(0.0).floor();
                        self.num_unit(&num, &txt, unit, &UNIT, nx, base, num_c, kr.label);
                    }
                    // Too narrow for the icon beside it: the name (or the
                    // icon) above.
                    (Some(num), Some(above)) => {
                        match above {
                            Above::Name(name) => {
                                self.text_c(Face::Word, &LABEL, name, cx, 20.0, lc);
                            }
                            Above::Icon => {
                                self.reading_icon(lost, glyph, plan.icon, cx, 15.0, icon_c);
                            }
                        }
                        let nx = (x + (w - plan.num_w) / 2.0).floor();
                        self.num_unit(&num, &txt, unit, &UNIT, nx, base, num_c, kr.label);
                    }
                    (None, _) => {
                        let iy = CARD_Y + 15.0;
                        self.reading_icon(lost, glyph, plan.icon, cx, iy, icon_c);
                    }
                }
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
                let right = x + w - PAD;
                // With a second reading the card is laid out for the widest
                // both can be (three digits each: "199°", "100%"), never for
                // the values now, so nothing on it goes or moves as they
                // change: the sizes step down until the second's icon fits.
                let plan = second.map(|s| self.second_plan(w, &r.widest, &s.widest));
                let (num, isz) = plan
                    .as_ref()
                    .map_or((NUM_BANDED, ICON_MEDIUM), |p| (p.num, p.icon));
                let ix = x + PAD + isz / 2.0;
                self.reading_icon(lost, glyph, isz, ix, fig_mid(&num, base), icon_c);
                let nx = x + PAD + isz + ICON_GAP;
                let nw = self.num_unit(&num, &txt, unit, &UNIT, nx, base, num_c, kr.label);
                if let (Some(s), Some(p)) = (second, plan) {
                    let ks = reading_ink(sev, s.severity);
                    let (stxt, sbar) = shown(s);
                    let live = s.avail == Avail::Live;
                    // Lost keeps its unit, greyed; no data has none.
                    let su = if s.avail == Avail::NoData {
                        ""
                    } else {
                        s.unit.as_str()
                    };
                    let sc = if live { ks.num } else { TEXT3 };
                    let sl = if live { ks.label } else { TEXT3 };
                    let sw = self.num_unit_w(&p.second, &stxt, su, &UNIT);
                    let sx = right - sw;
                    if sx > nx + nw + SECOND_GAP {
                        self.num_unit(&p.second, &stxt, su, &UNIT, sx, base, sc, sl);
                    }
                    // Its own icon names it ("6%" alone is a riddle), just
                    // before its number. The bars split where the widest
                    // second reading's icon would start, whatever the
                    // values: a fixed scale keeps its length.
                    let split = right - p.second_w - SECOND_ICON_W;
                    if let Some(si) = second_icon.filter(|_| p.fits) {
                        let gc = if live { ks.icon } else { TEXT3 };
                        let icx = sx - SECOND_ICON_W + ICON_SMALL / 2.0;
                        let icy = fig_mid(&p.second, base);
                        let slost = matches!(s.avail, Avail::Lost(_));
                        self.reading_icon(slost, si, ICON_SMALL, icx, icy, gc);
                    }
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
                    // Laid out for a two-digit value at least, so the name
                    // does not leave just as the reading climbs into its
                    // warning; when the whole name does not fit, its first
                    // word ("CPU" of "CPU load": the icon says load).
                    let probe: String = wide_probe(&txt, 2);
                    let pw = self.num_unit_w(&NUM_BANDED, &probe, unit, &UNIT).max(nw);
                    let room = right - (nx + pw + 8.0);
                    let first = words.split(' ').next().unwrap_or_default().to_string();
                    let words = [words.clone(), first].into_iter().find(|t| {
                        r.avail != Avail::Live || self.measure(Face::Word, &LABEL, t) <= room
                    });
                    if let Some(words) = words {
                        let lw = self.measure(Face::Word, &LABEL, &words);
                        if nx + nw + 8.0 + lw <= right {
                            self.text_r(Face::Word, &LABEL, &words, right, 20.0, lc);
                        }
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
                self.reading_icon(lost, glyph, ICON_HERO, ix, fig_mid(&num, base), icon_c);
                let nx = x + PAD + 2.0 + ICON_HERO + ICON_GAP + 2.0;
                let right = x + w - PAD;
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
                    // Lost keeps its unit, greyed; no data has none.
                    let su = if s.avail == Avail::NoData {
                        ""
                    } else {
                        s.unit.as_str()
                    };
                    let sc = if live { ks.num } else { TEXT3 };
                    let sl = if live { ks.label } else { TEXT3 };
                    // Its name before its number, on the numeral's baseline.
                    let sw = self.num_unit_w(&NUM_SECOND, &stxt, su, &UNIT);
                    let nm = self.measure(Face::Word, &LABEL, &s.name) + 6.0;
                    let named = right - sw - nm >= cx + 30.0;
                    let colw = if named { sw + nm } else { sw };
                    if right - colw < cx {
                        return;
                    }
                    let sx = right - sw;
                    self.num_unit(&NUM_SECOND, &stxt, su, &UNIT, sx, base, sc, sl);
                    if named {
                        self.text(Face::Word, &LABEL, &s.name, sx - nm, base, sl);
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
                // A name may be cut; the time it was lost is shortened
                // instead, never cut.
                let words = match &r.avail {
                    Avail::Lost(t) => {
                        let forms: Vec<_> =
                            [format!("Lost at {t}"), format!("Lost {t}"), "Lost".into()]
                                .into_iter()
                                .map(|v| (v.clone(), v, LABEL))
                                .collect();
                        self.first_fit(Face::Word, &forms, right - cx)
                            .map_or_else(String::new, |(v, _, _)| v)
                    }
                    _ => self.fit(Face::Word, &LABEL, &words, right - cx),
                };
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
                let down_txt = txt(down);
                let plan = self.rate_plan(w, &down_txt, unit);
                let arrow = |pen: &mut Self, size: f32, ax: f32, cy: f32| {
                    pen.icon(
                        true,
                        icons::ARROW_DOWN,
                        size,
                        ax + size / 2.0 - 3.0,
                        cy,
                        TEXT2,
                    );
                };
                match plan {
                    // ↓ 12 kB/s on one line, sized for "888 MB/s".
                    RatePlan::Line { num, icon, width } => {
                        let ax = x + PAD;
                        arrow(self, icon, ax, fig_mid(&num, BASE_COMPACT));
                        let nx = ax + rate_arrow_w(icon);
                        let room = x + w - PAD - nx;
                        let nx = nx + ((room - width) / 2.0).max(0.0).floor();
                        let (t, u) = (down_txt.as_str(), unit);
                        self.num_unit(&num, t, u, &LABEL, nx, BASE_COMPACT, num_c, TEXT2);
                    }
                    // Too narrow for the number and its unit on one line:
                    // the arrow and the unit above, the number under them.
                    // The unit is never shortened: "12 kB" would read as an
                    // amount, not a rate.
                    RatePlan::Stacked { top, num } => {
                        let tx = (x + (w - top) / 2.0).floor();
                        let icon = ICON_SMALL;
                        arrow(self, icon, tx, 20.0 - LABEL.size * 0.35);
                        let ux = tx + rate_arrow_w(icon);
                        let u = if unit.is_empty() { "" } else { unit };
                        self.text(Face::Num, &LABEL, u, ux, 20.0, TEXT2);
                        let nw = self.measure(Face::Num, &num, &down_txt);
                        let nx = (x + (w - nw) / 2.0).floor();
                        self.text(Face::Num, &num, &down_txt, nx, BASE_COMPACT, num_c);
                    }
                    RatePlan::Arrow => {
                        arrow(self, ICON_SMALL, x + w / 2.0 - 5.0, CARD_Y + 15.0);
                    }
                }
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
                // Laid out for the widest temperature ("888°", "-88°"),
                // never the one now: the largest step that holds it beside
                // the icon, with half a pad at the sides as before; else
                // the icon above a small number; else the icon alone.
                let room = w - PAD;
                let widest = |pen: &mut Self, num: &TextStyle| {
                    ["888°", "-88°", t]
                        .iter()
                        .map(|v| pen.measure(Face::Num, num, v))
                        .fold(0.0, f32::max)
                };
                let steps = [(NUM_MEDIUM, ICON_MEDIUM)].into_iter().chain(COMPACT_STEPS);
                let mut line = None;
                for (num, isz) in steps {
                    let tw = widest(self, &num);
                    if isz + ICON_GAP + tw <= room {
                        line = Some((num, isz, tw));
                        break;
                    }
                }
                let cx = x + w / 2.0;
                match line {
                    Some((num, isz, tw)) => {
                        let x0 = (x + (w - (isz + ICON_GAP + tw)) / 2.0).floor();
                        let base = BASE_MEDIUM;
                        self.icon(false, glyph, isz, x0 + isz / 2.0, fig_mid(&num, base), ic);
                        self.text(Face::Num, &num, t, x0 + isz + ICON_GAP, base, tc);
                    }
                    None => {
                        let num = NUM_SECOND_SMALL;
                        let tw = widest(self, &num);
                        if tw <= room {
                            self.icon(false, glyph, ICON_SMALL, cx, CARD_Y + 11.0, ic);
                            let nx = (x + (w - tw) / 2.0).floor();
                            self.text(Face::Num, &num, t, nx, BASE_MEDIUM, tc);
                        } else {
                            let cy = CARD_Y + CARD_H / 2.0;
                            self.icon(false, glyph, ICON_SMALL, cx, cy, ic);
                        }
                    }
                }
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
                let l2 = self.high_low(&l2, room);
                if l2.is_empty() {
                    self.text(Face::Word, &BODY, &l1, dx, 32.0, c1);
                } else {
                    self.text(Face::Word, &BODY, &l1, dx, BASE_LINE1, c1);
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
                // "From 9:30" is shown whole or not at all (the cloud-slash
                // still says it is old); a condition may be cut.
                let line1 = if matches!(avail, Avail::Lost(_)) {
                    let w1 = self.measure(Face::Word, &TITLE, &words);
                    if w1 <= room { words } else { String::new() }
                } else {
                    self.fit(Face::Word, &TITLE, &words, room)
                };
                let c1 = if live { TEXT } else { TEXT3 };
                let l2 = if live {
                    self.high_low(high_low.unwrap_or(""), room)
                } else {
                    String::new()
                };
                if l2.is_empty() {
                    self.text(Face::Word, &TITLE, &line1, dx, 32.0, c1);
                } else {
                    self.text(Face::Word, &TITLE, &line1, dx, BASE_LINE1, c1);
                    self.text(Face::Num, &BODY, &l2, dx, BASE_LINE2 - 1.0, TEXT2);
                }
            }
        }
    }

    /// The day's high and low ("61° / 49°") in `room` at `BODY`: closed
    /// up ("61°/49°") before it would be cut, and left out before that.
    /// Also any other short line of figures, such as a time.
    fn high_low(&mut self, l2: &str, room: f32) -> String {
        let tight = l2.replace(" / ", "/");
        [l2.to_string(), tight]
            .into_iter()
            .find(|t| self.measure(Face::Num, &BODY, t) <= room)
            .unwrap_or_default()
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
                    // Elapsed only; a step smaller, or none, before it is cut.
                    let el = fmt_time(pos);
                    let len = t.length.map(fmt_time);
                    let forms = time_forms(&el, len.as_deref(), false, &[TITLE, BODY]);
                    if let Some((el, st, _)) = self.first_fit(Face::Num, &forms, x + w - PAD - tx) {
                        self.text(Face::Num, &st, &el, tx, BASE_LINE1 + 1.0, tc);
                    }
                }
                if let Some(f) = t.progress() {
                    self.progress(tx, 34.0, x + w - PAD - tx, f, t.playing);
                }
            }
            Class::Medium => {
                let ax = x + PAD - 3.0;
                self.artwork(ax, art_y, ART, ART_R, t);
                // The whole card is play/pause; a raised key at its right,
                // as the hero card has, says so. It sinks with the card.
                let kx = x + w - PAD - MEDIA_KEY_W;
                self.key_face(kx, KEY_Y, MEDIA_KEY_W, KEY_H, KEY_R, card_press.on, true);
                let dy = if card_press.on { 1.0 } else { 0.0 };
                let cy = KEY_Y + KEY_H / 2.0 + dy;
                let icx = kx + MEDIA_KEY_W / 2.0;
                self.icon(false, glyph, ICON_KEY - 2.0, icx, cy, self.a.ink);
                let tx = ax + ART + 10.0;
                let right = kx - 5.0;
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
                self.artwork(ax, art_y, ART, ART_R, t);
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
                // The length goes before the artist would be cut: the
                // elapsed time is what changes. Measured against the artist
                // even while paused, so pausing never moves the times, and
                // for the times' widest, so the seconds ticking never do.
                // The elapsed time goes too before it would be cut, or
                // leave the artist no room.
                let aw = self.measure(Face::Word, &BODY, line2).max(self.measure(
                    Face::Word,
                    &BODY,
                    &t.artist,
                ));
                let len = t.length.filter(|l| *l > 0.0).map(fmt_time);
                let (times, tw) = match t.position {
                    Some(p) => {
                        let el = fmt_time(p);
                        let forms = time_forms(&el, len.as_deref(), true, &[LABEL]);
                        let (whole, short) = forms.split_at(forms.len() - 1);
                        let short_room = right - tx - 10.0 - MEDIA_ARTIST_MIN;
                        self.first_fit(Face::Num, whole, right - tx - aw - 10.0)
                            .or_else(|| self.first_fit(Face::Num, short, short_room))
                            .map_or((String::new(), 0.0), |(v, _, w)| (v, w))
                    }
                    None => (String::new(), 0.0),
                };
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
        self.artwork(x + 4.0, ay, s, ART_R + 1.0, t);
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

    /// The track's artwork, or the placeholder seeded by its title when
    /// the player gave none (or it has not arrived, or failed); dimmed
    /// while paused.
    fn artwork(&mut self, x: f32, y: f32, s: f32, r: f32, t: &Track) {
        let drawn = t
            .art
            .as_deref()
            .is_some_and(|a| draw::artwork(self.c, x, y, s, r, a, !t.playing));
        if !drawn {
            draw::art(self.c, x, y, s, r, &t.title, !t.playing);
        }
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

    /// Nothing playing, and a tap opens the music player: the whole card
    /// is a key (raised face, lit top edge, accent glyph), and says what
    /// a tap does.
    fn media_open(&mut self, x: f32, w: f32, cls: Class, press: Press) {
        let pressed = press.on;
        self.key_face(x, CARD_Y, w, CARD_H, CARD_R, pressed, false);
        let dy = if pressed { 1.0 } else { 0.0 };
        let ic = if pressed { TEXT } else { self.a.text };
        let cy = CARD_Y + CARD_H / 2.0 + 0.5 + dy;
        let isz = match cls {
            Class::Compact => ICON_KEY,
            Class::Medium => ICON_MEDIUM - 2.0,
            Class::Hero => ICON_HERO - 2.0,
        };
        let ix = x + PAD + 2.0 + isz / 2.0;
        self.icon(true, icons::MUSIC, isz, ix, cy, ic);
        let tx = x + PAD + 2.0 + isz + 9.0;
        let room = x + w - PAD - tx;
        let (l1, l2, st1) = match cls {
            Class::Compact => ("Open", "music", LABEL),
            Class::Medium => ("Nothing playing", "Tap to open music", BODY),
            Class::Hero => ("Nothing playing", "Tap to open music", TITLE),
        };
        let l1 = self.fit(Face::Word, &st1, l1, room);
        let l2 = if self.measure(Face::Word, &BODY, l2) <= room {
            l2
        } else {
            "Open music"
        };
        let l2 = self.fit(Face::Word, &BODY, l2, room);
        self.text(Face::Word, &st1, &l1, tx, BASE_LINE1 + dy, TEXT);
        self.text(Face::Word, &BODY, &l2, tx, BASE_LINE2 - 1.0 + dy, TEXT2);
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
                } else if let Some((v, st, _)) =
                    self.slider_words(s.value, &[BODY], x + w - PAD - nx)
                {
                    self.text(Face::Num, &st, &v, nx, 24.0, TEXT2);
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
                    // Laid out for the words' widest, so the track does not
                    // move as a time ticks.
                    match self.slider_words(s.value, &[BODY], (w - 2.0 * PAD) / 2.0) {
                        Some((v, st, vw)) => {
                            self.text(Face::Num, &st, &v, nx, 32.0, TEXT2);
                            vw
                        }
                        None => 0.0,
                    }
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
                    let room = (w - 2.0 * PAD) / 2.0;
                    match self.slider_words(s.value, &[TITLE, BODY], room) {
                        Some((v, st, vw)) => {
                            self.text(Face::Num, &st, &v, nx, 33.0, TEXT2);
                            vw
                        }
                        None => 0.0,
                    }
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

    /// A slider's words in `room`, in the first of `styles` that holds
    /// them, with the width they are laid out for. A time ("9:26 / 43:12")
    /// is never cut: it drops "/ 43:12", then takes the next style, then
    /// goes. Other words ("Muted") end in an ellipsis as usual.
    fn slider_words(
        &mut self,
        value: &str,
        styles: &[TextStyle],
        room: f32,
    ) -> Option<(String, TextStyle, f32)> {
        if let Some((elapsed, total)) = value.split_once(" / ") {
            let forms = time_forms(elapsed, Some(total), true, styles);
            return self.first_fit(Face::Num, &forms, room);
        }
        let st = *styles.first()?;
        let v = self.fit(Face::Num, &st, value, room);
        let vw = self.measure(Face::Num, &st, &v);
        Some((v, st, vw))
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

    /// A news headline. Live, the whole card is a key, as a launch key is:
    /// raised face, lit top edge, and the newspaper in the accent, since a
    /// tap opens the article; the headline and its source stay in the text
    /// ramp. From medium up the headline runs over two lines
    /// (`news_lines`) and never scrolls. A compact card has no room for a
    /// headline that can be read, so it shows the newspaper and the
    /// headline's age.
    #[allow(clippy::too_many_arguments)]
    fn news(
        &mut self,
        x: f32,
        w: f32,
        cls: Class,
        icon: char,
        headline: &str,
        source: &str,
        age: &str,
        avail: &Avail,
        press: Press,
    ) {
        if *avail == Avail::NotSetUp {
            let long = "Add a feed in Settings";
            let room = x + w - PAD - (x + PAD + 2.0 + ICON_MEDIUM + 9.0);
            let hint = if self.measure(Face::Word, &BODY, long) <= room {
                long
            } else {
                "Add a feed"
            };
            self.setup_prompt(x, w, cls, icons::PLUS, "News", hint, "News");
            return;
        }
        if *avail == Avail::NoData {
            // Waiting for the first headline: a quiet card that does
            // nothing, with the newspaper in outline.
            self.card(x, w, CARD, Press::default());
            let cy = CARD_Y + CARD_H / 2.0 + 0.5;
            let isz = if cls == Class::Compact {
                ICON_COMPACT
            } else {
                ICON_MEDIUM
            };
            let ix = x + PAD + isz / 2.0;
            self.icon(true, icon, isz, ix, cy, TEXT3);
            let tx = x + PAD + isz + 8.0;
            let room = x + w - PAD - tx;
            if cls == Class::Compact {
                self.text(Face::Word, &LABEL, "No news", tx, BASE_LINE1 - 1.0, TEXT2);
                self.text(Face::Word, &LABEL, "yet", tx, BASE_LINE2 - 3.0, TEXT2);
            } else {
                let st = if cls == Class::Hero { TITLE } else { BODY };
                let l = self.fit(Face::Word, &st, "No news yet", room);
                self.text(Face::Word, &st, &l, tx, 32.0, TEXT2);
            }
            return;
        }
        let live = *avail == Avail::Live;
        let pressed = live && press.on;
        let dy = if pressed { 1.0 } else { 0.0 };
        if live {
            self.key_face(x, CARD_Y, w, CARD_H, CARD_R, pressed, false);
            self.hold_fill(x, w, KEY.mix(self.a.base, HOLD_MIX), press);
        } else {
            // Lost: every feed failing and the last headline too old. A
            // flat card in TEXT3 with the broken link; a tap does nothing.
            self.card(x, w, CARD, Press::default());
        }
        let glyph_c = if pressed { TEXT } else { self.a.text };
        let head_c = if live { TEXT } else { TEXT3 };
        if cls == Class::Compact {
            let tw = self.measure(Face::Word, &TITLE, age);
            let total = ICON_KEY + ICON_GAP + tw;
            let x0 = x + ((w - total) / 2.0).max(PAD);
            let cy = CARD_Y + CARD_H / 2.0 + 0.5 + dy;
            self.news_glyph(live, icon, ICON_KEY, x0 + ICON_KEY / 2.0, cy, glyph_c);
            let tx = x0 + ICON_KEY + ICON_GAP;
            // The age whole ("12 min"), or not at all: never "12…".
            if tx + tw <= x + w - PAD {
                self.text(Face::Word, &TITLE, age, tx, cy + 7.0, head_c);
            }
            return;
        }
        let (left, right) = (x + PAD, x + w - PAD);
        if live {
            let hero = cls == Class::Hero;
            self.news_lines(left, right, dy, hero, icon, headline, source, age, glyph_c);
        } else {
            // Lost: the headline's start, and under it the broken link
            // with where it was from and how old it is, always shown.
            let line1 = self.fit(Face::Word, &LABEL, headline, right - left);
            self.text(Face::Word, &LABEL, &line1, left, NEWS_BASE1, TEXT3);
            self.news_from(left, right, NEWS_BASE2, false, icon, source, age, TEXT3);
        }
    }

    /// A live headline from medium up: over two lines at `LABEL` size, as
    /// many whole words as fit on the first, the rest on the second, cut
    /// there with an ellipsis by the shared rule. On a hero card the
    /// newspaper and "Source · 12 min" follow the headline's end on the
    /// second line when they fit there, and are left out when they do not:
    /// the headline matters more. A headline on one line with nothing under
    /// it is centred on the card.
    #[allow(clippy::too_many_arguments)]
    fn news_lines(
        &mut self,
        left: f32,
        right: f32,
        dy: f32,
        hero: bool,
        icon: char,
        headline: &str,
        source: &str,
        age: &str,
        glyph_c: Rgb,
    ) {
        let room = right - left;
        let words: Vec<&str> = headline.split_whitespace().collect();
        let mut n = 0;
        while n < words.len() && self.measure(Face::Word, &LABEL, &words[..=n].join(" ")) <= room {
            n += 1;
        }
        let (line1, rest) = if n == 0 {
            // One word longer than the card: cut it, and stop there.
            (self.fit(Face::Word, &LABEL, headline, room), String::new())
        } else {
            (words[..n].join(" "), words[n..].join(" "))
        };
        let rest = self.fit(Face::Word, &LABEL, &rest, room);
        let from = news_from_text(source, age);
        let after = if rest.is_empty() {
            left
        } else {
            left + self.measure(Face::Word, &LABEL, &rest) + NEWS_FROM_GAP
        };
        let from_w = ICON_SMALL + ICON_GAP + self.measure(Face::Word, &LABEL, &from);
        let show_from = hero && !from.is_empty() && after + from_w <= right;
        if rest.is_empty() && !show_from {
            self.text(Face::Word, &LABEL, &line1, left, NEWS_BASE_ONE + dy, TEXT);
            return;
        }
        self.text(Face::Word, &LABEL, &line1, left, NEWS_BASE1 + dy, TEXT);
        self.text(Face::Word, &LABEL, &rest, left, NEWS_BASE2 + dy, TEXT);
        if show_from {
            self.news_from(
                after,
                right,
                NEWS_BASE2 + dy,
                true,
                icon,
                source,
                age,
                glyph_c,
            );
        }
    }

    /// The newspaper, then "Harbour Gazette · 12 min" on the baseline
    /// `base`, from `x`. The age stays whole: the source is cut first, and
    /// left out before it would be cut to fewer than four letters.
    #[allow(clippy::too_many_arguments)]
    fn news_from(
        &mut self,
        x: f32,
        right: f32,
        base: f32,
        live: bool,
        icon: char,
        source: &str,
        age: &str,
        glyph_c: Rgb,
    ) {
        let icy = base - LABEL.size * 0.35;
        self.news_glyph(live, icon, ICON_SMALL, x + ICON_SMALL / 2.0, icy, glyph_c);
        let lx = x + ICON_SMALL + ICON_GAP;
        let room = right - lx;
        // The age is whole or left out, never "12…".
        let whole_age = |pen: &mut Self| {
            let aw = pen.measure(Face::Word, &LABEL, age);
            if aw <= room {
                age.to_string()
            } else {
                String::new()
            }
        };
        let words = if age.is_empty() {
            self.fit(Face::Word, &LABEL, source, room)
        } else if source.is_empty() {
            whole_age(self)
        } else {
            let tail = self.measure(Face::Word, &LABEL, &format!(" · {age}"));
            let fitted = self.fit(Face::Word, &LABEL, source, room - tail);
            if fitted.chars().count() < 4 {
                whole_age(self)
            } else {
                format!("{fitted} · {age}")
            }
        };
        let c = if live { TEXT2 } else { TEXT3 };
        self.text(Face::Word, &LABEL, &words, lx, base, c);
    }

    /// The newspaper on a news card: in outline, as on a key; in the
    /// accent while live, TEXT3 with the broken link once lost.
    fn news_glyph(&mut self, live: bool, ch: char, size: f32, cx: f32, cy: f32, c: Rgb) {
        if live {
            self.icon(true, ch, size, cx, cy, c);
            return;
        }
        self.icon(true, ch, size, cx, cy, TEXT3);
        let bs = ICON_BADGE + 1.0;
        let (bx, by) = (cx + size * 0.42, cy + size * 0.32);
        self.circle(bx, by, bs * 0.62 + 1.0, CARD);
        self.icon(true, icons::LINK_BREAK, bs, bx, by, TEXT2);
    }

    /// A page with no widgets: two quiet lines centred on the strip, in
    /// the secondary text colour, on the bare ground. No card, outline,
    /// icon or accent, so it reads as neither a fault nor a control.
    fn empty_page(&mut self) {
        let cx = WIDTH as f32 / 2.0;
        self.text_c(Face::Word, &LABEL, "Empty page", cx, BASE_LINE1, TEXT2);
        let hint = "Add widgets in Settings";
        self.text_c(Face::Word, &BODY, hint, cx, BASE_LINE2 - 1.0, TEXT2);
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
        // Too narrow for "Set up" beside the icon: the icon alone, in its
        // dashed outline, never words running into the next card.
        let set_up = self.measure(Face::Word, &BODY, "Set up");
        if cls == Class::Compact && w < ICON_COMPACT + 7.0 + set_up + PAD * 1.5 {
            self.icon(false, icon, ICON_COMPACT, x + w / 2.0, cy, self.a.base);
            return;
        }
        if cls == Class::Compact {
            // Two short lines: what it is, and what to do.
            let lw = self.measure(Face::Word, &BODY, short).max(set_up);
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

// ------------------------------------------------------- layout picker

/// Where the layout picker's cards sit: each card's left edge, in the
/// layouts' order. The row is centred on the current layout, then shifted
/// only as far as keeps the highlighted card `PICKER_MARGIN` inside the
/// strip, so it scrolls when there are more names than fit.
pub fn picker_lefts(count: usize, current: usize, selected: usize) -> Vec<f32> {
    let pitch = PICKER_CARD_W + PICKER_GAP;
    let mut origin = (WIDTH as f32 - PICKER_CARD_W) / 2.0 - current as f32 * pitch;
    let left = origin + selected as f32 * pitch;
    let (lo, hi) = (PICKER_MARGIN, WIDTH as f32 - PICKER_MARGIN - PICKER_CARD_W);
    if left < lo {
        origin += lo - left;
    } else if left > hi {
        origin -= left - hi;
    }
    (0..count)
        .map(|i| (origin + i as f32 * pitch).round())
        .collect()
}

impl Renderer {
    /// The layout picker, over the whole strip and without page markers:
    /// a row of key-coloured cards, one per layout, and the highlighted
    /// one (what lifting switches to) filled with the accent, since it is
    /// the control.
    pub fn render_picker(&mut self, names: &[String], current: usize, selected: usize) -> Canvas {
        let mut canvas = Canvas::new(WIDTH, HEIGHT, GROUND);
        self.alerts.clear();
        self.moving = false;
        self.pen(&mut canvas).picker(names, current, selected);
        canvas
    }

    /// The new layout's name, shown for a moment after a switch from the
    /// strip: one card the width of the strip, "Layout" and the name.
    /// Information, so no accent. Drawn over the page with the page
    /// markers on top, as a takeover is.
    pub fn render_layout_name(&mut self, name: &str) -> Canvas {
        let mut canvas = Canvas::new(WIDTH, HEIGHT, GROUND);
        self.alerts.clear();
        self.moving = false;
        self.pen(&mut canvas).layout_name(name);
        canvas
    }
}

impl Pen<'_> {
    fn picker(&mut self, names: &[String], current: usize, selected: usize) {
        let w = PICKER_CARD_W;
        let strip = WIDTH as f32;
        for (i, (name, &x)) in names
            .iter()
            .zip(&picker_lefts(names.len(), current, selected))
            .enumerate()
        {
            if x + w <= 0.0 || x >= strip {
                continue;
            }
            let on = i == selected;
            self.key_face(x, CARD_Y, w, CARD_H, CARD_R, false, on);
            // Only a whole card is named, so no letter is cut by the
            // strip's end; a sliver of card says there is more that way.
            if x >= 0.0 && x + w <= strip {
                let ink = if on { self.a.ink } else { TEXT };
                let label = self.fit(Face::Word, &TITLE, name, w - 2.0 * PAD);
                self.text_c(Face::Word, &TITLE, &label, x + w / 2.0, PICKER_BASE, ink);
            }
        }
    }

    fn layout_name(&mut self, name: &str) {
        let (x, w) = (EDGE, WIDTH as f32 - 2.0 * EDGE);
        self.rrect(x, CARD_Y, w, CARD_H, CARD_R, CARD);
        let lead = "Layout";
        let lead_w = self.measure(Face::Word, &LABEL, lead);
        let room = w - 2.0 * PAD - lead_w - LAYOUT_NAME_GAP;
        let name = self.fit(Face::Word, &TITLE_BIG, name, room);
        let name_w = self.measure(Face::Word, &TITLE_BIG, &name);
        let x0 = (x + (w - lead_w - LAYOUT_NAME_GAP - name_w) / 2.0).round();
        self.text(Face::Word, &LABEL, lead, x0, LAYOUT_NAME_BASE, TEXT2);
        let nx = x0 + lead_w + LAYOUT_NAME_GAP;
        self.text(Face::Word, &TITLE_BIG, &name, nx, LAYOUT_NAME_BASE, TEXT);
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

// ---------------------------------------------------------------- forecast

/// One column of a forecast: an hour, or a day.
#[derive(Clone, Debug, PartialEq)]
pub struct ForecastColumn {
    /// "15:00", "3 PM", "Mon".
    pub label: String,
    pub icon: char,
    /// The temperature at the hour, or the day's high: "48°".
    pub temp: String,
    /// The day's low: "39°".
    pub low: Option<String>,
    /// The chance of rain, 0–1, when it is at least `forecast::RAIN_SHOWN`.
    pub rain: Option<f32>,
}

/// The next change in the weather, worded (`forecast::Change::words`).
#[derive(Clone, Debug, PartialEq)]
pub struct ForecastNext {
    pub icon: char,
    /// "Rain from", "Snow", "Clearing by".
    pub head: String,
    /// "15:00", "tonight", "all day".
    pub tail: String,
    /// What a compact card says: "15:00", or "Dry".
    pub short: String,
}

/// What a forecast widget shows. Columns are laid out for the widest
/// label and temperature they can have (`label_widest`, `temp_widest`),
/// never the ones now, so nothing moves as the forecast changes.
#[derive(Clone, Debug, PartialEq)]
pub struct ForecastView {
    pub show: crate::config::ForecastShow,
    /// The hours, or the days, as `show` says; empty for `Next`.
    pub columns: Vec<ForecastColumn>,
    /// Also the compact card of every kind.
    pub next: Option<ForecastNext>,
    /// Every label a column can have at its widest: "88:00", or the day
    /// names.
    pub label_widest: Vec<String>,
    /// The widest temperature in the unit shown: "888°", "-88°".
    pub temp_widest: String,
    /// Every short a compact card can say: the widest times and words.
    pub short_widest: Vec<String>,
    pub avail: Avail,
}

impl Pen<'_> {
    /// The forecast widget: columns of hours or days, or the next change
    /// in words; at compact width always the next change's icon and time.
    fn forecast(&mut self, x: f32, w: f32, cls: Class, f: &ForecastView, press: Press) {
        use crate::config::ForecastShow;
        let at = match &f.avail {
            Avail::NotSetUp => {
                let pin = icons::MAP_PIN_PLUS;
                self.setup_prompt(x, w, cls, pin, "Weather", "Add your town", "Weather");
                return;
            }
            Avail::Lost(at) => Some(at.as_str()),
            _ => None,
        };
        self.card(x, w, CARD, press);
        if let Some(at) = at {
            // As the weather card: the cloud with a slash, and the time the
            // forecast is from. An old forecast's hours are past, so they
            // are not shown.
            let hero = format!("Forecast from {at}");
            self.forecast_quiet(x, w, cls, (&hero, "Forecast", &format!("from {at}")), at);
            return;
        }
        let drawn = f.avail == Avail::Live
            && match (cls, f.show, &f.next) {
                (Class::Compact, _, Some(n)) => {
                    self.forecast_compact(x, w, n, &f.short_widest);
                    true
                }
                (Class::Compact, _, None) => false,
                (_, ForecastShow::Next, Some(n)) => {
                    self.forecast_next(x, w, cls, n);
                    true
                }
                (_, ForecastShow::Next, None) => false,
                _ if f.columns.is_empty() => false,
                (_, show, next) => {
                    let days = show == ForecastShow::Days;
                    // Not even one column fits: the compact form.
                    self.forecast_columns(x, w, f, days)
                        || next.as_ref().is_some_and(|n| {
                            self.forecast_compact(x, w, n, &f.short_widest);
                            true
                        })
                }
            };
        if !drawn {
            let words = "No forecast yet";
            self.forecast_quiet(x, w, cls, (words, words, ""), DASHES);
        }
    }

    /// No forecast to show: the cloud with a slash and plain words, as the
    /// weather card says it. `words` is the hero line and the medium's two
    /// lines; `short` is what a compact card shows beside the icon.
    fn forecast_quiet(
        &mut self,
        x: f32,
        w: f32,
        cls: Class,
        words: (&str, &str, &str),
        short: &str,
    ) {
        let glyph = icons::CLOUD_SLASH;
        let mid = CARD_Y + CARD_H / 2.0;
        match cls {
            Class::Compact => {
                let s = NUM_SECOND_SMALL;
                let tw = self.measure(Face::Num, &s, short);
                let total = ICON_SMALL + ICON_GAP + tw;
                if total <= w - PAD {
                    let x0 = (x + (w - total) / 2.0).floor();
                    let base = (mid + s.size * 0.35).round();
                    self.icon(false, glyph, ICON_SMALL, x0 + ICON_SMALL / 2.0, mid, TEXT3);
                    self.text(
                        Face::Num,
                        &s,
                        short,
                        x0 + ICON_SMALL + ICON_GAP,
                        base,
                        TEXT3,
                    );
                } else {
                    self.icon(false, glyph, ICON_SMALL, x + w / 2.0, mid, TEXT3);
                }
            }
            Class::Medium => {
                let x0 = x + PAD;
                self.icon(
                    false,
                    glyph,
                    ICON_MEDIUM,
                    x0 + ICON_MEDIUM / 2.0,
                    mid,
                    TEXT3,
                );
                let dx = x0 + ICON_MEDIUM + 8.0;
                let room = x + w - PAD - dx;
                let (l1, l2) = (words.1, words.2);
                if l2.is_empty() {
                    let l1 = self.fit(Face::Word, &BODY, l1, room);
                    self.text(Face::Word, &BODY, &l1, dx, 32.0, TEXT2);
                } else {
                    let l1 = self.fit(Face::Word, &BODY, l1, room);
                    self.text(Face::Word, &BODY, &l1, dx, BASE_LINE1, TEXT3);
                    let l2 = self.high_low(l2, room);
                    self.text(Face::Word, &BODY, &l2, dx, BASE_LINE2 - 1.0, TEXT3);
                }
            }
            Class::Hero => {
                let x0 = x + PAD + 4.0;
                self.icon(false, glyph, ICON_HERO, x0 + ICON_HERO / 2.0, mid, TEXT3);
                let dx = x0 + ICON_HERO + 10.0;
                let room = x + w - PAD - dx;
                // "Forecast from 9:30" whole, or "No forecast yet" fitted.
                let line = if self.measure(Face::Word, &TITLE, words.0) <= room {
                    words.0.to_string()
                } else {
                    self.fit(Face::Word, &TITLE, "No forecast", room)
                };
                let c = if words.2.is_empty() { TEXT2 } else { TEXT3 };
                self.text(Face::Word, &TITLE, &line, dx, 33.0, c);
            }
        }
    }

    /// A compact forecast: the next change's icon and its time ("15:00")
    /// or word ("Dry"), at the largest step that holds the widest of them
    /// all beside the icon; narrower, the icon above a small one; else
    /// the icon alone.
    fn forecast_compact(&mut self, x: f32, w: f32, n: &ForecastNext, widest: &[String]) {
        let room = w - PAD;
        let face = if n.short.chars().any(|c| c.is_ascii_digit()) {
            Face::Num
        } else {
            Face::Word
        };
        let wide = |pen: &mut Self, s: &TextStyle| {
            widest
                .iter()
                .chain([&n.short])
                .map(|t| {
                    let a = pen.measure(Face::Num, s, t);
                    a.max(pen.measure(Face::Word, s, t))
                })
                .fold(0.0, f32::max)
        };
        let steps = [
            (NUM_SECOND, ICON_COMPACT),
            (NUM_SECOND_TIGHT, ICON_COMPACT),
            (NUM_SECOND_TIGHT, ICON_SMALL),
            (NUM_SECOND_SMALL, ICON_SMALL),
        ];
        let mid = CARD_Y + CARD_H / 2.0;
        for (s, isz) in steps {
            let tw = wide(self, &s);
            if isz + ICON_GAP + tw <= room {
                let x0 = (x + (w - (isz + ICON_GAP + tw)) / 2.0).floor();
                let base = (mid + s.size * 0.35).round();
                let cy = fig_mid(&s, base);
                self.icon(false, n.icon, isz, x0 + isz / 2.0, cy, TEXT2);
                self.text(face, &s, &n.short, x0 + isz + ICON_GAP, base, TEXT);
                return;
            }
        }
        let s = NUM_SECOND_SMALL;
        let cx = x + w / 2.0;
        if wide(self, &s) <= room {
            self.icon(false, n.icon, ICON_SMALL, cx, CARD_Y + 9.5, TEXT2);
            self.text_c(face, &s, &n.short, cx, BASE_LINE2, TEXT);
        } else {
            self.icon(false, n.icon, ICON_SMALL, cx, mid, TEXT2);
        }
    }

    /// The next change in words: on a hero card one line, the icon and the
    /// sentence centred; on a medium card the icon and two lines ("Rain
    /// from" / "15:00"). The time is never cut.
    fn forecast_next(&mut self, x: f32, w: f32, cls: Class, n: &ForecastNext) {
        let mid = CARD_Y + CARD_H / 2.0;
        let tail_face = if n.tail.chars().any(|c| c.is_ascii_digit()) {
            Face::Num
        } else {
            Face::Word
        };
        if cls == Class::Hero {
            let room = w - 2.0 * PAD;
            let gap = 10.0;
            for s in [TITLE_BIG, TITLE] {
                let hw = self.measure(Face::Word, &s, &n.head);
                let space = self.measure(Face::Word, &s, " ");
                let tw = self.measure(tail_face, &s, &n.tail);
                let total = ICON_HERO + gap + hw + space + tw;
                if total <= room || s == TITLE {
                    let x0 = (x + ((w - total) / 2.0).max(PAD)).round();
                    let base = (mid + s.size * 0.35).round();
                    let cy = base - s.size * 0.35;
                    self.icon(false, n.icon, ICON_HERO, x0 + ICON_HERO / 2.0, cy, TEXT2);
                    let tx = x0 + ICON_HERO + gap;
                    let head = self.fit(Face::Word, &s, &n.head, x + w - PAD - tx - space - tw);
                    let hw = self.text(Face::Word, &s, &head, tx, base, TEXT);
                    self.text(tail_face, &s, &n.tail, tx + hw + space, base, TEXT);
                    return;
                }
            }
        }
        // Medium: the sentence on one line at `TITLE` when it fits ("Dry
        // all day"), centred as a news card's single line; else two lines
        // at the strip's smallest size, as the weather card's words ("Rain
        // from" / "15:00"): a larger first line would touch the second.
        let x0 = x + PAD;
        self.icon(
            false,
            n.icon,
            ICON_MEDIUM,
            x0 + ICON_MEDIUM / 2.0,
            mid,
            TEXT2,
        );
        let dx = x0 + ICON_MEDIUM + 8.0;
        let room = x + w - PAD - dx;
        let space = self.measure(Face::Word, &TITLE, " ");
        let hw = self.measure(Face::Word, &TITLE, &n.head);
        if hw + space + self.measure(tail_face, &TITLE, &n.tail) <= room {
            let base = (mid + TITLE.size * 0.35).round();
            self.text(Face::Word, &TITLE, &n.head, dx, base, TEXT);
            self.text(tail_face, &TITLE, &n.tail, dx + hw + space, base, TEXT);
            return;
        }
        let head = self.fit(Face::Word, &BODY, &n.head, room);
        self.text(Face::Word, &BODY, &head, dx, FORECAST_BASE1, TEXT);
        if self.measure(tail_face, &LABEL, &n.tail) <= room {
            self.text(tail_face, &LABEL, &n.tail, dx, BASE_LINE2 - 1.0, TEXT);
        }
    }

    /// Columns of hours or days, each the same width whatever it holds:
    /// as many as fit, spread over the card. False when not even one
    /// fits, so the caller draws the compact form.
    fn forecast_columns(&mut self, x: f32, w: f32, f: &ForecastView, days: bool) -> bool {
        let label_face = if days { Face::Word } else { Face::Num };
        let (num, low_st) = (NUM_SECOND_SMALL, NUM_SECOND_SMALL);
        let widest = |pen: &mut Self, face: Face, s: &TextStyle, texts: &[&str]| {
            texts
                .iter()
                .map(|t| pen.measure(face, s, t))
                .fold(0.0, f32::max)
        };
        let mut labels: Vec<&str> = f.label_widest.iter().map(String::as_str).collect();
        labels.extend(f.columns.iter().map(|c| c.label.as_str()));
        let mut temps: Vec<&str> = vec![f.temp_widest.as_str()];
        temps.extend(f.columns.iter().map(|c| c.temp.as_str()));
        let mut lows: Vec<&str> = vec![f.temp_widest.as_str()];
        lows.extend(f.columns.iter().filter_map(|c| c.low.as_deref()));
        let label_w = widest(self, label_face, &LABEL, &labels);
        let temp_w = widest(self, Face::Num, &num, &temps);
        let content_w = if days {
            let low_w = widest(self, Face::Num, &low_st, &lows);
            (label_w + ICON_GAP + ICON_SMALL).max(temp_w + FORECAST_LOW_GAP + low_w)
        } else {
            label_w.max(ICON_SMALL + FORECAST_ICON_GAP + temp_w)
        };
        let block = FORECAST_GAUGE_W + FORECAST_GAUGE_GAP + content_w;
        let inner = w - 2.0 * PAD;
        let fit = ((inner + FORECAST_COL_GAP) / (block + FORECAST_COL_GAP)).floor() as usize;
        let n = fit.min(f.columns.len());
        if n == 0 {
            return false;
        }
        let slot = inner / n as f32;
        let base1 = FORECAST_BASE1;
        let row1_mid = base1 - LABEL.size * 0.35;
        let base2 = BASE_LINE2 - 1.0;
        for (i, col) in f.columns.iter().take(n).enumerate() {
            let bx = (x + PAD + i as f32 * slot + (slot - block) / 2.0).round();
            if let Some(rain) = col.rain {
                self.rain_gauge(bx, rain);
            }
            // Each line centred in the column's fixed box: the box never
            // moves, and the words in it sit together.
            let cx = bx + FORECAST_GAUGE_W + FORECAST_GAUGE_GAP;
            let centred = |row: f32| (cx + (content_w - row) / 2.0).round();
            let lw = self.measure(label_face, &LABEL, &col.label);
            let tw = self.measure(Face::Num, &num, &col.temp);
            if days {
                let x1 = centred(lw + ICON_GAP + ICON_SMALL);
                self.text(label_face, &LABEL, &col.label, x1, base1, TEXT2);
                let ix = x1 + lw + ICON_GAP + ICON_SMALL / 2.0;
                self.icon(false, col.icon, ICON_SMALL, ix, row1_mid, TEXT2);
                let low = col.low.as_deref().unwrap_or_default();
                let low_w = self.measure(Face::Num, &low_st, low);
                let x2 = centred(tw + FORECAST_LOW_GAP + low_w);
                self.text(Face::Num, &num, &col.temp, x2, base2, TEXT);
                let lx = x2 + tw + FORECAST_LOW_GAP;
                self.text(Face::Num, &low_st, low, lx, base2, TEXT2);
            } else {
                self.text(label_face, &LABEL, &col.label, centred(lw), base1, TEXT2);
                let x2 = centred(ICON_SMALL + FORECAST_ICON_GAP + tw);
                let cy = fig_mid(&num, base2);
                self.icon(
                    false,
                    col.icon,
                    ICON_SMALL,
                    x2 + ICON_SMALL / 2.0,
                    cy,
                    TEXT2,
                );
                let tx = x2 + ICON_SMALL + FORECAST_ICON_GAP;
                self.text(Face::Num, &num, &col.temp, tx, base2, TEXT);
            }
        }
        true
    }

    /// A column's chance of rain: a pill gauge filled from the bottom, on
    /// a fixed 0–100 % scale.
    fn rain_gauge(&mut self, x: f32, rain: f32) {
        let (y, h, gw) = (FORECAST_GAUGE_Y, FORECAST_GAUGE_H, FORECAST_GAUGE_W);
        let r = gw / 2.0;
        self.rrect(x, y, gw, h, r, TRACK);
        let fh = (h * rain.clamp(0.0, 1.0)).max(gw);
        self.rrect(x, y + h - fh, gw, fh, r, FILL);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_medium_second_reading_keeps_its_icon_at_its_widest() {
        let mut r = Renderer::new(Theme::default(), crate::fonts::FontChoice::default().load());
        let mut c = Canvas::new(WIDTH, HEIGHT, CARD);
        let mut p = r.pen(&mut c);
        let pc = ("100".to_string(), "%".to_string());
        for temp in ["212°", "100°"] {
            let first = (temp.to_string(), String::new());
            for zone in [200.0, 204.0] {
                let plan = p.second_plan(zone - 2.0 * CARD_INSET, &first, &pc);
                assert!(plan.fits, "{temp} in a {zone} px zone");
                assert!(plan.second.size >= TEXT_FLOOR);
            }
        }
    }

    fn temp(value: &str) -> Reading {
        Reading {
            name: "CPU".into(),
            value: value.into(),
            unit: String::new(),
            severity: Severity::Ok,
            avail: Avail::Live,
            bar: None,
            widest: ("212°".into(), String::new()),
        }
    }

    #[test]
    fn a_compact_reading_always_says_what_it_reads() {
        let mut r = Renderer::new(Theme::default(), crate::fonts::FontChoice::default().load());
        let mut c = Canvas::new(WIDTH, HEIGHT, CARD);
        let mut p = r.pen(&mut c);
        // Six widgets on a page: 106 and 107 px zones, three-digit °F.
        for zone in [106.0, 107.0, 128.0] {
            let plan = p.compact_plan(zone - 2.0 * CARD_INSET, &temp("111°"), "111°", "");
            assert!(plan.above.is_none(), "the icon beside it at {zone}");
            assert!(plan.num.is_some_and(|n| n.size >= TEXT_FLOOR));
        }
        // Narrower than any icon step: the name above the numeral.
        let plan = p.compact_plan(84.0, &temp("111°"), "111°", "");
        assert!(matches!(plan.above, Some(Above::Name(ref n)) if n == "CPU"));
        // Laid out for the widest, not the value now: "45°" and "212°"
        // get the same sizes.
        for w in [92.0, 100.0, 104.0, 112.0, 124.0] {
            let a = p.compact_plan(w, &temp("45°"), "45°", "");
            let b = p.compact_plan(w, &temp("212°"), "212°", "");
            assert_eq!((a.num, a.icon), (b.num, b.icon), "at {w}");
        }
    }

    #[test]
    fn a_compact_rate_fits_its_widest_or_stacks() {
        let mut r = Renderer::new(Theme::default(), crate::fonts::FontChoice::default().load());
        let mut c = Canvas::new(WIDTH, HEIGHT, CARD);
        let mut p = r.pen(&mut c);
        for zone in [96.0, 106.0, 112.0, 124.0, 128.0] {
            let w = zone - 2.0 * CARD_INSET;
            // The same plan whatever the rate now.
            let a = p.rate_plan(w, "12", "kB/s");
            let b = p.rate_plan(w, "999", "MB/s");
            match (a, b) {
                (
                    RatePlan::Line { num, icon, width },
                    RatePlan::Line {
                        num: n2, icon: i2, ..
                    },
                ) => {
                    assert_eq!((num, icon), (n2, i2), "at {zone}");
                    assert!(rate_arrow_w(icon) + width <= w - 2.0 * PAD);
                    assert!(num.size >= TEXT_FLOOR);
                }
                (RatePlan::Stacked { .. }, RatePlan::Stacked { .. }) => assert!(zone < 124.0),
                _ => panic!("the plan changed with the rate at {zone}"),
            }
        }
    }

    #[test]
    fn numbers_and_times_are_never_cut() {
        let mut r = Renderer::new(Theme::default(), crate::fonts::FontChoice::default().load());
        let mut c = Canvas::new(WIDTH, HEIGHT, CARD);
        let mut p = r.pen(&mut c);
        // The fit rule never stops inside a word with a figure in it.
        for room in 20..160 {
            let t = p.fit(Face::Num, &BODY, "Over its 194° limit", room as f32);
            assert!(!t.contains("19…") && !t.contains("1…"), "{t}");
        }
        // A time drops its total, then a step, then goes: never "43…".
        for room in 0..200 {
            if let Some((v, _, w)) = p.slider_words("9:26 / 43:12", &[TITLE, BODY], room as f32) {
                assert!(v == "9:26 / 43:12" || v == "9:26", "{v}");
                assert!(w <= room as f32);
            }
            let hours = p.slider_words("1:02:33 / 2:10:00", &[BODY], room as f32);
            if let Some((v, _, _)) = hours {
                assert!(v == "1:02:33 / 2:10:00" || v == "1:02:33", "{v}");
            }
        }
        // Chosen for the widest elapsed time, so it does not flip as the
        // seconds tick past ten minutes.
        let room = p.measure(Face::Num, &BODY, "9:26 / 43:12") + 2.0;
        let early = p.slider_words("9:26 / 43:12", &[BODY], room);
        assert_eq!(early.map(|e| e.0).as_deref(), Some("9:26"));
    }

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
