//! The user's accent, as the window uses it: the same derivation as the
//! strip (`DESIGN.md`, "The accent"; `Accent::from` in the design-system
//! example), so a colour picked here looks the same on both. Only its hue
//! is used, its chroma is capped, and every role has a fixed lightness.

use nexus_core::theme::Rgb;

/// The default, when the layout sets none.
pub const DEFAULT: Rgb = Rgb::hex(0xb6a2ff);

/// The colours offered as swatches. Hues near the warning amber (73°) and
/// the critical red (30°) are left out, as `DESIGN.md` open question 9
/// asks; a custom colour can still be anything.
pub const PRESETS: [(&str, Rgb); 5] = [
    ("Lilac", DEFAULT),
    ("Sky", Rgb::hex(0x5fb4ff)),
    ("Teal", Rgb::hex(0x4fc9c4)),
    ("Rose", Rgb::hex(0xf598c8)),
    ("Stone", Rgb::hex(0xb3a596)),
];

const L_BASE: f32 = 0.80;
const C_BASE: f32 = 0.15;
const L_TEXT: f32 = 0.84;
const C_TEXT: f32 = 0.12;
const L_FILL: f32 = 0.66;
const C_FILL: f32 = 0.12;
const L_SOFT: f32 = 0.38;
const C_SOFT: f32 = 0.07;
/// The window's own: accent words and rings on a light background, where
/// the strip's light roles would not show.
const L_DEEP: f32 = 0.50;
const C_DEEP: f32 = 0.13;
/// Below this chroma the colour counts as grey.
const GREY: f32 = 0.02;
/// Hues within this many degrees of warning or critical are flagged.
const NEAR: f32 = 25.0;
/// Below this chroma a colour is too muted to be taken for amber or red.
const MUTED: f32 = 0.05;
const WARN_HUE: f32 = 73.0;
const CRIT_HUE: f32 = 30.0;

/// The accent's roles.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Roles {
    /// Key faces, primary buttons, thumbs, the selection on the dark side.
    pub base: Rgb,
    /// Glyphs and words on `base`: always dark.
    pub ink: Rgb,
    /// Accent words and icons on a dark surface.
    pub text: Rgb,
    /// Slider fills.
    pub fill: Rgb,
    /// Pressed faces, the halo of a held thumb.
    pub soft: Rgb,
    /// Accent words, rings and outlines on a light surface.
    pub deep: Rgb,
}

pub fn roles(user: Rgb) -> Roles {
    let (_, c, h) = oklch(user);
    let role = |l: f32, cmax: f32| {
        let c = if c < GREY { 0.0 } else { c.min(cmax) };
        from_oklch(l, c, h)
    };
    Roles {
        base: role(L_BASE, C_BASE),
        ink: Rgb::hex(0x14110f),
        text: role(L_TEXT, C_TEXT),
        fill: role(L_FILL, C_FILL),
        soft: role(L_SOFT, C_SOFT),
        deep: role(L_DEEP, C_DEEP),
    }
}

/// Whether the colour's hue sits close to warning amber or critical red.
pub fn near_severity(user: Rgb) -> bool {
    let (_, c, h) = oklch(user);
    let dist = |a: f32| {
        let d = (h - a).abs() % 360.0;
        d.min(360.0 - d)
    };
    c >= MUTED && (dist(WARN_HUE) < NEAR || dist(CRIT_HUE) < NEAR)
}

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

/// sRGB to OKLCH: lightness 0–1, chroma, hue in degrees.
fn oklch(c: Rgb) -> (f32, f32, f32) {
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

/// OKLCH to sRGB; out of gamut, chroma is given up until it fits.
fn from_oklch(l: f32, c: f32, h: f32) -> Rgb {
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

/// A colour as Slint takes it.
pub fn slint(c: Rgb) -> slint::Color {
    slint::Color::from_rgb_u8(c.r, c.g, c.b)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The table in `DESIGN.md`: the window and the strip agree.
    #[test]
    fn the_roles_match_the_strips() {
        let lilac = roles(DEFAULT);
        assert_eq!(lilac.base, Rgb::hex(0xc0b0fe));
        assert_eq!(lilac.text, Rgb::hex(0xccc1fe));
        assert_eq!(lilac.fill, Rgb::hex(0x9583d5));
        assert_eq!(lilac.soft, Rgb::hex(0x443b64));
        let sky = roles(Rgb::hex(0x5fb4ff));
        assert_eq!(sky.base, Rgb::hex(0x86c4fe));
    }

    #[test]
    fn a_grey_pick_stays_grey() {
        let r = roles(Rgb::hex(0x808080));
        assert!(r.base.r.abs_diff(r.base.g) <= 1 && r.base.g.abs_diff(r.base.b) <= 1);
    }

    #[test]
    fn hues_near_amber_and_red_are_flagged_and_presets_are_not() {
        assert!(near_severity(Rgb::hex(0xffc53a))); // marigold
        assert!(near_severity(Rgb::hex(0xff9a62))); // apricot
        assert!(!near_severity(Rgb::hex(0x808080)));
        for (name, c) in PRESETS {
            assert!(!near_severity(c), "{name}");
        }
    }
}
