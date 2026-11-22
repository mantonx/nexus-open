//! Drawing helpers for the better-cards mockup: a small text engine with a
//! width axis (the shipped one only sets weight), shapes, icons and the
//! generated album-art placeholder.
//!
//! Everything draws onto the repo's `Canvas` through tiny-skia and
//! `Canvas::blend_mask`, so the pixels are what the strip would show.

use std::collections::HashMap;

use nexus_core::canvas::Canvas;
use nexus_core::theme::Rgb;
use swash::scale::{Render, ScaleContext, Source};
use swash::shape::ShapeContext;
use swash::zeno::Format;
use swash::{CacheKey, FontRef};
use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Mask, Paint, Path, PathBuilder, Point,
    SpreadMode, Stroke, StrokeDash, Transform,
};

const PLEX: &[u8] = include_bytes!("../../assets/fonts/IBMPlexSans.ttf");
const ICONS: &[u8] = include_bytes!("../../assets/fonts/FontAwesome-Solid.ttf");

/// One text style. `wdth` is IBM Plex Sans's width axis (75–100).
#[derive(Clone, Copy, Debug)]
pub struct St {
    pub size: f32,
    pub wght: f32,
    pub wdth: f32,
    pub track: f32,
}

pub const fn st(size: f32, wght: f32, wdth: f32) -> St {
    St {
        size,
        wght,
        wdth,
        track: 0.0,
    }
}

struct Face {
    data: &'static [u8],
    offset: u32,
    key: CacheKey,
}

impl Face {
    fn new(data: &'static [u8]) -> Self {
        let f = FontRef::from_index(data, 0).expect("bundled font parses");
        Self {
            data,
            offset: f.offset,
            key: f.key,
        }
    }
    fn r(&self) -> FontRef<'_> {
        FontRef {
            data: self.data,
            offset: self.offset,
            key: self.key,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct GKey {
    face: u8,
    glyph: u16,
    size: u32,
    wght: u32,
    wdth: u32,
}

#[derive(Clone)]
pub struct GMask {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    alpha: Vec<u8>,
}

pub struct Gfx {
    pub c: Canvas,
    shaper: ShapeContext,
    scaler: ScaleContext,
    cache: HashMap<GKey, Option<GMask>>,
    faces: [Face; 2],
}

/// Converts a colour plus alpha (0–255) to a tiny-skia colour.
pub fn col(c: Rgb, a: u8) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, a)
}

pub fn paint(c: Rgb, a: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(col(c, a));
    p.anti_alias = true;
    p
}

/// Linear mix of two colours, `t` = 0 gives `a`.
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Rgb {
        r: m(a.r, b.r),
        g: m(a.g, b.g),
        b: m(a.b, b.b),
    }
}

pub fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    let r = r.min(w / 2.0).min(h / 2.0);
    let mut pb = PathBuilder::new();
    // Circular corners via cubic approximation.
    let k = 0.552_284_8 * r;
    pb.move_to(x + r, y);
    pb.line_to(x + w - r, y);
    pb.cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
    pb.line_to(x + w, y + h - r);
    pb.cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
    pb.line_to(x + r, y + h);
    pb.cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
    pb.line_to(x, y + r);
    pb.cubic_to(x, y + r - k, x + r - k, y, x + r, y);
    pb.close();
    pb.finish()
}

impl Gfx {
    pub fn new(w: u32, h: u32, bg: Rgb) -> Self {
        Self {
            c: Canvas::new(w, h, bg),
            shaper: ShapeContext::new(),
            scaler: ScaleContext::new(),
            cache: HashMap::new(),
            faces: [Face::new(PLEX), Face::new(ICONS)],
        }
    }

    /// Starts a fresh frame of the given size.
    pub fn begin(&mut self, w: u32, h: u32, bg: Rgb) {
        self.c = Canvas::new(w, h, bg);
    }

    fn layout(&mut self, face: u8, s: &St, text: &str) -> (Vec<(u16, i32)>, i32) {
        let font = self.faces[face as usize].r();
        let mut shaper = self
            .shaper
            .builder(font)
            .size(s.size)
            .variations(&[("wght", s.wght), ("wdth", s.wdth)])
            .features(&[("tnum", 1)])
            .build();
        shaper.add_str(text);
        let mut out = Vec::new();
        let mut pen = 0.0f32;
        shaper.shape_with(|cl| {
            for g in cl.glyphs {
                out.push((g.id, (pen + g.x).round() as i32));
                pen += g.advance + s.track;
            }
        });
        if !out.is_empty() {
            pen -= s.track;
        }
        (out, pen.round() as i32)
    }

    fn mask(&mut self, face: u8, s: &St, glyph: u16) -> Option<GMask> {
        let key = GKey {
            face,
            glyph,
            size: s.size.to_bits(),
            wght: s.wght.to_bits(),
            wdth: s.wdth.to_bits(),
        };
        if let Some(m) = self.cache.get(&key) {
            return m.clone();
        }
        let font = self.faces[face as usize].r();
        let mut scaler = self
            .scaler
            .builder(font)
            .size(s.size)
            .hint(true)
            .variations(&[("wght", s.wght), ("wdth", s.wdth)])
            .build();
        let m = Render::new(&[Source::Outline])
            .format(Format::Alpha)
            .render(&mut scaler, glyph)
            .map(|img| GMask {
                left: img.placement.left,
                top: img.placement.top,
                width: img.placement.width,
                height: img.placement.height,
                alpha: img.data,
            });
        self.cache.insert(key, m.clone());
        m
    }

    pub fn measure(&mut self, s: &St, text: &str) -> f32 {
        self.layout(0, s, text).1 as f32
    }

    /// Draws text with its left edge at `x` and baseline at `base`.
    /// Returns the advance width.
    pub fn text(&mut self, s: &St, text: &str, x: f32, base: f32, color: Rgb) -> f32 {
        let (glyphs, w) = self.layout(0, s, text);
        let (ox, oy) = (x.round() as i32, base.round() as i32);
        for (g, gx) in glyphs {
            if let Some(m) = self.mask(0, s, g) {
                self.c.blend_mask(
                    ox + gx + m.left,
                    oy - m.top,
                    m.width,
                    m.height,
                    &m.alpha,
                    color,
                );
            }
        }
        w as f32
    }

    /// Right-aligned text ending at `right`.
    pub fn text_r(&mut self, s: &St, text: &str, right: f32, base: f32, color: Rgb) -> f32 {
        let w = self.measure(s, text);
        self.text(s, text, right - w, base, color);
        w
    }

    /// `text` shortened with an ellipsis to fit `room`.
    pub fn fit(&mut self, s: &St, text: &str, room: f32) -> String {
        if self.measure(s, text) <= room {
            return text.to_string();
        }
        let ends: Vec<usize> = text.char_indices().map(|(i, _)| i).skip(1).collect();
        let mut best = String::from("…");
        for e in ends {
            let cand = format!("{}…", text[..e].trim_end());
            if self.measure(s, &cand) <= room {
                best = cand;
            } else {
                break;
            }
        }
        best
    }

    /// Height of the capital letters for a style (from the glyph "H").
    pub fn cap(&mut self, s: &St) -> f32 {
        let (g, _) = self.layout(0, s, "H");
        g.first()
            .and_then(|(id, _)| self.mask(0, s, *id))
            .map_or(s.size * 0.7, |m| m.top as f32)
    }

    /// Draws an icon-font glyph centred on (`cx`, `cy`) by its ink box.
    /// Returns the ink width.
    pub fn icon(&mut self, ch: char, size: f32, cx: f32, cy: f32, color: Rgb) -> f32 {
        let s = st(size, 400.0, 100.0);
        let glyph = self.faces[1].r().charmap().map(ch);
        let Some(m) = self.mask(1, &s, glyph) else {
            return 0.0;
        };
        let x = (cx - m.width as f32 / 2.0).round() as i32;
        let y = (cy - m.height as f32 / 2.0).round() as i32;
        self.c.blend_mask(x, y, m.width, m.height, &m.alpha, color);
        m.width as f32
    }

    pub fn fill(&mut self, path: &Path, c: Rgb, a: u8) {
        self.c.pixmap_mut().fill_path(
            path,
            &paint(c, a),
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }

    pub fn fill_eo(&mut self, path: &Path, c: Rgb) {
        self.c.pixmap_mut().fill_path(
            path,
            &paint(c, 255),
            FillRule::EvenOdd,
            Transform::identity(),
            None,
        );
    }

    pub fn stroke(&mut self, path: &Path, c: Rgb, a: u8, width: f32, dash: Option<(f32, f32)>) {
        let stroke = Stroke {
            width,
            dash: dash.and_then(|(on, off)| StrokeDash::new(vec![on, off], 0.0)),
            ..Stroke::default()
        };
        self.c
            .pixmap_mut()
            .stroke_path(path, &paint(c, a), &stroke, Transform::identity(), None);
    }

    pub fn rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgb) {
        if let Some(p) = rrect_path(x, y, w, h, r) {
            self.fill(&p, c, 255);
        }
    }

    /// A rounded outline drawn on the half-pixel so a 1 px line is crisp.
    #[allow(clippy::too_many_arguments)]
    pub fn rrect_line(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgb, dash: bool) {
        if let Some(p) = rrect_path(x + 0.5, y + 0.5, w - 1.0, h - 1.0, r) {
            self.stroke(&p, c, 255, 1.0, dash.then_some((4.0, 3.0)));
        }
    }

    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, c: Rgb) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        if let Some(r) = tiny_skia::Rect::from_xywh(x, y, w, h) {
            self.fill(&PathBuilder::from_rect(r), c, 255);
        }
    }

    pub fn circle(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, a: u8) {
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, r);
        if let Some(p) = pb.finish() {
            self.fill(&p, c, a);
        }
    }

    pub fn hline_dash(&mut self, x0: f32, x1: f32, y: f32, c: Rgb, a: u8) {
        let mut pb = PathBuilder::new();
        pb.move_to(x0, y + 0.5);
        pb.line_to(x1, y + 0.5);
        if let Some(p) = pb.finish() {
            self.stroke(&p, c, a, 1.0, Some((3.0, 2.0)));
        }
    }

    /// The GPU icon, drawn here because Font Awesome Free has none: a card
    /// with one large fan, a bracket on the left and the slot tab below.
    /// `s` is the box height; the icon is a little wider than tall.
    pub fn gpu_icon(&mut self, cx: f32, cy: f32, s: f32, c: Rgb) -> f32 {
        let w = (s * 1.35).round();
        let h = (s * 0.72).round();
        let x = (cx - w / 2.0).round();
        let y = (cy - h / 2.0 - s * 0.05).round();
        let bw = (s * 0.12).max(2.0).round();
        let mut pb = PathBuilder::new();
        // Bracket and body joined, two fans cut out by even-odd.
        pb.push_rect(tiny_skia::Rect::from_xywh(x, y - 2.0, bw, h + 5.0).unwrap());
        if let Some(body) = rrect_path(x + bw, y, w - bw, h, s * 0.12) {
            pb.push_path(&body);
        }
        let body_w = w - bw;
        // One large fan: two read as a pair of eyes at this size.
        let fr = h * 0.36;
        let fans = [x + bw + body_w * 0.58];
        for fx in fans {
            pb.push_circle(fx, y + h / 2.0, fr);
        }
        pb.push_rect(
            tiny_skia::Rect::from_xywh(
                x + bw + s * 0.15,
                y + h,
                body_w * 0.5,
                (s * 0.14).max(2.0).round(),
            )
            .unwrap(),
        );
        if let Some(p) = pb.finish() {
            self.fill_eo(&p, c);
        }
        for fx in fans {
            self.circle(fx, y + h / 2.0, fr * 0.4, c, 255);
        }
        w
    }

    /// Abstract generated album-art placeholder: a two-colour gradient with
    /// rings spreading from one corner. Seeded by the title, never real art.
    pub fn art(&mut self, x: f32, y: f32, s: f32, seed: &str, dim: bool) {
        let h = seed.bytes().fold(2_166_136_261u32, |a, b| {
            (a ^ u32::from(b)).wrapping_mul(16_777_619)
        });
        let hues = [
            (Rgb::hex(0x2a2f8f), Rgb::hex(0x14b3a3)),
            (Rgb::hex(0x7a1f5c), Rgb::hex(0xf08a3c)),
            (Rgb::hex(0x173d6e), Rgb::hex(0x8fd16a)),
        ];
        let (a, b) = hues[(h % 3) as usize];
        let (a, b) = if dim {
            (
                mix(a, Rgb::hex(0x101317), 0.6),
                mix(b, Rgb::hex(0x101317), 0.6),
            )
        } else {
            (a, b)
        };
        let Some(clip_path) = rrect_path(x, y, s, s, 4.0) else {
            return;
        };
        let (cw, ch) = (self.c.width(), self.c.height());
        let mut clip = Mask::new(cw, ch).expect("mask");
        clip.fill_path(&clip_path, FillRule::Winding, true, Transform::identity());
        let shader = LinearGradient::new(
            Point::from_xy(x, y),
            Point::from_xy(x + s, y + s),
            vec![
                GradientStop::new(0.0, col(a, 255)),
                GradientStop::new(1.0, col(b, 255)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        )
        .expect("gradient");
        let p = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        self.c.pixmap_mut().fill_path(
            &clip_path,
            &p,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
        // Rings from the lower-left corner.
        let (ox, oy) = (x + s * 0.18, y + s * 0.86);
        for i in 1..6 {
            let r = s * 0.17 * i as f32 + (h >> 4 & 3) as f32;
            let mut pb = PathBuilder::new();
            pb.push_circle(ox, oy, r);
            if let Some(path) = pb.finish() {
                let stroke = Stroke {
                    width: 1.6,
                    ..Stroke::default()
                };
                let alpha = 150u8.saturating_sub(i as u8 * 22);
                self.c.pixmap_mut().stroke_path(
                    &path,
                    &paint(Rgb::hex(0xffffff), alpha),
                    &stroke,
                    Transform::identity(),
                    Some(&clip),
                );
            }
        }
        // A bright disc, the one focal point.
        let mut pb = PathBuilder::new();
        pb.push_circle(x + s * 0.72, y + s * 0.3, s * 0.11);
        if let Some(path) = pb.finish() {
            self.c.pixmap_mut().fill_path(
                &path,
                &paint(Rgb::hex(0xffffff), if dim { 90 } else { 220 }),
                FillRule::Winding,
                Transform::identity(),
                Some(&clip),
            );
        }
    }
}
