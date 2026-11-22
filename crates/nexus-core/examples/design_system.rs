//! The design system's reference frames, drawn by the shipped renderer
//! from real layouts and readings (`nexus_core::scenes`), plus a sheet per
//! group at 3× and, given the frames of an earlier run, a comparison per
//! group: the earlier frame above, this one below.
//!
//! ```sh
//! cargo run -p nexus-core --example design_system -- OUT_DIR [EARLIER_DIR]
//! ```
//!
//! Prints the tokens: the accent roles of every preset, and the type scale.

use std::path::{Path, PathBuf};

use nexus_core::canvas::Canvas;
use nexus_core::fonts::FontChoice;
use nexus_core::scenes::{self, Frame};
use nexus_core::text::{Font, TextEngine};
use nexus_core::theme::{
    ACCENT_DEFAULT, ACCENT_PRESETS, Accent, Rgb, TEXT_STYLES, TextStyle, accent_preset_name,
};
use nexus_core::{HEIGHT, WIDTH};

const SCALE: u32 = 3;
const CAPTION: u32 = 34;
const GAP: u32 = 10;
const SHEET_BG: Rgb = Rgb::hex(0x0b0908);
const CAPTION_INK: Rgb = Rgb::hex(0xb7a895);

/// The accents the accent scenes are drawn with: the presets, and the two
/// the design tried that sit in the hues kept for warning and critical,
/// to show where they snap to.
const ACCENTS: [(&str, Rgb); 8] = [
    ("lilac", Rgb::hex(0xb6a2ff)),
    ("sky", Rgb::hex(0x5fb4ff)),
    ("teal", Rgb::hex(0x4fd1c5)),
    ("mint", Rgb::hex(0x7ad9b0)),
    ("rose", Rgb::hex(0xff8fb1)),
    ("lemon", Rgb::hex(0xd8e04a)),
    ("apricot", Rgb::hex(0xff9a62)),
    ("marigold", Rgb::hex(0xffc53a)),
];

struct Captions {
    text: TextEngine,
    fonts: Vec<Font>,
}

impl Captions {
    fn draw(&mut self, c: &mut Canvas, s: &str) {
        let style = TextStyle {
            size: 22.0,
            weight: 500.0,
            width: 100.0,
            tabular: true,
            tracking: 0.3,
        };
        self.text
            .draw(c, &self.fonts, &style, s, (6.0, 25.0), CAPTION_INK);
    }
}

fn blit(dst: &mut Canvas, src: &Canvas, x0: u32, y0: u32, scale: u32) {
    for y in 0..src.height() {
        for x in 0..src.width() {
            let c = src.pixel(x, y);
            dst.fill_rect(x0 + x * scale, y0 + y * scale, scale, scale, c);
        }
    }
}

/// Frames at 3×, nearest-neighbour, each under its caption.
fn sheet(cap: &mut Captions, frames: &[(String, &Canvas)], path: &Path) {
    let row = CAPTION + HEIGHT * SCALE + GAP;
    let mut out = Canvas::new(WIDTH * SCALE, row * frames.len() as u32, SHEET_BG);
    for (i, (name, c)) in frames.iter().enumerate() {
        let y0 = i as u32 * row;
        let mut caption = Canvas::new(WIDTH * SCALE, CAPTION, SHEET_BG);
        cap.draw(&mut caption, name);
        blit(&mut out, &caption, 0, y0, 1);
        blit(&mut out, c, 0, y0 + CAPTION, SCALE);
    }
    out.save_png(path).expect("write sheet");
}

/// Reads a 640×48 PNG written by an earlier run.
fn load(path: &Path) -> Option<Canvas> {
    let data = std::fs::read(path).ok()?;
    let pm = tiny_skia::Pixmap::decode_png(&data).ok()?;
    if (pm.width(), pm.height()) != (WIDTH, HEIGHT) {
        return None;
    }
    let mut c = Canvas::new(WIDTH, HEIGHT, SHEET_BG);
    for (i, px) in pm.pixels().iter().enumerate() {
        let p = px.demultiply();
        let (x, y) = (i as u32 % WIDTH, i as u32 / WIDTH);
        c.fill_rect(
            x,
            y,
            1,
            1,
            Rgb {
                r: p.red(),
                g: p.green(),
                b: p.blue(),
            },
        );
    }
    Some(c)
}

fn print_tokens() {
    let hex = |c: Rgb| format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b);
    println!("accent roles (base, text, fill, soft, ink):");
    for (name, rgb) in ACCENT_PRESETS
        .iter()
        .copied()
        .chain([("apricot", ACCENTS[6].1), ("marigold", ACCENTS[7].1)])
    {
        let a = Accent::from(Some(rgb));
        let preset = accent_preset_name(rgb).map_or("", |_| " (preset)");
        println!(
            "  {name:9} {}{preset}: {} {} {} {} {}",
            hex(rgb),
            hex(a.base),
            hex(a.text),
            hex(a.fill),
            hex(a.soft),
            hex(a.ink),
        );
    }
    println!("  default: {}", hex(ACCENT_DEFAULT));
    println!("type scale (size, weight, tracking):");
    for s in TEXT_STYLES {
        println!("  {:>4} {:>4} {:>5}", s.size, s.weight, s.tracking);
    }
}

fn main() {
    let mut args = std::env::args().skip(1);
    let dir = args
        .next()
        .map_or_else(|| PathBuf::from("design-system-frames"), PathBuf::from);
    let earlier = args.next().map(PathBuf::from);
    std::fs::create_dir_all(&dir).expect("create output dir");
    let frames: Vec<Frame> = scenes::frames(&ACCENTS);
    for f in &frames {
        f.canvas
            .save_png(dir.join(format!("{}.png", f.name)))
            .expect("write frame");
    }
    let fonts = FontChoice::default().load();
    let mut cap = Captions {
        text: TextEngine::new(),
        fonts: vec![fonts.text, fonts.numerals],
    };
    for (group, file) in [
        ("scenes", "scenes"),
        ("default", "default"),
        ("takeover", "takeovers"),
        ("specimen", "specimens"),
        ("accents", "accents"),
    ] {
        let in_group: Vec<&Frame> = frames.iter().filter(|f| f.group == group).collect();
        let list: Vec<(String, &Canvas)> = in_group
            .iter()
            .map(|f| (f.name.clone(), &f.canvas))
            .collect();
        sheet(&mut cap, &list, &dir.join(format!("sheet-{file}.png")));
        let Some(earlier) = &earlier else { continue };
        // The earlier frame above, this one below; a frame it lacks is
        // left out.
        let olds: Vec<Canvas> = in_group
            .iter()
            .filter_map(|f| load(&earlier.join(format!("{}.png", f.name))))
            .collect();
        let mut pairs: Vec<(String, &Canvas)> = Vec::new();
        let mut olds = olds.iter();
        for f in &in_group {
            if earlier.join(format!("{}.png", f.name)).exists()
                && let Some(old) = olds.next()
            {
                pairs.push((format!("{} — reference", f.name), old));
                pairs.push((format!("{} — shipped", f.name), &f.canvas));
            }
        }
        if !pairs.is_empty() {
            sheet(&mut cap, &pairs, &dir.join(format!("compare-{file}.png")));
        }
    }
    print_tokens();
}
