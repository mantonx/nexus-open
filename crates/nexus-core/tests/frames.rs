//! Image tests: the strip as the design reference shows it, drawn by the
//! shipped renderer from real layouts and readings (`nexus_core::scenes`),
//! compared with the frames checked in under `tests/frames/`.
//!
//! Every scene, default page and takeover, every widget specimen (each
//! size class in one frame, one frame per state) and the scenes in two
//! accents. A frame matches when almost every pixel is within a few levels
//! per channel, which absorbs float rounding but not a moved edge.
//!
//! After a deliberate change to the drawing, look at the new frames
//! (`cargo run -p nexus-core --example design_system -- OUT`) and then
//! rewrite the checked-in ones:
//!
//! ```sh
//! NEXUS_UPDATE_FRAMES=1 cargo test -p nexus-core --test frames
//! ```

use std::path::{Path, PathBuf};

use nexus_core::canvas::Canvas;
use nexus_core::scenes::{self, Frame};
use nexus_core::theme::Rgb;
use nexus_core::{HEIGHT, WIDTH};

/// Two accents beside the default lilac: a cool one, and a warm one that
/// lands in the band kept for warning and snaps out of it.
const ACCENTS: [(&str, Rgb); 2] = [
    ("sky", Rgb::hex(0x5fb4ff)),
    ("marigold", Rgb::hex(0xffc53a)),
];

/// A channel may differ by this much before the pixel counts as changed.
const CHANNEL_TOLERANCE: u8 = 6;
/// Changed pixels a frame may have: 0.1 % of the panel.
const CHANGED_PIXELS: usize = (WIDTH * HEIGHT / 1000) as usize;

fn golden_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/frames")
}

fn load(path: &Path) -> Option<Vec<Rgb>> {
    let data = std::fs::read(path).ok()?;
    let pm = tiny_skia::Pixmap::decode_png(&data).ok()?;
    if (pm.width(), pm.height()) != (WIDTH, HEIGHT) {
        return None;
    }
    Some(
        pm.pixels()
            .iter()
            .map(|px| {
                let p = px.demultiply();
                Rgb {
                    r: p.red(),
                    g: p.green(),
                    b: p.blue(),
                }
            })
            .collect(),
    )
}

fn pixels(c: &Canvas) -> Vec<Rgb> {
    (0..HEIGHT)
        .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
        .map(|(x, y)| c.pixel(x, y))
        .collect()
}

/// Pixels whose largest channel difference is over the tolerance, and that
/// largest difference.
fn changed(a: &[Rgb], b: &[Rgb]) -> (usize, u8) {
    let mut count = 0;
    let mut worst = 0;
    for (p, q) in a.iter().zip(b) {
        let d =
            p.r.abs_diff(q.r)
                .max(p.g.abs_diff(q.g))
                .max(p.b.abs_diff(q.b));
        worst = worst.max(d);
        if d > CHANNEL_TOLERANCE {
            count += 1;
        }
    }
    (count, worst)
}

fn tested(frames: Vec<Frame>) -> Vec<Frame> {
    frames
        .into_iter()
        .filter(|f| {
            f.group != "accents"
                || ACCENTS
                    .iter()
                    .any(|(name, _)| f.name.starts_with(&format!("accent-{name}-")))
        })
        .collect()
}

#[test]
fn frames_match_the_checked_in_images() {
    let dir = golden_dir();
    let frames = tested(scenes::frames(&ACCENTS));
    assert!(frames.len() > 50, "only {} frames", frames.len());
    if std::env::var_os("NEXUS_UPDATE_FRAMES").is_some() {
        std::fs::create_dir_all(&dir).expect("create tests/frames");
        for f in &frames {
            f.canvas
                .save_png(dir.join(format!("{}.png", f.name)))
                .expect("write frame");
        }
        return;
    }
    let out = Path::new(env!("CARGO_TARGET_TMPDIR")).join("frames");
    let mut failures = Vec::new();
    for f in &frames {
        let path = dir.join(format!("{}.png", f.name));
        let Some(want) = load(&path) else {
            failures.push(format!("{}: no checked-in frame", f.name));
            continue;
        };
        let (count, worst) = changed(&want, &pixels(&f.canvas));
        if count > CHANGED_PIXELS {
            std::fs::create_dir_all(&out).expect("create output dir");
            let got = out.join(format!("{}.png", f.name));
            f.canvas.save_png(&got).expect("write frame");
            failures.push(format!(
                "{}: {count} pixels changed (worst by {worst}); drawn: {}",
                f.name,
                got.display()
            ));
        }
    }
    assert!(
        failures.is_empty(),
        "{} of {} frames differ from tests/frames:\n  {}",
        failures.len(),
        frames.len(),
        failures.join("\n  ")
    );
}

#[test]
fn the_tolerance_catches_a_moved_edge() {
    let frames = scenes::frames(&[]);
    let f = frames
        .iter()
        .find(|f| f.name == "default-p1-home")
        .expect("home page frame");
    let a = pixels(&f.canvas);
    // The same frame shifted one pixel right.
    let b: Vec<Rgb> = (0..HEIGHT as usize)
        .flat_map(|y| {
            let row = &a[y * WIDTH as usize..(y + 1) * WIDTH as usize];
            std::iter::once(row[0]).chain(row[..row.len() - 1].iter().copied())
        })
        .collect();
    assert!(changed(&a, &b).0 > CHANGED_PIXELS);
}
