//! Whether the starters were offered already. A fresh store is offered
//! them once; a note beside the window's size says it was, so choosing
//! "Keep the one on the strip" is not asked again.

use std::path::PathBuf;

fn path() -> Option<PathBuf> {
    let dir = dirs::state_dir().or_else(dirs::data_local_dir)?;
    Some(dir.join("nexusd").join("starters-offered"))
}

pub fn seen() -> bool {
    path().is_some_and(|p| p.exists())
}

/// Failing to write only means the offer may come once more.
pub fn mark_seen() {
    if let Some(p) = path() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, "");
    }
}
