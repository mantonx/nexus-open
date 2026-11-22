//! Drawing primitives for the design-system frames: a text engine that can
//! pick between bundled faces and turn on tabular figures, icon glyphs,
//! rounded shapes and the generated artwork placeholder.
//!
//! Everything lands on the repo's `Canvas` through tiny-skia and
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
    RadialGradient, SpreadMode, Stroke, StrokeDash, Transform,
};

/// The bundled faces this example draws with.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Face {
    /// Phosphor Fill: status and data icons.
    IconFill,
    /// Phosphor Bold: glyphs on keys and outline icons for empty states.
    IconBold,
}

const FACE_BYTES: [&[u8]; 3] = [
    include_bytes!("../../assets/fonts/Rubik.ttf"),
    include_bytes!("../../assets/fonts/Phosphor-Fill.ttf"),
    include_bytes!("../../assets/fonts/Phosphor-Bold.ttf"),
];

fn face_index(f: Face) -> usize {
    match f {
        Face::IconFill => 1,
        Face::IconBold => 2,
    }
}

/// One text style.
#[derive(Clone, Copy, Debug)]
pub struct St {
    pub size: f32,
    pub wght: f32,
    /// Extra pixels between letters.
    pub track: f32,
}

pub const fn st(size: f32, wght: f32, track: f32) -> St {
    St { size, wght, track }
}

struct LoadedFace {
    data: &'static [u8],
    offset: u32,
    key: CacheKey,
}

impl LoadedFace {
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
    face: usize,
    glyph: u16,
    size: u32,
    wght: u32,
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
    faces: Vec<LoadedFace>,
}

pub fn col(c: Rgb, a: u8) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, a)
}

pub fn paint(c: Rgb, a: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(col(c, a));
    p.anti_alias = true;
    p
}

/// Linear mix of two colours in sRGB; `t` = 0 gives `a`.
pub fn mix(a: Rgb, b: Rgb, t: f32) -> Rgb {
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t).round() as u8;
    Rgb {
        r: m(a.r, b.r),
        g: m(a.g, b.g),
        b: m(a.b, b.b),
    }
}

/// A rounded rectangle with its own radius per corner:
/// top-left, top-right, bottom-right, bottom-left.
pub fn rrect4(x: f32, y: f32, w: f32, h: f32, r: [f32; 4]) -> Option<Path> {
    let lim = (w / 2.0).min(h / 2.0);
    let [tl, tr, br, bl] = r.map(|v| v.min(lim).max(0.0));
    let k = 0.552_284_8;
    let mut pb = PathBuilder::new();
    pb.move_to(x + tl, y);
    pb.line_to(x + w - tr, y);
    pb.cubic_to(
        x + w - tr + k * tr,
        y,
        x + w,
        y + tr - k * tr,
        x + w,
        y + tr,
    );
    pb.line_to(x + w, y + h - br);
    pb.cubic_to(
        x + w,
        y + h - br + k * br,
        x + w - br + k * br,
        y + h,
        x + w - br,
        y + h,
    );
    pb.line_to(x + bl, y + h);
    pb.cubic_to(
        x + bl - k * bl,
        y + h,
        x,
        y + h - bl + k * bl,
        x,
        y + h - bl,
    );
    pb.line_to(x, y + tl);
    pb.cubic_to(x, y + tl - k * tl, x + tl - k * tl, y, x + tl, y);
    pb.close();
    pb.finish()
}

pub fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    rrect4(x, y, w, h, [r; 4])
}

impl Gfx {
    pub fn new(w: u32, h: u32, bg: Rgb) -> Self {
        Self {
            c: Canvas::new(w, h, bg),
            shaper: ShapeContext::new(),
            scaler: ScaleContext::new(),
            cache: HashMap::new(),
            faces: FACE_BYTES.iter().map(|b| LoadedFace::new(b)).collect(),
        }
    }

    /// Starts a fresh frame.
    pub fn begin(&mut self, w: u32, h: u32, bg: Rgb) {
        self.c = Canvas::new(w, h, bg);
    }

    fn layout(&mut self, face: usize, s: &St, text: &str) -> (Vec<(u16, i32)>, i32) {
        let font = self.faces[face].r();
        let mut shaper = self
            .shaper
            .builder(font)
            .size(s.size)
            .variations(&[("wght", s.wght)])
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

    fn mask(&mut self, face: usize, s: &St, glyph: u16) -> Option<GMask> {
        let key = GKey {
            face,
            glyph,
            size: s.size.to_bits(),
            wght: s.wght.to_bits(),
        };
        if let Some(m) = self.cache.get(&key) {
            return m.clone();
        }
        let font = self.faces[face].r();
        let mut scaler = self
            .scaler
            .builder(font)
            .size(s.size)
            .hint(true)
            .variations(&[("wght", s.wght)])
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

    /// Text clipped at `right`, with the last `fade` pixels faded into
    /// `bg`. Used for the at-rest state of a title that scrolls.
    #[allow(clippy::too_many_arguments)]
    pub fn text_fade(
        &mut self,
        s: &St,
        text: &str,
        x: f32,
        base: f32,
        color: Rgb,
        right: f32,
        fade: f32,
        bg: Rgb,
    ) -> bool {
        let w = self.measure(s, text);
        if x + w <= right {
            self.text(s, text, x, base, color);
            return false;
        }
        let (glyphs, _) = self.layout(0, s, text);
        let (ox, oy) = (x.round() as i32, base.round() as i32);
        let right_i = right.round() as i32;
        for (g, gx) in glyphs {
            let Some(m) = self.mask(0, s, g) else {
                continue;
            };
            let gx0 = ox + gx + m.left;
            if gx0 >= right_i {
                break;
            }
            // Fade each column by its distance from the clip edge.
            let mut alpha = m.alpha.clone();
            for row in 0..m.height as usize {
                for colx in 0..m.width as usize {
                    let px = gx0 + colx as i32;
                    let d = (right_i - px) as f32;
                    let k = if d <= 0.0 { 0.0 } else { (d / fade).min(1.0) };
                    let i = row * m.width as usize + colx;
                    alpha[i] = (f32::from(alpha[i]) * k * k).round() as u8;
                }
            }
            self.c
                .blend_mask(gx0, oy - m.top, m.width, m.height, &alpha, color);
        }
        let _ = bg;
        true
    }

    /// Right-aligned text ending at `right`.
    pub fn text_r(&mut self, s: &St, text: &str, right: f32, base: f32, color: Rgb) -> f32 {
        let w = self.measure(s, text);
        self.text(s, text, right - w, base, color);
        w
    }

    /// Centred text.
    pub fn text_c(&mut self, s: &St, text: &str, cx: f32, base: f32, color: Rgb) -> f32 {
        let w = self.measure(s, text);
        self.text(s, text, (cx - w / 2.0).round(), base, color);
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

    /// Draws an icon glyph centred by its ink box on (`cx`, `cy`).
    /// Returns the ink width.
    pub fn icon(&mut self, face: Face, ch: char, size: f32, cx: f32, cy: f32, color: Rgb) -> f32 {
        let fi = face_index(face);
        let s = st(size, 400.0, 0.0);
        let glyph = self.faces[fi].r().charmap().map(ch);
        let Some(m) = self.mask(fi, &s, glyph) else {
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

    pub fn fill_clipped(&mut self, path: &Path, c: Rgb, a: u8, clip: &Path) {
        let (cw, ch) = (self.c.width(), self.c.height());
        let mut m = Mask::new(cw, ch).expect("mask");
        m.fill_path(clip, FillRule::Winding, true, Transform::identity());
        self.c.pixmap_mut().fill_path(
            path,
            &paint(c, a),
            FillRule::Winding,
            Transform::identity(),
            Some(&m),
        );
    }

    pub fn stroke(&mut self, path: &Path, c: Rgb, a: u8, width: f32, dash: Option<(f32, f32)>) {
        let stroke = Stroke {
            width,
            line_cap: tiny_skia::LineCap::Round,
            line_join: tiny_skia::LineJoin::Round,
            dash: dash.and_then(|(on, off)| StrokeDash::new(vec![on, off], 0.0)),
            ..Stroke::default()
        };
        self.c
            .pixmap_mut()
            .stroke_path(path, &paint(c, a), &stroke, Transform::identity(), None);
    }

    pub fn rrect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, c: Rgb) {
        if w <= 0.0 || h <= 0.0 {
            return;
        }
        if let Some(p) = rrect_path(x, y, w, h, r) {
            self.fill(&p, c, 255);
        }
    }

    /// A rounded outline on the half-pixel so a 1 px line is crisp.
    #[allow(clippy::too_many_arguments)]
    pub fn rrect_line(
        &mut self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        c: Rgb,
        width: f32,
        dash: Option<(f32, f32)>,
    ) {
        let i = width / 2.0;
        if let Some(p) = rrect_path(x + i, y + i, w - width, h - width, r - i) {
            self.stroke(&p, c, 255, width, dash);
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

    /// A soft glow: a radial gradient from `c` at `a` to transparent.
    pub fn glow(&mut self, cx: f32, cy: f32, r: f32, c: Rgb, a: u8) {
        let Some(shader) = RadialGradient::new(
            Point::from_xy(cx, cy),
            0.0,
            Point::from_xy(cx, cy),
            r,
            vec![
                GradientStop::new(0.0, col(c, a)),
                GradientStop::new(1.0, col(c, 0)),
            ],
            SpreadMode::Pad,
            Transform::identity(),
        ) else {
            return;
        };
        let p = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, r);
        if let Some(path) = pb.finish() {
            self.c.pixmap_mut().fill_path(
                &path,
                &p,
                FillRule::Winding,
                Transform::identity(),
                None,
            );
        }
    }

    /// Abstract generated artwork placeholder, never real album art: a warm
    /// two-colour field with soft rounded hills and a sun, seeded by a string.
    pub fn art(&mut self, x: f32, y: f32, s: f32, r: f32, seed: &str, dim: bool) {
        let h = seed.bytes().fold(2_166_136_261u32, |a, b| {
            (a ^ u32::from(b)).wrapping_mul(16_777_619)
        });
        let sets = [
            (
                Rgb::hex(0x3b2f6e),
                Rgb::hex(0xe0795a),
                Rgb::hex(0xffd29a),
                Rgb::hex(0x2a1f45),
            ),
            (
                Rgb::hex(0x1f4a52),
                Rgb::hex(0x8ccf9e),
                Rgb::hex(0xfff1c4),
                Rgb::hex(0x173338),
            ),
            (
                Rgb::hex(0x5a2540),
                Rgb::hex(0xf2a65a),
                Rgb::hex(0xfff0d6),
                Rgb::hex(0x3a1729),
            ),
        ];
        let (top, bottom, sun, hill) = sets[(h % 3) as usize];
        let dimc = |c: Rgb| {
            if dim {
                mix(c, Rgb::hex(0x241f1b), 0.55)
            } else {
                c
            }
        };
        let (top, bottom, sun, hill) = (dimc(top), dimc(bottom), dimc(sun), dimc(hill));
        let Some(clip_path) = rrect_path(x, y, s, s, r) else {
            return;
        };
        let (cw, ch) = (self.c.width(), self.c.height());
        let mut clip = Mask::new(cw, ch).expect("mask");
        clip.fill_path(&clip_path, FillRule::Winding, true, Transform::identity());
        let shader = LinearGradient::new(
            Point::from_xy(x, y),
            Point::from_xy(x, y + s),
            vec![
                GradientStop::new(0.0, col(top, 255)),
                GradientStop::new(1.0, col(bottom, 255)),
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
        // The sun.
        let sx = x + s * (0.55 + ((h >> 3) % 20) as f32 / 100.0);
        let mut pb = PathBuilder::new();
        pb.push_circle(sx, y + s * 0.42, s * 0.17);
        if let Some(path) = pb.finish() {
            self.c.pixmap_mut().fill_path(
                &path,
                &paint(sun, 255),
                FillRule::Winding,
                Transform::identity(),
                Some(&clip),
            );
        }
        // Two soft hills.
        for (i, (cx, cy, rr)) in [(0.2, 1.15, 0.55), (0.85, 1.25, 0.6)].iter().enumerate() {
            let mut pb = PathBuilder::new();
            pb.push_circle(x + s * cx, y + s * cy, s * rr);
            if let Some(path) = pb.finish() {
                let c = if i == 0 {
                    hill
                } else {
                    mix(hill, bottom, 0.35)
                };
                self.c.pixmap_mut().fill_path(
                    &path,
                    &paint(c, 255),
                    FillRule::Winding,
                    Transform::identity(),
                    Some(&clip),
                );
            }
        }
    }
}
