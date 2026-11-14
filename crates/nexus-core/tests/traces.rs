//! Replays recorded touches (from `nexusd trace`) through the gesture
//! recognizer. Each file in `tests/traces/` holds one kind of touch, named
//! by the file's first word, and must produce exactly that:
//!
//! - `tap-*`: one tap and nothing else
//! - `hold-*`: one long press and nothing else
//! - `swipe-*`: drags ending in one release, and no tap
//! - `brush-*`: nothing at all (an accidental touch while typing)
//!
//! A file may hold several touches of its kind; each must match.

use std::fs;
use std::path::Path;

use nexus_core::gesture::{Gesture, Recognizer};

fn load(path: &Path) -> Vec<(f64, bool, f32)> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with('#'))
        .map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            (f[0].parse().unwrap(), f[1] == "1", f[2].parse().unwrap())
        })
        .collect()
}

/// The gestures of each touch in the trace, one list per touch.
fn touches(samples: &[(f64, bool, f32)]) -> Vec<Vec<Gesture>> {
    let mut r = Recognizer::new();
    let mut out: Vec<Vec<Gesture>> = Vec::new();
    let mut was_down = false;
    for &(t, down, x) in samples {
        if down && !was_down {
            out.push(Vec::new());
        }
        if let (Some(g), Some(current)) = (r.update(down, x, t), out.last_mut()) {
            current.push(g);
        }
        was_down = down;
    }
    out
}

#[test]
fn recorded_touches_mean_what_they_are_named() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/traces");
    let mut failures = Vec::new();
    for entry in fs::read_dir(&dir).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().is_none_or(|e| e != "trace") {
            continue;
        }
        let name = path.file_name().unwrap().to_string_lossy().into_owned();
        let kind = name
            .split(['-', '.'])
            .next()
            .unwrap_or_default()
            .to_string();
        for (i, gestures) in touches(&load(&path)).iter().enumerate() {
            let ok = match kind.as_str() {
                "tap" => matches!(gestures[..], [Gesture::Tap { .. }]),
                "hold" => matches!(gestures[..], [Gesture::LongPress { .. }]),
                "swipe" => {
                    matches!(gestures.last(), Some(Gesture::Release { .. }))
                        && !gestures.iter().any(|g| matches!(g, Gesture::Tap { .. }))
                }
                "brush" => gestures.is_empty(),
                other => panic!("{name}: unknown kind {other:?}"),
            };
            if !ok {
                failures.push(format!("{name}, touch {}: {gestures:?}", i + 1));
            }
        }
    }
    assert!(failures.is_empty(), "\n{}", failures.join("\n"));
}
