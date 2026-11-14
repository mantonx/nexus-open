//! Text shaping and hinted glyph rendering.
//!
//! Small text on a 48-pixel panel only looks sharp when glyph outlines are
//! hinted (snapped to the pixel grid by the font's own instructions) and every
//! glyph starts on a whole pixel. So this module:
//!
//! - splits text into runs by which font in the stack has each character,
//! - shapes each run with swash (kerning, ligatures),
//! - rounds each glyph's pen position to a whole pixel,
//! - rasterizes glyphs with hinting on, and caches the masks.

use std::collections::HashMap;
use std::sync::Arc;

use swash::scale::{Render, ScaleContext, Source};
use swash::shape::ShapeContext;
use swash::zeno::Format;
use swash::{CacheKey, FontRef};

use crate::canvas::Canvas;
use crate::theme::{Rgb, TextStyle};

/// A loaded font file.
///
/// `FontRef` from swash borrows the font bytes, so it can't be stored on its
/// own. `Font` owns the bytes and hands out a `FontRef` on demand. The bytes
/// sit behind an `Arc` so cloning a `Font` is cheap.
#[derive(Clone)]
pub struct Font {
    data: Arc<Vec<u8>>,
    offset: u32,
    key: CacheKey,
}

impl Font {
    /// Parses the first face in a font file. Returns `None` if the bytes are
    /// not a font swash can read.
    pub fn from_bytes(data: Vec<u8>) -> Option<Self> {
        let font = FontRef::from_index(&data, 0)?;
        let (offset, key) = (font.offset, font.key);
        Some(Self {
            data: Arc::new(data),
            offset,
            key,
        })
    }

    /// True if the font has a glyph for `c`.
    pub fn covers(&self, c: char) -> bool {
        self.as_ref().charmap().map(c) != 0
    }

    fn as_ref(&self) -> FontRef<'_> {
        FontRef {
            data: &self.data,
            offset: self.offset,
            key: self.key,
        }
    }
}

/// Identifies one rasterized glyph in the cache.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GlyphKey {
    font: u64,
    glyph: u16,
    // f32 can't be hashed, so sizes are stored by their bit pattern.
    size: u32,
    weight: u32,
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

    /// Width in pixels that `text` occupies in `style`, including tracking.
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
                .variations(&[("wght", style.weight)])
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
            .variations(&[("wght", style.weight)])
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
}
