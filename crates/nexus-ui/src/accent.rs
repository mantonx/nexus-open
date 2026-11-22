//! The user's accent, as the window uses it: the same derivation as the
//! strip (`DESIGN.md`, "The accent"; `Accent::from` in the design-system
//! example), so a colour picked here looks the same on both. Only its hue
//! is used, its chroma is capped, and every role has a fixed lightness.

use nexus_core::theme::{
    ACCENT_DEFAULT, ACCENT_GREY_C, ACCENT_PRESETS, Accent, Rgb, allowed_hue, from_oklch, oklch,
};

/// The default, when the layout sets none.
pub const DEFAULT: Rgb = ACCENT_DEFAULT;

/// The colours offered as swatches: the strip's named presets, so a
/// swatch is saved as its name (`accent = "sky"`).
pub const PRESETS: [(&str, Rgb); 6] = ACCENT_PRESETS;

/// The window's own: accent words and rings on a light background, where
/// the strip's light roles would not show.
const L_DEEP: f32 = 0.50;
const C_DEEP: f32 = 0.13;

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

/// The strip's roles (`Accent::from`, hue snapped out of the amber/red
/// band), plus the window's `deep` on the same snapped hue.
pub fn roles(user: Rgb) -> Roles {
    let a = Accent::from(Some(user));
    let (_, c, h) = oklch(user);
    let c = if c < ACCENT_GREY_C {
        0.0
    } else {
        c.min(C_DEEP)
    };
    Roles {
        base: a.base,
        ink: a.ink,
        text: a.text,
        fill: a.fill,
        soft: a.soft,
        deep: deep(c, allowed_hue(h)),
    }
}

/// Whether the strip moves this colour's hue: it sat in the band kept
/// for warning amber and alarm red.
pub fn snapped(user: Rgb) -> bool {
    let (_, c, h) = oklch(user);
    c >= ACCENT_GREY_C && allowed_hue(h) != h
}

/// The darkest light-theme surface accent words and rings sit on
/// (theme.slint's surface-2).
const LIGHT_SURFACE_2: Rgb = Rgb::hex(0xece2d3);

/// `deep` at L_DEEP, or darker for hues (greens, yellows) whose luminance
/// runs high at that lightness, until words read at 4.5:1 on every light
/// surface.
fn deep(c: f32, h: f32) -> Rgb {
    let mut l = L_DEEP;
    loop {
        let rgb = from_oklch(l, c, h);
        if contrast(rgb, LIGHT_SURFACE_2) >= 4.5 || l <= 0.2 {
            return rgb;
        }
        l -= 0.01;
    }
}

/// WCAG contrast of two colours.
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let y = |c: Rgb| 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
    let (hi, lo) = (y(a).max(y(b)), y(a).min(y(b)));
    (hi + 0.05) / (lo + 0.05)
}

fn lin(u: u8) -> f32 {
    let c = f32::from(u) / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
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

    /// Accent words and focus rings: `deep` on the light theme's three
    /// surfaces (theme.slint) and `base` on the dark theme's, at 4.5:1 or
    /// better for every hue, not only the presets.
    #[test]
    fn accent_words_and_rings_read_on_both_themes() {
        let light = [0xfffaf3, 0xf3ebdf, 0xece2d3].map(Rgb::hex);
        let dark = [0x1c1815, 0x251f1b, 0x2f2823].map(Rgb::hex);
        let hues = (0..24).map(|i| from_oklch(0.7, 0.15, i as f32 * 15.0));
        for user in hues.chain(PRESETS.map(|(_, c)| c)).chain([DEFAULT]) {
            let r = roles(user);
            for s in light {
                assert!(contrast(r.deep, s) >= 4.5, "{user:?} deep on {s:?}");
            }
            for s in dark {
                assert!(contrast(r.base, s) >= 4.5, "{user:?} base on {s:?}");
            }
        }
    }

    /// The light theme's words (theme.slint text, text-2, text-3, warn,
    /// crit) on its surfaces.
    #[test]
    fn light_theme_words_read_on_cream() {
        let surfaces = [0xfffaf3, 0xf3ebdf, 0xece2d3].map(Rgb::hex);
        for word in [0x2a211b, 0x5b4b3f, 0x76624f, 0x8a5600, 0xb3261e].map(Rgb::hex) {
            for s in surfaces {
                assert!(contrast(word, s) >= 4.5, "{word:?} on {s:?}");
            }
        }
    }

    /// Disabled words (theme.slint text-off) at 3:1 or better on every
    /// surface, in both themes.
    #[test]
    fn disabled_words_read_at_three_to_one() {
        let light = [0xf3ebdf, 0xfffaf3, 0xece2d3].map(Rgb::hex);
        let dark = [0x1c1815, 0x251f1b, 0x2f2823].map(Rgb::hex);
        for s in light {
            assert!(contrast(Rgb::hex(0x8c7a68), s) >= 3.0, "{s:?}");
        }
        for s in dark {
            assert!(contrast(Rgb::hex(0x857669), s) >= 3.0, "{s:?}");
        }
    }

    #[test]
    fn a_grey_pick_stays_grey() {
        let r = roles(Rgb::hex(0x808080));
        assert!(r.base.r.abs_diff(r.base.g) <= 1 && r.base.g.abs_diff(r.base.b) <= 1);
    }

    #[test]
    fn hues_near_amber_and_red_are_snapped_as_the_strip_snaps_them() {
        let marigold = Rgb::hex(0xffc53a);
        assert!(snapped(marigold));
        assert!(snapped(Rgb::hex(0xff9a62))); // apricot
        assert!(!snapped(Rgb::hex(0x808080)));
        assert_eq!(roles(marigold).base, Accent::from(Some(marigold)).base);
        // The preview shows the snapped hue, not the picked one.
        let (_, _, h) = oklch(roles(marigold).base);
        assert!(!(16.0..104.0).contains(&h), "{h}");
        for (name, c) in PRESETS {
            assert!(!snapped(c), "{name}");
        }
    }
}
