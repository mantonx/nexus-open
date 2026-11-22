//! Frames for `nexusd preview --card` and `--frames`: the legibility card,
//! or a folder of 640×48 PNG mockups, ready to send to the strip.
//!
//! Nothing here touches the device, so it builds on every platform and the
//! folder rules are tested without hardware.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use nexus_core::card;
use nexus_core::{HEIGHT, WIDTH};
use tiny_skia::{Color, Pixmap};

/// One frame to show, and what to print while it is showing.
pub struct Slide {
    pub name: String,
    /// Printed under the "showing" line: the question to ask, the key.
    pub notes: Vec<String>,
    /// 640×48 RGBA, every pixel opaque.
    pub rgba: Vec<u8>,
}

/// The legibility card.
pub fn card() -> Vec<Slide> {
    card::frames()
        .into_iter()
        .map(|f| {
            let mut notes = vec![f.question.to_string()];
            notes.extend(f.key.lines().map(|l| format!("  {l}")));
            Slide {
                name: f.name.to_string(),
                notes,
                rgba: f.canvas.rgba().to_vec(),
            }
        })
        .collect()
}

/// Every `*.png` in `dir` that is exactly 640×48, in file name order.
/// Each file left out is reported with the reason, for the caller to print.
pub fn from_dir(dir: &Path) -> io::Result<(Vec<Slide>, Vec<String>)> {
    let entries = fs::read_dir(dir)?
        .map(|e| e.map(|e| e.path()))
        .collect::<io::Result<Vec<_>>>()?;
    let (mut slides, mut skipped) = (Vec::new(), Vec::new());
    for path in pngs(entries) {
        let name = path
            .file_name()
            .map_or_else(String::new, |n| n.to_string_lossy().into_owned());
        match load(&path) {
            Ok(rgba) => slides.push(Slide {
                name,
                notes: Vec::new(),
                rgba,
            }),
            Err(reason) => skipped.push(format!("skipping {name}: {reason}")),
        }
    }
    Ok((slides, skipped))
}

/// The paths ending in `.png` (any case), sorted by file name.
fn pngs(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut pngs: Vec<PathBuf> = paths
        .into_iter()
        .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("png")))
        .collect();
    pngs.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
    pngs
}

/// Decodes one PNG into a frame, or says why it can't be shown.
fn load(path: &Path) -> Result<Vec<u8>, String> {
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    let pixmap = Pixmap::decode_png(&bytes).map_err(|e| format!("not a readable PNG ({e})"))?;
    check_size(pixmap.width(), pixmap.height())?;
    // tiny-skia stores premultiplied colour, which is the image composited
    // over black: what a transparent pixel looks like on the panel. Setting
    // alpha to opaque keeps that look and the frame's every-pixel-opaque rule.
    let mut rgba = pixmap.take();
    for px in rgba.as_chunks_mut::<4>().0 {
        px[3] = 255;
    }
    Ok(rgba)
}

fn check_size(width: u32, height: u32) -> Result<(), String> {
    if (width, height) == (WIDTH, HEIGHT) {
        Ok(())
    } else {
        Err(format!("{width}×{height}, not {WIDTH}×{HEIGHT}"))
    }
}

/// Scale of the contact sheet: each panel pixel becomes a 3×3 square.
const SHEET_SCALE: u32 = 3;
/// Gap between frames on the sheet, in panel pixels.
const SHEET_GAP: u32 = 4;

/// Writes the card as `NN-name.png` files at panel size and `sheet.png`, all
/// frames stacked and scaled 3× without smoothing, for checking the card
/// without the strip. Returns the paths written.
pub fn write_card(dir: &Path) -> io::Result<Vec<PathBuf>> {
    fs::create_dir_all(dir)?;
    let frames = card::frames();
    let mut written = Vec::new();
    let rows = frames.len() as u32;
    let mut sheet = Pixmap::new(
        WIDTH * SHEET_SCALE,
        rows * (HEIGHT + SHEET_GAP) * SHEET_SCALE,
    )
    .expect("sheet size is non-zero");
    sheet.fill(Color::from_rgba8(40, 40, 40, 255));
    for (i, f) in frames.iter().enumerate() {
        let path = dir.join(format!("{:02}-{}.png", i + 1, f.name));
        f.canvas.save_png(&path)?;
        written.push(path);
        blit_scaled(&mut sheet, f.canvas.rgba(), i as u32 * (HEIGHT + SHEET_GAP));
    }
    let path = dir.join("sheet.png");
    sheet.save_png(&path).map_err(io::Error::other)?;
    written.push(path);
    Ok(written)
}

/// Copies a panel-sized RGBA frame into the sheet at row `top`, scaled up by
/// pixel replication so every panel pixel stays a sharp square.
fn blit_scaled(sheet: &mut Pixmap, rgba: &[u8], top: u32) {
    let sheet_w = sheet.width();
    let data = sheet.data_mut();
    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let src = ((y * WIDTH + x) * 4) as usize;
            for dy in 0..SHEET_SCALE {
                let sy = (top + y) * SHEET_SCALE + dy;
                let dst = ((sy * sheet_w + x * SHEET_SCALE) * 4) as usize;
                for dx in 0..SHEET_SCALE as usize {
                    data[dst + dx * 4..dst + dx * 4 + 4].copy_from_slice(&rgba[src..src + 4]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A fresh, empty folder under the system temp dir for one test.
    fn scratch(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("nexusd-slides-{}-{test}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_png(path: &Path, w: u32, h: u32) {
        let mut p = Pixmap::new(w, h).unwrap();
        p.fill(Color::from_rgba8(200, 10, 10, 255));
        p.save_png(path).unwrap();
    }

    #[test]
    fn pngs_are_picked_by_extension_and_sorted_by_name() {
        let paths = [
            "b.png",
            "a.PNG",
            "notes.txt",
            "c.png.bak",
            "dir/0.png",
            "png",
        ]
        .map(PathBuf::from)
        .to_vec();
        let picked: Vec<_> = pngs(paths)
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        assert_eq!(picked, ["dir/0.png", "a.PNG", "b.png"]);
    }

    #[test]
    fn size_must_be_exactly_the_panel() {
        assert!(check_size(640, 48).is_ok());
        assert_eq!(check_size(1920, 144).unwrap_err(), "1920×144, not 640×48");
        assert!(check_size(48, 640).is_err());
    }

    #[test]
    fn folder_keeps_panel_sized_pngs_and_names_the_rest() {
        let dir = scratch("folder");
        write_png(&dir.join("02-music.png"), 640, 48);
        write_png(&dir.join("01-idle.png"), 640, 48);
        write_png(&dir.join("03-big.png"), 1280, 96);
        fs::write(dir.join("04-broken.png"), b"not a png").unwrap();
        fs::write(dir.join("readme.txt"), b"ignored").unwrap();

        let (slides, skipped) = from_dir(&dir).unwrap();
        let names: Vec<_> = slides.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["01-idle.png", "02-music.png"]);
        assert_eq!(slides[0].rgba.len(), (WIDTH * HEIGHT * 4) as usize);
        assert_eq!(&slides[0].rgba[..4], &[200, 10, 10, 255]);
        assert_eq!(skipped.len(), 2);
        assert_eq!(skipped[0], "skipping 03-big.png: 1280×96, not 640×48");
        assert!(skipped[1].starts_with("skipping 04-broken.png: not a readable PNG"));
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn transparent_pixels_become_opaque_black() {
        let dir = scratch("alpha");
        let mut p = Pixmap::new(640, 48).unwrap();
        p.fill(Color::from_rgba8(255, 255, 255, 0));
        p.save_png(dir.join("clear.png")).unwrap();
        let (slides, _) = from_dir(&dir).unwrap();
        assert_eq!(&slides[0].rgba[..4], &[0, 0, 0, 255]);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn card_slides_carry_their_question() {
        let slides = card();
        assert_eq!(slides.len(), card::frames().len());
        for s in &slides {
            assert_eq!(s.rgba.len(), (WIDTH * HEIGHT * 4) as usize);
            assert!(!s.notes.is_empty(), "{} has no question", s.name);
        }
    }

    #[test]
    fn write_card_writes_every_frame_and_a_sheet() {
        let dir = scratch("out");
        let written = write_card(&dir).unwrap();
        assert_eq!(written.len(), card::frames().len() + 1);
        let sheet = Pixmap::load_png(dir.join("sheet.png")).unwrap();
        assert_eq!(sheet.width(), WIDTH * SHEET_SCALE);
        let first = Pixmap::load_png(&written[0]).unwrap();
        assert_eq!((first.width(), first.height()), (WIDTH, HEIGHT));
        fs::remove_dir_all(&dir).unwrap();
    }
}
