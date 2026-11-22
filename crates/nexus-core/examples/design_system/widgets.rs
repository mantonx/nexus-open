//! Every widget of the design system at compact, medium and hero width, in
//! every state, plus the takeovers and the page markers.
//!
//! Each `draw_*` takes the card's left edge `x` and width `w`; the card's
//! top, height and padding come from `tokens`.

use nexus_core::theme::Rgb;

use crate::gfx::{Face, Gfx, mix, rrect_path, st};
use crate::tokens::*;

// ---------------------------------------------------------------- icons

/// Phosphor code points (Fill and Bold share them).
pub mod ico {
    pub const CPU: char = '\u{e610}';
    pub const GPU: char = '\u{e612}';
    pub const MEMORY: char = '\u{e9c4}';
    pub const GAUGE: char = '\u{e628}';
    pub const CLOUD: char = '\u{e1aa}';
    pub const CLOUD_SLASH: char = '\u{e1b6}';
    pub const SPEAKER_HIGH: char = '\u{e44a}';
    pub const SPEAKER_LOW: char = '\u{e44c}';
    pub const SPEAKER_X: char = '\u{e45c}';
    pub const PLAY: char = '\u{e3d0}';
    pub const PAUSE: char = '\u{e39e}';
    pub const SKIP_BACK: char = '\u{e5a4}';
    pub const SKIP_FWD: char = '\u{e5a6}';
    pub const MUSIC: char = '\u{e33c}';
    pub const MAP_PIN_PLUS: char = '\u{e314}';
    pub const PLUS: char = '\u{e3d4}';
    pub const LINK_BREAK: char = '\u{e2e4}';
    pub const ARROW_DOWN: char = '\u{e03e}';
    pub const ARROW_UP: char = '\u{e08e}';
    pub const WARNING: char = '\u{e4e0}';
    pub const GLOBE: char = '\u{e28e}';
    pub const FOLDER: char = '\u{e25a}';
    pub const GAME: char = '\u{e26e}';
    pub const GEAR: char = '\u{e270}';
}

// ---------------------------------------------------------------- data

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Sev {
    Normal,
    Warn,
    Crit,
}

/// Whether a reading has a value to show.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Avail {
    Live,
    /// The source works but has given no reading yet.
    NoData,
    /// The widget has no source chosen (or no weather location).
    NotSetUp,
    /// The source stopped answering; the last value is kept, greyed, with
    /// the time it was last heard from.
    Lost(&'static str),
}

/// A fixed scale. Bars never autoscale.
#[derive(Clone, Copy, Debug)]
pub struct Scale {
    pub lo: f32,
    pub hi: f32,
    pub warn: Option<f32>,
    pub crit: Option<f32>,
    /// Logarithmic (network rates).
    pub log: bool,
}

impl Scale {
    pub fn frac(&self, v: f32) -> f32 {
        let f = if self.log {
            (v.max(self.lo).ln() - self.lo.ln()) / (self.hi.ln() - self.lo.ln())
        } else {
            (v - self.lo) / (self.hi - self.lo)
        };
        f.clamp(0.0, 1.0)
    }
    pub fn sev(&self, v: f32) -> Sev {
        match (self.warn, self.crit) {
            (_, Some(c)) if v >= c => Sev::Crit,
            (Some(w), _) if v >= w => Sev::Warn,
            _ => Sev::Normal,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Reading {
    pub icon: char,
    pub name: &'static str,
    pub value: f32,
    /// The numeral as shown ("115°", "31").
    pub text: String,
    /// A unit drawn smaller after the numeral ("%", "kB/s"), or empty.
    pub unit: &'static str,
    pub scale: Scale,
    pub avail: Avail,
    /// The limit named in words on hero cards ("limit 167°").
    pub limit_words: Option<String>,
}

impl Reading {
    pub fn sev(&self) -> Sev {
        if self.avail == Avail::Live {
            self.scale.sev(self.value)
        } else {
            Sev::Normal
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum KeyState {
    Rest,
    Pressed,
    /// A long press on its way: 0–1 of the 0.5 s.
    Holding(f32),
    NotSetUp,
}

#[derive(Clone, Debug)]
pub enum W {
    Clock {
        hh_mm: &'static str,
        weekday: &'static str,
        wd_short: &'static str,
        date: &'static str,
        date_short: &'static str,
    },
    Value {
        r: Reading,
        second: Option<Reading>,
    },
    Network {
        down: Reading,
        up: Reading,
    },
    Weather {
        temp: &'static str,
        cond: &'static str,
        hi_lo: &'static str,
        icon: char,
        avail: Avail,
    },
    Media {
        track: Option<Track>,
        /// Which transport key is under a finger: 0 prev, 1 play/pause, 2 next.
        pressed: Option<usize>,
    },
    Slider {
        value: f32,
        muted: bool,
        held: bool,
    },
    Key {
        icon: char,
        label: &'static str,
        sub: &'static str,
        state: KeyState,
    },
}

// ---------------------------------------------------------------- classes

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Class {
    Compact,
    Medium,
    Hero,
}

pub fn class(w: f32) -> Class {
    if w <= COMPACT_MAX {
        Class::Compact
    } else if w <= MEDIUM_MAX {
        Class::Medium
    } else {
        Class::Hero
    }
}

// ---------------------------------------------------------------- inks

/// Colours for a card at one severity.
#[derive(Clone, Copy)]
pub struct Ink {
    pub surface: Rgb,
    pub num: Rgb,
    pub label: Rgb,
    pub quiet: Rgb,
    pub icon: Rgb,
    pub fill: Rgb,
    pub track: Rgb,
}

pub fn ink(sev: Sev) -> Ink {
    match sev {
        Sev::Normal => Ink {
            surface: CARD,
            num: TEXT,
            label: TEXT2,
            quiet: TEXT3,
            icon: TEXT2,
            fill: FILL,
            track: TRACK,
        },
        Sev::Warn => Ink {
            surface: WARN_SURFACE,
            num: WARN,
            label: mix(WARN, TEXT, 0.55),
            quiet: mix(WARN, WARN_SURFACE, 0.35),
            icon: WARN,
            fill: WARN,
            track: mix(WARN_SURFACE, GROUND, 0.55),
        },
        Sev::Crit => Ink {
            surface: CRIT,
            num: CRIT_INK,
            label: CRIT_INK,
            quiet: mix(CRIT_INK, CRIT, 0.25),
            icon: CRIT_INK,
            fill: CRIT_INK,
            track: CRIT_DEEP,
        },
    }
}

/// Greys a reading whose source is lost.
fn lost_ink() -> Ink {
    Ink {
        num: TEXT3,
        icon: TEXT3,
        fill: mix(TEXT3, CARD, 0.3),
        ..ink(Sev::Normal)
    }
}

// ---------------------------------------------------------------- pieces

pub fn card(g: &mut Gfx, x: f32, w: f32, surface: Rgb) {
    g.rrect(x, CARD_Y, w, CARD_H, CARD_R, surface);
}

/// The "not set up" outline: dashed, in the accent, no fill.
pub fn setup_card(g: &mut Gfx, a: &Accent, x: f32, w: f32) {
    g.rrect_line(x, CARD_Y, w, CARD_H, CARD_R, a.base, DASH_W, Some(DASH));
}

/// A raised key face. Returns nothing; `pressed` sinks it and lights it.
#[allow(clippy::too_many_arguments)]
pub fn key_face(
    g: &mut Gfx,
    a: &Accent,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    pressed: bool,
    primary: bool,
) {
    if pressed {
        let face = if primary {
            mix(a.base, a.ink, 0.25)
        } else {
            a.soft
        };
        g.rrect(x, y, w, h, r, face);
        g.rrect_line(x, y, w, h, r, a.base, 1.5, None);
        return;
    }
    let (face, top) = if primary {
        (a.base, mix(a.base, TEXT, 0.45))
    } else {
        (KEY, KEY_TOP)
    };
    // The lit top edge: the face drawn 1 px lower over a lighter copy.
    g.rrect(x, y, w, h, r, top);
    g.rrect(x, y + KEY_LIFT, w, h - KEY_LIFT, r, face);
}

/// The fixed-scale bar with its warning and critical zones and notches.
#[allow(clippy::too_many_arguments)]
pub fn bar(g: &mut Gfx, x: f32, y: f32, w: f32, h: f32, scale: &Scale, v: Option<f32>, k: &Ink) {
    let r = h / 2.0;
    let Some(track) = rrect_path(x, y, w, h, r) else {
        return;
    };
    g.fill(&track, k.track, 255);
    let crit_card = k.surface == CRIT;
    // Zones, tinted on the track (not on a critical card: it is all red).
    if !crit_card {
        if let Some(wv) = scale.warn {
            let x0 = x + w * scale.frac(wv);
            let x1 = scale.crit.map_or(x + w, |c| x + w * scale.frac(c));
            if let Some(p) = rrect_path(x0, y, x1 - x0, h, 0.0) {
                g.fill_clipped(&p, WARN_ZONE, 255, &track);
            }
        }
        if let Some(cv) = scale.crit {
            let x0 = x + w * scale.frac(cv);
            if let Some(p) = rrect_path(x0, y, x + w - x0, h, 0.0) {
                g.fill_clipped(&p, CRIT_ZONE, 255, &track);
            }
        }
    }
    if let Some(v) = v {
        let fw = (w * scale.frac(v)).max(h);
        if let Some(p) = rrect_path(x, y, fw, h, r) {
            g.fill_clipped(&p, k.fill, 255, &track);
        }
    }
    // Notches at the limits, sticking out above and below the track.
    for (lim, c) in [(scale.warn, WARN), (scale.crit, CRIT)] {
        let Some(lv) = lim else { continue };
        let nx = (x + w * scale.frac(lv) - NOTCH_W / 2.0).round();
        let col = if crit_card {
            CRIT_INK
        } else if k.surface == WARN_SURFACE && c == WARN {
            TEXT
        } else {
            mix(c, TEXT, 0.15)
        };
        g.rrect(
            nx,
            y - NOTCH_OUT,
            NOTCH_W,
            h + 2.0 * NOTCH_OUT,
            NOTCH_W / 2.0,
            col,
        );
    }
}

/// A numeral with a smaller unit after it, on one baseline. Returns the width.
#[allow(clippy::too_many_arguments)]
fn num_unit(
    g: &mut Gfx,
    num: &crate::gfx::St,
    text: &str,
    unit: &str,
    unit_st: &crate::gfx::St,
    x: f32,
    base: f32,
    c: Rgb,
    uc: Rgb,
) -> f32 {
    let w = g.text(num, text, x, base, c);
    if unit.is_empty() {
        return w;
    }
    let gap = 2.0;
    w + gap + g.text(unit_st, unit, x + w + gap, base, uc)
}

fn num_unit_w(
    g: &mut Gfx,
    num: &crate::gfx::St,
    text: &str,
    unit: &str,
    unit_st: &crate::gfx::St,
) -> f32 {
    let w = g.measure(num, text);
    if unit.is_empty() {
        w
    } else {
        w + 2.0 + g.measure(unit_st, unit)
    }
}

/// Vertical centre of a numeral's figures for a style and baseline.
fn fig_mid(num: &crate::gfx::St, base: f32) -> f32 {
    base - num.size * 0.35
}

fn icon_col(r: &Reading, k: &Ink) -> Rgb {
    if matches!(r.avail, Avail::Lost(_)) {
        TEXT3
    } else {
        k.icon
    }
}

// ---------------------------------------------------------------- widgets

pub fn draw(g: &mut Gfx, a: &Accent, x: f32, w: f32, wd: &W) {
    match wd {
        W::Clock {
            hh_mm,
            weekday,
            wd_short,
            date,
            date_short,
        } => clock(g, x, w, hh_mm, weekday, wd_short, date, date_short),
        W::Value { r, second } => value(g, a, x, w, r, second.as_ref()),
        W::Network { down, up } => network(g, a, x, w, down, up),
        W::Weather {
            temp,
            cond,
            hi_lo,
            icon,
            avail,
        } => weather(g, a, x, w, temp, cond, hi_lo, *icon, *avail),
        W::Media { track, pressed } => media(g, a, x, w, track.as_ref(), *pressed),
        W::Slider { value, muted, held } => slider(g, a, x, w, *value, *muted, *held),
        W::Key {
            icon,
            label,
            sub,
            state,
        } => key(g, a, x, w, *icon, label, sub, *state),
    }
}

#[allow(clippy::too_many_arguments)]
fn clock(
    g: &mut Gfx,
    x: f32,
    w: f32,
    hh_mm: &str,
    weekday: &str,
    wd_short: &str,
    date: &str,
    date_short: &str,
) {
    card(g, x, w, CARD);
    match class(w) {
        Class::Compact => {
            let tw = g.measure(&NUM_COMPACT, hh_mm);
            g.text(
                &NUM_COMPACT,
                hh_mm,
                x + (w - tw) / 2.0,
                BASE_COMPACT + 1.0,
                TEXT,
            );
        }
        Class::Medium => {
            let tw = g.measure(&NUM_MEDIUM, hh_mm);
            let dw = g
                .measure(&TITLE, wd_short)
                .max(g.measure(&BODY, date_short));
            let total = tw + 10.0 + dw;
            let x0 = x + ((w - total) / 2.0).max(PAD);
            g.text(&NUM_MEDIUM, hh_mm, x0, BASE_MEDIUM, TEXT);
            let dx = x0 + tw + 10.0;
            g.text(&TITLE, wd_short, dx, BASE_LINE1, TEXT);
            g.text(&BODY, date_short, dx, BASE_LINE2 - 1.0, TEXT2);
        }
        Class::Hero => {
            let tw = g.measure(&NUM_HERO, hh_mm);
            let dw = g.measure(&TITLE, weekday).max(g.measure(&BODY, date));
            let total = tw + 14.0 + dw;
            let x0 = x + ((w - total) / 2.0).max(PAD + 4.0);
            g.text(&NUM_HERO, hh_mm, x0, BASE_HERO, TEXT);
            let dx = x0 + tw + 14.0;
            g.text(&TITLE, weekday, dx, BASE_LINE1, TEXT);
            g.text(&BODY, date, dx, BASE_LINE2 - 1.0, TEXT2);
        }
    }
}

/// A reading (temperature, load, memory) with an optional second reading.
fn value(g: &mut Gfx, a: &Accent, x: f32, w: f32, r: &Reading, second: Option<&Reading>) {
    if r.avail == Avail::NotSetUp {
        setup_prompt(g, a, x, w, "Pick a sensor", "Sensor");
        return;
    }
    // The card takes the worse of the two severities: area, not a dot.
    let sev = match (r.sev(), second.map(Reading::sev)) {
        (Sev::Crit, _) | (_, Some(Sev::Crit)) => Sev::Crit,
        (Sev::Warn, _) | (_, Some(Sev::Warn)) => Sev::Warn,
        _ => Sev::Normal,
    };
    let k = ink(sev);
    let lost = matches!(r.avail, Avail::Lost(_));
    let kr = if lost { lost_ink() } else { k };
    card(g, x, w, k.surface);
    let shown = |rd: &Reading| -> (String, Option<f32>) {
        match rd.avail {
            Avail::Live => (rd.text.clone(), Some(rd.value)),
            Avail::Lost(_) => (rd.text.clone(), None),
            _ => ("––".into(), None),
        }
    };
    let (txt, v) = shown(r);
    let unit = if r.avail == Avail::NoData { "" } else { r.unit };
    let num_c = if r.avail == Avail::NoData {
        TEXT3
    } else {
        kr.num
    };
    let icon_glyph = if lost { ico::LINK_BREAK } else { r.icon };
    match class(w) {
        Class::Compact => {
            let ix = x + PAD + ICON_COMPACT / 2.0;
            g.icon(
                Face::IconFill,
                icon_glyph,
                ICON_COMPACT,
                ix,
                fig_mid(&NUM_COMPACT, BASE_COMPACT),
                icon_col(r, &kr),
            );
            let nx = x + PAD + ICON_COMPACT + ICON_GAP;
            let nw = num_unit_w(g, &NUM_COMPACT, &txt, unit, &UNIT);
            // Centre the numeral in the room right of the icon.
            let room = x + w - PAD - nx;
            let nx = nx + ((room - nw) / 2.0).max(0.0);
            num_unit(
                g,
                &NUM_COMPACT,
                &txt,
                unit,
                &UNIT,
                nx,
                BASE_COMPACT,
                num_c,
                kr.label,
            );
            bar(
                g,
                x + PAD,
                BAR_Y_COMPACT,
                w - 2.0 * PAD,
                BAR_H - 1.0,
                &r.scale,
                v,
                &kr,
            );
        }
        Class::Medium => {
            let base = BASE_COMPACT;
            let ix = x + PAD + ICON_MEDIUM / 2.0;
            g.icon(
                Face::IconFill,
                icon_glyph,
                ICON_MEDIUM,
                ix,
                fig_mid(&NUM_BANDED, base),
                icon_col(r, &kr),
            );
            let nx = x + PAD + ICON_MEDIUM + ICON_GAP;
            let nw = num_unit(g, &NUM_BANDED, &txt, unit, &UNIT, nx, base, num_c, kr.label);
            let right = x + w - PAD;
            if let Some(s) = second {
                let (stxt, sv) = shown(s);
                let su = if sv.is_some() { s.unit } else { "" };
                let sc = if sv.is_some() { k.num } else { TEXT3 };
                let sw = num_unit_w(g, &NUM_SECOND, &stxt, su, &UNIT);
                let sx = right - sw;
                num_unit(g, &NUM_SECOND, &stxt, su, &UNIT, sx, base, sc, k.label);
                // Two bars in the band, each under its own number.
                let split = (sx - 10.0).max(nx + nw + 6.0);
                bar(
                    g,
                    x + PAD,
                    BAR_Y_COMPACT,
                    split - 8.0 - (x + PAD),
                    BAR_H - 1.0,
                    &r.scale,
                    v,
                    &kr,
                );
                bar(
                    g,
                    split,
                    BAR_Y_COMPACT,
                    right - split,
                    BAR_H - 1.0,
                    &s.scale,
                    sv,
                    &k,
                );
            } else {
                // The name, or why there is no number, top right.
                let words = match r.avail {
                    Avail::NoData => "No reading".to_string(),
                    Avail::Lost(t) => format!("Lost {t}"),
                    _ => r.name.to_string(),
                };
                let lc = if r.avail == Avail::Live {
                    kr.label
                } else {
                    TEXT3
                };
                let lw = g.measure(&LABEL, &words);
                if nx + nw + 8.0 + lw <= right {
                    g.text_r(&LABEL, &words, right, 20.0, lc);
                }
                bar(
                    g,
                    x + PAD,
                    BAR_Y_COMPACT,
                    w - 2.0 * PAD,
                    BAR_H - 1.0,
                    &r.scale,
                    v,
                    &kr,
                );
            }
        }
        Class::Hero => {
            let ix = x + PAD + 2.0 + ICON_HERO / 2.0;
            g.icon(
                Face::IconFill,
                icon_glyph,
                ICON_HERO,
                ix,
                fig_mid(&NUM_HERO, BASE_HERO),
                icon_col(r, &kr),
            );
            let nx = x + PAD + 2.0 + ICON_HERO + ICON_GAP + 2.0;
            let nw = num_unit(
                g, &NUM_HERO, &txt, unit, &UNIT_HERO, nx, BASE_HERO, num_c, kr.label,
            );
            let cx = nx + nw + 14.0;
            let right = x + w - PAD - 2.0;
            let line_base = 22.0;
            let bar_y = 33.0;
            if let Some(s) = second {
                // Two columns: the first reading's name and bar, then the
                // second reading's number and bar.
                let (stxt, sv) = shown(s);
                let su = if sv.is_some() { s.unit } else { "" };
                let sc = if sv.is_some() { k.num } else { TEXT3 };
                let sw = num_unit_w(g, &NUM_SECOND, &stxt, su, &UNIT);
                let col2 = right - sw.max(56.0);
                g.text(&LABEL, r.name, cx, line_base - 2.0, kr.label);
                bar(g, cx, bar_y, col2 - 12.0 - cx, BAR_H, &r.scale, v, &kr);
                num_unit(
                    g,
                    &NUM_SECOND,
                    &stxt,
                    su,
                    &UNIT,
                    right - sw,
                    27.0,
                    sc,
                    k.label,
                );
                bar(g, col2, bar_y, right - col2, BAR_H, &s.scale, sv, &k);
            } else {
                let words = match r.avail {
                    Avail::NoData => "No reading yet".to_string(),
                    Avail::Lost(t) => format!("Lost at {t}"),
                    _ => r.name.to_string(),
                };
                let lc = if r.avail == Avail::Live {
                    kr.label
                } else {
                    TEXT3
                };
                let lw = g.text(&LABEL, &words, cx, line_base - 2.0, lc);
                if let (Some(lim), Avail::Live) = (&r.limit_words, r.avail) {
                    let limw = g.measure(&LABEL, lim);
                    if cx + lw + 10.0 + limw <= right {
                        g.text_r(&LABEL, lim, right, line_base - 2.0, kr.quiet);
                    }
                }
                bar(g, cx, bar_y, right - cx, BAR_H, &r.scale, v, &kr);
            }
        }
    }
}

fn network(g: &mut Gfx, a: &Accent, x: f32, w: f32, down: &Reading, up: &Reading) {
    if down.avail == Avail::NotSetUp {
        setup_prompt(g, a, x, w, "Pick a network", "Network");
        return;
    }
    let k = ink(Sev::Normal);
    card(g, x, w, k.surface);
    let nodata = down.avail == Avail::NoData;
    let txt = |r: &Reading| {
        if nodata {
            "––".to_string()
        } else {
            r.text.clone()
        }
    };
    let num_c = if nodata { TEXT3 } else { TEXT };
    let unit = if nodata { "" } else { down.unit };
    match class(w) {
        Class::Compact => {
            let ix = x + PAD + 8.0;
            g.icon(
                Face::IconBold,
                ico::ARROW_DOWN,
                ICON_COMPACT,
                ix,
                fig_mid(&NUM_COMPACT, BASE_COMPACT),
                TEXT2,
            );
            let nx = x + PAD + 16.0 + 4.0;
            num_unit(
                g,
                &NUM_COMPACT,
                &txt(down),
                unit,
                &LABEL,
                nx,
                BASE_COMPACT,
                num_c,
                TEXT2,
            );
            bar(
                g,
                x + PAD,
                BAR_Y_COMPACT,
                w - 2.0 * PAD,
                BAR_H - 1.0,
                &down.scale,
                (!nodata).then_some(down.value),
                &k,
            );
        }
        Class::Medium | Class::Hero => {
            // Down is the primary reading; up sits beside it like a second
            // reading, each over its own bar.
            let hero = class(w) == Class::Hero;
            let (num, isz) = if hero {
                (NUM_BANDED, ICON_MEDIUM)
            } else {
                (NUM_COMPACT, ICON_COMPACT)
            };
            let base = BASE_COMPACT;
            let right = x + w - PAD;
            let ix = x + PAD + isz / 2.0 - 2.0;
            g.icon(
                Face::IconBold,
                ico::ARROW_DOWN,
                isz,
                ix,
                fig_mid(&num, base),
                TEXT2,
            );
            let nx = x + PAD + isz - 2.0 + 3.0;
            let nw = num_unit(g, &num, &txt(down), unit, &UNIT, nx, base, num_c, TEXT2);
            let up_unit = if nodata || (!hero && up.unit == down.unit) {
                ""
            } else {
                up.unit
            };
            let sw = num_unit_w(g, &NUM_SECOND, &txt(up), up_unit, &UNIT);
            let ux = right - sw;
            num_unit(
                g,
                &NUM_SECOND,
                &txt(up),
                up_unit,
                &UNIT,
                ux,
                base,
                num_c,
                TEXT2,
            );
            g.icon(
                Face::IconBold,
                ico::ARROW_UP,
                ICON_SMALL,
                ux - 3.0 - ICON_SMALL / 2.0,
                fig_mid(&NUM_SECOND, base),
                TEXT2,
            );
            let split = (ux - ICON_SMALL - 6.0).max(nx + nw + 8.0);
            let nd = (!nodata).then_some(down.value);
            bar(
                g,
                x + PAD,
                BAR_Y_COMPACT,
                split - 10.0 - (x + PAD),
                BAR_H - 1.0,
                &down.scale,
                nd,
                &k,
            );
            bar(
                g,
                split,
                BAR_Y_COMPACT,
                right - split,
                BAR_H - 1.0,
                &up.scale,
                (!nodata).then_some(up.value),
                &k,
            );
            if nodata {
                g.text_r(&LABEL, "No reading", right, 17.0, TEXT3);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn weather(
    g: &mut Gfx,
    a: &Accent,
    x: f32,
    w: f32,
    temp: &str,
    cond: &str,
    hi_lo: &str,
    icon: char,
    avail: Avail,
) {
    if avail == Avail::NotSetUp {
        setup_prompt_icon(
            g,
            a,
            x,
            w,
            ico::MAP_PIN_PLUS,
            "Weather",
            "Add your town",
            "Weather",
        );
        return;
    }
    card(g, x, w, CARD);
    let (t, tc, ic, glyph, words): (&str, Rgb, Rgb, char, String) = match avail {
        Avail::NoData => (
            "––",
            TEXT3,
            TEXT3,
            ico::CLOUD_SLASH,
            "No forecast yet".into(),
        ),
        Avail::Lost(at) => (temp, TEXT3, TEXT3, ico::CLOUD_SLASH, format!("From {at}")),
        _ => (temp, TEXT, TEXT2, icon, cond.to_string()),
    };
    match class(w) {
        Class::Compact => {
            let tw = g.measure(&NUM_MEDIUM, t);
            let total = ICON_MEDIUM + ICON_GAP + tw;
            let x0 = x + ((w - total) / 2.0).max(PAD);
            g.icon(
                Face::IconFill,
                glyph,
                ICON_MEDIUM,
                x0 + ICON_MEDIUM / 2.0,
                fig_mid(&NUM_MEDIUM, BASE_MEDIUM),
                ic,
            );
            g.text(&NUM_MEDIUM, t, x0 + ICON_MEDIUM + ICON_GAP, BASE_MEDIUM, tc);
        }
        Class::Medium => {
            let x0 = x + PAD;
            g.icon(
                Face::IconFill,
                glyph,
                ICON_MEDIUM,
                x0 + ICON_MEDIUM / 2.0,
                fig_mid(&NUM_MEDIUM, BASE_MEDIUM),
                ic,
            );
            let nx = x0 + ICON_MEDIUM + ICON_GAP;
            let nw = g.text(&NUM_MEDIUM, t, nx, BASE_MEDIUM, tc);
            let dx = nx + nw + 8.0;
            let room = x + w - PAD - dx;
            // Two short lines; words chosen to fit, never cut.
            let (l1, l2) = match avail {
                Avail::NoData => ("No data".to_string(), String::new()),
                Avail::Lost(at) => ("From".to_string(), at.to_string()),
                _ => (words, hi_lo.to_string()),
            };
            let c1 = if avail == Avail::Live { TEXT } else { TEXT3 };
            if l2.is_empty() {
                let l1 = g.fit(&BODY, &l1, room);
                g.text(&BODY, &l1, dx, 32.0, c1);
            } else {
                let l1 = g.fit(&BODY, &l1, room);
                g.text(&BODY, &l1, dx, BASE_LINE1, c1);
                let l2 = g.fit(&LABEL, &l2, room);
                g.text(&LABEL, &l2, dx, BASE_LINE2 - 1.0, TEXT2);
            }
        }
        Class::Hero => {
            let x0 = x + PAD + 4.0;
            g.icon(
                Face::IconFill,
                glyph,
                ICON_HERO,
                x0 + ICON_HERO / 2.0,
                fig_mid(&NUM_HERO, BASE_HERO),
                ic,
            );
            let nx = x0 + ICON_HERO + ICON_GAP + 2.0;
            let nw = g.text(&NUM_HERO, t, nx, BASE_HERO, tc);
            let dx = nx + nw + 14.0;
            let room = x + w - PAD - dx;
            let line1 = g.fit(&TITLE, &words, room);
            g.text(
                &TITLE,
                &line1,
                dx,
                BASE_LINE1,
                if avail == Avail::Live { TEXT } else { TEXT3 },
            );
            let l2 = if avail == Avail::Live {
                hi_lo.to_string()
            } else {
                "Tap to retry".to_string()
            };
            let l2 = g.fit(&BODY, &l2, room);
            g.text(&BODY, &l2, dx, BASE_LINE2 - 1.0, TEXT2);
        }
    }
}

fn fmt_time(s: u32) -> String {
    format!("{}:{:02}", s / 60, s % 60)
}

fn media(g: &mut Gfx, a: &Accent, x: f32, w: f32, track: Option<&Track>, pressed: Option<usize>) {
    card(g, x, w, CARD);
    let Some(t) = track else {
        media_nothing(g, a, x, w);
        return;
    };
    let art_s = 33.0;
    let art_y = CARD_Y + (CARD_H - art_s) / 2.0;
    match class(w) {
        Class::Compact => {
            // A play/pause key and the elapsed time.
            let kw = TARGET_MIN;
            let kx = x + PAD - 3.0;
            key_face(g, a, kx, KEY_Y, kw, KEY_H, KEY_R, pressed == Some(1), true);
            let glyph = if t.playing { ico::PAUSE } else { ico::PLAY };
            let dy = if pressed == Some(1) { 1.0 } else { 0.0 };
            g.icon(
                Face::IconFill,
                glyph,
                ICON_KEY,
                kx + kw / 2.0,
                KEY_Y + KEY_H / 2.0 + dy,
                a.ink,
            );
            let tx = kx + kw + 8.0;
            let el = fmt_time(t.pos);
            g.text(
                &TITLE,
                &el,
                tx,
                BASE_LINE1 + 1.0,
                if t.playing { TEXT } else { TEXT2 },
            );
            let lw = x + w - PAD - tx;
            progress(g, a, tx, 34.0, lw, t, 3.0);
        }
        Class::Medium => {
            let ax = x + PAD - 3.0;
            g.art(ax, art_y, art_s, 7.0, t.title, !t.playing);
            // The card is a play/pause key: a small accent badge says so.
            let badge_c = (ax + art_s - 4.0, art_y + art_s - 4.0);
            g.circle(badge_c.0, badge_c.1, 8.5, CARD, 255);
            g.circle(badge_c.0, badge_c.1, 7.0, a.base, 255);
            let glyph = if t.playing { ico::PAUSE } else { ico::PLAY };
            g.icon(Face::IconFill, glyph, 10.0, badge_c.0, badge_c.1, a.ink);
            let tx = ax + art_s + 10.0;
            let right = x + w - PAD;
            let tc = if t.playing { TEXT } else { TEXT2 };
            g.text_fade(&TITLE, t.title, tx, BASE_LINE1, tc, right, 18.0, CARD);
            let line2 = if t.playing { t.artist } else { "Paused" };
            g.text_fade(&BODY, line2, tx, BASE_LINE2 - 1.0, TEXT2, right, 18.0, CARD);
        }
        Class::Hero => {
            let ax = x + PAD - 3.0;
            g.art(ax, art_y, art_s, 7.0, t.title, !t.playing);
            // Prev and next join play/pause once there is room for them
            // and a whole title.
            let n = if w >= MEDIA_KEYS_MIN { 3.0 } else { 1.0 };
            let keys_w = n * TARGET_MIN + (n - 1.0) * 4.0;
            let kx0 = x + w - PAD + 3.0 - keys_w;
            if n > 1.0 {
                transport(g, a, kx0, KEY_Y, t.playing, pressed);
            } else {
                key_face(
                    g,
                    a,
                    kx0,
                    KEY_Y,
                    TARGET_MIN,
                    KEY_H,
                    KEY_R,
                    pressed == Some(1),
                    true,
                );
                let glyph = if t.playing { ico::PAUSE } else { ico::PLAY };
                let dy = if pressed == Some(1) { 1.0 } else { 0.0 };
                g.icon(
                    Face::IconFill,
                    glyph,
                    ICON_KEY - 2.0,
                    kx0 + TARGET_MIN / 2.0,
                    KEY_Y + KEY_H / 2.0 + dy,
                    a.ink,
                );
            }
            let tx = ax + art_s + 10.0;
            let right = kx0 - 10.0;
            let tc = if t.playing { TEXT } else { TEXT2 };
            g.text_fade(&TITLE, t.title, tx, BASE_LINE1, tc, right, 18.0, CARD);
            let times = format!("{} / {}", fmt_time(t.pos), fmt_time(t.len));
            let tw = g.measure(&LABEL, &times);
            let line2 = if t.playing { t.artist } else { "Paused" };
            let artist_right = right - tw - 10.0;
            g.text_fade(
                &BODY,
                line2,
                tx,
                BASE_LINE2 - 1.0,
                TEXT2,
                artist_right,
                14.0,
                CARD,
            );
            g.text_r(&LABEL, &times, right, BASE_LINE2 - 1.0, TEXT3);
        }
    }
}

/// Prev, play/pause, next. The middle key is filled with the accent.
fn transport(g: &mut Gfx, a: &Accent, kx0: f32, ky: f32, playing: bool, pressed: Option<usize>) {
    for (i, glyph) in [
        ico::SKIP_BACK,
        if playing { ico::PAUSE } else { ico::PLAY },
        ico::SKIP_FWD,
    ]
    .into_iter()
    .enumerate()
    {
        let kx = kx0 + i as f32 * (TARGET_MIN + 4.0);
        let p = pressed == Some(i);
        key_face(g, a, kx, ky, TARGET_MIN, KEY_H, KEY_R, p, i == 1);
        let c = if i == 1 {
            a.ink
        } else if p {
            TEXT
        } else {
            a.text
        };
        let dy = if p { 1.0 } else { 0.0 };
        g.icon(
            Face::IconFill,
            glyph,
            ICON_KEY - 2.0,
            kx + TARGET_MIN / 2.0,
            ky + KEY_H / 2.0 + dy,
            c,
        );
    }
}

/// A thin progress line with no thumb: information, not a control.
fn progress(g: &mut Gfx, a: &Accent, x: f32, y: f32, w: f32, t: &Track, h: f32) {
    g.rrect(x, y, w, h, h / 2.0, TRACK);
    let f = t.pos as f32 / t.len as f32;
    let c = if t.playing { a.fill } else { TEXT3 };
    g.rrect(x, y, (w * f).max(h), h, h / 2.0, c);
}

fn media_nothing(g: &mut Gfx, _a: &Accent, x: f32, w: f32) {
    // An empty sleeve: a dashed rounded square in a quiet neutral.
    let s = 31.0;
    let sx = x + PAD - 2.0;
    let sy = CARD_Y + (CARD_H - s) / 2.0;
    g.rrect_line(sx, sy, s, s, 7.0, TEXT3, 1.2, Some((3.0, 3.0)));
    g.icon(
        Face::IconBold,
        ico::MUSIC,
        17.0,
        sx + s / 2.0,
        sy + s / 2.0,
        TEXT3,
    );
    let tx = sx + s + 9.0;
    match class(w) {
        Class::Compact => {
            g.text(&LABEL, "Nothing", tx, BASE_LINE1 - 1.0, TEXT2);
            g.text(&LABEL, "playing", tx, BASE_LINE2 - 3.0, TEXT2);
        }
        _ => {
            let st1 = if class(w) == Class::Hero { TITLE } else { BODY };
            g.text(&st1, "Nothing playing", tx, BASE_LINE1, TEXT2);
            let hint = if class(w) == Class::Hero {
                "Music you play shows up here"
            } else {
                "Play something"
            };
            let hint = g.fit(&BODY, hint, x + w - PAD - tx);
            g.text(&BODY, &hint, tx, BASE_LINE2 - 1.0, TEXT3);
        }
    }
}

fn slider(g: &mut Gfx, a: &Accent, x: f32, w: f32, value: f32, muted: bool, held: bool) {
    card(g, x, w, CARD);
    let glyph = if muted {
        ico::SPEAKER_X
    } else if value < 34.0 {
        ico::SPEAKER_LOW
    } else {
        ico::SPEAKER_HIGH
    };
    let txt = format!("{}", value.round() as i32);
    let num_c = if muted {
        TEXT3
    } else if held {
        a.text
    } else {
        TEXT
    };
    match class(w) {
        Class::Compact => {
            let ix = x + PAD + 9.0;
            g.icon(
                Face::IconFill,
                glyph,
                ICON_SMALL + 1.0,
                ix,
                18.0,
                if muted { TEXT3 } else { TEXT2 },
            );
            let nx = x + PAD + 22.0;
            if muted {
                g.text(&TITLE, "Muted", nx, 25.0, TEXT2);
            } else {
                num_unit(g, &TITLE, &txt, "%", &LABEL, nx, 25.0, num_c, TEXT2);
            }
            slider_track(
                g,
                a,
                x + PAD + 2.0,
                37.0,
                w - 2.0 * PAD - 4.0,
                value,
                muted,
                held,
                None,
                false,
            );
        }
        Class::Medium => {
            // The track gets the room: the number stays at compact size
            // and drops its percent sign.
            let base = 37.0;
            let ix = x + PAD + ICON_COMPACT / 2.0;
            g.icon(
                Face::IconFill,
                glyph,
                ICON_COMPACT,
                ix,
                fig_mid(&NUM_COMPACT, base),
                if muted { TEXT3 } else { TEXT2 },
            );
            let nx = x + PAD + ICON_COMPACT + ICON_GAP - 1.0;
            let nw = if muted {
                g.text(&BODY, "Muted", nx, 32.0, TEXT2)
            } else {
                num_unit(g, &NUM_COMPACT, &txt, "%", &LABEL, nx, base, num_c, TEXT2)
            };
            let tx = nx + nw.max(g.measure(&NUM_COMPACT, "00")) + 12.0;
            let tw = x + w - PAD - 4.0 - tx;
            slider_track(g, a, tx, 26.5, tw, value, muted, held, None, false);
        }
        Class::Hero => {
            let ix = x + PAD + 2.0 + ICON_HERO / 2.0;
            g.icon(
                Face::IconFill,
                glyph,
                ICON_HERO,
                ix,
                fig_mid(&NUM_HERO, BASE_HERO),
                if muted { TEXT3 } else { TEXT2 },
            );
            let nx = x + PAD + 2.0 + ICON_HERO + ICON_GAP + 2.0;
            let nw = num_unit(
                g, &NUM_HERO, &txt, "%", &UNIT_HERO, nx, BASE_HERO, num_c, TEXT2,
            );
            let tx = nx + nw + 18.0;
            let tw = x + w - PAD - 6.0 - tx;
            slider_track(g, a, tx, 24.0, tw, value, muted, held, None, !muted);
            if muted {
                g.text_c(&LABEL, "Muted", tx + tw / 2.0, 44.0, TEXT2);
            }
        }
    }
}

/// A slider track with its thumb. `from` marks where the value was before
/// a change (the volume takeover's ghost mark). `ticks` adds a 0–100 scale.
#[allow(clippy::too_many_arguments)]
pub fn slider_track(
    g: &mut Gfx,
    a: &Accent,
    x: f32,
    cy: f32,
    w: f32,
    value: f32,
    muted: bool,
    held: bool,
    from: Option<f32>,
    ticks: bool,
) {
    let h = SLIDER_TRACK_H;
    let y = cy - h / 2.0;
    if ticks {
        for i in 0..=4 {
            let tx = (x + w * i as f32 / 4.0).round() - 1.0;
            g.rrect(
                tx,
                y + h + 6.0,
                2.0,
                if i % 2 == 0 { 6.0 } else { 4.0 },
                1.0,
                TICK,
            );
        }
    }
    g.rrect(x, y, w, h, h / 2.0, TRACK);
    let f = (value / 100.0).clamp(0.0, 1.0);
    let tx = x + w * f;
    let fill_c = if muted {
        mix(TEXT3, CARD, 0.35)
    } else {
        a.fill
    };
    if let Some(fr) = from {
        // The change just made is bright; what was there before is calm.
        let fx = x + w * (fr / 100.0);
        g.rrect(x, y, tx - x, h, h / 2.0, mix(a.fill, CARD, 0.5));
        g.rrect(fx - h / 2.0, y, tx - fx + h / 2.0, h, h / 2.0, a.base);
        g.rrect((fx - 1.0).round(), y - 4.0, 2.0, h + 8.0, 1.0, TEXT2);
    } else {
        g.rrect(x, y, (tx - x).max(h), h, h / 2.0, fill_c);
    }
    if held {
        g.glow(tx, cy, THUMB_HALO + 4.0, a.base, 110);
        g.circle(tx, cy, THUMB_R_HELD + 1.5, CARD, 255);
        g.circle(tx, cy, THUMB_R_HELD, a.base, 255);
        g.circle(tx, cy, 3.0, a.ink, 255);
    } else {
        // A cream disc with an accent ring: something to take hold of.
        let ring = if muted { TEXT3 } else { a.base };
        g.circle(tx, cy, THUMB_R + 1.5, CARD, 255);
        g.circle(tx, cy, THUMB_R, ring, 255);
        g.circle(tx, cy, THUMB_R - 2.5, TEXT, 255);
    }
}

#[allow(clippy::too_many_arguments)]
fn key(
    g: &mut Gfx,
    a: &Accent,
    x: f32,
    w: f32,
    icon: char,
    label: &str,
    sub: &str,
    state: KeyState,
) {
    if state == KeyState::NotSetUp {
        setup_prompt_icon(g, a, x, w, ico::PLUS, "Add a key", "Tap to set up", "Key");
        return;
    }
    let pressed = state == KeyState::Pressed;
    // The whole card is the key: raised face, lit top edge.
    key_face(g, a, x, CARD_Y, w, CARD_H, CARD_R, pressed, false);
    if let KeyState::Holding(p) = state {
        // The hold fills the key from the left; it fires when full.
        let fw = (w * p).max(CARD_R * 2.0);
        if let (Some(fill), Some(clip)) = (
            rrect_path(x, CARD_Y + KEY_LIFT, fw, CARD_H - KEY_LIFT, CARD_R),
            rrect_path(x, CARD_Y, w, CARD_H, CARD_R),
        ) {
            g.fill_clipped(&fill, mix(KEY, a.base, HOLD_MIX), 255, &clip);
        }
    }
    let dy = if pressed { 1.0 } else { 0.0 };
    let ic = if pressed { TEXT } else { a.text };
    let cy = CARD_Y + CARD_H / 2.0 + 0.5 + dy;
    let show_label = w >= KEY_LABEL_MIN && !label.is_empty();
    match class(w) {
        Class::Hero if !sub.is_empty() => {
            let lw = g.measure(&TITLE, label).max(g.measure(&BODY, sub));
            let total = ICON_HERO + 12.0 + lw;
            let x0 = x + ((w - total) / 2.0).max(PAD);
            g.icon(
                Face::IconBold,
                icon,
                ICON_HERO - 2.0,
                x0 + ICON_HERO / 2.0,
                cy,
                ic,
            );
            let tx = x0 + ICON_HERO + 12.0;
            g.text(&TITLE, label, tx, BASE_LINE1 + dy, TEXT);
            g.text(&BODY, sub, tx, BASE_LINE2 - 1.0 + dy, TEXT2);
        }
        _ if show_label => {
            let isz = if class(w) == Class::Compact {
                ICON_KEY
            } else {
                ICON_MEDIUM - 2.0
            };
            let lw = g.measure(&KEY_LABEL, label);
            let total = isz + 7.0 + lw;
            let x0 = x + ((w - total) / 2.0).max(PAD);
            g.icon(Face::IconBold, icon, isz, x0 + isz / 2.0, cy, ic);
            g.text(&KEY_LABEL, label, x0 + isz + 7.0, cy + 6.5, TEXT);
        }
        _ => {
            g.icon(Face::IconBold, icon, ICON_MEDIUM, x + w / 2.0, cy, ic);
        }
    }
}

/// "Not set up": a dashed accent outline and plain words; tapping opens the
/// settings window at the right place.
fn setup_prompt(g: &mut Gfx, a: &Accent, x: f32, w: f32, long: &str, short: &str) {
    setup_prompt_icon(g, a, x, w, ico::PLUS, long, "Tap to set up", short);
}

#[allow(clippy::too_many_arguments)]
fn setup_prompt_icon(
    g: &mut Gfx,
    a: &Accent,
    x: f32,
    w: f32,
    icon: char,
    title: &str,
    hint: &str,
    short: &str,
) {
    setup_card(g, a, x, w);
    let cy = CARD_Y + CARD_H / 2.0 + 0.5;
    if class(w) == Class::Compact {
        // Two short lines: what it is, and what to do.
        let lw = g.measure(&BODY, short).max(g.measure(&BODY, "Set up"));
        let total = ICON_COMPACT + 7.0 + lw;
        let x0 = x + ((w - total) / 2.0).max(PAD - 2.0);
        g.icon(
            Face::IconFill,
            icon,
            ICON_COMPACT,
            x0 + ICON_COMPACT / 2.0,
            cy,
            a.base,
        );
        let tx = x0 + ICON_COMPACT + 7.0;
        g.text(&BODY, short, tx, BASE_LINE1 - 1.0, TEXT);
        g.text(&BODY, "Set up", tx, BASE_LINE2 - 2.0, a.text);
    } else {
        let x0 = x + PAD + 2.0;
        g.icon(
            Face::IconFill,
            icon,
            ICON_MEDIUM,
            x0 + ICON_MEDIUM / 2.0,
            cy,
            a.base,
        );
        let tx = x0 + ICON_MEDIUM + 9.0;
        let room = x + w - PAD - tx;
        let t = g.fit(&TITLE, title, room);
        g.text(&TITLE, &t, tx, BASE_LINE1, TEXT);
        let h = g.fit(&BODY, hint, room);
        g.text(&BODY, &h, tx, BASE_LINE2 - 1.0, a.text);
    }
}

// ---------------------------------------------------------------- page markers

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Marker {
    pub current: bool,
    pub sev: Sev,
}

/// A page in trouble gets a longer marker, not only a coloured one.
fn marker_w(m: &Marker) -> f32 {
    let base = if m.current {
        MARKER_W_CURRENT
    } else {
        MARKER_W
    };
    if m.sev == Sev::Normal {
        base
    } else {
        base + MARKER_W_ALERT
    }
}

pub fn markers(g: &mut Gfx, ms: &[Marker]) {
    let total: f32 =
        ms.iter().map(marker_w).sum::<f32>() + MARKER_GAP * (ms.len().saturating_sub(1)) as f32;
    let mut x = ((STRIP_W - total) / 2.0).round();
    for m in ms {
        let w = marker_w(m);
        let c = match (m.sev, m.current) {
            (Sev::Crit, _) => CRIT,
            (Sev::Warn, _) => WARN,
            (Sev::Normal, true) => TEXT,
            (Sev::Normal, false) => mix(TEXT3, GROUND, 0.45),
        };
        g.rrect(x, MARKER_Y, w, MARKER_H, MARKER_H / 2.0, c);
        x += w + MARKER_GAP;
    }
}

// ---------------------------------------------------------------- takeovers

/// Now playing: the media scene over the cards, full title, transport keys.
pub fn now_playing(g: &mut Gfx, a: &Accent, t: &Track, pressed: Option<usize>) {
    let (x, w) = (EDGE, STRIP_W - 2.0 * EDGE);
    g.rrect(x, CARD_Y, w, CARD_H, CARD_R, CARD);
    let s = 35.0;
    let ay = CARD_Y + (CARD_H - s) / 2.0;
    g.art(x + 4.0, ay, s, 8.0, t.title, !t.playing);
    let keys_w = 3.0 * (TARGET_MIN + 6.0) + 2.0 * 4.0;
    let kx0 = x + w - 4.0 - keys_w;
    // Bigger keys in the takeover: there is room, and it is a moment to act.
    for (i, glyph) in [
        ico::SKIP_BACK,
        if t.playing { ico::PAUSE } else { ico::PLAY },
        ico::SKIP_FWD,
    ]
    .into_iter()
    .enumerate()
    {
        let kw = TARGET_MIN + 6.0;
        let kx = kx0 + i as f32 * (kw + 4.0);
        let p = pressed == Some(i);
        key_face(g, a, kx, KEY_Y, kw, KEY_H, KEY_R, p, i == 1);
        let c = if i == 1 { a.ink } else { a.text };
        g.icon(
            Face::IconFill,
            glyph,
            ICON_KEY,
            kx + kw / 2.0,
            KEY_Y + KEY_H / 2.0 + if p { 1.0 } else { 0.0 },
            c,
        );
    }
    let tx = x + 4.0 + s + 12.0;
    let right = kx0 - 14.0;
    // Line one: artist, progress, time. Line two: the full title, big.
    let aw = g.text(&BODY, t.artist, tx, 20.0, a.text);
    let times = format!("{} / {}", fmt_time(t.pos), fmt_time(t.len));
    let tw = g.text_r(&LABEL, &times, right, 20.0, TEXT2);
    let px0 = tx + aw + 12.0;
    let px1 = right - tw - 12.0;
    if px1 - px0 > 30.0 {
        progress(g, a, px0, 14.0, px1 - px0, t, 3.0);
    }
    g.text_fade(&TITLE_BIG, t.title, tx, 43.0, TEXT, right, 24.0, CARD);
}

/// The volume takeover: a card over part of the page, aligned left or right.
pub fn volume_takeover(g: &mut Gfx, a: &Accent, x: f32, w: f32, from: f32, to: f32, held: bool) {
    // A soft drop shadow on the ground side so it reads as on top.
    g.rrect(
        x - 3.0,
        CARD_Y - 1.0,
        w + 6.0,
        CARD_H + 2.0,
        CARD_R + 2.0,
        GROUND,
    );
    g.rrect(x, CARD_Y, w, CARD_H, CARD_R, mix(CARD, KEY, 0.45));
    g.rrect_line(
        x,
        CARD_Y,
        w,
        CARD_H,
        CARD_R,
        mix(a.base, CARD, 0.35),
        1.5,
        None,
    );
    let ix = x + PAD + 2.0 + ICON_MEDIUM / 2.0;
    g.icon(
        Face::IconFill,
        ico::SPEAKER_HIGH,
        ICON_MEDIUM,
        ix,
        fig_mid(&NUM_MEDIUM, BASE_MEDIUM),
        TEXT2,
    );
    let nx = x + PAD + 2.0 + ICON_MEDIUM + ICON_GAP;
    let nw = num_unit(
        g,
        &NUM_MEDIUM,
        &format!("{}", to as i32),
        "%",
        &UNIT,
        nx,
        BASE_MEDIUM,
        TEXT,
        TEXT2,
    );
    let d = (to - from).round() as i32;
    let delta = if d >= 0 {
        format!("+{d}")
    } else {
        format!("−{}", -d)
    };
    let dx = nx + nw + 8.0;
    let dw = g.text(&LABEL, &delta, dx, 21.0, a.text);
    let tx = dx + dw.max(22.0) + 12.0;
    let tw = x + w - PAD - 8.0 - tx;
    slider_track(g, a, tx, 26.5, tw, to, false, held, Some(from), false);
}

/// The critical alarm: the whole strip, until OK, a swipe, or 10 s back
/// under the limit.
#[allow(clippy::too_many_arguments)]
pub fn alarm(
    g: &mut Gfx,
    a: &Accent,
    icon: char,
    headline: &str,
    detail: &str,
    value: &str,
    ok_pressed: bool,
) {
    let _ = a;
    g.rect(0.0, 0.0, STRIP_W, STRIP_H, CRIT);
    // A darker inner well behind the reading, so the numeral is the anchor.
    let x0 = EDGE + 10.0;
    g.icon(
        Face::IconFill,
        ico::WARNING,
        ICON_HERO,
        x0 + ICON_HERO / 2.0,
        24.0,
        CRIT_INK,
    );
    let tx = x0 + ICON_HERO + 12.0;
    g.icon(
        Face::IconFill,
        icon,
        ICON_SMALL + 1.0,
        tx + 9.0,
        15.0,
        CRIT_INK,
    );
    g.text(&TITLE, headline, tx + 24.0, 22.0, CRIT_INK);
    g.text(&BODY, detail, tx, 42.0, mix(CRIT_INK, CRIT, 0.18));
    // OK key: a cream key on the red, the one thing to press.
    let kw = 84.0;
    let kh = 36.0;
    let kx = STRIP_W - EDGE - 8.0 - kw;
    let ky = (STRIP_H - kh) / 2.0;
    if ok_pressed {
        g.rrect(
            kx,
            ky + 1.0,
            kw,
            kh - 1.0,
            KEY_R + 1.0,
            mix(CRIT_INK, CRIT, 0.3),
        );
    } else {
        g.rrect(kx, ky, kw, kh, KEY_R + 1.0, mix(CRIT_INK, CRIT, 0.35));
        g.rrect(kx, ky + 1.0, kw, kh - 1.0, KEY_R + 1.0, CRIT_INK);
    }
    g.text_c(
        &st(22.0, W_NUM, 0.5),
        "OK",
        kx + kw / 2.0,
        ky + kh / 2.0 + 8.0 + if ok_pressed { 1.0 } else { 0.0 },
        CRIT,
    );
    let vw = g.measure(&NUM_HERO, value);
    g.text(&NUM_HERO, value, kx - 22.0 - vw, BASE_HERO, CRIT_INK);
}
