//! Shapes for the widgets: rounded rectangles and their outlines, discs, a
//! soft glow, fills clipped to a shape, and the generated artwork
//! placeholder. All antialiased through tiny-skia onto the `Canvas`.

use tiny_skia::{
    Color, FillRule, GradientStop, LinearGradient, Mask, Paint, Path, PathBuilder, Point,
    RadialGradient, SpreadMode, Stroke, StrokeDash, Transform,
};

use crate::canvas::Canvas;
use crate::theme::{CARD, Rgb};

fn colour(c: Rgb, a: u8) -> Color {
    Color::from_rgba8(c.r, c.g, c.b, a)
}

fn paint(c: Rgb, a: u8) -> Paint<'static> {
    let mut p = Paint::default();
    p.set_color(colour(c, a));
    p.anti_alias = true;
    p
}

/// A rounded rectangle; the radius is limited to half the shorter side.
pub fn rrect_path(x: f32, y: f32, w: f32, h: f32, r: f32) -> Option<Path> {
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let k = 0.552_284_8 * r;
    let mut pb = PathBuilder::new();
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

pub fn fill(c: &mut Canvas, path: &Path, colour: Rgb, alpha: u8) {
    c.pixmap_mut().fill_path(
        path,
        &paint(colour, alpha),
        FillRule::Winding,
        Transform::identity(),
        None,
    );
}

/// Fills `path` only where it is inside `clip`.
pub fn fill_clipped(c: &mut Canvas, path: &Path, colour: Rgb, clip: &Path) {
    let Some(mut mask) = Mask::new(c.width(), c.height()) else {
        return;
    };
    mask.fill_path(clip, FillRule::Winding, true, Transform::identity());
    c.pixmap_mut().fill_path(
        path,
        &paint(colour, 255),
        FillRule::Winding,
        Transform::identity(),
        Some(&mask),
    );
}

pub fn rrect(c: &mut Canvas, x: f32, y: f32, w: f32, h: f32, r: f32, colour: Rgb) {
    if let Some(p) = rrect_path(x, y, w, h, r) {
        fill(c, &p, colour, 255);
    }
}

/// A rounded outline `width` thick, inside the box, optionally dashed
/// (on, off).
#[allow(clippy::too_many_arguments)]
pub fn rrect_line(
    c: &mut Canvas,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    r: f32,
    colour: Rgb,
    width: f32,
    dash: Option<(f32, f32)>,
) {
    let i = width / 2.0;
    let Some(path) = rrect_path(x + i, y + i, w - width, h - width, r - i) else {
        return;
    };
    let stroke = Stroke {
        width,
        line_cap: tiny_skia::LineCap::Round,
        line_join: tiny_skia::LineJoin::Round,
        dash: dash.and_then(|(on, off)| StrokeDash::new(vec![on, off], 0.0)),
        ..Stroke::default()
    };
    c.pixmap_mut().stroke_path(
        &path,
        &paint(colour, 255),
        &stroke,
        Transform::identity(),
        None,
    );
}

pub fn circle(c: &mut Canvas, cx: f32, cy: f32, r: f32, colour: Rgb) {
    let mut pb = PathBuilder::new();
    pb.push_circle(cx, cy, r);
    if let Some(p) = pb.finish() {
        fill(c, &p, colour, 255);
    }
}

/// A soft glow: `colour` at `alpha` in the middle, fading out by `r`.
pub fn glow(c: &mut Canvas, cx: f32, cy: f32, r: f32, colour_: Rgb, alpha: u8) {
    let Some(shader) = RadialGradient::new(
        Point::from_xy(cx, cy),
        0.0,
        Point::from_xy(cx, cy),
        r,
        vec![
            GradientStop::new(0.0, colour(colour_, alpha)),
            GradientStop::new(1.0, colour(colour_, 0)),
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
        c.pixmap_mut()
            .fill_path(&path, &p, FillRule::Winding, Transform::identity(), None);
    }
}

/// Artwork when the player gives none: a warm two-colour sky, a sun and
/// two soft hills, picked by `seed` (the title) so a track keeps its
/// picture. Never real artwork. `dim` mixes it toward the card, as for a
/// paused track.
#[allow(clippy::too_many_arguments)]
pub fn art(c: &mut Canvas, x: f32, y: f32, s: f32, r: f32, seed: &str, dim: bool) {
    let h = seed.bytes().fold(2_166_136_261u32, |a, b| {
        (a ^ u32::from(b)).wrapping_mul(16_777_619)
    });
    const SETS: [[Rgb; 4]; 3] = [
        [
            Rgb::hex(0x3b2f6e),
            Rgb::hex(0xe0795a),
            Rgb::hex(0xffd29a),
            Rgb::hex(0x2a1f45),
        ],
        [
            Rgb::hex(0x1f4a52),
            Rgb::hex(0x8ccf9e),
            Rgb::hex(0xfff1c4),
            Rgb::hex(0x173338),
        ],
        [
            Rgb::hex(0x5a2540),
            Rgb::hex(0xf2a65a),
            Rgb::hex(0xfff0d6),
            Rgb::hex(0x3a1729),
        ],
    ];
    let [top, bottom, sun, hill] =
        SETS[(h % 3) as usize].map(|k| if dim { k.mix(CARD, 0.55) } else { k });
    let Some(clip_path) = rrect_path(x, y, s, s, r) else {
        return;
    };
    let Some(mut clip) = Mask::new(c.width(), c.height()) else {
        return;
    };
    clip.fill_path(&clip_path, FillRule::Winding, true, Transform::identity());
    if let Some(shader) = LinearGradient::new(
        Point::from_xy(x, y),
        Point::from_xy(x, y + s),
        vec![
            GradientStop::new(0.0, colour(top, 255)),
            GradientStop::new(1.0, colour(bottom, 255)),
        ],
        SpreadMode::Pad,
        Transform::identity(),
    ) {
        let p = Paint {
            shader,
            anti_alias: true,
            ..Paint::default()
        };
        c.pixmap_mut().fill_path(
            &clip_path,
            &p,
            FillRule::Winding,
            Transform::identity(),
            None,
        );
    }
    let mut disc = |cx: f32, cy: f32, rr: f32, k: Rgb| {
        let mut pb = PathBuilder::new();
        pb.push_circle(cx, cy, rr);
        if let Some(path) = pb.finish() {
            c.pixmap_mut().fill_path(
                &path,
                &paint(k, 255),
                FillRule::Winding,
                Transform::identity(),
                Some(&clip),
            );
        }
    };
    disc(
        x + s * (0.55 + ((h >> 3) % 20) as f32 / 100.0),
        y + s * 0.42,
        s * 0.17,
        sun,
    );
    disc(x + s * 0.2, y + s * 1.15, s * 0.55, hill);
    disc(x + s * 0.85, y + s * 1.25, s * 0.6, hill.mix(bottom, 0.35));
}
