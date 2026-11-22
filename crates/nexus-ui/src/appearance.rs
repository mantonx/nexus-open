//! The window's Appearance: match the system, light or dark. A preference
//! of this window only, so it is kept in the window's own config file, not
//! in the layout: the strip stays dark whatever it says, and choosing it
//! never enters the layout's undo history.

use std::path::PathBuf;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Appearance {
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    /// As the window's Theme.forced takes it: 0, 1 or 2.
    pub fn index(self) -> i32 {
        match self {
            Appearance::System => 0,
            Appearance::Light => 1,
            Appearance::Dark => 2,
        }
    }

    pub fn from_index(i: i32) -> Self {
        match i {
            1 => Appearance::Light,
            2 => Appearance::Dark,
            _ => Appearance::System,
        }
    }

    fn word(self) -> &'static str {
        match self {
            Appearance::System => "system",
            Appearance::Light => "light",
            Appearance::Dark => "dark",
        }
    }
}

/// `$XDG_CONFIG_HOME/nexus-open/window`, beside nothing the daemon reads.
fn path() -> Option<PathBuf> {
    Some(dirs::config_dir()?.join("nexus-open").join("window"))
}

/// The file's text as a choice. Anything not understood is the default,
/// so a damaged file costs only the preference.
pub fn parse(text: &str) -> Appearance {
    for line in text.lines() {
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == "appearance" {
            return match value.trim().trim_matches('"') {
                "light" => Appearance::Light,
                "dark" => Appearance::Dark,
                _ => Appearance::System,
            };
        }
    }
    Appearance::System
}

pub fn format(a: Appearance) -> String {
    format!(
        "# The Nexus Open window's own preferences.\nappearance = \"{}\"\n",
        a.word()
    )
}

pub fn load() -> Appearance {
    path()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map_or(Appearance::System, |t| parse(&t))
}

/// Failing to save only means the next window matches the system.
pub fn save(a: Appearance) {
    let Some(path) = path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    let _ = std::fs::write(path, format(a));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn choices_round_trip_and_anything_else_matches_the_system() {
        for a in [Appearance::System, Appearance::Light, Appearance::Dark] {
            assert_eq!(parse(&format(a)), a);
            assert_eq!(Appearance::from_index(a.index()), a);
        }
        assert_eq!(parse(""), Appearance::System);
        assert_eq!(parse("appearance = purple"), Appearance::System);
        assert_eq!(parse("appearance=dark"), Appearance::Dark);
        assert_eq!(Appearance::from_index(9), Appearance::System);
    }
}
