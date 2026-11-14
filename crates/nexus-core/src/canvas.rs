//! A 640×48 frame buffer.
//!
//! Wraps a tiny-skia `Pixmap` so shapes (graphs, fills) can use tiny-skia's
//! antialiased paths, while text is blended in directly from glyph masks.
//!
//! Invariant: every pixel is fully opaque. The frame starts filled with the
//! background colour and nothing ever lowers alpha. That is what lets
//! `blend_mask` treat tiny-skia's premultiplied storage as plain RGB.

use tiny_skia::{Color, Pixmap};

use crate::theme::Rgb;

#[derive(Clone)]
pub struct Canvas {
    pixmap: Pixmap,
}

impl Canvas {
    /// Creates a canvas filled with `background`.
    ///
    /// # Panics
    /// Panics if `width` or `height` is zero.
    pub fn new(width: u32, height: u32, background: Rgb) -> Self {
        let mut pixmap = Pixmap::new(width, height).expect("canvas size must be non-zero");
        pixmap.fill(to_color(background));
        Self { pixmap }
    }

    pub fn width(&self) -> u32 {
        self.pixmap.width()
    }

    pub fn height(&self) -> u32 {
        self.pixmap.height()
    }

    /// Raw RGBA bytes, row-major, 4 bytes per pixel.
    pub fn rgba(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// Copies `src` onto this canvas shifted `dx` pixels right (left if
    /// negative). The part that falls outside is clipped. Both canvases must
    /// be the same height. Used to slide pages during a swipe.
    pub fn copy_shifted(&mut self, src: &Canvas, dx: i32) {
        assert_eq!(self.height(), src.height(), "canvases must match in height");
        let (dst_w, src_w) = (self.width() as i32, src.width() as i32);
        let from = (-dx).max(0); // first source column that lands on canvas
        let to = (dst_w - dx).min(src_w); // one past the last
        if from >= to {
            return;
        }
        let bytes = (to - from) as usize * 4;
        let src_x = from as usize * 4;
        let dst_x = (from + dx) as usize * 4;
        let (src_stride, dst_stride) = (src_w as usize * 4, dst_w as usize * 4);
        let rows = self.height() as usize;
        let src_data = src.pixmap.data();
        let dst_data = self.pixmap.data_mut();
        for row in 0..rows {
            let s = row * src_stride + src_x;
            let d = row * dst_stride + dst_x;
            dst_data[d..d + bytes].copy_from_slice(&src_data[s..s + bytes]);
        }
    }

    pub fn pixmap_mut(&mut self) -> &mut Pixmap {
        &mut self.pixmap
    }

    /// Blends an 8-bit coverage mask onto the canvas in `color`.
    ///
    /// `mask` is `w × h` bytes. The top-left corner lands at (`x`, `y`), which
    /// may be partly or fully off-canvas; the off-canvas part is clipped.
    pub fn blend_mask(&mut self, x: i32, y: i32, w: u32, h: u32, mask: &[u8], color: Rgb) {
        let (cw, ch) = (self.width() as i32, self.height() as i32);
        let data = self.pixmap.data_mut();
        for row in 0..h as i32 {
            let py = y + row;
            if py < 0 || py >= ch {
                continue;
            }
            for col in 0..w as i32 {
                let px = x + col;
                if px < 0 || px >= cw {
                    continue;
                }
                let a = u32::from(mask[(row as u32 * w + col as u32) as usize]);
                if a == 0 {
                    continue;
                }
                let i = ((py * cw + px) * 4) as usize;
                data[i] = mix(color.r, data[i], a);
                data[i + 1] = mix(color.g, data[i + 1], a);
                data[i + 2] = mix(color.b, data[i + 2], a);
            }
        }
    }

    /// Fills a rectangle of whole pixels. Coordinates are clipped to the canvas.
    pub fn fill_rect(&mut self, x: u32, y: u32, w: u32, h: u32, color: Rgb) {
        let x_end = (x + w).min(self.width());
        let y_end = (y + h).min(self.height());
        let cw = self.width();
        let data = self.pixmap.data_mut();
        for py in y..y_end {
            for px in x..x_end {
                let i = ((py * cw + px) * 4) as usize;
                data[i] = color.r;
                data[i + 1] = color.g;
                data[i + 2] = color.b;
                data[i + 3] = 255;
            }
        }
    }

    /// Saves the canvas as a PNG. Used by tests and development tools.
    pub fn save_png(&self, path: impl AsRef<std::path::Path>) -> std::io::Result<()> {
        self.pixmap
            .save_png(path)
            .map_err(|e| std::io::Error::other(e.to_string()))
    }
}

pub(crate) fn to_color(c: Rgb) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, 255)
}

/// `src` over `dst` with coverage `a` (0–255), rounded.
fn mix(src: u8, dst: u8, a: u32) -> u8 {
    ((u32::from(src) * a + u32::from(dst) * (255 - a) + 127) / 255) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copy_shifted_clips_both_ways() {
        let red = Canvas::new(4, 1, Rgb::hex(0xff0000));
        let first_red = |c: &Canvas| c.rgba().chunks(4).position(|p| p[0] == 255);

        let mut c = Canvas::new(4, 1, Rgb::hex(0x000000));
        c.copy_shifted(&red, 3);
        assert_eq!(first_red(&c), Some(3), "shifted right, one column visible");

        let mut c = Canvas::new(4, 1, Rgb::hex(0x000000));
        c.copy_shifted(&red, -3);
        assert_eq!(first_red(&c), Some(0));
        assert_eq!(c.rgba()[4], 0, "only one column visible");

        let mut c = Canvas::new(4, 1, Rgb::hex(0x000000));
        c.copy_shifted(&red, 4);
        assert_eq!(first_red(&c), None, "entirely off canvas");
    }

    #[test]
    fn mix_endpoints() {
        assert_eq!(mix(200, 10, 0), 10);
        assert_eq!(mix(200, 10, 255), 200);
    }

    #[test]
    fn blend_mask_clips_off_canvas() {
        let mut c = Canvas::new(4, 2, Rgb::hex(0x000000));
        // A 3×3 mask placed so only its bottom-right pixel is on the canvas.
        c.blend_mask(-2, -2, 3, 3, &[255; 9], Rgb::hex(0xffffff));
        assert_eq!(&c.rgba()[0..4], &[255, 255, 255, 255]);
        assert_eq!(&c.rgba()[4..8], &[0, 0, 0, 255]);
    }
}
