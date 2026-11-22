//! Signals as people know them: "CPU temperature" with the CPU chip, not
//! `cpu.temp`. The raw name stays available as secondary text, for those
//! who write their own sources.

/// A signal's name and icon for the window.
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    pub name: String,
    /// A Phosphor glyph, the same one the strip draws for this reading.
    pub icon: char,
}

/// Phosphor code points used in the window (Fill weight for things).
pub mod icon {
    pub const CPU: char = '\u{e610}';
    pub const GPU: char = '\u{e612}';
    pub const MEMORY: char = '\u{e9c4}';
    pub const GAUGE: char = '\u{e628}';
    pub const DOWN: char = '\u{e03e}';
    pub const UP: char = '\u{e08e}';
    pub const CLOUD_SUN: char = '\u{e540}';
    pub const SPEAKER: char = '\u{e44a}';
    pub const SPEAKER_X: char = '\u{e45c}';
    pub const MUSIC: char = '\u{e33c}';
    pub const PLAY: char = '\u{e3d0}';
    pub const CLOCK: char = '\u{e19a}';
    pub const THERMOMETER: char = '\u{e5c6}';
    pub const LIGHTNING: char = '\u{e2de}';
    pub const SLIDERS: char = '\u{e434}';
    pub const KEY: char = '\u{e2d6}';
}

/// What the built-in sources publish, in the order a picker lists them.
const KNOWN: [(&str, &str, char); 15] = [
    ("cpu.temp", "CPU temperature", icon::CPU),
    ("cpu.load", "CPU load", icon::GAUGE),
    ("gpu.temp", "GPU temperature", icon::GPU),
    ("gpu.load", "GPU load", icon::GAUGE),
    ("mem.used", "Memory in use", icon::MEMORY),
    ("net.down", "Download speed", icon::DOWN),
    ("net.up", "Upload speed", icon::UP),
    ("weather.temp", "Outside temperature", icon::CLOUD_SUN),
    ("volume.level", "Volume", icon::SPEAKER),
    ("volume.muted", "Muted", icon::SPEAKER_X),
    ("media.title", "Track title", icon::MUSIC),
    ("media.artist", "Artist", icon::MUSIC),
    ("media.status", "Playing or paused", icon::PLAY),
    ("media.position", "Track position", icon::PLAY),
    ("media.length", "Track length", icon::PLAY),
];

/// The signal's human name and icon. One the app does not know (a user's
/// own source) is named from its parts: `room.humidity` is "Room humidity".
pub fn named(signal: &str) -> Named {
    let signal = signal.trim();
    if let Some((_, name, icon)) = KNOWN.iter().find(|(id, _, _)| *id == signal) {
        return Named {
            name: (*name).into(),
            icon: *icon,
        };
    }
    let words: Vec<String> = signal
        .split(['.', '_', '-'])
        .filter(|w| !w.is_empty())
        .map(str::to_string)
        .collect();
    let mut name = words.join(" ");
    if let Some(first) = name.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    let icon = if signal.contains("temp") {
        icon::THERMOMETER
    } else {
        icon::LIGHTNING
    };
    Named {
        name: if name.is_empty() {
            "Nothing chosen".into()
        } else {
            name
        },
        icon,
    }
}

/// Where a signal sorts in a picker: the known ones first, in their
/// order, then the rest by name.
pub fn order(signal: &str) -> (usize, String) {
    let at = KNOWN
        .iter()
        .position(|(id, _, _)| *id == signal)
        .unwrap_or(KNOWN.len());
    (at, signal.to_string())
}

/// Every known signal, for a picker that lists them even before a source
/// has published them.
pub fn known() -> impl Iterator<Item = &'static str> {
    KNOWN.iter().map(|(id, _, _)| *id)
}

/// Whether `query` finds `signal`: by its human name or its raw name,
/// any case, every word somewhere.
pub fn matches(signal: &str, query: &str) -> bool {
    let hay = format!("{} {}", named(signal).name, signal).to_lowercase();
    query
        .split_whitespace()
        .all(|w| hay.contains(&w.to_lowercase()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_signals_have_plain_names_and_the_strips_icons() {
        assert_eq!(named("cpu.temp").name, "CPU temperature");
        assert_eq!(named("cpu.temp").icon, '\u{e610}');
        assert_eq!(named(" mem.used ").name, "Memory in use");
    }

    #[test]
    fn unknown_signals_are_named_from_their_parts() {
        assert_eq!(named("room.humidity").name, "Room humidity");
        assert_eq!(named("desk_temp").icon, icon::THERMOMETER);
        assert_eq!(named("").name, "Nothing chosen");
    }

    #[test]
    fn search_finds_by_either_name() {
        assert!(matches("cpu.temp", "temperature"));
        assert!(matches("cpu.temp", "CPU TEMP"));
        assert!(matches("gpu.load", "gpu.l"));
        assert!(!matches("cpu.temp", "gpu"));
    }

    #[test]
    fn known_signals_sort_first() {
        let mut list = vec!["zz.custom", "mem.used", "cpu.temp", "aa.custom"];
        list.sort_by_key(|s| order(s));
        assert_eq!(list, ["cpu.temp", "mem.used", "aa.custom", "zz.custom"]);
    }
}
