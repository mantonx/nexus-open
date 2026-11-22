//! The strip's own settings, as the Preferences sheet sets them:
//! temperature unit, clock, accent, where the weather is for, and the
//! strip's lettering.

use nexus_core::config::{
    ClockFormat, Config, Night, Settings, Temperature, Weather, colour_text, parse_colour,
};
use nexus_core::fonts::FontChoice;

use crate::inspector::{Missing, Values};

pub const TEMPERATURE: &str = "settings.temperature";
pub const CLOCK: &str = "settings.clock";
pub const FONT: &str = "settings.font";
pub const WEATHER: &str = "settings.weather";
pub const ACCENT: &str = "settings.accent";
/// Not shown as fields yet; carried through the form so saving it keeps
/// them. "22:30 07:00", or empty for no night hours.
pub const NIGHT: &str = "settings.night";
/// "on" or "off".
pub const IDLE_DIM: &str = "settings.idle_dim";

const FAHRENHEIT: &str = "°F (Fahrenheit)";
const CELSIUS: &str = "°C (Celsius)";
const H24: &str = "24-hour (14:30)";
const H12: &str = "12-hour (2:30 PM)";

pub fn flatten(c: &Config) -> Values {
    let weather = match &c.weather {
        Some(Weather {
            latitude: Some(lat),
            longitude: Some(lon),
            ..
        }) => format!("{lat}, {lon}"),
        Some(w) => w.location.clone().unwrap_or_default(),
        None => String::new(),
    };
    Values::from([
        (
            TEMPERATURE.into(),
            match c.settings.temperature {
                Temperature::Fahrenheit => FAHRENHEIT,
                Temperature::Celsius => CELSIUS,
            }
            .into(),
        ),
        (
            CLOCK.into(),
            match c.settings.clock {
                ClockFormat::H24 => H24,
                ClockFormat::H12 => H12,
            }
            .into(),
        ),
        (FONT.into(), c.settings.font.display_name().into()),
        (WEATHER.into(), weather),
        (
            ACCENT.into(),
            c.settings.accent.map(colour_text).unwrap_or_default(),
        ),
        (
            NIGHT.into(),
            c.settings
                .night
                .as_ref()
                .map(|n| format!("{} {}", n.from, n.to))
                .unwrap_or_default(),
        ),
        (
            IDLE_DIM.into(),
            if c.settings.idle_dim { "on" } else { "off" }.into(),
        ),
    ])
}

/// Reads the form: the settings, and the weather place (none when empty).
pub fn build(v: &Values) -> Result<(Settings, Option<Weather>), Missing> {
    let get = |k: &str| v.get(k).map(String::as_str).unwrap_or_default().trim();
    let font = FontChoice::ALL
        .into_iter()
        .find(|f| f.display_name() == get(FONT))
        .ok_or_else(|| Missing::Invalid("Choose one of the listed fonts.".into()))?;
    let settings = Settings {
        font,
        temperature: if get(TEMPERATURE) == FAHRENHEIT {
            Temperature::Fahrenheit
        } else {
            Temperature::Celsius
        },
        clock: if get(CLOCK) == H12 {
            ClockFormat::H12
        } else {
            ClockFormat::H24
        },
        accent: match get(ACCENT) {
            "" => None,
            text => Some(parse_colour(text).map_err(|_| {
                Missing::Invalid(
                    "The accent is a colour written #rrggbb, as in #5aa0e0. Empty: the default."
                        .into(),
                )
            })?),
        },
        night: get(NIGHT).split_once(' ').map(|(from, to)| Night {
            from: from.trim().into(),
            to: to.trim().into(),
        }),
        idle_dim: get(IDLE_DIM) != "off",
    };
    Ok((settings, weather(get(WEATHER))?))
}

/// "Paris, France" is a place to look up; "48.85, 2.35" is coordinates.
fn weather(text: &str) -> Result<Option<Weather>, Missing> {
    if text.is_empty() {
        return Ok(None);
    }
    let numbers: Vec<Option<f64>> = text.split(',').map(|p| p.trim().parse().ok()).collect();
    if let [Some(lat), Some(lon)] = numbers[..] {
        if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
            return Err(Missing::Invalid(
                "Coordinates are latitude −90 to 90, then longitude −180 to 180.".into(),
            ));
        }
        return Ok(Some(Weather {
            location: None,
            latitude: Some(lat),
            longitude: Some(lon),
        }));
    }
    Ok(Some(Weather {
        location: Some(text.into()),
        latitude: None,
        longitude: None,
    }))
}

/// The preferences as the Preferences sheet shows them.
#[derive(Clone, Debug, PartialEq)]
pub struct Prefs {
    pub fahrenheit: bool,
    pub twelve_hour: bool,
    /// "#rrggbb", or empty for the default.
    pub accent: String,
    /// Which swatch in `accent::PRESETS` it is, if any.
    pub preset: Option<usize>,
    pub weather: String,
    pub fonts: Vec<String>,
    pub font: usize,
}

pub fn prefs(c: &Config) -> Prefs {
    let v = flatten(c);
    let accent = c.settings.accent.unwrap_or(crate::accent::DEFAULT);
    Prefs {
        fahrenheit: c.settings.temperature == Temperature::Fahrenheit,
        twelve_hour: c.settings.clock == ClockFormat::H12,
        accent: v[ACCENT].clone(),
        preset: crate::accent::PRESETS
            .iter()
            .position(|(_, p)| *p == accent),
        weather: v[WEATHER].clone(),
        fonts: FontChoice::ALL
            .iter()
            .map(|f| f.display_name().to_string())
            .collect(),
        font: FontChoice::ALL
            .iter()
            .position(|f| *f == c.settings.font)
            .unwrap_or(0),
    }
}

/// One preference from the sheet into the form's values: `key` is
/// "temperature" (c or f), "clock" (24 or 12), "accent" (#rrggbb),
/// "accent-preset" (a swatch's place), "weather" or "font" (a place in
/// the list). `None` for an unknown key or place.
pub fn set(v: &mut Values, key: &str, value: &str) -> Option<()> {
    let (k, val) = match key {
        "temperature" => (
            TEMPERATURE,
            if value == "f" { FAHRENHEIT } else { CELSIUS }.to_string(),
        ),
        "clock" => (CLOCK, if value == "12" { H12 } else { H24 }.to_string()),
        "accent" => (ACCENT, value.trim().to_string()),
        "accent-preset" => {
            let (_, c) = crate::accent::PRESETS.get(value.parse::<usize>().ok()?)?;
            let text = if *c == crate::accent::DEFAULT {
                String::new()
            } else {
                colour_text(*c)
            };
            (ACCENT, text)
        }
        "weather" => (WEATHER, value.trim().to_string()),
        "font" => (
            FONT,
            FontChoice::ALL
                .get(value.parse::<usize>().ok()?)?
                .display_name()
                .to_string(),
        ),
        _ => return None,
    };
    v.insert(k.into(), val);
    Some(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_the_default_layout() {
        let c = Config::default_layout();
        let (settings, weather) = build(&flatten(&c)).unwrap();
        assert_eq!(settings, c.settings);
        assert_eq!(weather, c.weather);
    }

    #[test]
    fn weather_is_a_place_or_coordinates() {
        assert_eq!(weather(""), Ok(None));
        assert_eq!(
            weather("48.85, 2.35"),
            Ok(Some(Weather {
                location: None,
                latitude: Some(48.85),
                longitude: Some(2.35)
            }))
        );
        assert_eq!(
            weather("Paris, France")
                .unwrap()
                .unwrap()
                .location
                .as_deref(),
            Some("Paris, France")
        );
        assert!(weather("95, 2").is_err());
    }

    #[test]
    fn the_accent_is_typed_as_hex_or_left_empty() {
        let mut v = flatten(&Config::default_layout());
        assert_eq!(v[ACCENT], "");
        assert_eq!(build(&v).unwrap().0.accent, None);
        v.insert(ACCENT.into(), "#FF8800".into());
        let (s, _) = build(&v).unwrap();
        assert_eq!(s.accent, Some(nexus_core::theme::Rgb::hex(0xff8800)));
        let mut c = Config::default_layout();
        c.settings = s;
        assert_eq!(flatten(&c)[ACCENT], "#ff8800");
        v.insert(ACCENT.into(), "orange".into());
        assert!(matches!(build(&v), Err(Missing::Invalid(_))));
    }

    #[test]
    fn the_preferences_sheet_sets_each_setting() {
        let c = Config::default_layout();
        let mut v = flatten(&c);
        set(&mut v, "temperature", "f").unwrap();
        set(&mut v, "clock", "12").unwrap();
        set(&mut v, "accent-preset", "1").unwrap();
        set(&mut v, "font", "0").unwrap();
        let (s, _) = build(&v).unwrap();
        assert_eq!(s.temperature, Temperature::Fahrenheit);
        assert_eq!(s.clock, ClockFormat::H12);
        assert_eq!(s.accent, Some(crate::accent::PRESETS[1].1));
        assert_eq!(s.font, FontChoice::ALL[0]);
        // The default swatch stores no colour at all.
        set(&mut v, "accent-preset", "0").unwrap();
        assert_eq!(build(&v).unwrap().0.accent, None);
        assert_eq!(set(&mut v, "accent-preset", "99"), None);
        assert_eq!(set(&mut v, "volume", "1"), None);
        let p = prefs(&c);
        assert_eq!(p.preset, Some(0));
        assert!(!p.fahrenheit);
    }

    #[test]
    fn units_and_clock_follow_the_choices() {
        let mut v = flatten(&Config::default_layout());
        v.insert(TEMPERATURE.into(), FAHRENHEIT.into());
        v.insert(CLOCK.into(), H12.into());
        let (s, _) = build(&v).unwrap();
        assert_eq!(
            (s.temperature, s.clock),
            (Temperature::Fahrenheit, ClockFormat::H12)
        );
    }
}
