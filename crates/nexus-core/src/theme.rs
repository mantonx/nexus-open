//! Design tokens: every colour, size, spacing and timing the strip is
//! drawn with, in one place. `DESIGN.md` at the repository root names the
//! same constants and says why each is what it is.
//!
//! All sizes are provisional: the panel has not been measured yet (the
//! legibility card of `docs/DESIGN_PLAN.md`, U0). When it has, change the
//! numbers here and nowhere else; no widget code holds a size of its own.

/// An opaque sRGB colour.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

    /// This colour moved `t` (0–1) of the way to `other`, in sRGB.
    pub fn mix(self, other: Rgb, t: f32) -> Rgb {
        let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
        Rgb {
            r: m(self.r, other.r),
            g: m(self.g, other.g),
            b: m(self.b, other.b),
        }
    }

    /// Every channel scaled by `k` (0–1), as a dimmed backlight shows it.
    pub fn scaled(self, k: f32) -> Rgb {
        let f = |v: u8| (f32::from(v) * k).round() as u8;
        Rgb {
            r: f(self.r),
            g: f(self.g),
            b: f(self.b),
        }
    }
}

// ---------------------------------------------------------------- colour

/// Neutrals: warm and brown-black, never blue (OKLCH hue about 60).
pub const GROUND: Rgb = Rgb::hex(0x14110f);
pub const CARD: Rgb = Rgb::hex(0x241e1a);
/// Raised key faces: two steps lighter than a card, so a key never looks
/// like a reading.
pub const KEY: Rgb = Rgb::hex(0x4a3e35);
/// The 1 px lit top edge of a key.
pub const KEY_TOP: Rgb = Rgb::hex(0x65564a);
/// The empty part of bars, sliders and progress lines.
pub const TRACK: Rgb = Rgb::hex(0x3d342d);
/// Scale ticks under a hero slider.
pub const TICK: Rgb = Rgb::hex(0x7a6b5e);
/// Numerals and titles: paper cream.
pub const TEXT: Rgb = Rgb::hex(0xfbf1e3);
/// Second lines, labels, data icons.
pub const TEXT2: Rgb = Rgb::hex(0xd2c2ad);
/// Quiet words, "no data" dashes, lost readings.
pub const TEXT3: Rgb = Rgb::hex(0x9c8c7c);
/// A bar's fill when nothing is wrong.
pub const FILL: Rgb = Rgb::hex(0xe6d6c0);

/// Severity. Fixed: never taken from the accent.
pub const WARN: Rgb = Rgb::hex(0xffb23e);
pub const WARN_SURFACE: Rgb = Rgb::hex(0x4f3612);
pub const WARN_ZONE: Rgb = Rgb::hex(0x6b4a1d);
pub const CRIT: Rgb = Rgb::hex(0xd93425);
pub const CRIT_INK: Rgb = Rgb::hex(0xffffff);
pub const CRIT_DEEP: Rgb = Rgb::hex(0x8a1d14);
pub const CRIT_ZONE: Rgb = Rgb::hex(0x6e2a22);

/// The surface of a takeover that covers part of a page: a step above a
/// card, so it reads as on top.
pub const RAISED: Rgb = Rgb::hex(0x342b25);

// ---------------------------------------------------------------- the accent

/// The accent's roles, derived from the user's colour by `Accent::from`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Accent {
    /// Key faces, slider thumbs, the dashed "set up" outline, the badge.
    pub base: Rgb,
    /// Glyphs and text drawn on `base`.
    pub ink: Rgb,
    /// Accent words and icons on a card.
    pub text: Rgb,
    /// Slider fill: a step darker, so a long fill stays calm.
    pub fill: Rgb,
    /// Pressed key faces, the halo round a held thumb.
    pub soft: Rgb,
}

/// The accent when the user has chosen none: lilac.
pub const ACCENT_DEFAULT: Rgb = Rgb::hex(0xb6a2ff);

/// Accents by name, as the `accent` setting accepts them. Every one is
/// outside the band `ACCENT_AVOID` keeps clear of amber and red.
pub const ACCENT_PRESETS: [(&str, Rgb); 6] = [
    ("lilac", ACCENT_DEFAULT),
    ("sky", Rgb::hex(0x5fb4ff)),
    ("teal", Rgb::hex(0x4fd1c5)),
    ("mint", Rgb::hex(0x7ad9b0)),
    ("rose", Rgb::hex(0xff8fb1)),
    ("lemon", Rgb::hex(0xd8e04a)),
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
/// Hues (OKLCH degrees) an accent may not take: from a little below CRIT's
/// hue (30) to well past WARN's (73). A colour picked inside is moved to the
/// nearer end, so no accent can be read as a warning or an alarm. The top
/// sits 32° past WARN: at 98° a snapped marigold still read as amber's
/// mustard cousin beside a warning card; lemon (112°) stays outside.
pub const ACCENT_AVOID: (f32, f32) = (15.0, 105.0);

impl Accent {
    /// The roles for the colour the user picked (`None`: the default).
    pub fn from(user: Option<Rgb>) -> Self {
        let (_, c, h) = oklch(user.unwrap_or(ACCENT_DEFAULT));
        let h = allowed_hue(h);
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

/// `h` moved out of `ACCENT_AVOID` to its nearer end.
pub fn allowed_hue(h: f32) -> f32 {
    let (lo, hi) = ACCENT_AVOID;
    if h > lo && h < hi {
        if h - lo <= hi - h { lo } else { hi }
    } else {
        h
    }
}

/// The preset with this name.
pub fn accent_preset(name: &str) -> Option<Rgb> {
    ACCENT_PRESETS
        .iter()
        .find(|(n, _)| n.eq_ignore_ascii_case(name.trim()))
        .map(|(_, c)| *c)
}

/// The name of the preset that is exactly this colour.
pub fn accent_preset_name(c: Rgb) -> Option<&'static str> {
    ACCENT_PRESETS
        .iter()
        .find(|(_, p)| *p == c)
        .map(|(n, _)| *n)
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

/// WCAG contrast ratio of two colours.
pub fn contrast(a: Rgb, b: Rgb) -> f32 {
    let y = |c: Rgb| 0.2126 * lin(c.r) + 0.7152 * lin(c.g) + 0.0722 * lin(c.b);
    let (ya, yb) = (y(a), y(b));
    (ya.max(yb) + 0.05) / (ya.min(yb) + 0.05)
}

// ---------------------------------------------------------------- type

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

const fn style(size: f32, weight: f32, tracking: f32) -> TextStyle {
    TextStyle {
        size,
        weight,
        width: 100.0,
        tabular: true,
        tracking,
    }
}

/// Weights: numerals and titles, words on keys and labels, body text.
pub const W_NUM: f32 = 600.0;
pub const W_TITLE: f32 = 560.0;
pub const W_BODY: f32 = 480.0;

/// Hero numerals, the alarm's reading.
pub const NUM_HERO: TextStyle = style(50.0, W_NUM, -0.5);
/// Medium numerals with no bar below (clock, weather), weather at compact.
pub const NUM_MEDIUM: TextStyle = style(40.0, W_NUM, -0.4);
/// Hero numerals above a bar band (a reading's card): a step under
/// `NUM_HERO`, so the bar can run the whole card under the figures.
pub const NUM_HERO_BANDED: TextStyle = style(40.0, W_NUM, -0.4);
/// Medium numerals above a bar band.
pub const NUM_BANDED: TextStyle = style(36.0, W_NUM, -0.4);
/// Compact numerals; the medium slider's number.
pub const NUM_COMPACT: TextStyle = style(32.0, W_NUM, -0.3);
/// A compact numeral that would not fit at `NUM_COMPACT` (three digits in
/// a narrow card) steps down to this before anything is cut.
pub const NUM_COMPACT_TIGHT: TextStyle = style(28.0, W_NUM, -0.3);
/// A second reading beside the first; the upload rate.
pub const NUM_SECOND: TextStyle = style(26.0, W_NUM, -0.2);
/// A second reading in a medium value card too narrow for `NUM_SECOND`
/// beside the widest first reading and the second's icon.
pub const NUM_SECOND_TIGHT: TextStyle = style(22.0, W_NUM, -0.2);
/// The step under that: a 200 px card with "199°" and "100%" (both
/// three digits) and the second's icon. Never under `TEXT_FLOOR`.
pub const NUM_SECOND_SMALL: TextStyle = style(20.0, W_NUM, -0.2);
/// The track title in the now-playing takeover.
pub const TITLE_BIG: TextStyle = style(26.0, W_TITLE, -0.2);
/// "%" after a hero numeral.
pub const UNIT_HERO: TextStyle = style(22.0, W_TITLE, 0.0);
/// The alarm's OK.
pub const OK_LABEL: TextStyle = style(22.0, W_NUM, 0.5);
/// Track title, weekday, weather condition, alarm headline, compact
/// slider number.
pub const TITLE: TextStyle = style(20.0, W_TITLE, 0.0);
/// Key labels; the short words on compact "set up" cards.
pub const KEY_LABEL: TextStyle = style(18.0, W_TITLE, 0.0);
/// "%" and rates' units after a numeral, on its baseline.
pub const UNIT: TextStyle = style(18.0, W_TITLE, 0.0);
/// Artist, date, second lines, hints.
pub const BODY: TextStyle = style(18.0, W_BODY, 0.1);
/// Reading names, times, "limit 167°", "Muted". The smallest text.
pub const LABEL: TextStyle = style(18.0, W_TITLE, 0.2);
/// Nothing on the strip is smaller than this. Provisional, like every
/// size: 18 px (2.5 mm caps) is the review's ask for 60 cm, not yet read
/// on the panel (U0's legibility card).
pub const TEXT_FLOOR: f32 = 18.0;

/// Every text style, for checks such as the floor.
pub const TEXT_STYLES: [TextStyle; 17] = [
    NUM_HERO,
    NUM_MEDIUM,
    NUM_HERO_BANDED,
    NUM_BANDED,
    NUM_COMPACT,
    NUM_COMPACT_TIGHT,
    NUM_SECOND,
    NUM_SECOND_TIGHT,
    NUM_SECOND_SMALL,
    TITLE_BIG,
    UNIT_HERO,
    OK_LABEL,
    TITLE,
    KEY_LABEL,
    UNIT,
    BODY,
    LABEL,
];

/// Icons (Phosphor), by the em size of the glyph.
pub const ICON_HERO: f32 = 30.0;
pub const ICON_MEDIUM: f32 = 26.0;
pub const ICON_COMPACT: f32 = 22.0;
pub const ICON_KEY: f32 = 22.0;
pub const ICON_SMALL: f32 = 17.0;
/// The play/pause glyph on the badge of a medium media card.
pub const ICON_BADGE: f32 = 10.0;

// ---------------------------------------------------------------- geometry

/// Rows and columns the bezel or the viewing angle may hide. Nothing
/// essential is drawn in them; card fills may run into them.
pub const SAFE: f32 = 2.0;
/// Ground between a card and the edge of its zone. A layout's widths are
/// zones that add up to 640; each card is its zone less this on both
/// sides, which leaves `GUTTER` between two cards and `EDGE` at the ends.
pub const CARD_INSET: f32 = 2.0;
pub const EDGE: f32 = CARD_INSET;
pub const GUTTER: f32 = 2.0 * CARD_INSET;
/// Cards: top edge, height and corner radius. The rows above hold the
/// page markers.
pub const CARD_Y: f32 = 6.0;
pub const CARD_H: f32 = 41.0;
pub const CARD_R: f32 = 10.0;
/// Inner padding at a card's left and right.
pub const PAD: f32 = 9.0;
/// Between an icon and the numeral it labels.
pub const ICON_GAP: f32 = 6.0;
/// Between a numeral and its smaller unit.
pub const UNIT_GAP: f32 = 2.0;

/// Size classes, by the widget's width (its zone, as the layout gives it):
/// compact up to `COMPACT_MAX`, medium up to `MEDIUM_MAX`, hero above.
pub const COMPACT_MAX: u32 = 149;
pub const MEDIUM_MAX: u32 = 299;

/// Baselines inside a card, in strip rows.
pub const BASE_HERO: f32 = 44.0;
pub const BASE_MEDIUM: f32 = 41.0;
pub const BASE_COMPACT: f32 = 36.0;
pub const BASE_LINE1: f32 = 24.0;
pub const BASE_LINE2: f32 = 42.0;
/// A news headline over two lines at `LABEL` size: an 18 px pitch, the
/// pair centred on the card.
pub const NEWS_BASE1: f32 = 23.5;
pub const NEWS_BASE2: f32 = 41.5;
/// A headline on one line with nothing under it, centred on the card.
pub const NEWS_BASE_ONE: f32 = 33.0;
/// Between a headline's end and the newspaper and "Source · 12 min".
pub const NEWS_FROM_GAP: f32 = 12.0;
/// A hero reading's name line (its limit sits on the numeral's baseline).
pub const BASE_HERO_NAME: f32 = 20.0;

/// The fixed-scale bar's band, under the numeral, at every size: it runs
/// the card's width (less a second reading's share at medium).
pub const BAR_Y_COMPACT: f32 = 40.0;
pub const BAR_H_BANDED: f32 = 5.0;
/// Notches at the limits stick out this far above and below the track.
pub const NOTCH_OUT: f32 = 2.0;
pub const NOTCH_W: f32 = 2.0;
/// A media widget's progress line.
pub const PROGRESS_H: f32 = 3.0;

/// The smallest width of anything a finger is meant to hit: 44 px is
/// 8.7 mm on this panel (about 128 px per inch).
pub const TARGET_MIN: f32 = 44.0;
/// Keys inside a card (transport keys).
pub const KEY_H: f32 = 33.0;
pub const KEY_R: f32 = 9.0;
pub const KEY_Y: f32 = CARD_Y + (CARD_H - KEY_H) / 2.0;
pub const KEY_GAP: f32 = 4.0;
/// The raised look: a lighter face over a 1 px lighter top edge.
pub const KEY_LIFT: f32 = 1.0;
/// A key widget this wide or wider shows its label beside its icon.
pub const KEY_LABEL_MIN: u32 = 96;
/// A media widget this wide shows prev and next beside play/pause.
pub const MEDIA_KEYS_MIN: u32 = 400;
/// A media widget this wide is the media scene: big title, bigger keys.
pub const MEDIA_SCENE_MIN: u32 = 560;
/// Transport keys in the media scene.
pub const SCENE_KEY_W: f32 = 50.0;
/// The play/pause key drawn on a medium media card (the whole card is the
/// hit zone; this is what says so).
pub const MEDIA_KEY_W: f32 = 36.0;
/// Artwork: a rounded square.
pub const ART: f32 = 33.0;
pub const ART_SCENE: f32 = 35.0;
pub const ART_R: f32 = 7.0;
/// How far the hold fill mixes a face toward the accent.
pub const HOLD_MIX: f32 = 0.45;
/// The rim of anything pressed.
pub const PRESS_RIM: f32 = 1.5;
/// Slider thumb radius at rest, while held, and its halo.
pub const THUMB_R: f32 = 7.5;
pub const THUMB_R_HELD: f32 = 10.0;
pub const THUMB_HALO: f32 = 19.0;
pub const SLIDER_TRACK_H: f32 = 8.0;
/// Dashes of the "not set up" outline: on, off; and its line width.
pub const DASH: (f32, f32) = (5.0, 4.0);
pub const DASH_W: f32 = 1.5;
/// A title that does not fit fades out over this many pixels.
pub const FADE: f32 = 18.0;
/// The ground rim round a partial takeover, so it reads as on top.
pub const TAKEOVER_RIM: f32 = 3.0;

/// The alarm's OK key: its size and its distance from the strip's right
/// end. It is the only thing on the alarm a finger can hit.
pub const ALARM_OK_W: f32 = 84.0;
pub const ALARM_OK_H: f32 = 36.0;
pub const ALARM_OK_RIGHT: f32 = 10.0;

/// Page markers: small pills above the cards, centred.
pub const MARKER_Y: f32 = SAFE;
pub const MARKER_H: f32 = 2.0;
pub const MARKER_W: f32 = 14.0;
pub const MARKER_W_CURRENT: f32 = 26.0;
pub const MARKER_GAP: f32 = 6.0;
/// A page with a warning or critical reading gets a longer, taller marker
/// (rows 2–4, still one clear row above the cards).
pub const MARKER_W_ALERT: f32 = 16.0;
pub const MARKER_H_ALERT: f32 = 3.0;
/// On the current page's marker the alert is a separate segment after the
/// cream, this far from it.
pub const MARKER_SPLIT: f32 = 2.0;
/// Trouble on a page to one side of the one shown: a pill standing at
/// that end of the strip, in the warning's or the alarm's colour. It sits
/// over the strip's edge and the end card's padding (content starts
/// `PAD` in), never over a widget's content.
pub const EDGE_FLAG_W: f32 = 4.0;
pub const EDGE_FLAG_Y: f32 = 13.0;
pub const EDGE_FLAG_H: f32 = 28.0;

// ---------------------------------------------------------------- scales

/// Temperatures share one fixed scale, in °C (86–212 °F).
pub const TEMP_LO: f64 = 30.0;
pub const TEMP_HI: f64 = 100.0;
/// Network rates: logarithmic, 1 kB/s to 1 GB/s, in bytes per second.
pub const NET_LO: f64 = 1_000.0;
pub const NET_HI: f64 = 1_000_000_000.0;

// ---------------------------------------------------------------- motion

/// In milliseconds.
pub const MS_PRESS: u32 = 60;
pub const MS_HOLD: u32 = 500;
pub const MS_TAKEOVER_IN: u32 = 220;
pub const MS_TAKEOVER_OUT: u32 = 320;
pub const MS_ALARM_BREATHE: u32 = 2400;
pub const MS_MARQUEE_REST: u32 = 2500;
/// How far a takeover rises as it fades in, in pixels.
pub const TAKEOVER_RISE: f32 = 3.0;
/// The alarm's red eases down to this brightness and back.
pub const ALARM_BREATHE_LOW: f32 = 0.88;
pub const MARQUEE_PX_PER_S: f32 = 28.0;
/// Idle dimming: after this long with no touch and nothing at warning.
pub const IDLE_DIM_AFTER_S: u32 = 300;
/// Brightness kept when idle, and at night: what the ground and the cards
/// keep. Figures keep more (`DIM_FIGURES`), so a dimmed live number never
/// sinks to a lost one's grey.
pub const IDLE_DIM: f32 = 0.6;
pub const NIGHT_DIM: f32 = 0.35;
/// How much of the dimming the brightest pixels (figures, cream text) are
/// spared: they lose this share of what the surfaces lose. At night the
/// surfaces keep 35 % and cream figures about 68 %; pixels between are
/// eased by their lightness, so the order of tones never changes.
pub const DIM_FIGURES: f32 = 0.5;

/// A colour as a frame dimmed to `k` shows it: dark surfaces keep `k`,
/// light figures keep more (`DIM_FIGURES`), by the colour's lightness.
pub fn dimmed(c: Rgb, k: f32) -> Rgb {
    if k >= 1.0 {
        return c;
    }
    let k = k.max(0.0);
    let hi = 1.0 - (1.0 - k) * DIM_FIGURES;
    let luma =
        (0.2126 * f32::from(c.r) + 0.7152 * f32::from(c.g) + 0.0722 * f32::from(c.b)) / 255.0;
    // Ground and cards (luma under 0.15) take the full dimming; cream
    // (over 0.75) the least.
    let t = ((luma - 0.15) / 0.6).clamp(0.0, 1.0);
    let t = t * t * (3.0 - 2.0 * t);
    c.scaled(k + (hi - k) * t)
}

/// What frames are drawn with: the accent roles from the user's colour.
#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    /// The accent colour the user chose (`settings.accent`), or `None` for
    /// the default.
    pub accent: Option<Rgb>,
    /// The roles drawn with, from `accent`.
    pub roles: Accent,
}

impl Theme {
    pub fn new(accent: Option<Rgb>) -> Self {
        Self {
            accent,
            roles: Accent::from(accent),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::new(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_default_accent_is_lilac_as_designed() {
        let a = Accent::from(None);
        assert_eq!(a.base, Rgb::hex(0xc0b0fe));
        assert_eq!(a.text, Rgb::hex(0xccc1fe));
        assert_eq!(a.fill, Rgb::hex(0x9583d5));
        assert_eq!(a.soft, Rgb::hex(0x443b64));
        assert_eq!(a.ink, GROUND);
    }

    #[test]
    fn any_accent_keeps_its_lightness_and_stays_off_amber_and_red() {
        for user in [
            Rgb::hex(0xffc53a), // marigold: amber's neighbour
            Rgb::hex(0xff9a62), // apricot
            Rgb::hex(0xff2010), // red
            Rgb::hex(0x00ff00),
            Rgb::hex(0x0000ff),
        ] {
            let a = Accent::from(Some(user));
            let (l, _, h) = oklch(a.base);
            assert!((l - ACCENT_L).abs() < 0.01, "{user:?}: L {l}");
            assert!(
                !(ACCENT_AVOID.0 + 0.5..ACCENT_AVOID.1 - 0.5).contains(&h),
                "{user:?}: hue {h}"
            );
            assert!(contrast(a.ink, a.base) > 7.0);
        }
        assert_eq!(allowed_hue(80.0), ACCENT_AVOID.1);
        assert_eq!(allowed_hue(40.0), ACCENT_AVOID.0);
        assert_eq!(allowed_hue(290.0), 290.0);
    }

    #[test]
    fn a_grey_pick_gives_grey_roles() {
        let a = Accent::from(Some(Rgb::hex(0x808080)));
        let (_, c, _) = oklch(a.base);
        assert!(c < 0.01, "{c}");
    }

    #[test]
    fn presets_are_named_and_allowed() {
        assert_eq!(accent_preset("Lilac"), Some(ACCENT_DEFAULT));
        assert_eq!(accent_preset("nope"), None);
        for (name, c) in ACCENT_PRESETS {
            assert_eq!(accent_preset_name(c), Some(name));
            let h = oklch(c).2;
            assert_eq!(allowed_hue(h), h, "{name} sits in the avoided band");
        }
    }

    #[test]
    fn dimmed_live_figures_stay_above_lost_ones() {
        for k in [NIGHT_DIM, IDLE_DIM] {
            let live = contrast(dimmed(TEXT, k), dimmed(CARD, k));
            // A lost reading is TEXT3: undimmed by day, dimmed beside it.
            assert!(live > contrast(TEXT3, CARD), "{k}: {live}");
            let lost = contrast(dimmed(TEXT3, k), dimmed(CARD, k));
            assert!(live > lost * 1.5, "{k}: {live} vs {lost}");
            // The ground and cards lose more than the figures.
            assert!(dimmed(CARD, k).r < CARD.scaled(k).r + 1);
            assert!(dimmed(TEXT, k).r > TEXT.scaled(k).r);
        }
        assert_eq!(dimmed(TEXT, 1.0), TEXT);
    }

    #[test]
    fn no_text_is_below_the_floor() {
        for s in TEXT_STYLES {
            assert!(s.size >= TEXT_FLOOR, "{s:?}");
        }
    }
}
