//! The bundled fonts and the user's font choice.
//!
//! The strip's design face is Rubik (`DESIGN.md`): every numeral is drawn
//! in it, always. The `font` setting chooses the face for words (titles,
//! labels, key labels): Rubik by default, or one of five other bundled
//! faces as an override. The fallback face and the two icon weights
//! (Phosphor Fill and Bold) are fixed.

use std::fmt;
use std::str::FromStr;

use crate::text::Font;
use crate::widget::Fonts;

/// Which bundled font the strip draws text with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum FontChoice {
    #[default]
    Rubik,
    IbmPlexSans,
    AtkinsonHyperlegible,
    Roboto,
    NotoSans,
    Lato,
}

impl FontChoice {
    /// Every choice, in the order a settings screen should list them.
    pub const ALL: [FontChoice; 6] = [
        FontChoice::Rubik,
        FontChoice::IbmPlexSans,
        FontChoice::AtkinsonHyperlegible,
        FontChoice::Roboto,
        FontChoice::NotoSans,
        FontChoice::Lato,
    ];

    /// The value written in the config file.
    pub const fn id(self) -> &'static str {
        match self {
            FontChoice::Rubik => "rubik",
            FontChoice::IbmPlexSans => "ibm-plex-sans",
            FontChoice::AtkinsonHyperlegible => "atkinson-hyperlegible",
            FontChoice::Roboto => "roboto",
            FontChoice::NotoSans => "noto-sans",
            FontChoice::Lato => "lato",
        }
    }

    /// The name shown to people.
    pub const fn display_name(self) -> &'static str {
        match self {
            FontChoice::Rubik => "Rubik",
            FontChoice::IbmPlexSans => "IBM Plex Sans",
            FontChoice::AtkinsonHyperlegible => "Atkinson Hyperlegible",
            FontChoice::Roboto => "Roboto",
            FontChoice::NotoSans => "Noto Sans",
            FontChoice::Lato => "Lato",
        }
    }

    const fn bytes(self) -> &'static [u8] {
        match self {
            FontChoice::Rubik => RUBIK,
            FontChoice::IbmPlexSans => include_bytes!("../assets/fonts/IBMPlexSans.ttf"),
            FontChoice::AtkinsonHyperlegible => {
                include_bytes!("../assets/fonts/AtkinsonHyperlegible-Regular.ttf")
            }
            FontChoice::Roboto => include_bytes!("../assets/fonts/Roboto.ttf"),
            FontChoice::NotoSans => include_bytes!("../assets/fonts/NotoSans-Regular.ttf"),
            FontChoice::Lato => include_bytes!("../assets/fonts/Lato-Regular.ttf"),
        }
    }

    /// The fonts the strip draws with when words are in this face.
    pub fn load(self) -> Fonts {
        Fonts {
            text: bundled(self.bytes()),
            numerals: bundled(RUBIK),
            fallback: vec![bundled(FALLBACK)],
            icon: bundled(ICONS_FILL),
            icon_bold: bundled(ICONS_BOLD),
        }
    }
}

impl fmt::Display for FontChoice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.id())
    }
}

/// Error for a config value that names no bundled font.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnknownFont(pub String);

impl fmt::Display for UnknownFont {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let options: Vec<&str> = FontChoice::ALL.iter().map(|c| c.id()).collect();
        write!(
            f,
            "unknown font \"{}\"; choose one of: {}",
            self.0,
            options.join(", ")
        )
    }
}

impl std::error::Error for UnknownFont {}

impl FromStr for FontChoice {
    type Err = UnknownFont;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        FontChoice::ALL
            .into_iter()
            .find(|c| c.id() == s)
            .ok_or_else(|| UnknownFont(s.to_string()))
    }
}

const FALLBACK: &[u8] = include_bytes!("../assets/fonts/DejaVuSans.ttf");
const RUBIK: &[u8] = include_bytes!("../assets/fonts/Rubik.ttf");
const ICONS_FILL: &[u8] = include_bytes!("../assets/fonts/Phosphor-Fill.ttf");
const ICONS_BOLD: &[u8] = include_bytes!("../assets/fonts/Phosphor-Bold.ttf");

/// Bundled fonts are checked by the tests below, so a parse failure here is a
/// build mistake, not a runtime condition.
fn bundled(bytes: &'static [u8]) -> Font {
    Font::from_static(bytes).expect("bundled font must parse")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_choice_loads_and_covers_display_text() {
        for choice in FontChoice::ALL {
            let fonts = choice.load();
            for c in "Tue, Jun 09 4:17 PM Jersey City 79°F CPU 28°C".chars() {
                assert!(fonts.text.covers(c), "{choice} lacks {c:?}");
            }
        }
    }

    #[test]
    fn fallback_covers_symbols_the_choices_may_lack() {
        let fonts = FontChoice::default().load();
        for c in "↓↑→←•…".chars() {
            assert!(fonts.fallback[0].covers(c), "fallback lacks {c:?}");
        }
    }

    #[test]
    fn icon_fonts_have_weather_icons() {
        let fonts = FontChoice::default().load();
        assert!(fonts.icon.covers(crate::icons::SUN), "sun icon missing");
        assert!(
            fonts.icon_bold.covers(crate::icons::SUN),
            "sun icon missing"
        );
    }

    #[test]
    fn ids_round_trip() {
        for choice in FontChoice::ALL {
            assert_eq!(choice.id().parse::<FontChoice>(), Ok(choice));
        }
    }

    #[test]
    fn unknown_id_lists_the_options() {
        let err = "comic-sans".parse::<FontChoice>().unwrap_err();
        let message = err.to_string();
        assert!(message.contains("comic-sans"));
        assert!(message.contains("ibm-plex-sans"));
    }

    #[test]
    fn default_is_rubik() {
        assert_eq!(FontChoice::default(), FontChoice::Rubik);
    }
}
