//! The forecast widget's three variants, drawn by the shipped renderer
//! from generated forecasts (`nexus_core::scenes::outlooks`, never a live
//! fetch), as two sheets at 3×, nearest-neighbour: every variant at 200,
//! 320, 440 and 640 px (`variants-3x.png`), and each beside the clock and
//! the weather now on Home (`home-3x.png`).
//!
//! ```sh
//! cargo run -p nexus-core --example forecast_sheet -- OUT_DIR
//! ```

use std::path::{Path, PathBuf};

use nexus_core::canvas::Canvas;
use nexus_core::fonts::FontChoice;
use nexus_core::scenes::{self, Frame};
use nexus_core::text::TextEngine;
use nexus_core::theme::{Rgb, TextStyle};
use nexus_core::{HEIGHT, WIDTH};

const SCALE: u32 = 3;
const CAPTION: u32 = 34;
const GAP: u32 = 10;
const SHEET_BG: Rgb = Rgb::hex(0x0b0908);
const CAPTION_INK: Rgb = Rgb::hex(0xb7a895);

fn blit(dst: &mut Canvas, src: &Canvas, x0: u32, y0: u32, scale: u32) {
    for y in 0..src.height() {
        for x in 0..src.width() {
            dst.fill_rect(
                x0 + x * scale,
                y0 + y * scale,
                scale,
                scale,
                src.pixel(x, y),
            );
        }
    }
}

/// Frames at 3× under their captions.
fn sheet(frames: &[Frame], path: &Path) {
    let fonts = FontChoice::default().load();
    let caption_fonts = vec![fonts.text.clone(), fonts.numerals.clone()];
    let mut text = TextEngine::new();
    let style = TextStyle {
        size: 22.0,
        weight: 500.0,
        width: 100.0,
        tabular: true,
        tracking: 0.3,
    };
    let row = CAPTION + HEIGHT * SCALE + GAP;
    let mut out = Canvas::new(WIDTH * SCALE, row * frames.len() as u32, SHEET_BG);
    for (i, f) in frames.iter().enumerate() {
        let y0 = i as u32 * row;
        let mut caption = Canvas::new(WIDTH * SCALE, CAPTION, SHEET_BG);
        text.draw(
            &mut caption,
            &caption_fonts,
            &style,
            &f.name,
            (6.0, 25.0),
            CAPTION_INK,
        );
        blit(&mut out, &caption, 0, y0, 1);
        blit(&mut out, &f.canvas, 0, y0 + CAPTION, SCALE);
    }
    out.save_png(path).expect("write the sheet");
    println!("{}", path.display());
}

fn main() {
    let dir: PathBuf = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: forecast_sheet OUT_DIR");
    std::fs::create_dir_all(&dir).expect("create the output folder");
    let (variants, home) = scenes::forecast_sheets();
    sheet(&variants, &dir.join("variants-3x.png"));
    sheet(&home, &dir.join("home-3x.png"));
    // Each frame on its own too, at 1×, for a closer look.
    let one = dir.join("frames");
    std::fs::create_dir_all(&one).expect("create the frames folder");
    for f in variants.iter().chain(&home) {
        let name = f.name.replace(" · ", "_").replace(" + ", "-");
        f.canvas
            .save_png(one.join(format!("{name}.png")))
            .expect("write a frame");
    }
}
