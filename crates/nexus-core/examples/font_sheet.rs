//! Renders the System page once per font and stacks the results into a sheet,
//! so fonts can be compared on the exact pixels the strip would show.
//!
//! Usage:
//!   cargo run -p nexus-core --example font_sheet -- <out-dir> <icon-font> <fallback-font> <font>...
//!
//! Writes `<out-dir>/<font-name>.png` at panel size (for showing on the strip)
//! and `<out-dir>/sheet.png`, all fonts stacked and scaled 3× without smoothing.

use std::path::{Path, PathBuf};

use nexus_core::text::Font;
use nexus_core::theme::Theme;
use nexus_core::widget::{Fonts, Graph, Renderer, Severity, WidgetView, Zone};
use nexus_core::{HEIGHT, WIDTH};
use tiny_skia::{Color, Pixmap};

const SCALE: u32 = 3;
/// Gap between rows on the sheet, in panel pixels.
const GAP: u32 = 4;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 4 {
        eprintln!("usage: font_sheet <out-dir> <icon-font> <fallback-font> <font>...");
        std::process::exit(2);
    }
    let out_dir = PathBuf::from(&args[0]);
    std::fs::create_dir_all(&out_dir).expect("create output directory");
    let icon = load(Path::new(&args[1]));
    let fallback = load(Path::new(&args[2]));

    let pages: Vec<(String, Vec<u8>)> = args[3..]
        .iter()
        .map(|path| {
            let path = Path::new(path);
            let name = path.file_stem().unwrap().to_string_lossy().into_owned();
            let fonts = Fonts {
                text: load(path),
                fallback: vec![fallback.clone()],
                icon: icon.clone(),
            };
            let mut renderer = Renderer::new(Theme::default(), fonts);
            let frame = renderer.render_page(&system_page());
            frame
                .save_png(out_dir.join(format!("{name}.png")))
                .expect("save page");
            (name, frame.rgba().to_vec())
        })
        .collect();

    let rows = pages.len() as u32;
    let mut sheet = Pixmap::new(WIDTH * SCALE, (rows * (HEIGHT + GAP)) * SCALE).unwrap();
    sheet.fill(Color::from_rgba8(40, 40, 40, 255));
    for (row, (_, rgba)) in pages.iter().enumerate() {
        let top = row as u32 * (HEIGHT + GAP);
        blit_scaled(&mut sheet, rgba, top);
    }
    sheet
        .save_png(out_dir.join("sheet.png"))
        .expect("save sheet");

    for (row, (name, _)) in pages.iter().enumerate() {
        println!("row {}: {name}", row + 1);
    }
}

fn load(path: &Path) -> Font {
    let bytes = std::fs::read(path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    Font::from_bytes(bytes).unwrap_or_else(|| panic!("not a font: {}", path.display()))
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

/// The System page from the current app, with representative data.
fn system_page() -> Vec<Zone> {
    let flat: Vec<f32> = (0..60)
        .map(|i| 0.12 + 0.03 * ((i as f32) * 0.7).sin())
        .collect();
    let spiky: Vec<f32> = (0..60)
        .map(|i| match i {
            6 => 0.85,
            7 => 0.4,
            8 => 0.2,
            _ => 0.08 + 0.02 * ((i as f32) * 1.3).sin(),
        })
        .collect();
    vec![
        Zone {
            width: 128,
            view: WidgetView::Clock {
                date: "Tue, Jun 09".into(),
                time: "4:17".into(),
                meridiem: Some("PM".into()),
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Value {
                label: "Jersey City".into(),
                value: "79".into(),
                unit: "°F".into(),
                icon: Some('\u{f185}'),
                graph: None,
                severity: Severity::Ok,
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Value {
                label: "CPU".into(),
                value: "28".into(),
                unit: "°C".into(),
                icon: None,
                graph: Some(Graph::Area(flat.clone())),
                severity: Severity::Ok,
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Value {
                label: "GPU".into(),
                value: "82".into(),
                unit: "°C".into(),
                icon: None,
                graph: Some(Graph::Area(flat)),
                severity: Severity::Warn,
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Rate {
                label: "Net".into(),
                caption: "↓292K/s ↑309K/s".into(),
                graph: Some(Graph::Line(spiky)),
            },
        },
    ]
}
