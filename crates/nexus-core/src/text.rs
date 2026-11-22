//! Text shaping and hinted glyph rendering.
//!
//! Small text on a 48-pixel panel only looks sharp when glyph outlines are
//! hinted (snapped to the pixel grid by the font's own instructions) and every
//! glyph starts on a whole pixel. So this module:
//!
//! - splits text into runs by which font in the stack has each character,
//! - shapes each run with swash (kerning, ligatures, and tabular figures
//!   when the style asks), at the style's weight and width,
//! - rounds each glyph's pen position to a whole pixel,
//! - rasterizes glyphs with hinting on, and caches the masks.

use std::collections::HashMap;
use std::sync::Arc;

use swash::scale::{Render, ScaleContext, Source};
use swash::shape::ShapeContext;
use swash::zeno::Format;
use swash::{CacheKey, FontRef, Tag, tag_from_bytes};

use crate::canvas::Canvas;
use crate::theme::{Rgb, TextStyle};

/// A loaded font file.
///
/// `FontRef` from swash borrows the font bytes, so it can't be stored on its
/// own. `Font` holds the bytes and hands out a `FontRef` on demand. Cloning a
/// `Font` is cheap either way: bundled fonts point at bytes compiled into the
/// binary, and loaded files sit behind an `Arc`.
#[derive(Clone)]
pub struct Font {
    data: FontData,
    offset: u32,
    key: CacheKey,
}

#[derive(Clone)]
enum FontData {
    /// Compiled into the binary with `include_bytes!`; never copied.
    Static(&'static [u8]),
    /// Read at runtime, shared between clones.
    Owned(Arc<Vec<u8>>),
}

impl FontData {
    fn bytes(&self) -> &[u8] {
        match self {
            FontData::Static(b) => b,
            FontData::Owned(b) => b,
        }
    }
}

impl Font {
    /// Parses the first face in a font file read at runtime. Returns `None` if
    /// the bytes are not a font swash can read.
    pub fn from_bytes(data: Vec<u8>) -> Option<Self> {
        Self::new(FontData::Owned(Arc::new(data)))
    }

    /// Like `from_bytes`, for font bytes compiled into the binary.
    pub fn from_static(data: &'static [u8]) -> Option<Self> {
        Self::new(FontData::Static(data))
    }

    fn new(data: FontData) -> Option<Self> {
        let font = FontRef::from_index(data.bytes(), 0)?;
        let (offset, key) = (font.offset, font.key);
        Some(Self { data, offset, key })
    }

    /// True if the font has a glyph for `c`.
    pub fn covers(&self, c: char) -> bool {
        self.as_ref().charmap().map(c) != 0
    }

    /// The lowest and highest `TextStyle::width` this font can draw, or
    /// `None` if it has no width axis and so draws every width as normal.
    pub fn width_range(&self) -> Option<(f32, f32)> {
        self.as_ref()
            .variations()
            .find_by_tag(WDTH)
            .map(|axis| (axis.min_value(), axis.max_value()))
    }

    /// True if `TextStyle::tabular` gives this font's digits 0–9 one shared
    /// advance: either the font has the `tnum` feature, or its digits are
    /// tabular already. False means tabular figures fall back to the
    /// font's proportional ones, or that the font has no digits.
    pub fn has_tabular_figures(&self) -> bool {
        if !('0'..='9').all(|c| self.covers(c)) {
            return false;
        }
        let font = self.as_ref();
        if font.features().any(|f| f.tag() == TNUM) {
            return true;
        }
        let (charmap, metrics) = (font.charmap(), font.glyph_metrics(&[]));
        let mut advances = ('0'..='9').map(|c| metrics.advance_width(charmap.map(c)));
        let first = advances.next();
        advances.all(|a| Some(a) == first)
    }

    fn as_ref(&self) -> FontRef<'_> {
        FontRef {
            data: self.data.bytes(),
            offset: self.offset,
            key: self.key,
        }
    }
}

const WDTH: Tag = tag_from_bytes(b"wdth");
const TNUM: Tag = tag_from_bytes(b"tnum");

/// Identifies one rasterized glyph in the cache. Tabular figures need no
/// field: `tnum` picks other glyph ids, which the key already has.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font: u64,
    glyph: u16,
    // f32 can't be hashed, so sizes are stored by their bit pattern.
    size: u32,
    weight: u32,
    width: u32,
}

struct GlyphMask {
    left: i32,
    top: i32,
    width: u32,
    height: u32,
    alpha: Vec<u8>,
}

/// One glyph after shaping, positioned relative to the start of the text.
struct Placed {
    /// Index into the font stack.
    font: usize,
    glyph: u16,
    x: i32,
}

/// Shapes and draws text. Holds scratch state and the glyph cache, so keep one
/// per renderer rather than creating it per frame.
#[derive(Default)]
pub struct TextEngine {
    shaper: ShapeContext,
    scaler: ScaleContext,
    cache: HashMap<GlyphKey, Option<GlyphMask>>,
}

impl TextEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Width in pixels that `text` occupies in `style`, including tracking,
    /// width and tabular figures: always the advance `draw` returns.
    ///
    /// `fonts` is a fallback stack: each character uses the first font that
    /// has a glyph for it, or the first font if none do.
    pub fn measure(&mut self, fonts: &[Font], style: &TextStyle, text: &str) -> f32 {
        let (_, width) = self.layout(fonts, style, text);
        width as f32
    }

    /// Draws `text` with its left edge and baseline at `at` = (x, baseline).
    /// Returns the width drawn, in pixels. See `measure` for `fonts`.
    pub fn draw(
        &mut self,
        canvas: &mut Canvas,
        fonts: &[Font],
        style: &TextStyle,
        text: &str,
        at: (f32, f32),
        color: Rgb,
    ) -> f32 {
        let (x, baseline) = at;
        let (glyphs, width) = self.layout(fonts, style, text);
        let origin_x = x.round() as i32;
        let origin_y = baseline.round() as i32;
        for g in glyphs {
            let font = &fonts[g.font];
            let key = GlyphKey {
                font: font.key.value(),
                glyph: g.glyph,
                size: style.size.to_bits(),
                weight: style.weight.to_bits(),
                width: style.width.to_bits(),
            };
            if !self.cache.contains_key(&key) {
                let mask = self.rasterize(font, style, g.glyph);
                self.cache.insert(key, mask);
            }
            if let Some(Some(m)) = self.cache.get(&key) {
                canvas.blend_mask(
                    origin_x + g.x + m.left,
                    origin_y - m.top,
                    m.width,
                    m.height,
                    &m.alpha,
                    color,
                );
            }
        }
        width as f32
    }

    /// Shapes `text` and snaps every glyph to a whole pixel.
    /// Returns the placed glyphs and the total advance in pixels.
    fn layout(&mut self, fonts: &[Font], style: &TextStyle, text: &str) -> (Vec<Placed>, i32) {
        let mut placed = Vec::new();
        // The pen moves in fractional pixels so rounding errors don't add up
        // along a run; each glyph is then placed on the nearest whole pixel.
        let mut pen = 0.0f32;
        for (font, run) in runs(fonts, text) {
            let mut shaper = self
                .shaper
                .builder(fonts[font].as_ref())
                .size(style.size)
                .variations(&variations(style))
                .features(features(style))
                .build();
            shaper.add_str(run);
            shaper.shape_with(|cluster| {
                for g in cluster.glyphs {
                    placed.push(Placed {
                        font,
                        glyph: g.id,
                        x: (pen + g.x).round() as i32,
                    });
                    pen += g.advance + style.tracking;
                }
            });
        }
        // Tracking is space *between* letters, so none after the last one.
        if !placed.is_empty() {
            pen -= style.tracking;
        }
        (placed, pen.round() as i32)
    }

    fn rasterize(&mut self, font: &Font, style: &TextStyle, glyph: u16) -> Option<GlyphMask> {
        let mut scaler = self
            .scaler
            .builder(font.as_ref())
            .size(style.size)
            .hint(true)
            .variations(&variations(style))
            .build();
        let image = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .render(&mut scaler, glyph)?;
        Some(GlyphMask {
            left: image.placement.left,
            top: image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            alpha: image.data,
        })
    }
}

/// The axis settings for `style`. Fonts ignore axes they lack, and clamp
/// values to the range of those they have.
fn variations(style: &TextStyle) -> [(&'static str, f32); 2] {
    [("wght", style.weight), ("wdth", style.width)]
}

/// The OpenType features `style` turns on beyond the shaper's defaults.
/// Fonts ignore features they lack.
fn features(style: &TextStyle) -> &'static [(&'static str, u16)] {
    if style.tabular { &[("tnum", 1)] } else { &[] }
}

/// Splits `text` into runs that each use one font from the stack.
/// Yields `(font index, run)`.
fn runs<'t>(fonts: &[Font], text: &'t str) -> Vec<(usize, &'t str)> {
    let pick = |c: char| fonts.iter().position(|f| f.covers(c)).unwrap_or(0);
    let mut runs = Vec::new();
    let mut start = 0;
    let mut current = None;
    for (i, c) in text.char_indices() {
        // Spaces belong to whatever run they are in, so a fallback glyph
        // doesn't split a word's spacing between two fonts.
        let font = if c == ' ' {
            current.unwrap_or(0)
        } else {
            pick(c)
        };
        match current {
            Some(f) if f != font => {
                runs.push((f, &text[start..i]));
                start = i;
                current = Some(font);
            }
            None => current = Some(font),
            _ => {}
        }
    }
    if let Some(f) = current {
        runs.push((f, &text[start..]));
    }
    runs
}

#[cfg(test)]
mod tests {
    use super::*;

    // Hack lacks some symbols DejaVu Sans has, which makes the pair a real
    // fallback case without inventing font data. Tests skip if either is missing.
    fn fonts() -> Option<Vec<Font>> {
        let primary = std::fs::read("/usr/share/fonts/TTF/Hack-Regular.ttf").ok()?;
        let fallback = std::fs::read("/usr/share/fonts/TTF/DejaVuSans.ttf").ok()?;
        Some(vec![
            Font::from_bytes(primary)?,
            Font::from_bytes(fallback)?,
        ])
    }

    #[test]
    fn single_font_text_is_one_run() {
        let Some(fonts) = fonts() else { return };
        assert_eq!(runs(&fonts, "CPU 28"), vec![(0, "CPU 28")]);
    }

    #[test]
    fn uncovered_character_falls_back() {
        let Some(fonts) = fonts() else { return };
        // U+2603 SNOWMAN: in DejaVu Sans, not in Hack.
        assert!(!fonts[0].covers('☃') && fonts[1].covers('☃'));
        assert_eq!(runs(&fonts, "a☃b"), vec![(0, "a"), (1, "☃"), (0, "b")]);
    }

    #[test]
    fn space_stays_with_current_run() {
        let Some(fonts) = fonts() else { return };
        assert_eq!(runs(&fonts, "☃ ☃"), vec![(1, "☃ ☃")]);
    }

    #[test]
    fn character_no_font_covers_uses_primary() {
        let Some(fonts) = fonts() else { return };
        // A private-use codepoint neither font defines.
        assert_eq!(runs(&fonts, "\u{F8FF}"), vec![(0, "\u{F8FF}")]);
    }

    #[test]
    fn empty_text_has_no_runs() {
        let Some(fonts) = fonts() else { return };
        assert!(runs(&fonts, "").is_empty());
    }

    // Width and tabular figures, on the bundled faces.

    use crate::fonts::FontChoice;

    const BG: Rgb = Rgb::hex(0x000000);
    const INK: Rgb = Rgb::hex(0xffffff);

    fn style(width: f32, tabular: bool) -> TextStyle {
        TextStyle {
            size: 22.0,
            weight: 400.0,
            width,
            tabular,
            tracking: 0.0,
        }
    }

    fn text_font(choice: FontChoice) -> Vec<Font> {
        vec![choice.load().text]
    }

    /// Draws `text` at x = 10 on a wide canvas; returns the canvas and the
    /// advance `draw` reported.
    fn drawn(engine: &mut TextEngine, fonts: &[Font], s: &TextStyle, text: &str) -> (Canvas, f32) {
        let mut canvas = Canvas::new(400, 48, BG);
        let w = engine.draw(&mut canvas, fonts, s, text, (10.0, 36.0), INK);
        (canvas, w)
    }

    /// First and one-past-last column with any ink, if any.
    fn ink_columns(canvas: &Canvas) -> Option<(u32, u32)> {
        let width = canvas.width();
        let inked: Vec<u32> = (0..width)
            .filter(|&x| {
                (0..canvas.height()).any(|y| {
                    let i = ((y * width + x) * 4) as usize;
                    canvas.rgba()[i..i + 3] != [0, 0, 0]
                })
            })
            .collect();
        Some((*inked.first()?, inked.last()? + 1))
    }

    /// Unrounded advance of each digit 0–9, from the shaped run "0123456789".
    fn digit_advances(fonts: &[Font], s: &TextStyle) -> Vec<f32> {
        let mut engine = TextEngine::new();
        let mut shaper = engine
            .shaper
            .builder(fonts[0].as_ref())
            .size(s.size)
            .variations(&variations(s))
            .features(features(s))
            .build();
        shaper.add_str("0123456789");
        let mut advances = Vec::new();
        shaper.shape_with(|c| advances.extend(c.glyphs.iter().map(|g| g.advance)));
        advances
    }

    fn all_equal(v: &[f32]) -> bool {
        v.windows(2).all(|w| w[0] == w[1])
    }

    #[test]
    fn faces_report_width_axes() {
        let range = |c| text_font(c)[0].width_range();
        assert_eq!(range(FontChoice::IbmPlexSans), Some((75.0, 100.0)));
        assert_eq!(range(FontChoice::Roboto), Some((75.0, 100.0)));
        for c in [
            FontChoice::AtkinsonHyperlegible,
            FontChoice::NotoSans,
            FontChoice::Lato,
        ] {
            assert_eq!(range(c), None, "{c}");
        }
        let fonts = FontChoice::default().load();
        assert_eq!(fonts.fallback[0].width_range(), None);
        assert_eq!(fonts.icon.width_range(), None);
    }

    #[test]
    fn faces_report_tabular_figures() {
        // Every text choice and the fallback can set tabular figures; the
        // icon font's digit glyphs are pictures of differing widths.
        for c in FontChoice::ALL {
            assert!(text_font(c)[0].has_tabular_figures(), "{c}");
        }
        let fonts = FontChoice::default().load();
        assert!(fonts.fallback[0].has_tabular_figures());
        assert!(!fonts.icon.has_tabular_figures());
    }

    #[test]
    fn tabular_figures_share_one_advance() {
        // Atkinson Hyperlegible's digits are proportional unless `tnum` is on.
        let fonts = text_font(FontChoice::AtkinsonHyperlegible);
        assert!(!all_equal(&digit_advances(&fonts, &style(100.0, false))));
        let tabular = digit_advances(&fonts, &style(100.0, true));
        assert_eq!(tabular.len(), 10);
        assert!(all_equal(&tabular), "{tabular:?}");

        // Faces whose digits are tabular already keep them so, also when
        // condensed, with or without the flag.
        for c in FontChoice::ALL {
            let fonts = text_font(c);
            for width in [75.0, 87.5, 100.0] {
                let a = digit_advances(&fonts, &style(width, true));
                assert!(all_equal(&a), "{c} at {width}: {a:?}");
            }
        }
    }

    #[test]
    fn tabular_values_do_not_jitter() {
        // A clock and a reading through changing values: with tabular
        // figures every string of one shape measures the same on every face.
        let mut engine = TextEngine::new();
        for c in FontChoice::ALL {
            let fonts = text_font(c);
            for width in [85.0, 100.0] {
                let s = style(width, true);
                for set in [
                    ["10:11", "12:58", "20:47", "11:11"],
                    ["71°", "18°", "40°", "99°"],
                ] {
                    let first = engine.measure(&fonts, &s, set[0]);
                    for t in set {
                        let w = engine.measure(&fonts, &s, t);
                        assert_eq!(w, first, "{c} at {width}: {t}");
                    }
                }
            }
        }
    }

    #[test]
    fn condensed_measures_narrower_and_ink_stays_inside() {
        let fonts = text_font(FontChoice::IbmPlexSans);
        let mut engine = TextEngine::new();
        let text = "12:34 56°";
        let normal = engine.measure(&fonts, &style(100.0, false), text);
        for tabular in [false, true] {
            let s = style(85.0, tabular);
            let condensed = engine.measure(&fonts, &s, text);
            assert!(condensed < normal, "{condensed} !< {normal}");
            let (canvas, w) = drawn(&mut engine, &fonts, &s, text);
            assert_eq!(w, condensed);
            let (first, last) = ink_columns(&canvas).expect("text drew ink");
            assert!(
                first >= 10 && last as f32 <= 10.0 + w,
                "ink {first}..{last}, box 10..{}",
                10.0 + w
            );
        }
    }

    #[test]
    fn measure_equals_drawn_advance() {
        let mut engine = TextEngine::new();
        for c in FontChoice::ALL {
            let mut fonts = text_font(c);
            fonts.extend(c.load().fallback);
            for width in [75.0, 85.0, 100.0] {
                for tabular in [false, true] {
                    let s = style(width, tabular);
                    for text in ["12:34", "Tue, Jun 09 4:17 PM", "CPU 28°C ↓1.4 MB/s", ""] {
                        let m = engine.measure(&fonts, &s, text);
                        let (_, d) = drawn(&mut engine, &fonts, &s, text);
                        assert_eq!(m, d, "{c} {width} {tabular} {text:?}");
                    }
                }
            }
        }
    }

    #[test]
    fn unsupported_settings_fall_back_to_the_face_as_it_is() {
        let mut engine = TextEngine::new();
        // No width axis: condensed draws exactly as normal.
        let fonts = text_font(FontChoice::AtkinsonHyperlegible);
        let normal = drawn(&mut engine, &fonts, &style(100.0, false), "12:34");
        let condensed = drawn(&mut engine, &fonts, &style(85.0, false), "12:34");
        assert_eq!(normal.1, condensed.1);
        assert_eq!(normal.0.rgba(), condensed.0.rgba());

        // No `tnum` and proportional digits: tabular draws as proportional.
        let icons = vec![FontChoice::default().load().icon];
        let plain = drawn(&mut engine, &icons, &style(100.0, false), "1170");
        let tabular = drawn(&mut engine, &icons, &style(100.0, true), "1170");
        assert_eq!(plain.1, tabular.1);
        assert_eq!(plain.0.rgba(), tabular.0.rgba());

        // Widths past the axis are clamped to its ends.
        let plex = text_font(FontChoice::IbmPlexSans);
        let at = |engine: &mut TextEngine, w| drawn(engine, &plex, &style(w, false), "12:34");
        assert_eq!(
            at(&mut engine, 50.0).0.rgba(),
            at(&mut engine, 75.0).0.rgba()
        );
        assert_eq!(
            at(&mut engine, 150.0).0.rgba(),
            at(&mut engine, 100.0).0.rgba()
        );
    }

    #[test]
    fn glyph_cache_keeps_widths_apart() {
        let fonts = text_font(FontChoice::IbmPlexSans);
        let mut warm = TextEngine::new();
        drawn(&mut warm, &fonts, &style(75.0, false), "12:34");
        let after_condensed = drawn(&mut warm, &fonts, &style(100.0, false), "12:34");
        let fresh = drawn(
            &mut TextEngine::new(),
            &fonts,
            &style(100.0, false),
            "12:34",
        );
        assert_eq!(after_condensed.0.rgba(), fresh.0.rgba());
        let condensed = drawn(&mut TextEngine::new(), &fonts, &style(75.0, false), "12:34");
        assert_ne!(condensed.0.rgba(), fresh.0.rgba());
    }
}
