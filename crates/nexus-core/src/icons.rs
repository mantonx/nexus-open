//! The strip's icons: Phosphor 2.1.2, subset into two bundled fonts (Fill
//! for things, Bold for actions on keys and for empty states). Both fonts
//! share code points, so an icon is a `char` plus the weight to draw it in.
//!
//! Layouts name icons by Phosphor's names (`icon = "fan"`); `NAMES` is
//! every icon the bundled subset has.

/// Phosphor code points, by role.
pub const CPU: char = '\u{e610}';
pub const GPU: char = '\u{e612}';
pub const MEMORY: char = '\u{e9c4}';
pub const GAUGE: char = '\u{e628}';
pub const THERMOMETER: char = '\u{e5cc}';
pub const ACTIVITY: char = '\u{e000}';
pub const CLOUD: char = '\u{e1aa}';
pub const CLOUD_SUN: char = '\u{e540}';
pub const CLOUD_MOON: char = '\u{e53e}';
pub const CLOUD_RAIN: char = '\u{e1b4}';
pub const CLOUD_SLASH: char = '\u{e1b6}';
pub const SUN: char = '\u{e472}';
pub const MOON: char = '\u{e330}';
pub const SPEAKER_HIGH: char = '\u{e44a}';
pub const SPEAKER_LOW: char = '\u{e44c}';
pub const SPEAKER_X: char = '\u{e45c}';
pub const PLAY: char = '\u{e3d0}';
pub const PAUSE: char = '\u{e39e}';
pub const SKIP_BACK: char = '\u{e5a4}';
pub const SKIP_FWD: char = '\u{e5a6}';
pub const MUSIC: char = '\u{e33c}';
pub const MAP_PIN_PLUS: char = '\u{e314}';
pub const PLUS: char = '\u{e3d4}';
pub const LINK_BREAK: char = '\u{e2e4}';
pub const ARROW_DOWN: char = '\u{e03e}';
pub const ARROW_UP: char = '\u{e08e}';
pub const WARNING: char = '\u{e4e0}';
pub const ARROWS: char = '\u{e0a0}';
pub const NEWSPAPER: char = '\u{e344}';

/// Every icon a layout can name, with its code point.
pub const NAMES: [(&str, char); 67] = [
    ("activity", '\u{e000}'),
    ("arrow-down", '\u{e03e}'),
    ("arrow-up", '\u{e08e}'),
    ("arrows-left-right", '\u{e0a0}'),
    ("bell", '\u{e0ce}'),
    ("browser", '\u{e0f4}'),
    ("calendar-blank", '\u{e10a}'),
    ("camera", '\u{e10e}'),
    ("chat-circle", '\u{e168}'),
    ("check", '\u{e182}'),
    ("check-circle", '\u{e184}'),
    ("clock", '\u{e19a}'),
    ("cloud", '\u{e1aa}'),
    ("cloud-rain", '\u{e1b4}'),
    ("cloud-slash", '\u{e1b6}'),
    ("code", '\u{e1bc}'),
    ("drop", '\u{e210}'),
    ("envelope-simple", '\u{e218}'),
    ("folder-simple", '\u{e25a}'),
    ("game-controller", '\u{e26e}'),
    ("gear", '\u{e270}'),
    ("globe-simple", '\u{e28e}'),
    ("hand-pointing", '\u{e29a}'),
    ("headphones", '\u{e2a6}'),
    ("house", '\u{e2c2}'),
    ("image", '\u{e2ca}'),
    ("lightning", '\u{e2de}'),
    ("link-break", '\u{e2e4}'),
    ("map-pin-plus", '\u{e314}'),
    ("map-pin", '\u{e316}'),
    ("moon", '\u{e330}'),
    ("music-note", '\u{e33c}'),
    ("music-notes", '\u{e340}'),
    ("newspaper", '\u{e344}'),
    ("pause", '\u{e39e}'),
    ("play", '\u{e3d0}'),
    ("plus", '\u{e3d4}'),
    ("power", '\u{e3da}'),
    ("rocket-launch", '\u{e3fe}'),
    ("speaker-high", '\u{e44a}'),
    ("speaker-low", '\u{e44c}'),
    ("speaker-none", '\u{e44e}'),
    ("speaker-x", '\u{e45c}'),
    ("squares-four", '\u{e464}'),
    ("sun", '\u{e472}'),
    ("warning", '\u{e4e0}'),
    ("warning-circle", '\u{e4e2}'),
    ("wifi-high", '\u{e4ea}'),
    ("x", '\u{e4f6}'),
    ("arrow-fat-down", '\u{e518}'),
    ("arrow-fat-up", '\u{e52e}'),
    ("calculator", '\u{e538}'),
    ("cloud-moon", '\u{e53e}'),
    ("cloud-sun", '\u{e540}'),
    ("moon-stars", '\u{e58e}'),
    ("skip-back", '\u{e5a4}'),
    ("skip-forward", '\u{e5a6}'),
    ("thermometer", '\u{e5c6}'),
    ("thermometer-simple", '\u{e5cc}'),
    ("cpu", '\u{e610}'),
    ("graphics-card", '\u{e612}'),
    ("gauge", '\u{e628}'),
    ("memory", '\u{e9c4}'),
    ("fan", '\u{e9f2}'),
    ("terminal-window", '\u{eae8}'),
    ("plugs", '\u{eb56}'),
    ("hand-tap", '\u{ec90}'),
];

/// The icon with this Phosphor name, if the bundled subset has it.
pub fn by_name(name: &str) -> Option<char> {
    let name = name.trim();
    NAMES.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// The icon a reading gets when the layout names none, from its signal:
/// the part it measures, or else what kind of reading it is.
pub fn for_signal(signal: &str) -> char {
    let (part, what) = signal.split_once('.').unwrap_or((signal, ""));
    match (part, what) {
        ("cpu", "temp") => CPU,
        ("gpu", "temp") => GPU,
        ("cpu" | "gpu", "load") => GAUGE,
        ("mem", _) => MEMORY,
        ("net", "up") => ARROW_UP,
        ("net", _) => ARROW_DOWN,
        ("weather", _) => CLOUD,
        ("volume", _) => SPEAKER_HIGH,
        ("media", _) => MUSIC,
        ("news", _) => NEWSPAPER,
        (_, "temp") => THERMOMETER,
        (_, "load") => GAUGE,
        _ => ACTIVITY,
    }
}

/// A weather condition's icon, from its WMO code (as Open-Meteo gives
/// it) and whether it is day.
pub fn for_weather(code: Option<f64>, day: bool) -> char {
    match code.map(|c| c.round() as i64) {
        Some(0) => {
            if day {
                SUN
            } else {
                MOON
            }
        }
        Some(1 | 2) => {
            if day {
                CLOUD_SUN
            } else {
                CLOUD_MOON
            }
        }
        Some(51..=67 | 80..=82 | 95..=99) => CLOUD_RAIN,
        _ => CLOUD,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::FontChoice;

    #[test]
    fn every_named_icon_is_in_both_weights() {
        let fonts = FontChoice::default().load();
        for (name, c) in NAMES {
            assert!(fonts.icon.covers(c), "fill lacks {name}");
            assert!(fonts.icon_bold.covers(c), "bold lacks {name}");
            assert_eq!(by_name(name), Some(c));
        }
        assert_eq!(by_name("unicorn"), None);
    }

    #[test]
    fn signals_pick_their_icons() {
        assert_eq!(for_signal("cpu.temp"), CPU);
        assert_eq!(for_signal("gpu.temp"), GPU);
        assert_eq!(for_signal("gpu.load"), GAUGE);
        assert_eq!(for_signal("mem.used"), MEMORY);
        assert_eq!(for_signal("nvme.temp"), THERMOMETER);
        assert_eq!(for_signal("mode"), ACTIVITY);
        assert_eq!(for_weather(Some(0.0), false), MOON);
        assert_eq!(for_weather(Some(63.0), true), CLOUD_RAIN);
    }
}
