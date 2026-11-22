//! The current temperature from Open-Meteo (no account or key needed).
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
                .user_agent(crate::USER_AGENT)
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
                            Ok(celsius) => {
                                last_error.clear();
                                let reading =
                                    ("weather.temp", Value::Number(celsius, Unit::Celsius));
                                if !publish(vec![reading]) {
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

/// The current temperature in °C, looking the place up first if needed.
fn fetch(agent: &ureq::Agent, c: &Weather, place: &mut Option<(f64, f64)>) -> Result<f64, String> {
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
    #[derive(Deserialize)]
    struct Forecast {
        current: Current,
    }
    #[derive(Deserialize)]
    struct Current {
        temperature_2m: f64,
    }
    let forecast: Forecast = get_json(agent.get(FORECAST_URL).query_pairs([
        ("latitude", lat.to_string().as_str()),
        ("longitude", lon.to_string().as_str()),
        ("current", "temperature_2m"),
    ]))?;
    Ok(forecast.current.temperature_2m)
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
    fn split_trims_and_drops_empty_parts() {
        assert_eq!(
            split(" Jersey City , New Jersey,, United States "),
            ("Jersey City", vec!["New Jersey", "United States"])
        );
    }
}
