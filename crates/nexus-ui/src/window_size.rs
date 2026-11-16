//! Remembers the window's size between runs, in logical pixels so that it
//! is the same size after the desktop's scaling changes.

use std::path::PathBuf;

use slint::{ComponentHandle, LogicalSize};

use crate::AppWindow;

/// Sizes outside these are not a size anyone chose; ignore them.
const SMALLEST: f32 = 300.0;
const LARGEST: f32 = 16_384.0;

fn path() -> Option<PathBuf> {
    let dir = dirs::state_dir().or_else(dirs::data_local_dir)?;
    Some(dir.join("nexusd").join("window-size"))
}

/// "1340x820" as a size, if it is a sensible one.
pub fn parse(text: &str) -> Option<(f32, f32)> {
    let (w, h) = text.trim().split_once('x')?;
    let (w, h): (f32, f32) = (w.trim().parse().ok()?, h.trim().parse().ok()?);
    let ok = |v: f32| (SMALLEST..=LARGEST).contains(&v);
    (ok(w) && ok(h)).then_some((w, h))
}

pub fn format(w: f32, h: f32) -> String {
    format!("{}x{}\n", w.round(), h.round())
}

/// Sets the size saved last time, if any. Before the window is shown.
pub fn restore(app: &AppWindow) {
    let saved = path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|t| parse(&t));
    if let Some((w, h)) = saved {
        app.window().set_size(LogicalSize::new(w, h));
    }
}

/// Saves the window's size, when it closes. Failing to save only means
/// the next window opens at the default size.
pub fn save(app: &AppWindow) {
    let Some(path) = path() else {
        return;
    };
    let window = app.window();
    let size = window.size().to_logical(window.scale_factor());
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, format(size.width, size.height));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_round_trip_and_nonsense_is_ignored() {
        assert_eq!(parse(&format(1340.4, 819.6)), Some((1340.0, 820.0)));
        assert_eq!(parse(" 800 x 600 "), Some((800.0, 600.0)));
        assert_eq!(parse("10x10"), None);
        assert_eq!(parse("wide"), None);
        assert_eq!(parse("1e9x600"), None);
    }
}
