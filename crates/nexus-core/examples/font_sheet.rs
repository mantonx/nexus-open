//! Renders the System page once per bundled font choice and stacks the
//! results into a sheet, so the choices can be compared on the exact pixels
//! the strip would show.
//!
//! Usage:
//!   cargo run -p nexus-core --example font_sheet -- <out-dir>
//!
//! Writes `<out-dir>/<font-id>.png` at panel size (for showing on the strip)
//! and `<out-dir>/sheet.png`, all choices stacked and scaled 3× without
//! smoothing.

use std::path::PathBuf;

use nexus_core::demo::system_page;
use nexus_core::fonts::FontChoice;
use nexus_core::theme::Theme;
use nexus_core::widget::Renderer;
use nexus_core::{HEIGHT, WIDTH};
use tiny_skia::{Color, Pixmap};

const SCALE: u32 = 3;
/// Gap between rows on the sheet, in panel pixels.
const GAP: u32 = 4;

fn main() {
    let Some(out_dir) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: font_sheet <out-dir>");
        std::process::exit(2);
    };
    std::fs::create_dir_all(&out_dir).expect("create output directory");

    let rows = FontChoice::ALL.len() as u32;
    let mut sheet = Pixmap::new(WIDTH * SCALE, (rows * (HEIGHT + GAP)) * SCALE).unwrap();
    sheet.fill(Color::from_rgba8(40, 40, 40, 255));

    for (row, choice) in FontChoice::ALL.into_iter().enumerate() {
        let mut renderer = Renderer::new(Theme::default(), choice.load());
        let frame = renderer.render_page(&system_page());
        frame
            .save_png(out_dir.join(format!("{}.png", choice.id())))
            .expect("save page");
        blit_scaled(&mut sheet, frame.rgba(), row as u32 * (HEIGHT + GAP));
        println!("row {}: {}", row + 1, choice.display_name());
    }
    sheet
        .save_png(out_dir.join("sheet.png"))
        .expect("save sheet");
}

/// Copies a panel-sized RGBA frame into the sheet at row `top`, scaled up by
/// pixel replication so every panel pixel stays a sharp square.
fn blit_scaled(sheet: &mut Pixmap, rgba: &[u8], top: u32) {
    let sheet_w = sheet.width();
    let data = sheet.data_mut();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let src = ((y * WIDTH + x) * 4) as usize;
            for dy in 0..SCALE {
                for dx in 0..SCALE {
                    let sy = (top + y) * SCALE + dy;
                    let sx = x * SCALE + dx;
                    let dst = ((sy * sheet_w + sx) * 4) as usize;
                    data[dst..dst + 4].copy_from_slice(&rgba[src..src + 4]);
                }
            }
        }
    }
}
