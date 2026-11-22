//! The better-cards widget set: tokens, the data each card shows, and every
//! widget's compact, medium and hero composition.

use nexus_core::theme::Rgb;
use tiny_skia::PathBuilder;

use crate::gfx::{Gfx, St, mix, st};

// ---- Colour tokens -------------------------------------------------------
// Colour has three jobs and only three: neutral greys carry data, the
// accent marks what a finger can act on, amber and red carry severity.

pub const GROUND: Rgb = Rgb::hex(0x000000);
/// Card surface: just lifted off the glass so card edges read.
pub const CARD: Rgb = Rgb::hex(0x111418);
/// Raised keycap: a tappable control inside a card.
pub const KEY: Rgb = Rgb::hex(0x272c35);
/// Keycap top edge, lit from above like a real key.
pub const KEY_HI: Rgb = Rgb::hex(0x434a56);
/// Tracks, rules, empty bars. Visible on CARD (about 1.6:1), not a ghost.
pub const LINE: Rgb = Rgb::hex(0x2f353e);
/// Ticks and dashed outlines.
pub const TICK: Rgb = Rgb::hex(0x56606d);
pub const TEXT: Rgb = Rgb::hex(0xf3f5f8);
pub const TEXT2: Rgb = Rgb::hex(0xa9b1bc);
pub const TEXT3: Rgb = Rgb::hex(0x6c7480);
/// Neutral data fill: bars and graphs that are not in any alarm.
pub const DATA: Rgb = Rgb::hex(0xc3cad3);
pub const ACCENT: Rgb = Rgb::hex(0x3ee0c3);
pub const WARN: Rgb = Rgb::hex(0xffb21e);
pub const WARN_BG: Rgb = Rgb::hex(0x4a3100);
pub const CRIT: Rgb = Rgb::hex(0xff4040);
pub const CRIT_BG: Rgb = Rgb::hex(0xe0242c);
pub const WHITE: Rgb = Rgb::hex(0xffffff);

// ---- Type scale ------------------------------------------------------------
// IBM Plex Sans, variable. Numerals are semi-condensed (wdth 82–85) so a
// 13:1 strip fits more digits at full height; text stays at normal width.

pub const NUM_C: St = st(30.0, 600.0, 75.0);
pub const NUM_M: St = st(35.0, 600.0, 84.0);
pub const NUM_H: St = st(41.0, 600.0, 84.0);
/// The full-width clock face only.
pub const NUM_FACE: St = st(46.0, 600.0, 82.0);
/// The secondary numeral in a medium card (load next to a temperature).
pub const NUM_S: St = st(26.0, 600.0, 84.0);
pub const UNIT: St = st(16.0, 600.0, 92.0);
pub const TITLE: St = st(18.0, 600.0, 88.0);
pub const TITLE_H: St = st(21.0, 600.0, 92.0);
pub const BODY: St = st(16.0, 500.0, 96.0);
/// The floor: nothing on the strip is smaller.
pub const MIN: St = st(14.0, 600.0, 96.0);

// ---- Grid --------------------------------------------------------------------

/// Cards run from here to the bottom edge; rows 0–1 hold the page rail.
pub const CARD_Y: f32 = 4.0;
pub const CARD_H: f32 = 44.0;
/// Space between cards.
pub const GAP: u32 = 3;
pub const PAD: f32 = 8.0;
pub const RADIUS: f32 = 6.0;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Size {
    Compact,
    Medium,
    Hero,
}

pub fn size_of(w: f32) -> Size {
    if w < 140.0 {
        Size::Compact
    } else if w < 300.0 {
        Size::Medium
    } else {
        Size::Hero
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Sev {
    Ok,
    Warn,
    Crit,
}

/// Whether a widget has something to show.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Avail {
    Live,
    /// Set up, but the source has gone quiet or never answered.
    NoData,
    /// Needs a setting before it can show anything.
    NotSetUp,
}

#[derive(Clone, Copy, Debug)]
pub enum Ico {
    Fa(char),
    Gpu,
}

pub const I_CPU: Ico = Ico::Fa('\u{f2db}');
pub const I_MEM: Ico = Ico::Fa('\u{f538}');
pub const I_CLOUD: char = '\u{f0c2}';
pub const I_PIN: char = '\u{f3c5}';
pub const I_UNLINK: char = '\u{f127}';
pub const I_DOWN: char = '\u{f063}';
pub const I_UP: char = '\u{f062}';
pub const I_PLAY: char = '\u{f04b}';
pub const I_PAUSE: char = '\u{f04c}';
pub const I_PREV: char = '\u{f048}';
pub const I_NEXT: char = '\u{f051}';
pub const I_NOTE: char = '\u{f001}';
pub const I_VOL_HI: char = '\u{f028}';
pub const I_VOL_LO: char = '\u{f027}';
pub const I_VOL_X: char = '\u{f6a9}';

#[derive(Clone, Debug)]
pub struct Reading {
    pub icon: Ico,
    pub name: &'static str,
    pub value: f32,
    /// "°F" or "%".
    pub unit: &'static str,
    /// The fixed scale every bar and graph of this reading uses.
    pub lo: f32,
    pub hi: f32,
    pub warn: Option<f32>,
    pub crit: Option<f32>,
    /// Last ten minutes, oldest first, same units as `value`.
    pub hist: Vec<f32>,
    pub avail: Avail,
    /// A second reading shown beside the first from medium up, such as
    /// load beside a temperature: (value in %, history).
    pub load: Option<f32>,
}

impl Reading {
    pub fn sev(&self) -> Sev {
        if self.avail != Avail::Live {
            return Sev::Ok;
        }
        match (self.warn, self.crit) {
            (_, Some(c)) if self.value >= c => Sev::Crit,
            (Some(w), _) if self.value >= w => Sev::Warn,
            _ => Sev::Ok,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Track {
    pub title: &'static str,
    pub artist: &'static str,
    pub pos: u32,
    pub len: u32,
    pub playing: bool,
}

#[derive(Clone, Debug)]
pub enum W {
    Clock {
        hh_mm: &'static str,
        weekday: &'static str,
        day: u32,
        month: &'static str,
        /// Minutes since midnight, for the full face's day ruler.
        minutes: u32,
    },
    Weather {
        temp: f32,
        cond: &'static str,
        avail: Avail,
    },
    Value(Reading),
    Rate {
        down: f32,
        up: f32,
        hist_down: Vec<f32>,
        hist_up: Vec<f32>,
        avail: Avail,
    },
    Media(Option<Track>),
    Slider {
        level: f32,
        /// The level before the change that is happening now.
        was: Option<f32>,
        active: bool,
        muted: bool,
    },
}

#[derive(Clone, Debug)]
pub struct Card {
    pub w: u32,
    pub widget: W,
}

pub fn card(w: u32, widget: W) -> Card {
    Card { w, widget }
}

/// Card colours for a severity: (surface, numerals, icon, data fill).
fn sev_colors(sev: Sev) -> (Rgb, Rgb, Rgb, Rgb) {
    match sev {
        Sev::Ok => (CARD, TEXT, TEXT2, DATA),
        Sev::Warn => (WARN_BG, WARN, WARN, WARN),
        Sev::Crit => (CRIT_BG, WHITE, WHITE, WHITE),
    }
}

/// Draws one card into the column `x`..`x + w`.
pub fn draw(g: &mut Gfx, x: f32, w: f32, widget: &W) {
    match widget {
        W::Clock {
            hh_mm,
            weekday,
            day,
            month,
            minutes,
        } => clock(g, x, w, hh_mm, weekday, *day, month, *minutes),
        W::Weather { temp, cond, avail } => weather(g, x, w, *temp, cond, *avail),
        W::Value(r) => value(g, x, w, r),
        W::Rate {
            down,
            up,
            hist_down,
            hist_up,
            avail,
        } => rate(g, x, w, *down, *up, hist_down, hist_up, *avail),
        W::Media(t) => media(g, x, w, t.as_ref()),
        W::Slider {
            level,
            was,
            active,
            muted,
        } => slider(g, x, w, *level, *was, *active, *muted, CARD),
    }
}

pub fn surface(g: &mut Gfx, x: f32, w: f32, c: Rgb) {
    g.rrect(x, CARD_Y, w, CARD_H + RADIUS, RADIUS, c);
}

/// A "not set up" card: no surface, a dashed outline in the accent, so it
/// reads as a place waiting for you rather than a broken reading.
fn unset_surface(g: &mut Gfx, x: f32, w: f32) {
    g.rrect_line(
        x,
        CARD_Y,
        w,
        CARD_H,
        RADIUS,
        mix(ACCENT, GROUND, 0.35),
        true,
    );
}

/// Baseline for a numeral style so its capitals sit centred on `cy`.
fn base_for(g: &mut Gfx, s: &St, cy: f32) -> f32 {
    (cy + g.cap(s) / 2.0).round()
}

/// Draws a numeral with its unit set small at cap height. Returns width.
#[allow(clippy::too_many_arguments)]
fn numeral(
    g: &mut Gfx,
    s: &St,
    value: &str,
    unit: &str,
    x: f32,
    base: f32,
    c: Rgb,
    uc: Rgb,
) -> f32 {
    let w = g.text(s, value, x, base, c);
    if unit.is_empty() {
        return w;
    }
    let cap = g.cap(s);
    let ucap = g.cap(&UNIT);
    let w2 = g.text(&UNIT, unit, x + w + 2.0, base - cap + ucap, uc);
    w + 2.0 + w2
}

fn numeral_w(g: &mut Gfx, s: &St, value: &str, unit: &str) -> f32 {
    let w = g.measure(s, value);
    if unit.is_empty() {
        w
    } else {
        w + 2.0 + g.measure(&UNIT, unit)
    }
}

fn draw_ico(g: &mut Gfx, ico: Ico, size: f32, cx: f32, cy: f32, c: Rgb) -> f32 {
    match ico {
        Ico::Fa(ch) => g.icon(ch, size, cx, cy, c),
        Ico::Gpu => g.gpu_icon(cx, cy, size * 0.95, c),
    }
}

fn frac(v: f32, lo: f32, hi: f32) -> f32 {
    ((v - lo) / (hi - lo)).clamp(0.0, 1.0)
}

/// A fixed-scale bar in a band of its own. The track itself is zoned like
/// a tachometer's redline: plain up to the warning threshold, amber to the
/// critical one, red beyond. The fill covers the zones it has reached.
#[allow(clippy::too_many_arguments)]
fn range_bar(g: &mut Gfx, x0: f32, x1: f32, y: f32, r: &Reading, value: f32, fill: Rgb, sev: Sev) {
    let (track, zw, zc) = zone_colors(sev);
    let len = x1 - x0;
    g.rrect(x0, y, len, 4.0, 2.0, track);
    let at = |t: f32| (x0 + len * frac(t, r.lo, r.hi)).round();
    if let Some(w) = r.warn {
        let end = r.crit.map_or(x1, at);
        g.rect(at(w), y, end - at(w) - 2.0, 4.0, zw);
    }
    if let Some(c) = r.crit {
        g.rrect(at(c), y, x1 - at(c), 4.0, 2.0, zc);
        g.rect(at(c), y, 2.0, 4.0, zc);
    }
    let f = frac(value, r.lo, r.hi);
    if r.avail == Avail::Live && f > 0.0 {
        g.rrect(x0, y, (len * f).max(4.0), 4.0, 2.0, fill);
    }
}

/// Track, warning-zone and critical-zone colours on each card surface.
fn zone_colors(sev: Sev) -> (Rgb, Rgb, Rgb) {
    match sev {
        Sev::Ok => (LINE, mix(WARN, CARD, 0.62), mix(CRIT, CARD, 0.5)),
        Sev::Warn => (
            mix(WARN_BG, WARN, 0.18),
            mix(WARN_BG, WARN, 0.4),
            mix(CRIT, WARN_BG, 0.25),
        ),
        Sev::Crit => (
            mix(CRIT_BG, WHITE, 0.25),
            mix(CRIT_BG, WHITE, 0.4),
            mix(CRIT_BG, WHITE, 0.55),
        ),
    }
}

/// A history graph with a fixed scale in its own band. Thresholds are
/// dashed rules; the area is neutral unless the card is in alarm.
#[allow(clippy::too_many_arguments)]
fn history(
    g: &mut Gfx,
    x0: f32,
    x1: f32,
    y0: f32,
    y1: f32,
    hist: &[f32],
    lo: f32,
    hi: f32,
    zones: &[(f32, f32, Rgb)],
    line: Rgb,
    area: Rgb,
    area_a: u8,
) {
    if hist.len() < 2 {
        return;
    }
    let h = y1 - y0;
    let step = (x1 - x0) / (hist.len() - 1) as f32;
    let pt = |i: usize, v: f32| (x0 + i as f32 * step, y1 - frac(v, lo, hi) * h);
    // Threshold zones behind the data: the band above warning, the band
    // above critical. Fixed, so a climb toward them is visible.
    for (from, to, c) in zones {
        let ya = (y1 - frac(*to, lo, hi) * h).round();
        let yb = (y1 - frac(*from, lo, hi) * h).round();
        g.rect(x0, ya, x1 - x0, yb - ya, *c);
    }
    // Floor rule: where the scale starts.
    g.rect(x0, y1, x1 - x0, 1.0, line_floor(area));
    let mut a = PathBuilder::new();
    a.move_to(x0, y1);
    for (i, v) in hist.iter().enumerate() {
        let (px, py) = pt(i, *v);
        a.line_to(px, py);
    }
    a.line_to(x1, y1);
    a.close();
    if let Some(p) = a.finish() {
        g.fill(&p, area, area_a);
    }
    let mut l = PathBuilder::new();
    for (i, v) in hist.iter().enumerate() {
        let (px, py) = pt(i, *v);
        if i == 0 {
            l.move_to(px, py);
        } else {
            l.line_to(px, py);
        }
    }
    if let Some(p) = l.finish() {
        g.stroke(&p, line, 255, 1.6, None);
    }
}

fn line_floor(area: Rgb) -> Rgb {
    if area == WHITE {
        mix(CRIT_BG, WHITE, 0.4)
    } else {
        LINE
    }
}

fn fmt_val(v: f32) -> String {
    format!("{}", v.round() as i64)
}

// ---- Value -----------------------------------------------------------------

fn value(g: &mut Gfx, x: f32, w: f32, r: &Reading) {
    let sev = r.sev();
    let (bg, num_c, ico_c, fill) = sev_colors(sev);
    surface(g, x, w, bg);
    let (x0, x1) = (x + PAD, x + w - PAD);
    let live = r.avail == Avail::Live;
    let num_c = if live { num_c } else { TEXT3 };
    let ico_c = if live { ico_c } else { TEXT3 };
    let unit_c = match sev {
        Sev::Ok => TEXT2,
        _ => num_c,
    };
    let v = if live {
        fmt_val(r.value)
    } else {
        "––".into()
    };
    let deg = r.unit == "°F" && live;
    let unit_s = if live { r.unit } else { "" };

    match size_of(w) {
        Size::Compact => {
            // Icon and numeral on one line, the fixed-scale bar below.
            let base = base_for(g, &NUM_C, 21.0);
            let iw = if live {
                draw_ico(g, r.icon, 16.0, x0 + 9.0, 21.0, ico_c)
            } else {
                g.icon(I_UNLINK, 15.0, x0 + 8.0, 21.0, TEXT3)
            };
            let (txt, unit) = if deg {
                (format!("{v}°"), "")
            } else {
                (v.clone(), unit_s)
            };
            let nw = numeral_w(g, &NUM_C, &txt, unit);
            // Right-aligned so digits do not wander; nudged right of the
            // icon if a wide reading would touch it.
            let nx = (x1 - nw).max(x0 + iw + 5.0);
            numeral(g, &NUM_C, &txt, unit, nx, base, num_c, unit_c);
            range_bar(g, x0, x1, 39.0, r, r.value, fill, sev);
        }
        Size::Medium => {
            let cy = if r.load.is_some() { 21.0 } else { 25.0 };
            let base = base_for(g, &NUM_M, cy);
            let iw = draw_ico(g, r.icon, 22.0, x0 + 12.0, cy, ico_c);
            let nx = x0 + iw.max(22.0) + 8.0;
            let used = numeral(g, &NUM_M, &v, unit_s, nx, base, num_c, unit_c);
            if let Some(load) = r.load {
                // Temperature with its bar on the left, load with its own
                // 0–100 bar on the right: two readings, two bands.
                let lw = 58.0;
                let lx = x1 - lw;
                let lv = if live { fmt_val(load) } else { "––".into() };
                let sb = base_for(g, &NUM_S, cy);
                let lnw = numeral_w(g, &NUM_S, &lv, "%");
                numeral(g, &NUM_S, &lv, "%", x1 - lnw, sb, num_c, unit_c);
                range_bar(g, x0, lx - 12.0, 39.0, r, r.value, fill, sev);
                let lr = Reading {
                    lo: 0.0,
                    hi: 100.0,
                    warn: None,
                    crit: None,
                    ..r.clone()
                };
                range_bar(g, lx, x1, 39.0, &lr, load, fill, sev);
            } else if live {
                let gx = nx + used + 12.0;
                if x1 - gx >= 40.0 {
                    graph_for(g, r, gx, x1, 10.0, 40.0, sev);
                }
            } else {
                g.icon(I_UNLINK, 14.0, x1 - 7.0, 16.0, TEXT3);
                g.text_r(&MIN, "No data", x1, 39.0, TEXT3);
            }
        }
        Size::Hero => {
            let base = base_for(g, &NUM_H, 25.0);
            let iw = draw_ico(g, r.icon, 26.0, x0 + 14.0, 25.0, ico_c);
            let nx = x0 + iw.max(26.0) + 10.0;
            let used = numeral(g, &NUM_H, &v, unit_s, nx, base, num_c, unit_c);
            // Name and the ten-minute peak, stacked.
            let tx = nx + used + 12.0;
            let name_c = if sev == Sev::Ok { TEXT } else { num_c };
            let tw = g.text(&TITLE, r.name, tx, 22.0, name_c);
            let sub = if live {
                let peak = r.hist.iter().copied().fold(r.value, f32::max);
                if let Some(load) = r.load {
                    format!("{}% load", fmt_val(load))
                } else {
                    format!("peak {}{}", fmt_val(peak), if deg { "°" } else { r.unit })
                }
            } else {
                "No data".into()
            };
            let sub_c = match sev {
                Sev::Ok => TEXT2,
                _ => num_c,
            };
            let sw = g.text(&MIN, &sub, tx, 39.0, if live { sub_c } else { TEXT3 });
            let gx = tx + tw.max(sw) + 14.0;
            if live && x1 - gx >= 40.0 {
                graph_for(g, r, gx, x1, 10.0, 40.0, sev);
            }
        }
    }
}

fn graph_for(g: &mut Gfx, r: &Reading, x0: f32, x1: f32, y0: f32, y1: f32, sev: Sev) {
    let (_, zw, zc) = zone_colors(sev);
    let mut zones = Vec::new();
    if let Some(w) = r.warn {
        zones.push((w, r.crit.unwrap_or(r.hi), zw));
    }
    if let Some(c) = r.crit {
        zones.push((c, r.hi, zc));
    }
    let (line, area, a) = match sev {
        Sev::Ok => (DATA, DATA, 50),
        Sev::Warn => (WARN, WARN, 70),
        Sev::Crit => (WHITE, WHITE, 80),
    };
    history(
        g, x0, x1, y0, y1, &r.hist, r.lo, r.hi, &zones, line, area, a,
    );
}

// ---- Weather -------------------------------------------------------------

fn weather(g: &mut Gfx, x: f32, w: f32, temp: f32, cond: &str, avail: Avail) {
    let (x0, x1) = (x + PAD, x + w - PAD);
    let size = size_of(w);
    if avail == Avail::NotSetUp {
        unset_surface(g, x, w);
        let (title, sub) = match size {
            Size::Compact => ("Set up", None),
            Size::Medium => ("Set location", Some("Tap for Settings")),
            Size::Hero => ("Weather needs a place", Some("Tap to open Settings")),
        };
        let pin_c = ACCENT;
        g.icon(I_PIN, 20.0, x0 + 9.0, 25.0, pin_c);
        let tx = x0 + 26.0;
        if let Some(sub) = sub {
            g.text(&TITLE, title, tx, 22.0, TEXT);
            g.text(&MIN, sub, tx, 39.0, TEXT2);
        } else {
            g.text(&TITLE, title, tx, 31.0, TEXT);
        }
        return;
    }
    surface(g, x, w, CARD);
    let live = avail == Avail::Live;
    let (num_c, ico_c) = if live { (TEXT, TEXT) } else { (TEXT3, TEXT3) };
    let v = if live {
        format!("{}°", fmt_val(temp))
    } else {
        "––".into()
    };
    match size {
        Size::Compact => {
            let base = base_for(g, &NUM_C, 25.0);
            if live {
                g.icon(I_CLOUD, 22.0, x0 + 12.0, 25.0, ico_c);
            } else {
                g.icon(I_UNLINK, 17.0, x0 + 9.0, 25.0, TEXT3);
            }
            g.text_r(&NUM_C, &v, x1, base, num_c);
        }
        Size::Medium | Size::Hero => {
            let (ns, isz) = if size == Size::Hero {
                (NUM_H, 30.0)
            } else {
                (NUM_M, 26.0)
            };
            let base = base_for(g, &ns, 25.0);
            let iw = g.icon(I_CLOUD, isz, x0 + isz / 2.0 + 2.0, 25.0, ico_c);
            let nx = x0 + iw + 12.0;
            let nw = g.text(&ns, &v, nx, base, num_c);
            let tx = nx + nw + 12.0;
            if live {
                let ts = if size == Size::Hero { TITLE_H } else { TITLE };
                let c = g.fit(&ts, cond, x1 - tx);
                g.text(&ts, &c, tx, base - 1.0, TEXT2);
            } else {
                g.icon(I_UNLINK, 14.0, tx + 7.0, 16.0, TEXT3);
                g.text(&MIN, "No data", tx, 39.0, TEXT3);
                if size == Size::Hero {
                    g.text(&MIN, "since 10:21, retrying", tx + 64.0, 39.0, TEXT3);
                }
            }
        }
    }
}

// ---- Clock -----------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
fn clock(
    g: &mut Gfx,
    x: f32,
    w: f32,
    hh_mm: &str,
    weekday: &str,
    day: u32,
    month: &str,
    minutes: u32,
) {
    surface(g, x, w, CARD);
    let (x0, x1) = (x + PAD, x + w - PAD);
    match size_of(w) {
        Size::Compact => {
            let base = base_for(g, &NUM_C, 25.0);
            let tw = g.measure(&NUM_C, hh_mm);
            g.text(&NUM_C, hh_mm, x + (w - tw) / 2.0, base, TEXT);
        }
        Size::Medium => {
            let base = base_for(g, &NUM_M, 25.0);
            let tw = g.text(&NUM_M, hh_mm, x0, base, TEXT);
            // The date stack: the longest form that fits.
            let room = x1 - (x0 + tw + 10.0);
            let short = &weekday[..3];
            let forms = [
                (weekday.to_string(), format!("{month} {day}")),
                (short.to_string(), format!("{} {day}", &month[..3])),
                (short.to_string(), format!("{day}")),
            ];
            for (a, b) in forms {
                let wa = g.measure(&TITLE, &b).max(g.measure(&MIN, &a));
                if wa <= room {
                    g.text_r(&MIN, &a.to_uppercase(), x1, 20.0, TEXT2);
                    g.text_r(&TITLE, &b, x1, base, TEXT);
                    break;
                }
            }
        }
        Size::Hero => {
            // The full face sets the time a size up from every other hero.
            let ns = if w >= 500.0 { NUM_FACE } else { NUM_H };
            let base = base_for(g, &ns, 25.0);
            let tw = g.text(&ns, hh_mm, x0 + 2.0, base, TEXT);
            let dx = x0 + tw + 16.0;
            g.text(&MIN, &weekday.to_uppercase(), dx, 20.0, TEXT2);
            let dw = g.text(&TITLE, &format!("{month} {day}"), dx, base, TEXT);
            if w >= 500.0 {
                day_ruler(g, dx + dw.max(70.0) + 24.0, x1 - 6.0, minutes);
            }
        }
    }
}

/// The full-width clock face: the day unrolled into a ruler, hours as
/// ticks, the part already lived tinted, and the now-needle in the accent.
fn day_ruler(g: &mut Gfx, x0: f32, x1: f32, minutes: u32) {
    let len = x1 - x0;
    let ty = 22.0; // track top
    let at = |m: f32| (x0 + len * m / 1440.0).round();
    let now = at(minutes as f32);
    // Past and future.
    g.rrect(x0, ty, len, 6.0, 3.0, LINE);
    g.rrect(x0, ty, now - x0, 6.0, 3.0, mix(ACCENT, CARD, 0.72));
    for h in 0..=24 {
        let hx = at(h as f32 * 60.0);
        let major = h % 6 == 0;
        let (y, tall) = if major {
            (ty - 8.0, 6.0)
        } else {
            (ty - 4.0, 3.0)
        };
        g.rect(
            hx - if major { 1.0 } else { 0.0 },
            y,
            if major { 2.0 } else { 1.0 },
            tall,
            if major { TEXT2 } else { TICK },
        );
        if major && h % 24 != 0 {
            let label = format!("{h:02}");
            let lw = g.measure(&MIN, &label);
            g.text(&MIN, &label, hx - lw / 2.0, 44.0, TEXT2);
        }
    }
    // Needle.
    g.rrect(now - 1.5, ty - 12.0, 3.0, 26.0, 1.5, ACCENT);
}

// ---- Rate ------------------------------------------------------------------

fn fmt_rate(bps: f32) -> (String, &'static str) {
    let k = bps / 1000.0;
    if k < 1.0 {
        (format!("{}", bps.round() as i64), "B/s")
    } else if k < 1000.0 {
        (format!("{}", k.round() as i64), "kB/s")
    } else {
        (format!("{:.1}", k / 1000.0), "MB/s")
    }
}

/// Network rates sit on a fixed log scale: 1 kB/s to 1 GB/s.
fn log_frac(bps: f32) -> f32 {
    ((bps.max(1.0).log10() - 3.0) / 6.0).clamp(0.0, 1.0)
}

#[allow(clippy::too_many_arguments)]
fn rate(
    g: &mut Gfx,
    x: f32,
    w: f32,
    down: f32,
    up: f32,
    hist_down: &[f32],
    hist_up: &[f32],
    avail: Avail,
) {
    surface(g, x, w, CARD);
    let (x0, x1) = (x + PAD, x + w - PAD);
    let live = avail == Avail::Live;
    let (num_c, ico_c) = if live { (TEXT, TEXT2) } else { (TEXT3, TEXT3) };
    let (dv, du) = if live {
        fmt_rate(down)
    } else {
        ("––".into(), "kB/s")
    };
    let (uv, uu) = if live {
        fmt_rate(up)
    } else {
        ("––".into(), "kB/s")
    };
    let bar = |g: &mut Gfx, x0: f32, x1: f32, y: f32, v: f32| {
        let len = x1 - x0;
        g.rrect(x0, y, len, 4.0, 2.0, LINE);
        // Decade ticks: 1 k, 1 M, 1 G… the scale's landmarks.
        for d in [0.5f32, 1.0] {
            let tx = (x0 + len * d).round();
            g.rect(tx - 1.0, y - 3.0, 2.0, 10.0, TICK);
        }
        if live {
            g.rrect(x0, y, (len * log_frac(v)).max(4.0), 4.0, 2.0, DATA);
        }
    };
    match size_of(w) {
        Size::Compact => {
            let base = base_for(g, &NUM_C, 21.0);
            g.icon(I_DOWN, 17.0, x0 + 7.0, 21.0, ico_c);
            let nw = g.measure(&NUM_C, &dv);
            let uw = g.measure(&MIN, du);
            g.text(&NUM_C, &dv, x1 - uw - 3.0 - nw, base, num_c);
            g.text(&MIN, du, x1 - uw, base - 12.0, TEXT2);
            bar(g, x0, x1, 39.0, down);
        }
        Size::Medium => {
            let base = base_for(g, &NUM_M, 21.0);
            g.icon(I_DOWN, 20.0, x0 + 8.0, 21.0, ico_c);
            numeral(g, &NUM_M, &dv, du, x0 + 22.0, base, num_c, TEXT2);
            let uw = numeral_w(g, &NUM_S, &uv, uu);
            let ub = base_for(g, &NUM_S, 21.0);
            numeral(g, &NUM_S, &uv, uu, x1 - uw, ub, TEXT2, TEXT2);
            g.icon(I_UP, 14.0, x1 - uw - 10.0, 21.0, ico_c);
            bar(g, x0, x1, 39.0, down);
        }
        Size::Hero => {
            let base = base_for(g, &NUM_M, 21.0);
            g.icon(I_DOWN, 20.0, x0 + 8.0, 21.0, ico_c);
            let dw = numeral(g, &NUM_M, &dv, du, x0 + 22.0, base, num_c, TEXT2);
            let ux = x0 + 22.0 + dw + 16.0;
            g.icon(I_UP, 14.0, ux + 5.0, 21.0, ico_c);
            let ub = base_for(g, &NUM_S, 21.0);
            let uw = numeral(g, &NUM_S, &uv, uu, ux + 14.0, ub, TEXT2, TEXT2);
            let lx1 = ux + 14.0 + uw;
            bar(g, x0, lx1, 39.0, down);
            let gx = lx1 + 16.0;
            if live && x1 - gx > 60.0 {
                // Log scale, fixed: the same height means the same rate.
                let to_log = |v: &f32| log_frac(*v) * 100.0;
                let hd: Vec<f32> = hist_down.iter().map(to_log).collect();
                let hu: Vec<f32> = hist_up.iter().map(to_log).collect();
                let lw = g.measure(&MIN, "1M");
                let gx1 = x1 - lw - 6.0;
                history(g, gx, gx1, 10.0, 40.0, &hd, 0.0, 100.0, &[], DATA, DATA, 50);
                g.hline_dash(gx, gx1, 25.0, TICK, 255);
                history(
                    g,
                    gx,
                    gx1,
                    10.0,
                    40.0,
                    &hu,
                    0.0,
                    100.0,
                    &[],
                    ACCENT_LINE,
                    CARD,
                    0,
                );
                g.text_r(&MIN, "1M", x1, 30.0, TEXT3);
                g.text_r(&MIN, "1k", x1, 44.0, TEXT3);
            }
        }
    }
}

/// Upload line in the rate graph: the second series, a muted tone.
const ACCENT_LINE: Rgb = Rgb::hex(0x7d8794);

// ---- Media -----------------------------------------------------------------

fn mmss(s: u32) -> String {
    format!("{}:{:02}", s / 60, s % 60)
}

/// A raised key: what a finger can press. `primary` keys are filled with
/// the accent.
#[allow(clippy::too_many_arguments)]
pub fn key(
    g: &mut Gfx,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    glyph: char,
    gs: f32,
    primary: bool,
    enabled: bool,
) {
    if !enabled {
        g.rrect_line(x, y, w, h, 5.0, LINE, false);
        g.icon(glyph, gs, x + w / 2.0, y + h / 2.0, TEXT3);
        return;
    }
    let (face, edge, ink) = if primary {
        (ACCENT, Rgb::hex(0x9af2e2), Rgb::hex(0x04201b))
    } else {
        (KEY, KEY_HI, TEXT)
    };
    g.rrect(x, y + 1.0, w, h, 5.0, mix(face, GROUND, 0.55));
    g.rrect(x, y, w, h - 1.0, 5.0, face);
    g.rrect(x + 2.0, y, w - 4.0, 1.0, 0.5, edge);
    g.icon(glyph, gs, x + w / 2.0, y + (h - 1.0) / 2.0, ink);
}

fn progress(g: &mut Gfx, x0: f32, x1: f32, y: f32, f: f32, playing: bool) {
    g.rect(x0, y, x1 - x0, 2.0, LINE);
    let c = if playing { ACCENT } else { TEXT3 };
    g.rect(x0, y, ((x1 - x0) * f).round(), 2.0, c);
}

fn media(g: &mut Gfx, x: f32, w: f32, t: Option<&Track>) {
    surface(g, x, w, CARD);
    let (x0, x1) = (x + PAD, x + w - PAD);
    let size = size_of(w);
    let Some(t) = t else {
        // Nothing playing: an empty sleeve, a plain sentence, a play key
        // that is drawn but not raised, since there is nothing to resume.
        match size {
            Size::Compact => {
                g.icon(I_NOTE, 20.0, x + w / 2.0, 25.0, TEXT3);
            }
            _ => {
                let s = if size == Size::Hero { 36.0 } else { 32.0 };
                let ay = CARD_Y + (CARD_H - s) / 2.0;
                g.rrect_line(x0 - 2.0, ay, s, s, 4.0, TICK, true);
                g.icon(I_NOTE, 15.0, x0 - 2.0 + s / 2.0, ay + s / 2.0, TEXT3);
                g.text(&TITLE, "Nothing playing", x0 + s + 8.0, 31.0, TEXT2);
                if size == Size::Hero {
                    let kx = x1 - 3.0 * 32.0 - 8.0;
                    for (i, k) in [I_PREV, I_PLAY, I_NEXT].into_iter().enumerate() {
                        key(
                            g,
                            kx + i as f32 * 36.0,
                            10.0,
                            32.0,
                            32.0,
                            k,
                            13.0,
                            false,
                            false,
                        );
                    }
                }
            }
        }
        return;
    };
    let f = t.pos as f32 / t.len as f32;
    let glyph = if t.playing { I_PAUSE } else { I_PLAY };
    let title_c = if t.playing { TEXT } else { TEXT2 };
    match size {
        Size::Compact => {
            key(g, x0 - 2.0, 9.0, 36.0, 30.0, glyph, 15.0, t.playing, true);
            let tw = g.measure(&TITLE, &mmss(t.pos));
            g.text(&TITLE, &mmss(t.pos), x1 - tw, 30.0, title_c);
            progress(g, x0 - 2.0, x1 + 2.0, 43.0, f, t.playing);
        }
        Size::Medium => {
            // The sleeve is the key: tapping the card plays or pauses, and
            // the sleeve carries the glyph for what a tap will do.
            let s = 34.0;
            g.art(x0 - 2.0, 8.0, s, t.title, true);
            g.circle(x0 - 2.0 + s / 2.0, 8.0 + s / 2.0, 11.0, GROUND, 150);
            g.icon(glyph, 12.0, x0 - 2.0 + s / 2.0, 8.0 + s / 2.0, WHITE);
            let tx = x0 + s + 7.0;
            let room = x1 - tx;
            let title = g.fit(&TITLE, t.title, room);
            g.text(&TITLE, &title, tx, 22.0, title_c);
            let artist = g.fit(&BODY, t.artist, room);
            g.text(&BODY, &artist, tx, 39.0, TEXT2);
            progress(g, x + 2.0, x + w - 2.0, 45.0, f, t.playing);
        }
        Size::Hero => {
            let s = 38.0;
            g.art(x0 - 2.0, 7.0, s, t.title, !t.playing);
            let kw = if w < 400.0 { 28.0 } else { 34.0 };
            let keys_w = kw * 3.0 + 8.0 + 6.0;
            let kx = x1 - keys_w;
            key(g, kx, 9.0, kw, 32.0, I_PREV, 13.0, false, true);
            key(
                g,
                kx + kw + 4.0,
                9.0,
                kw + 6.0,
                32.0,
                glyph,
                15.0,
                true,
                true,
            );
            key(
                g,
                kx + 2.0 * kw + 14.0,
                9.0,
                kw,
                32.0,
                I_NEXT,
                13.0,
                false,
                true,
            );
            let tx = x0 + s + 8.0;
            let room = kx - 12.0 - tx;
            // The bigger title if the whole name fits, else the medium one.
            let ts = if g.measure(&TITLE_H, t.title) <= room {
                TITLE_H
            } else {
                TITLE
            };
            let title = g.fit(&ts, t.title, room);
            g.text(&ts, &title, tx, 22.0, title_c);
            let times = format!("{} / {}", mmss(t.pos), mmss(t.len));
            let tw = g.measure(&MIN, &times);
            let aw = g.measure(&BODY, t.artist);
            if aw + tw + 12.0 <= room {
                g.text(&MIN, &times, kx - 12.0 - tw, 39.0, TEXT2);
            }
            let artist = g.fit(&BODY, t.artist, room);
            g.text(&BODY, &artist, tx, 39.0, TEXT2);
            progress(g, x + 2.0, x + w - 2.0, 45.0, f, t.playing);
        }
    }
}

// ---- Slider ----------------------------------------------------------------

/// Volume as a real slider: a thick track at mid-height, a round thumb you
/// could put a finger on, and (while it changes) a ghost of where it was.
#[allow(clippy::too_many_arguments)]
pub fn slider(
    g: &mut Gfx,
    x: f32,
    w: f32,
    level: f32,
    was: Option<f32>,
    active: bool,
    muted: bool,
    bg: Rgb,
) {
    surface(g, x, w, bg);
    let (x0, x1) = (x + PAD, x + w - PAD);
    let size = size_of(w);
    let glyph = if muted {
        I_VOL_X
    } else if level < 34.0 {
        I_VOL_LO
    } else {
        I_VOL_HI
    };
    let (num_c, fill) = if muted {
        (TEXT3, TEXT3)
    } else {
        (TEXT, ACCENT)
    };
    let v = fmt_val(level);
    if size == Size::Compact {
        let base = base_for(g, &NUM_C, 20.0);
        g.icon(
            glyph,
            17.0,
            x0 + 9.0,
            20.0,
            if muted { TEXT3 } else { TEXT2 },
        );
        g.text_r(&NUM_C, &v, x1, base, num_c);
        let (tx0, tx1) = (x0 + 5.0, x1 - 5.0);
        let ty = 38.0;
        g.rrect(tx0, ty, tx1 - tx0, 4.0, 2.0, LINE);
        let fx = tx0 + (tx1 - tx0) * level / 100.0;
        g.rrect(tx0, ty, fx - tx0, 4.0, 2.0, fill);
        thumb(g, fx, ty + 2.0, 6.0, active, muted);
        return;
    }
    let (ns, isz) = if size == Size::Hero {
        (NUM_H, 24.0)
    } else {
        (NUM_M, 22.0)
    };
    let base = base_for(g, &ns, 25.0);
    let iw = g.icon(
        glyph,
        isz,
        x0 + isz / 2.0,
        25.0,
        if muted { TEXT3 } else { TEXT2 },
    );
    let nx = x0 + iw.max(isz) + 8.0;
    // A fixed slot for "100" so the track does not move as digits change.
    let slot = g.measure(&ns, "100");
    let nw = g.text(&ns, &v, nx, base, num_c);
    if let Some(was) = was.filter(|_| size == Size::Hero) {
        // How far it moved, set like a unit beside the number.
        let delta = level - was;
        let label = format!(
            "{}{}",
            if delta >= 0.0 { "+" } else { "−" },
            fmt_val(delta.abs())
        );
        let cap = g.cap(&ns);
        let ucap = g.cap(&UNIT);
        g.text(&UNIT, &label, nx + nw + 3.0, base - cap + ucap, ACCENT);
    }
    let slot = slot
        + if was.is_some() && size == Size::Hero {
            26.0
        } else {
            0.0
        };
    let r = if active { 11.0 } else { 9.0 };
    let tx0 = nx + slot + 8.0 + r;
    let tx1 = x1 - r - 1.0;
    let ty = 25.0;
    let len = tx1 - tx0;
    g.rrect(tx0 - 3.0, ty - 3.0, len + 6.0, 6.0, 3.0, LINE);
    if size == Size::Hero {
        for t in [0.0f32, 0.25, 0.5, 0.75, 1.0] {
            let px = (tx0 + len * t).round();
            g.rect(px - 1.0, ty + 7.0, 2.0, 5.0, TICK);
        }
    }
    let fx = tx0 + len * level / 100.0;
    g.rrect(tx0 - 3.0, ty - 3.0, fx - tx0 + 3.0, 6.0, 3.0, fill);
    if let Some(was) = was {
        let wx = (tx0 + len * was / 100.0).round();
        g.rect(wx - 1.0, ty - 9.0, 2.0, 18.0, TEXT2);
    }
    thumb(g, fx, ty, r, active, muted);
}

fn thumb(g: &mut Gfx, cx: f32, cy: f32, r: f32, active: bool, muted: bool) {
    if active {
        g.circle(cx, cy, r + 5.0, ACCENT, 60);
    }
    g.circle(cx, cy + 1.0, r, GROUND, 200);
    g.circle(cx, cy, r, if muted { TEXT2 } else { WHITE }, 255);
    if r >= 8.0 {
        // A grip: a short groove that says "this moves".
        g.rect(cx - 0.5, cy - r * 0.45, 1.0, r * 0.9, Rgb::hex(0x9aa3ae));
    }
}
