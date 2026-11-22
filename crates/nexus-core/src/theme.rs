//! Design tokens: colours, the type scale and the layout grid.
//!
//! Values come from `design/tokens.json`, which the Go renderer also used.
//! Every widget reads from here; nothing hardcodes a colour or a size.

/// An opaque sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// Builds a colour from `0xRRGGBB`.
    pub const fn hex(v: u32) -> Self {
        Self {
            r: (v >> 16) as u8,
            g: (v >> 8) as u8,
            b: v as u8,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Palette {
    pub background: Rgb,
    pub divider: Rgb,
    pub label: Rgb,
    pub value: Rgb,
    pub unit: Rgb,
    pub info: Rgb,
    pub info_area: Rgb,
    /// Behind a zone while a finger rests on it and a tap would act.
    pub pressed: Rgb,
    pub ok: Rgb,
    pub warn: Rgb,
    pub crit: Rgb,
}

/// One text style: a size in pixels, a weight, a width, the kind of figures
/// and extra space between letters.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    /// CSS-style weight (400 regular, 500 medium). Only applies to variable fonts.
    pub weight: f32,
    /// Value on the font's `wdth` axis, in percent of normal: 100 is normal,
    /// 87.5 semi-condensed, 75 condensed (the CSS `font-width` names). Only applies to fonts with a width axis (see
    /// `Font::width_range`); others draw at their one width. Values outside
    /// a font's range are clamped to it.
    pub width: f32,
    /// Tabular figures (OpenType `tnum`): every digit has the same advance,
    /// so a changing number does not shift sideways. Fonts whose digits are
    /// proportional and lack `tnum` keep proportional digits (see
    /// `Font::has_tabular_figures`).
    pub tabular: bool,
    /// Extra pixels added after every character.
    pub tracking: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct TypeScale {
    pub label: TextStyle,
    pub value: TextStyle,
    pub unit: TextStyle,
    pub caption: TextStyle,
    pub icon: TextStyle,
}

/// Vertical rhythm and spacing. All values are in panel pixels.
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    pub pad_left: f32,
    pub label_baseline: f32,
    pub caption_baseline: f32,
    pub value_baseline: f32,
    pub divider_top: u32,
    pub divider_bottom: u32,
    /// Gap between a value and its unit.
    pub unit_gap: f32,
    /// Graphs occupy the band from this y to the bottom edge.
    pub graph_top: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub palette: Palette,
    pub type_scale: TypeScale,
    pub grid: Grid,
    /// The accent colour the user chose (`settings.accent`), or `None` for
    /// the default. Nothing draws with it yet.
    pub accent: Option<Rgb>,
}

impl Default for Theme {
    fn default() -> Self {
        let style = |size, tracking| TextStyle {
            size,
            weight: 400.0,
            width: 100.0,
            tabular: false,
            tracking,
        };
        Self {
            palette: Palette {
                background: Rgb::hex(0x050505),
                divider: Rgb::hex(0x222222),
                label: Rgb::hex(0xb8b8b8),
                value: Rgb::hex(0xececec),
                unit: Rgb::hex(0x8a8a8a),
                info: Rgb::hex(0x5aa0e0),
                info_area: Rgb::hex(0x16344a),
                pressed: Rgb::hex(0x1b2633),
                ok: Rgb::hex(0x79c267),
                warn: Rgb::hex(0xf0a830),
                crit: Rgb::hex(0xe2625f),
            },
            type_scale: TypeScale {
                label: style(10.0, 1.0),
                value: style(22.0, 0.0),
                unit: style(13.0, 0.0),
                caption: style(11.0, 0.0),
                icon: style(18.0, 0.0),
            },
            grid: Grid {
                pad_left: 8.0,
                label_baseline: 15.0,
                caption_baseline: 28.0,
                value_baseline: 38.0,
                divider_top: 9,
                divider_bottom: 39,
                unit_gap: 2.0,
                graph_top: 30.0,
            },
            accent: None,
        }
    }
}
