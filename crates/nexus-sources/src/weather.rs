//! The weather from Open-Meteo (no account or key needed): the current
//! temperature, condition and whether it is day, and today's high and
//! low, as the `weather.*` signals the weather widget reads
//! (`nexus_core::layout::WEATHER_SIGNALS`); and from the same answer the
//! forecast: eight columns of hours, the five days after today, and the
//! next change in the weather (`nexus_core::forecast`).
//!
//! A place name is turned into coordinates with Open-Meteo's geocoding
//! service once per change of location; after that only the forecast is
//! fetched, every [`REFRESH`]. The service updates its data every 15
//! minutes, so asking more often gains nothing.

use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use nexus_core::config::Weather;
use nexus_core::forecast::{self, HOUR_STEP, Hour, next_change};
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

/// One forecast, as Open-Meteo answers `current`, `hourly` and `daily`.
#[derive(Debug, Deserialize)]
struct Forecast {
    current: Current,
    #[serde(default)]
    hourly: Option<Hourly>,
    #[serde(default)]
    daily: Option<Daily>,
}

#[derive(Debug, Deserialize)]
struct Current {
    /// "2026-11-22T10:45", local to the place (`timezone=auto`).
    #[serde(default)]
    time: Option<String>,
    temperature_2m: f64,
    #[serde(default)]
    weather_code: Option<f64>,
    #[serde(default)]
    is_day: Option<f64>,
}

/// Hour by hour from midnight today, local to the place.
#[derive(Debug, Default, Deserialize)]
struct Hourly {
    #[serde(default)]
    time: Vec<String>,
    #[serde(default)]
    temperature_2m: Vec<Option<f64>>,
    #[serde(default)]
    weather_code: Vec<Option<f64>>,
    #[serde(default)]
    precipitation_probability: Vec<Option<f64>>,
    #[serde(default)]
    is_day: Vec<Option<f64>>,
}

/// Day by day from today, local to the place.
#[derive(Debug, Default, Deserialize)]
struct Daily {
    #[serde(default)]
    time: Vec<String>,
    #[serde(default)]
    temperature_2m_max: Vec<Option<f64>>,
    #[serde(default)]
    temperature_2m_min: Vec<Option<f64>>,
    #[serde(default)]
    weather_code: Vec<Option<f64>>,
    #[serde(default)]
    precipitation_probability_max: Vec<Option<f64>>,
}

/// The `i`th of a list the answer may have cut short or left gaps in.
fn at(list: &[Option<f64>], i: usize) -> Option<f64> {
    list.get(i).copied().flatten()
}

/// "2026-11-22T15:00" → 15.
fn hour_of(time: &str) -> Option<u8> {
    time.get(11..13)?.parse().ok().filter(|h| *h < 24)
}

impl Hourly {
    fn hour(&self, i: usize) -> Option<Hour> {
        Some(Hour {
            hour: hour_of(self.time.get(i)?)?,
            code: at(&self.weather_code, i),
            rain: at(&self.precipitation_probability, i),
            day: at(&self.is_day, i).is_none_or(|d| d != 0.0),
        })
    }

    /// The index of the first hour after `now` ("2026-11-22T10:45"):
    /// ISO times of one format sort as text.
    fn after(&self, now: &str) -> Option<usize> {
        let this_hour = format!("{}:00", now.get(..13)?);
        self.time
            .iter()
            .position(|t| t.as_str() > this_hour.as_str())
    }
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
        let daily = self.daily.as_ref();
        let today = daily.map_or(0, |d| self.today(d));
        let pick = |list: fn(&Daily) -> &Vec<Option<f64>>| daily.and_then(|d| at(list(d), today));
        let high = pick(|d| &d.temperature_2m_max);
        let low = pick(|d| &d.temperature_2m_min);
        if let (Some(high), Some(low)) = (high, low) {
            out.push(("weather.high", Value::Number(high, Unit::Celsius)));
            out.push(("weather.low", Value::Number(low, Unit::Celsius)));
        }
        if let Some(h) = &self.hourly {
            self.hours(h, &mut out);
        }
        if let Some(d) = daily {
            days(d, today, &mut out);
        }
        out
    }

    /// Today's place in the daily lists: the day of `current.time`, else
    /// the first.
    fn today(&self, d: &Daily) -> usize {
        let date = self.current.time.as_deref().and_then(|t| t.get(..10));
        date.and_then(|date| d.time.iter().position(|t| t == date))
            .unwrap_or(0)
    }

    /// `weather.h1` … `h8` and `weather.next`.
    fn hours(&self, h: &Hourly, out: &mut Vec<Reading>) {
        let Some(first) = self.current.time.as_deref().and_then(|t| h.after(t)) else {
            return;
        };
        let now = Hour {
            hour: first
                .checked_sub(1)
                .and_then(|i| h.hour(i))
                .map_or(0, |p| p.hour),
            code: self.current.weather_code,
            // Rain now is rain, whatever its chance was.
            rain: None,
            day: self.current.is_day.is_none_or(|d| d != 0.0),
        };
        let ahead: Vec<Hour> = (first..h.time.len()).map_while(|i| h.hour(i)).collect();
        let (change, decided) = next_change(&now, &ahead);
        out.push((forecast::NEXT, Value::Text(change.key())));
        if let Some(at) = change.at() {
            out.push((forecast::NEXT_AT, Value::Number(at.into(), Unit::None)));
        }
        if let Some(code) = decided.code {
            out.push((forecast::NEXT_CODE, Value::Number(code, Unit::None)));
        }
        let day = f64::from(u8::from(decided.day));
        out.push((forecast::NEXT_DAY, Value::Number(day, Unit::None)));
        // Columns at the hours that divide by three, each standing for the
        // three hours from it.
        let starts =
            (first..h.time.len()).filter(|&i| h.hour(i).is_some_and(|x| x.hour % HOUR_STEP == 0));
        for (names, i) in forecast::HOURS.iter().zip(starts) {
            let Some(hour) = h.hour(i) else { break };
            let span: Vec<usize> = (i..(i + usize::from(HOUR_STEP)).min(h.time.len())).collect();
            out.push((names.hour, Value::Number(hour.hour.into(), Unit::None)));
            if let Some(t) = at(&h.temperature_2m, i) {
                out.push((names.temp, Value::Number(t, Unit::Celsius)));
            }
            if let Some(code) = slot_code(h, &span) {
                out.push((names.code, Value::Number(code, Unit::None)));
            }
            let rain = span
                .iter()
                .filter_map(|&j| at(&h.precipitation_probability, j))
                .reduce(f64::max);
            if let Some(r) = rain {
                out.push((names.rain, Value::Number(r, Unit::Percent)));
            }
            let day = f64::from(u8::from(hour.day));
            out.push((names.day, Value::Number(day, Unit::None)));
        }
    }
}

/// An hour column's weather: the worst that falls in its three hours
/// (the highest WMO code among the wet ones), so rain at 13:00 shows on
/// the 12:00 column; with nothing likely to fall, the weather at its hour.
fn slot_code(h: &Hourly, span: &[usize]) -> Option<f64> {
    let wet = span
        .iter()
        .filter_map(|&i| h.hour(i))
        .filter(|x| x.wet().is_some())
        .filter_map(|x| x.code)
        .reduce(f64::max);
    wet.or_else(|| at(&h.weather_code, *span.first()?))
}

/// `weather.d1` … `d5`: the days after today.
fn days(d: &Daily, today: usize, out: &mut Vec<Reading>) {
    for (names, i) in forecast::DAYS.iter().zip(today + 1..d.time.len()) {
        let Some(key) = forecast::day_key(&d.time[i]) else {
            continue;
        };
        out.push((names.day, Value::Text(key.into())));
        let (high, low) = (at(&d.temperature_2m_max, i), at(&d.temperature_2m_min, i));
        if let (Some(high), Some(low)) = (high, low) {
            out.push((names.high, Value::Number(high, Unit::Celsius)));
            out.push((names.low, Value::Number(low, Unit::Celsius)));
        }
        if let Some(code) = at(&d.weather_code, i) {
            out.push((names.code, Value::Number(code, Unit::None)));
        }
        if let Some(r) = at(&d.precipitation_probability_max, i) {
            out.push((names.rain, Value::Number(r, Unit::Percent)));
        }
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
        (
            "hourly",
            "temperature_2m,weather_code,precipitation_probability,is_day",
        ),
        (
            "daily",
            "temperature_2m_max,temperature_2m_min,weather_code,precipitation_probability_max",
        ),
        // Today and the five days after it. The hours come for all six
        // days; the next day of them is read.
        ("forecast_days", "6"),
        // Days and hours as the place counts them, not as UTC does.
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

    /// An answer for 10:45 on Sunday 22 November: partly cloudy, rain
    /// from 15:00 to 18:00, then a cold week with snow on Wednesday and
    /// Thursday.
    const RAINY: &str = include_str!("testdata/weather/rainy-afternoon.json");

    fn get<'a>(readings: &'a [Reading], name: &str) -> Option<&'a Value> {
        readings.iter().find(|(n, _)| *n == name).map(|(_, v)| v)
    }

    fn num(readings: &[Reading], name: &str) -> Option<f64> {
        match get(readings, name)? {
            Value::Number(n, _) => Some(*n),
            Value::Text(_) => None,
        }
    }

    #[test]
    fn the_hours_come_every_three_hours_from_the_next() {
        let f: Forecast = serde_json::from_str(RAINY).unwrap();
        let r = f.readings();
        let hours: Vec<f64> = forecast::HOURS
            .iter()
            .filter_map(|h| num(&r, h.hour))
            .collect();
        assert_eq!(hours, [12.0, 15.0, 18.0, 21.0, 0.0, 3.0, 6.0, 9.0]);
        let h = &forecast::HOURS;
        // Noon: cloudy, its three hours at most 25 % (shown from 30).
        assert_eq!(num(&r, h[0].code), Some(3.0));
        assert_eq!(num(&r, h[0].rain), Some(25.0));
        assert!(matches!(
            get(&r, h[0].temp),
            Some(Value::Number(_, Unit::Celsius))
        ));
        // 15:00: the rain, at the highest chance of 15–17.
        assert_eq!(num(&r, h[1].code), Some(63.0));
        assert_eq!(num(&r, h[1].rain), Some(85.0));
        assert_eq!(num(&r, h[1].day), Some(1.0));
        // 18:00: the last of the rain is its column's weather, though
        // 19:00 and 20:00 are clear.
        assert_eq!(num(&r, h[2].code), Some(61.0));
        assert_eq!(num(&r, h[2].day), Some(0.0));
        assert_eq!(num(&r, h[3].code), Some(1.0));
    }

    #[test]
    fn the_days_start_tomorrow() {
        let f: Forecast = serde_json::from_str(RAINY).unwrap();
        let r = f.readings();
        let d = &forecast::DAYS;
        let names: Vec<&Value> = d.iter().filter_map(|d| get(&r, d.day)).collect();
        let text = |s: &str| Value::Text(s.into());
        assert_eq!(
            names,
            [
                &text("mon"),
                &text("tue"),
                &text("wed"),
                &text("thu"),
                &text("fri")
            ]
        );
        assert_eq!(num(&r, d[0].high), Some(12.5));
        assert_eq!(num(&r, d[3].low), Some(-4.0));
        assert_eq!(num(&r, d[2].code), Some(71.0));
        assert_eq!(num(&r, d[1].rain), Some(60.0));
        // Today's high and low are still today's.
        assert_eq!(num(&r, "weather.high"), Some(14.0));
        assert_eq!(num(&r, "weather.low"), Some(6.5));
    }

    #[test]
    fn the_next_change_is_the_rain_at_three() {
        let f: Forecast = serde_json::from_str(RAINY).unwrap();
        let r = f.readings();
        let next = get(&r, forecast::NEXT);
        assert_eq!(next, Some(&Value::Text("rain-from".into())));
        assert_eq!(num(&r, forecast::NEXT_AT), Some(15.0));
        assert_eq!(num(&r, forecast::NEXT_CODE), Some(63.0));
        assert_eq!(num(&r, forecast::NEXT_DAY), Some(1.0));
    }

    #[test]
    fn rain_now_clears_and_every_signal_is_one_the_widgets_read() {
        let mut f: Forecast = serde_json::from_str(RAINY).unwrap();
        f.current.time = Some("2026-11-22T15:30".into());
        f.current.weather_code = Some(63.0);
        let r = f.readings();
        let next = get(&r, forecast::NEXT);
        assert_eq!(next, Some(&Value::Text("clearing".into())));
        assert_eq!(num(&r, forecast::NEXT_AT), Some(19.0));
        // The first column after 15:30 is 18:00.
        assert_eq!(num(&r, forecast::HOURS[0].hour), Some(18.0));
        let known = forecast::signals();
        for (name, _) in &r {
            let weather = nexus_core::layout::WEATHER_SIGNALS.contains(name);
            assert!(known.contains(name) || weather, "{name}");
        }
    }

    #[test]
    fn an_answer_without_the_time_gives_no_hours() {
        // Without the time now the hours cannot be placed: none are given,
        // rather than the wrong ones.
        let f: Forecast = serde_json::from_str(
            r#"{"current":{"temperature_2m":3.0,"weather_code":71},
                "hourly":{"time":["2026-11-22T10:00"],"weather_code":[71]}}"#,
        )
        .unwrap();
        let r = f.readings();
        assert!(get(&r, forecast::NEXT).is_none());
        assert!(get(&r, forecast::HOURS[0].hour).is_none());
        assert_eq!(hour_of("2026-11-22T07:00"), Some(7));
        assert_eq!(hour_of("2026-11-22"), None);
    }
}
