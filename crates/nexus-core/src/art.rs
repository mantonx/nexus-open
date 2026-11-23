//! Album artwork, prepared once per track at exactly the sizes the media
//! widget draws it (`theme::ART` on medium and hero cards, `ART_SCENE` in
//! the media scene and the now-playing takeover), so a frame only copies
//! pixels.
//!
//! Where the picture comes from is not this crate's business: the media
//! source reads, fetches and decodes it on its own thread and hands the
//! decoded pixels to `Art::prepare`. That crops a square from the middle
//! and scales it down by area averaging in linear light: every source
//! pixel counts for exactly the share of an output pixel it covers, so a
//! fine pattern becomes its true average colour, without moiré, ringing or
//! the darkening that averaging gamma-encoded values gives. The corners
//! are rounded when drawn, as the placeholder's are.
//!
//! The engine keys artwork by a short text (`media.art`), so the signal
//! shows something readable and never the picture.

use std::fmt;
use std::sync::OnceLock;

use tiny_skia::{ColorU8, Pixmap, PixmapRef};

use crate::theme::{ART, ART_SCENE, CARD};

/// The squares prepared for each track, in pixels.
pub const SIZES: [u32; 2] = [ART as u32, ART_SCENE as u32];

/// The largest square `from_squares` takes: what the control protocol
/// carries to the settings window.
pub const MAX_SQUARE: u32 = 64;

/// How far paused artwork is mixed toward the card, as the placeholder.
pub const DIM: f32 = 0.55;

/// One track's artwork, ready to draw.
pub struct Art {
    key: String,
    squares: Vec<Square>,
}

struct Square {
    size: u32,
    /// Straight-alpha sRGB, row-major: what the protocol sends.
    rgba: Vec<u8>,
    pixmap: Pixmap,
    /// Mixed `DIM` toward the card, for a paused track.
    dimmed: Pixmap,
}

impl Art {
    /// Prepares decoded artwork, `width`×`height` straight-alpha sRGB RGBA
    /// rows, for every size in `SIZES`: the largest centred square, scaled
    /// by area averaging in linear light. `None` if the buffer does not
    /// hold that many pixels or either side is 0.
    pub fn prepare(key: impl Into<String>, width: u32, height: u32, rgba: &[u8]) -> Option<Art> {
        let len = (width as usize)
            .checked_mul(height as usize)?
            .checked_mul(4)?;
        if width == 0 || height == 0 || rgba.len() != len {
            return None;
        }
        let side = width.min(height);
        let crop = Crop {
            x: (width - side) / 2,
            y: (height - side) / 2,
            side,
            stride: width as usize * 4,
        };
        let squares = SIZES
            .iter()
            .map(|&size| (size, area_average(rgba, &crop, size)))
            .collect();
        Art::from_squares(key, squares)
    }

    /// Artwork already prepared: `(size, rgba)` squares of straight-alpha
    /// sRGB, as `squares` gives them and the control protocol carries
    /// them. `None` if a square's buffer is the wrong length, or a size is
    /// 0 or over `MAX_SQUARE`.
    pub fn from_squares(key: impl Into<String>, squares: Vec<(u32, Vec<u8>)>) -> Option<Art> {
        let squares = squares
            .into_iter()
            .map(|(size, rgba)| {
                let ok =
                    (1..=MAX_SQUARE).contains(&size) && rgba.len() == (size * size * 4) as usize;
                if !ok {
                    return None;
                }
                Some(Square {
                    pixmap: pixmap(size, &rgba, 0.0)?,
                    dimmed: pixmap(size, &rgba, DIM)?,
                    size,
                    rgba,
                })
            })
            .collect::<Option<Vec<_>>>()?;
        Some(Art {
            key: key.into(),
            squares,
        })
    }

    /// What the engine knows this artwork by: the `media.art` signal's
    /// value.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Every prepared square, `(size, rgba)` straight-alpha sRGB.
    pub fn squares(&self) -> impl Iterator<Item = (u32, &[u8])> {
        self.squares.iter().map(|s| (s.size, s.rgba.as_slice()))
    }

    /// Roughly the memory it holds, for a cache sized in bytes.
    pub fn bytes(&self) -> usize {
        self.key.len() + self.squares.iter().map(|s| s.rgba.len() * 3).sum::<usize>()
    }

    /// The square `size` pixels on a side, dimmed for a paused track.
    pub(crate) fn pixmap(&self, size: u32, dim: bool) -> Option<PixmapRef<'_>> {
        let s = self.squares.iter().find(|s| s.size == size)?;
        Some(if dim { &s.dimmed } else { &s.pixmap }.as_ref())
    }
}

/// Two artworks are the same when their keys are: the key names the image.
impl PartialEq for Art {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}

/// The key and sizes, never the pixels.
impl fmt::Debug for Art {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let sizes: Vec<u32> = self.squares.iter().map(|s| s.size).collect();
        f.debug_struct("Art")
            .field("key", &self.key)
            .field("sizes", &sizes)
            .finish()
    }
}

/// A premultiplied pixmap of a straight-alpha square, its colour mixed
/// `dim` of the way toward the card in sRGB (as `Rgb::mix` does).
fn pixmap(size: u32, rgba: &[u8], dim: f32) -> Option<Pixmap> {
    let mut pm = Pixmap::new(size, size)?;
    let card = [CARD.r, CARD.g, CARD.b];
    for (px, src) in pm
        .pixels_mut()
        .iter_mut()
        .zip(rgba.as_chunks::<4>().0.iter())
    {
        let m = |i: usize| {
            let (x, y) = (f32::from(src[i]), f32::from(card[i]));
            (x + (y - x) * dim).round() as u8
        };
        *px = ColorU8::from_rgba(m(0), m(1), m(2), src[3]).premultiply();
    }
    Some(pm)
}

/// The square of the source that is used.
struct Crop {
    x: u32,
    y: u32,
    side: u32,
    /// Bytes per source row.
    stride: usize,
}

/// sRGB-encoded 0–255 to linear 0–1.
fn to_linear(v: u8) -> f32 {
    static TABLE: OnceLock<[f32; 256]> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0.0; 256];
        for (i, out) in t.iter_mut().enumerate() {
            let c = i as f32 / 255.0;
            *out = if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            };
        }
        t
    })[usize::from(v)]
}

/// Linear 0–1 to sRGB-encoded 0–255.
fn to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let e = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (e * 255.0).round() as u8
}

/// For each source pixel along one axis, the output pixels it falls in
/// and the share of each it makes up. The shares into any one output add
/// up to 1.
fn coverage(side: u32, size: u32) -> Vec<Vec<(usize, f32)>> {
    let scale = f64::from(side) / f64::from(size);
    (0..side)
        .map(|i| {
            let (a, b) = (f64::from(i), f64::from(i) + 1.0);
            let first = (a / scale).floor() as usize;
            let last = ((b / scale).ceil() as usize).min(size as usize);
            (first..last)
                .filter_map(|j| {
                    let lo = a.max(j as f64 * scale);
                    let hi = b.min((j + 1) as f64 * scale);
                    (hi > lo).then(|| (j, ((hi - lo) / scale) as f32))
                })
                .collect()
        })
        .collect()
}

/// The crop scaled to `size`×`size` by area averaging, in linear light
/// with premultiplied alpha (so a transparent pixel's colour counts for
/// nothing), back to straight-alpha sRGB. One pass over the source, a
/// row at a time: memory is a few rows of the output, whatever the
/// source's size.
fn area_average(rgba: &[u8], crop: &Crop, size: u32) -> Vec<u8> {
    let n = size as usize;
    let across = coverage(crop.side, size);
    let down = coverage(crop.side, size);
    let mut out = vec![0.0f32; n * n * 4];
    let mut row = vec![0.0f32; n * 4];
    for (sy, into) in down.iter().enumerate() {
        row.fill(0.0);
        let start = (crop.y as usize + sy) * crop.stride + crop.x as usize * 4;
        let src = &rgba[start..start + crop.side as usize * 4];
        for (px, to) in src.as_chunks::<4>().0.iter().zip(&across) {
            let a = f32::from(px[3]) / 255.0;
            let lin = [
                to_linear(px[0]) * a,
                to_linear(px[1]) * a,
                to_linear(px[2]) * a,
                a,
            ];
            for &(j, w) in to {
                for (c, v) in lin.iter().enumerate() {
                    row[j * 4 + c] += v * w;
                }
            }
        }
        for &(j, w) in into {
            let dst = &mut out[j * n * 4..(j + 1) * n * 4];
            for (d, v) in dst.iter_mut().zip(&row) {
                *d += v * w;
            }
        }
    }
    out.as_chunks::<4>()
        .0
        .iter()
        .flat_map(|p| {
            let a = p[3];
            if a <= 0.0 {
                return [0, 0, 0, 0];
            }
            [
                to_srgb(p[0] / a),
                to_srgb(p[1] / a),
                to_srgb(p[2] / a),
                (a.clamp(0.0, 1.0) * 255.0).round() as u8,
            ]
        })
        .collect()
}

/// A test card for artwork: bold two-colour shapes that show any scaling
/// fault. Two quarters of a 4-pixel checker beside two solid ones (a
/// checker must come out an even mid tone, not a dark one: linear
/// light, and without moiré), a thick diagonal across the solid quarters
/// (stair-steps or a blur), a ring (lopsidedness), and a cream border 4 %
/// wide (the crop, and an off-by-one at an edge). Never real artwork.
/// `width`×`height` straight-alpha RGBA; make it wider than tall to see
/// the centre crop: what it cuts away is green.
pub fn test_card(width: u32, height: u32) -> Vec<u8> {
    const A: [u8; 3] = [0xf2, 0x6b, 0x3a];
    const B: [u8; 3] = [0x1d, 0x3f, 0x8a];
    const EDGE: [u8; 3] = [0xff, 0xf3, 0xc4];
    const CUT: [u8; 3] = [0x3a, 0xa8, 0x5c];
    let side = width.min(height) as f32;
    let (x0, y0) = ((width as f32 - side) / 2.0, (height as f32 - side) / 2.0);
    let mut out = Vec::with_capacity((width * height * 4) as usize);
    for y in 0..height {
        for x in 0..width {
            // Square coordinates, 0–1 across the part kept.
            let (u, v) = ((x as f32 + 0.5 - x0) / side, (y as f32 + 0.5 - y0) / side);
            let inside = (0.0..1.0).contains(&u) && (0.0..1.0).contains(&v);
            let border = u.min(v).min(1.0 - u).min(1.0 - v) < 0.04;
            let rgb = if !inside {
                CUT
            } else if border {
                EDGE
            } else {
                let checker = (x / 4 + y / 4) % 2 == 0;
                let quarter = match (u < 0.5, v < 0.5) {
                    (true, true) | (false, false) => checker,
                    (true, false) => true,
                    (false, true) => false,
                };
                let r = ((u - 0.5).powi(2) + (v - 0.5).powi(2)).sqrt();
                let ring = (0.27..0.35).contains(&r);
                let diagonal = (u + v - 1.0).abs() < 0.09;
                if quarter != (ring || diagonal) { A } else { B }
            };
            out.extend_from_slice(&[rgb[0], rgb[1], rgb[2], 255]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn solid(w: u32, h: u32, px: [u8; 4]) -> Vec<u8> {
        px.repeat((w * h) as usize)
    }

    #[test]
    fn coverage_shares_add_up_to_one_per_output() {
        for (side, size) in [(640, 33), (33, 33), (100, 35), (20, 33), (4096, 35)] {
            let mut sums = vec![0.0f32; size as usize];
            for list in coverage(side, size) {
                for (j, w) in list {
                    sums[j] += w;
                }
            }
            for s in sums {
                assert!((s - 1.0).abs() < 1e-4, "{side}→{size}: {s}");
            }
        }
    }

    #[test]
    fn prepares_every_size_from_any_shape() {
        let art = Art::prepare("k", 300, 200, &solid(300, 200, [10, 200, 30, 255])).unwrap();
        let sizes: Vec<u32> = art.squares().map(|(s, _)| s).collect();
        assert_eq!(sizes, SIZES);
        for (s, rgba) in art.squares() {
            assert_eq!(rgba.len(), (s * s * 4) as usize);
            assert!(
                rgba.as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| *p == [10, 200, 30, 255])
            );
        }
        assert!(Art::prepare("k", 3, 3, &[0; 10]).is_none(), "short buffer");
        assert!(Art::prepare("k", 0, 3, &[]).is_none());
    }

    #[test]
    fn a_checker_averages_in_linear_light() {
        // Black and white pixels average to linear 0.5: sRGB 188, not the
        // 128 a gamma-blind average gives.
        let mut px = Vec::new();
        for y in 0..66u32 {
            for x in 0..66u32 {
                let v = if (x + y) % 2 == 0 { 255 } else { 0 };
                px.extend_from_slice(&[v, v, v, 255]);
            }
        }
        let art = Art::prepare("k", 66, 66, &px).unwrap();
        let (_, sq) = art.squares().next().unwrap();
        let mid = &sq[(16 * 33 + 16) * 4..][..4];
        assert!((186..=190).contains(&mid[0]), "{mid:?}");
    }

    #[test]
    fn the_crop_is_the_middle_square() {
        // Red bands left and right of a blue middle square: none of the
        // red survives.
        let (w, h) = (300u32, 100u32);
        let mut px = Vec::new();
        for _ in 0..h {
            for x in 0..w {
                let blue = (100..200).contains(&x);
                px.extend_from_slice(&if blue {
                    [0, 0, 255, 255]
                } else {
                    [255, 0, 0, 255]
                });
            }
        }
        let art = Art::prepare("k", w, h, &px).unwrap();
        for (_, sq) in art.squares() {
            assert!(sq.as_chunks::<4>().0.iter().all(|p| *p == [0, 0, 255, 255]));
        }
    }

    #[test]
    fn transparent_pixels_lend_no_colour() {
        let mut px = solid(70, 70, [255, 0, 0, 0]);
        for i in (0..px.len()).step_by(8) {
            px[i..i + 4].copy_from_slice(&[0, 255, 0, 255]);
        }
        let art = Art::prepare("k", 70, 70, &px).unwrap();
        let (_, sq) = art.squares().nth(1).unwrap();
        let p = &sq[(17 * 35 + 17) * 4..][..4];
        assert_eq!((p[0], p[1]), (0, 255), "{p:?}");
        assert!((120..=135).contains(&p[3]), "half covered: {p:?}");
    }

    #[test]
    fn from_squares_checks_sizes_and_round_trips() {
        let art = Art::prepare("k", 40, 40, &test_card(40, 40)).unwrap();
        let squares: Vec<(u32, Vec<u8>)> = art.squares().map(|(s, p)| (s, p.to_vec())).collect();
        let back = Art::from_squares("k", squares).unwrap();
        assert!(back.squares().eq(art.squares()));
        assert!(Art::from_squares("k", vec![(65, vec![0; 65 * 65 * 4])]).is_none());
        assert!(Art::from_squares("k", vec![(2, vec![0; 15])]).is_none());
        assert_eq!(format!("{back:?}"), "Art { key: \"k\", sizes: [33, 35] }");
    }

    #[test]
    fn paused_artwork_is_mixed_toward_the_card() {
        let art = Art::from_squares("k", vec![(1, vec![255, 255, 255, 255])]).unwrap();
        let px = art.pixmap(1, true).unwrap().pixels()[0].demultiply();
        let want = crate::theme::Rgb::hex(0xffffff).mix(CARD, DIM);
        assert_eq!((px.red(), px.green(), px.blue()), (want.r, want.g, want.b));
    }
}
