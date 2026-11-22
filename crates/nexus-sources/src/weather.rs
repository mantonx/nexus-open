//! The weather from Open-Meteo (no account or key needed): the current
//! temperature, condition and whether it is day, and today's high and
//! low, as the `weather.*` signals the weather widget reads
//! (`nexus_core::layout::WEATHER_SIGNALS`).
//!
//! A place name is turned into coordinates with Open-Meteo's geocoding
//! service once per change of location; after that only the forecast is
//! fetched, every [`REFRESH`]. The service updates its data every 15
//! minutes, so asking more often gains nothing.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use nexus_core::config::Weather;
use nexus_core::signal::{Unit, Value};
use serde::Deserialize;

use crate::Reading;

const REFRESH: Duration = Duration::from_secs(15 * 60);
/// Wait before trying again after a failure (offline, service down).
const RETRY: Duration = Duration::from_secs(60);
const TIMEOUT: Duration = Duration::from_secs(10);
const GEOCODE_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";

/// Starts the weather source. Send it the `[weather]` section whenever the
/// config changes (`None` stops fetching); it fetches straight away. It
/// passes readings to `publish` and stops when that returns false or the
/// sender is dropped.
pub fn spawn(
    mut publish: impl FnMut(Vec<Reading>) -> bool + Send + 'static,
) -> Sender<Option<Weather>> {
    let (tx, rx) = mpsc::channel::<Option<Weather>>();
    thread::Builder::new()
        .name("weather-source".into())
        .spawn(move || {
            let agent: ureq::Agent = ureq::Agent::config_builder()
                .timeout_global(Some(TIMEOUT))
                .user_agent(concat!("nexus-open/", env!("CARGO_PKG_VERSION")))
                .build()
                .into();
            let mut config: Option<Weather> = None;
            // Coordinates for the current config, once looked up.
            let mut place: Option<(f64, f64)> = None;
            let mut last_error = String::new();
            loop {
                let wait = match &config {
                    None => None,
                    Some(c) => {
                        let result = fetch(&agent, c, &mut place);
                        match result {
                            Ok(now) => {
                                last_error.clear();
                                if !publish(now.readings()) {
                                    return;
                                }
                                Some(REFRESH)
                            }
                            Err(e) => {
                                // Said once, not every minute while offline.
                                if e != last_error {
                                    eprintln!("nexusd: weather: {e}");
                                    last_error = e;
                                }
                                Some(RETRY)
                            }
                        }
                    }
                };
                let next = match wait {
                    None => rx.recv().map_err(|_| RecvTimeoutError::Disconnected),
                    Some(w) => rx.recv_timeout(w),
                };
                match next {
                    Ok(new) => {
                        if new != config {
                            place = None;
                            last_error.clear();
                        }
                        config = new;
                    }
                    Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            }
        })
        .expect("spawning a thread");
    tx
}

/// One forecast, as Open-Meteo answers `current` and `daily`.
#[derive(Debug, Deserialize)]
struct Forecast {
    current: Current,
    #[serde(default)]
    daily: Option<Daily>,
}

#[derive(Debug, Deserialize)]
struct Current {
    temperature_2m: f64,
    #[serde(default)]
    weather_code: Option<f64>,
    #[serde(default)]
    is_day: Option<f64>,
}

/// Today only (`forecast_days=1`): one entry per list.
#[derive(Debug, Deserialize)]
struct Daily {
    #[serde(default)]
    temperature_2m_max: Vec<Option<f64>>,
    #[serde(default)]
    temperature_2m_min: Vec<Option<f64>>,
}

impl Forecast {
    /// The signals it gives. A field the answer lacks is left out, so the
    /// widget shows what it has.
    fn readings(&self) -> Vec<Reading> {
        let c = &self.current;
        let mut out = vec![(
            "weather.temp",
            Value::Number(c.temperature_2m, Unit::Celsius),
        )];
        if let Some(code) = c.weather_code {
            out.push(("weather.code", Value::Number(code, Unit::None)));
            out.push(("weather.condition", Value::Text(condition(code).into())));
        }
        if let Some(day) = c.is_day {
            out.push(("weather.day", Value::Number(day, Unit::None)));
        }
        let today = |pick: fn(&Daily) -> &Vec<Option<f64>>| {
            self.daily
                .as_ref()
                .and_then(|d| pick(d).first().copied().flatten())
        };
        let high = today(|d| &d.temperature_2m_max);
        let low = today(|d| &d.temperature_2m_min);
        if let (Some(high), Some(low)) = (high, low) {
            out.push(("weather.high", Value::Number(high, Unit::Celsius)));
            out.push(("weather.low", Value::Number(low, Unit::Celsius)));
        }
        out
    }
}

/// A WMO weather code (as Open-Meteo gives it) in a few words, short
/// enough for a medium weather widget.
fn condition(code: f64) -> &'static str {
    match code.round() as i64 {
        0 => "Clear",
        1 => "Mostly clear",
        2 => "Partly cloudy",
        3 => "Cloudy",
        45 | 48 => "Fog",
        51..=55 => "Drizzle",
        56 | 57 => "Icy drizzle",
        61 | 63 => "Rain",
        65 => "Heavy rain",
        66 | 67 => "Icy rain",
        71 | 73 | 77 => "Snow",
        75 => "Heavy snow",
        80..=82 => "Showers",
        85 | 86 => "Snow showers",
        95..=99 => "Thunder",
        _ => "",
    }
}

/// The weather now, looking the place up first if needed.
fn fetch(
    agent: &ureq::Agent,
    c: &Weather,
    place: &mut Option<(f64, f64)>,
) -> Result<Forecast, String> {
    let (lat, lon) = match (c.latitude, c.longitude, *place) {
        (Some(lat), Some(lon), _) => (lat, lon),
        (_, _, Some(p)) => p,
        _ => {
            let name = c.location.as_deref().unwrap_or_default();
            let p = geocode(agent, name)?;
            *place = Some(p);
            p
        }
    };
    get_json(agent.get(FORECAST_URL).query_pairs([
        ("latitude", lat.to_string().as_str()),
        ("longitude", lon.to_string().as_str()),
        ("current", "temperature_2m,weather_code,is_day"),
        ("daily", "temperature_2m_max,temperature_2m_min"),
        ("forecast_days", "1"),
        // Today as the place counts it, not as UTC does.
        ("timezone", "auto"),
    ]))
}

#[derive(Debug, Deserialize)]
struct Place {
    latitude: f64,
    longitude: f64,
    #[serde(default)]
    admin1: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    country_code: Option<String>,
}

fn geocode(agent: &ureq::Agent, location: &str) -> Result<(f64, f64), String> {
    #[derive(Deserialize)]
    struct Found {
        #[serde(default)]
        results: Vec<Place>,
    }
    let (name, _) = split(location);
    let found: Found = get_json(
        agent
            .get(GEOCODE_URL)
            .query("name", name)
            .query("count", "10")
            .query("language", "en"),
    )?;
    best(location, &found.results)
        .map(|p| (p.latitude, p.longitude))
        .ok_or_else(|| format!("no place called \"{location}\"; check [weather] location"))
}

/// "Jersey City, New Jersey, US" → ("Jersey City", ["New Jersey", "US"]).
fn split(location: &str) -> (&str, Vec<&str>) {
    let mut parts = location.split(',').map(str::trim).filter(|p| !p.is_empty());
    let name = parts.next().unwrap_or_default();
    (name, parts.collect())
}

/// The first result whose region and country match every part after the
/// name (so "Portland, Maine" is not Portland, Oregon), else the first
/// result: the service ranks by population.
fn best<'a>(location: &str, results: &'a [Place]) -> Option<&'a Place> {
    let (_, qualifiers) = split(location);
    let matches = |p: &Place| {
        qualifiers.iter().all(|q| {
            [&p.admin1, &p.country, &p.country_code]
                .into_iter()
                .flatten()
                .any(|field| field.eq_ignore_ascii_case(q))
        })
    };
    results.iter().find(|p| matches(p)).or(results.first())
}

fn get_json<T: serde::de::DeserializeOwned>(
    request: ureq::RequestBuilder<ureq::typestate::WithoutBody>,
) -> Result<T, String> {
    let mut response = request.call().map_err(|e| e.to_string())?;
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|e| e.to_string())?;
    serde_json::from_str(&body).map_err(|e| format!("unexpected answer: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(admin1: &str, country: &str, code: &str, lat: f64) -> Place {
        Place {
            latitude: lat,
            longitude: 0.0,
            admin1: Some(admin1.into()),
            country: Some(country.into()),
            country_code: Some(code.into()),
        }
    }

    #[test]
    fn qualifiers_choose_among_same_names() {
        let portlands = [
            place("Oregon", "United States", "US", 45.5),
            place("Maine", "United States", "US", 43.7),
        ];
        assert_eq!(best("Portland", &portlands).unwrap().latitude, 45.5);
        assert_eq!(best("Portland, Maine", &portlands).unwrap().latitude, 43.7);
        assert_eq!(
            best("portland, maine, us", &portlands).unwrap().latitude,
            43.7
        );
        // An unrecognised qualifier ("ME") falls back to the top result.
        assert_eq!(best("Portland, ME", &portlands).unwrap().latitude, 45.5);
        assert!(best("Nowhere", &[]).is_none());
    }

    #[test]
    fn a_forecast_gives_every_weather_signal() {
        let f: Forecast = serde_json::from_str(
            r#"{"current":{"time":"2026-11-22T10:45","temperature_2m":14.4,
                "weather_code":3,"is_day":1},
                "daily":{"time":["2026-11-22"],"temperature_2m_max":[16.1],
                "temperature_2m_min":[9.4]}}"#,
        )
        .unwrap();
        let readings = f.readings();
        let mut got: Vec<&str> = readings.iter().map(|(n, _)| *n).collect();
        let mut want = nexus_core::layout::WEATHER_SIGNALS.to_vec();
        got.sort_unstable();
        want.sort_unstable();
        assert_eq!(got, want);
        assert!(readings.contains(&("weather.condition", Value::Text("Cloudy".into()))));
        assert!(readings.contains(&("weather.low", Value::Number(9.4, Unit::Celsius))));
    }

    #[test]
    fn a_short_answer_gives_what_it_has() {
        let f: Forecast = serde_json::from_str(r#"{"current":{"temperature_2m":-2.5}}"#).unwrap();
        assert_eq!(
            f.readings(),
            [("weather.temp", Value::Number(-2.5, Unit::Celsius))]
        );
        assert_eq!(condition(99.0), "Thunder");
        assert_eq!(condition(42.0), "");
    }

    #[test]
    fn split_trims_and_drops_empty_parts() {
        assert_eq!(
            split(" Jersey City , New Jersey,, United States "),
            ("Jersey City", vec!["New Jersey", "United States"])
        );
    }
}
