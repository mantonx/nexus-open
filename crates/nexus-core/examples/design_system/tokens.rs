//! Every size, colour and timing of the strip's design system, in one place.
//!
//! Sizes are provisional: the panel has not been measured yet (U0's
//! legibility card). When it has, change the numbers here and nowhere else.
//! DESIGN.md lists the same constants by name.

use nexus_core::theme::Rgb;

use crate::gfx::{St, st};

// ---------------------------------------------------------------- geometry

/// Panel size.
pub const STRIP_W: f32 = 640.0;
pub const STRIP_H: f32 = 48.0;
/// Rows and columns the bezel or the viewing angle may hide. Nothing
/// essential is drawn in them; card fills may run into them.
pub const SAFE: f32 = 2.0;
/// Space at the left and right ends of the strip.
pub const EDGE: f32 = 2.0;
/// Space between two cards.
pub const GUTTER: f32 = 4.0;
/// Cards: top edge, height and corner radius. The 4 rows above hold the
/// page markers.
pub const CARD_Y: f32 = 6.0;
pub const CARD_H: f32 = 41.0;
pub const CARD_R: f32 = 10.0;
/// Inner padding at a card's left and right.
pub const PAD: f32 = 9.0;
/// Space between an icon and the text it labels.
pub const ICON_GAP: f32 = 6.0;

/// Size classes, by the widget's width in pixels.
pub const COMPACT_MAX: f32 = 149.0; // compact: up to 149
pub const MEDIUM_MAX: f32 = 299.0; // medium: 150 to 299; hero: 300 and up

/// Baselines inside a card (y of the baseline, in strip pixels).
pub const BASE_HERO: f32 = 44.0; // 50 px numerals: ink 9..44
pub const BASE_MEDIUM: f32 = 41.0; // 40 px numerals: ink 13..41
pub const BASE_COMPACT: f32 = 36.0; // 32 px numerals, above a bar band
pub const BASE_LINE1: f32 = 24.0; // first of two text lines
pub const BASE_LINE2: f32 = 42.0; // second of two text lines

// ---------------------------------------------------------------- bars

/// The fixed-scale bar: a pill track with its warning and critical zones
/// tinted and notched.
pub const BAR_H: f32 = 6.0;
/// The bar's band in a compact card, under the numeral.
pub const BAR_Y_COMPACT: f32 = 40.0;
/// Notch marks at the limits stick out this far above and below the track.
pub const NOTCH_OUT: f32 = 2.0;
pub const NOTCH_W: f32 = 2.0;

// ---------------------------------------------------------------- touch

/// Smallest width of anything a finger is meant to hit: 44 px is 8.7 mm
/// on this panel (about 128 px per inch).
pub const TARGET_MIN: f32 = 44.0;
/// Keys inside a card (transport keys, OK on the alarm).
pub const KEY_H: f32 = 33.0;
pub const KEY_R: f32 = 9.0;
pub const KEY_Y: f32 = CARD_Y + (CARD_H - KEY_H) / 2.0;
/// The raised look: a lighter face and a 1 px lighter top edge.
pub const KEY_LIFT: f32 = 1.0;
/// A key widget this wide or wider shows its label beside its icon.
pub const KEY_LABEL_MIN: f32 = 96.0;
/// A media widget this wide shows prev and next beside play/pause.
pub const MEDIA_KEYS_MIN: f32 = 400.0;
/// How far the hold fill mixes the key face toward the accent.
pub const HOLD_MIX: f32 = 0.45;
/// Slider thumb radius at rest, while held, and the halo around it.
pub const THUMB_R: f32 = 7.5;
pub const THUMB_R_HELD: f32 = 10.0;
pub const THUMB_HALO: f32 = 15.0;
pub const SLIDER_TRACK_H: f32 = 8.0;
/// Dashes of the "not set up" outline: on, off, and line width.
pub const DASH: (f32, f32) = (5.0, 4.0);
pub const DASH_W: f32 = 1.5;

// ---------------------------------------------------------------- page markers

pub const MARKER_Y: f32 = SAFE; // rows 2..4
pub const MARKER_H: f32 = 3.0;
pub const MARKER_W: f32 = 14.0;
pub const MARKER_W_CURRENT: f32 = 26.0;
pub const MARKER_GAP: f32 = 6.0;
/// A page with a warning or critical reading gets a longer marker, not
/// only a coloured one.
pub const MARKER_W_ALERT: f32 = 8.0;

// ---------------------------------------------------------------- type

/// The one text face is Rubik (variable, 300–900). Numerals use tabular
/// figures. Weights: 600 for numerals and titles, 500 for words.
pub const W_NUM: f32 = 600.0;
pub const W_TITLE: f32 = 560.0;
pub const W_BODY: f32 = 480.0;

pub const NUM_HERO: St = st(50.0, W_NUM, -0.5);
pub const NUM_MEDIUM: St = st(40.0, W_NUM, -0.4);
pub const NUM_COMPACT: St = st(32.0, W_NUM, -0.3);
/// A medium numeral that shares its card with a bar band below it.
pub const NUM_BANDED: St = st(36.0, W_NUM, -0.4);
/// A second reading beside the first (load beside temperature).
pub const NUM_SECOND: St = st(26.0, W_NUM, -0.2);
/// Units and percent signs that follow a numeral.
pub const UNIT_HERO: St = st(22.0, W_TITLE, 0.0);
pub const UNIT: St = st(17.0, W_TITLE, 0.0);
/// The track title in the now-playing takeover.
pub const TITLE_BIG: St = st(26.0, W_TITLE, -0.2);
/// Titles: track title in widgets, weekday, key labels, alarm headline.
pub const TITLE: St = st(20.0, W_TITLE, 0.0);
/// Key labels and the short words on compact "set up" cards.
pub const KEY_LABEL: St = st(18.0, W_TITLE, 0.0);
/// Body: artist, condition, date.
pub const BODY: St = st(17.0, W_BODY, 0.1);
/// Labels: reading names, times, scale words. The smallest text there is.
pub const LABEL: St = st(15.0, W_TITLE, 0.3);
/// Nothing on the strip is smaller than this.
pub const TEXT_FLOOR: f32 = 14.0;

/// Icons (Phosphor), by the em size of the glyph.
pub const ICON_HERO: f32 = 30.0;
pub const ICON_MEDIUM: f32 = 26.0;
pub const ICON_COMPACT: f32 = 22.0;
pub const ICON_KEY: f32 = 22.0;
pub const ICON_SMALL: f32 = 17.0;

// ---------------------------------------------------------------- motion
// Not drawn in still frames; DESIGN.md describes them. In milliseconds.

pub const MS_PRESS: u32 = 60;
pub const MS_VALUE: u32 = 400;
pub const MS_PAGE: u32 = 280;
pub const MS_TAKEOVER_IN: u32 = 220;
pub const MS_TAKEOVER_OUT: u32 = 320;
pub const MS_HOLD: u32 = 500;
pub const MS_ALARM_BREATHE: u32 = 2400;
pub const MS_MARQUEE_REST: u32 = 2500;
pub const MARQUEE_PX_PER_S: f32 = 28.0;
/// Idle dimming: after this long with no touch, and at night.
pub const IDLE_DIM_AFTER_S: u32 = 300;
/// Brightness kept when idle-dimmed, and at night.
pub const IDLE_DIM: f32 = 0.6;
pub const NIGHT_DIM: f32 = 0.35;

// ---------------------------------------------------------------- colour

/// Neutrals: warm, brown-black, never blue. OKLCH hue about 60.
pub const GROUND: Rgb = Rgb::hex(0x14110f);
pub const CARD: Rgb = Rgb::hex(0x241e1a);
pub const KEY: Rgb = Rgb::hex(0x4a3e35);
pub const KEY_TOP: Rgb = Rgb::hex(0x65564a);
pub const TRACK: Rgb = Rgb::hex(0x3d342d);
pub const TICK: Rgb = Rgb::hex(0x7a6b5e);
pub const TEXT: Rgb = Rgb::hex(0xfbf1e3);
pub const TEXT2: Rgb = Rgb::hex(0xd2c2ad);
pub const TEXT3: Rgb = Rgb::hex(0x9c8c7c);
/// Data fills (bars) when nothing is wrong: warm cream, a step below text.
pub const FILL: Rgb = Rgb::hex(0xe6d6c0);

/// Severity. Fixed: never taken from the accent.
pub const WARN: Rgb = Rgb::hex(0xffb23e);
pub const WARN_SURFACE: Rgb = Rgb::hex(0x4f3612);
pub const WARN_ZONE: Rgb = Rgb::hex(0x6b4a1d);
pub const CRIT: Rgb = Rgb::hex(0xd93425);
pub const CRIT_INK: Rgb = Rgb::hex(0xffffff);
pub const CRIT_DEEP: Rgb = Rgb::hex(0x8a1d14);
pub const CRIT_ZONE: Rgb = Rgb::hex(0x6e2a22);

/// The accent's roles, derived from the user's colour by `Accent::from`.
#[derive(Clone, Copy, Debug)]
pub struct Accent {
    /// Key fills, slider thumbs, the dashed "set up" outline.
    pub base: Rgb,
    /// Glyphs and text drawn on `base`.
    pub ink: Rgb,
    /// Text and icons in the accent on a card.
    pub text: Rgb,
    /// Slider fill: the accent, a step darker, so a long fill stays calm.
    pub fill: Rgb,
    /// Pressed key faces, the hold fill, the halo round a held thumb.
    pub soft: Rgb,
}

/// The default accent and the alternatives shown in the frames.
/// (name, the colour a user might pick)
pub const ACCENT_DEFAULT: (&str, Rgb) = ("lilac", Rgb::hex(0xb6a2ff));
pub const ACCENT_ALTERNATIVES: [(&str, Rgb); 3] = [
    ("apricot", Rgb::hex(0xff9a62)),
    ("sky", Rgb::hex(0x5fb4ff)),
    ("marigold", Rgb::hex(0xffc53a)),
];

/// Lightness and chroma per accent role, in OKLCH. The user's colour gives
/// only the hue (and caps the chroma); every role's lightness is fixed, so
/// any accent sits at the same brightness and severity stays the brightest
/// and largest thing on the strip.
pub const ACCENT_L: f32 = 0.80;
pub const ACCENT_C_MAX: f32 = 0.15;
pub const ACCENT_TEXT_L: f32 = 0.84;
pub const ACCENT_TEXT_C_MAX: f32 = 0.12;
pub const ACCENT_FILL_L: f32 = 0.66;
pub const ACCENT_FILL_C_MAX: f32 = 0.12;
pub const ACCENT_SOFT_L: f32 = 0.38;
pub const ACCENT_SOFT_C_MAX: f32 = 0.07;
/// Below this chroma the user's colour counts as grey; the roles stay grey.
pub const ACCENT_GREY_C: f32 = 0.02;

impl Accent {
    pub fn from(user: Rgb) -> Self {
        let (_, c, h) = oklch(user);
        let role = |l: f32, cmax: f32| {
            let c = if c < ACCENT_GREY_C { 0.0 } else { c.min(cmax) };
            from_oklch(l, c, h)
        };
        Self {
            base: role(ACCENT_L, ACCENT_C_MAX),
            ink: GROUND,
            text: role(ACCENT_TEXT_L, ACCENT_TEXT_C_MAX),
            fill: role(ACCENT_FILL_L, ACCENT_FILL_C_MAX),
            soft: role(ACCENT_SOFT_L, ACCENT_SOFT_C_MAX),
        }
    }
}

// ---------------------------------------------------------------- OKLCH

fn lin(u: u8) -> f32 {
    let c = f32::from(u) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn gam(c: f32) -> f32 {
    if c <= 0.003_130_8 {
        12.92 * c
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// sRGB to OKLCH: (lightness 0–1, chroma, hue in degrees).
pub fn oklch(c: Rgb) -> (f32, f32, f32) {
    let (r, g, b) = (lin(c.r), lin(c.g), lin(c.b));
    let l = 0.412_221_46 * r + 0.536_332_55 * g + 0.051_445_995 * b;
    let m = 0.211_903_5 * r + 0.680_699_5 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_85 * g + 0.629_978_7 * b;
    let (l, m, s) = (l.cbrt(), m.cbrt(), s.cbrt());
    let ll = 0.210_454_26 * l + 0.793_617_8 * m - 0.004_072_047 * s;
    let a = 1.977_998_5 * l - 2.428_592_2 * m + 0.450_593_7 * s;
    let bb = 0.025_904_037 * l + 0.782_771_77 * m - 0.808_675_77 * s;
    let h = bb.atan2(a).to_degrees().rem_euclid(360.0);
    (ll, (a * a + bb * bb).sqrt(), h)
}

fn oklch_to_linear(l: f32, c: f32, h: f32) -> [f32; 3] {
    let (a, b) = (c * h.to_radians().cos(), c * h.to_radians().sin());
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    [
        4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3,
        -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3,
        -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3,
    ]
}

/// OKLCH to sRGB. Out-of-gamut colours lose chroma (hue and lightness
/// kept) until they fit.
pub fn from_oklch(l: f32, c: f32, h: f32) -> Rgb {
    let mut c = c;
    let mut rgb = oklch_to_linear(l, c, h);
    while c > 0.0 && rgb.iter().any(|v| !(-0.0001..=1.0001).contains(v)) {
        c -= 0.002;
        rgb = oklch_to_linear(l, c.max(0.0), h);
    }
    let u = |v: f32| (gam(v.clamp(0.0, 1.0)) * 255.0).round() as u8;
    Rgb {
        r: u(rgb[0]),
        g: u(rgb[1]),
        b: u(rgb[2]),
    }
}

/// WCAG relative luminance and contrast ratio.
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let y = |c: Rgb| 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
    let (ya, yb) = (y(a), y(b));
    (ya.max(yb) + 0.05) / (ya.min(yb) + 0.05)
}
