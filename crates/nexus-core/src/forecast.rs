//! The weather forecast: the signals the weather source publishes for the
//! coming hours and days, the rule that names the next change in the
//! weather, and the words the forecast widget says it in.
//!
//! The source publishes numbers and keys only ("rain-from", "mon", an
//! hour of the day); the words and the time's format are decided here, on
//! the strip's side, from the settings (`signal`'s rule: a source never
//! decides how a value looks).
//!
//! Hours come every [`HOUR_STEP`] hours, at the hours of the clock that
//! divide by it (12:00, 15:00, 18:00 …), [`HOUR_SLOTS`] of them: a day
//! ahead in eight columns, where hourly columns would end before the
//! evening. The next change is found hour by hour, over the next
//! [`NEXT_WINDOW`] hours, so rain at 13:00 is not missed between columns.

use jiff::civil::{Date, Weekday};

use crate::config::ClockFormat;

/// The signals of one hour ahead: its hour of the day (0–23, local to the
/// place), the temperature (°C), the WMO weather code, the chance of rain
/// (percent) and 1 by day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HourSignals {
    pub hour: &'static str,
    pub temp: &'static str,
    pub code: &'static str,
    pub rain: &'static str,
    pub day: &'static str,
}

/// The signals of one day ahead: its name as a key (`"mon"` … `"sun"`),
/// its high and low (°C), WMO weather code and highest chance of rain
/// (percent).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DaySignals {
    pub day: &'static str,
    pub high: &'static str,
    pub low: &'static str,
    pub code: &'static str,
    pub rain: &'static str,
}

macro_rules! hour {
    ($n:literal) => {
        HourSignals {
            hour: concat!("weather.h", $n, ".hour"),
            temp: concat!("weather.h", $n, ".temp"),
            code: concat!("weather.h", $n, ".code"),
            rain: concat!("weather.h", $n, ".rain"),
            day: concat!("weather.h", $n, ".day"),
        }
    };
}

macro_rules! day {
    ($n:literal) => {
        DaySignals {
            day: concat!("weather.d", $n, ".day"),
            high: concat!("weather.d", $n, ".high"),
            low: concat!("weather.d", $n, ".low"),
            code: concat!("weather.d", $n, ".code"),
            rain: concat!("weather.d", $n, ".rain"),
        }
    };
}

/// Hours between two hour columns.
pub const HOUR_STEP: u8 = 3;
/// Hour columns published: `weather.h1` … `weather.h8`.
pub const HOUR_SLOTS: usize = 8;
/// Days published after today: `weather.d1` (tomorrow) … `weather.d5`.
pub const DAY_SLOTS: usize = 5;
/// The hours ahead the next change is looked for in.
pub const NEXT_WINDOW: usize = 12;
/// A chance of rain under this (percent) is not shown, and an hour whose
/// code says rain at a lower chance does not count as wet.
pub const RAIN_SHOWN: f64 = 30.0;

pub const HOURS: [HourSignals; HOUR_SLOTS] = [
    hour!(1),
    hour!(2),
    hour!(3),
    hour!(4),
    hour!(5),
    hour!(6),
    hour!(7),
    hour!(8),
];

pub const DAYS: [DaySignals; DAY_SLOTS] = [day!(1), day!(2), day!(3), day!(4), day!(5)];

/// The next change, as a key (`Change::key`).
pub const NEXT: &str = "weather.next";
/// The hour of the day (0–23) the next change comes at, when it has one.
pub const NEXT_AT: &str = "weather.next.at";
/// The WMO code and day flag of the hour that decides the change: its
/// icon.
pub const NEXT_CODE: &str = "weather.next.code";
pub const NEXT_DAY: &str = "weather.next.day";

/// Every forecast signal, for the widget's redraws.
pub fn signals() -> Vec<&'static str> {
    let mut out = vec![NEXT, NEXT_AT, NEXT_CODE, NEXT_DAY];
    for h in &HOURS {
        out.extend([h.hour, h.temp, h.code, h.rain, h.day]);
    }
    for d in &DAYS {
        out.extend([d.day, d.high, d.low, d.code, d.rain]);
    }
    out
}

/// One hour of the forecast, as the rule reads it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hour {
    /// 0–23, local to the place.
    pub hour: u8,
    /// WMO weather code.
    pub code: Option<f64>,
    /// Chance of rain, percent.
    pub rain: Option<f64>,
    pub day: bool,
}

/// What falls.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wet {
    Drizzle,
    Rain,
    Snow,
    Storms,
}

impl Wet {
    /// What a WMO code says falls, if anything.
    pub fn of_code(code: f64) -> Option<Wet> {
        match code.round() as i64 {
            51..=57 => Some(Wet::Drizzle),
            61..=67 | 80..=82 => Some(Wet::Rain),
            71..=77 | 85 | 86 => Some(Wet::Snow),
            95..=99 => Some(Wet::Storms),
            _ => None,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Wet::Drizzle => "drizzle",
            Wet::Rain => "rain",
            Wet::Snow => "snow",
            Wet::Storms => "storms",
        }
    }

    fn from_key(key: &str) -> Option<Wet> {
        [Wet::Drizzle, Wet::Rain, Wet::Snow, Wet::Storms]
            .into_iter()
            .find(|w| w.key() == key)
    }

    /// The word the strip says.
    fn word(self) -> &'static str {
        match self {
            Wet::Drizzle => "Drizzle",
            Wet::Rain => "Rain",
            Wet::Snow => "Snow",
            Wet::Storms => "Storms",
        }
    }
}

impl Hour {
    /// What falls in this hour: its code says something falls, at a
    /// chance of at least [`RAIN_SHOWN`] (or no chance given).
    pub fn wet(&self) -> Option<Wet> {
        let likely = self.rain.is_none_or(|r| r >= RAIN_SHOWN);
        self.code.and_then(Wet::of_code).filter(|_| likely)
    }
}

/// From 18:00 to 06:00.
fn is_night(hour: u8) -> bool {
    !(6..18).contains(&hour)
}

/// The next notable change in the weather, in the next [`NEXT_WINDOW`]
/// hours.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    /// Dry now, wet from `at`: "Rain from 15:00".
    Starts { wet: Wet, at: u8 },
    /// Dry now, in the day, and wet from `at` in the night: "Snow
    /// tonight".
    Tonight { wet: Wet, at: u8 },
    /// Wet now, dry from `at` for two hours or more: "Clearing by 18:00".
    Clears { at: u8 },
    /// Wet now and all through the window: "Rain all day".
    Lasts { wet: Wet, night: bool },
    /// Dry now and all through the window: "Dry all day", "Dry tonight".
    Dry { night: bool },
}

/// The rule (`DESIGN.md`, Forecast), hour by hour over `ahead` (the hours
/// after `now`, in order; only the first [`NEXT_WINDOW`] are read). An
/// hour is wet as `Hour::wet` says.
///
/// - Dry now: the first wet hour is the change. It is "tonight" when now
///   is day (06:00–18:00) and that hour is night; else "from" its hour.
///   None: dry, "all day" by day and "tonight" by night.
/// - Wet now: the first dry hour that the next is dry too (or that ends
///   the window) is when it clears, so one dry hour between showers is
///   not called clearing. None: it lasts, "all day" or "all night".
///
/// Also returns the hour that decides it (`now` or one of `ahead`), whose
/// weather is the change's icon.
pub fn next_change(now: &Hour, ahead: &[Hour]) -> (Change, Hour) {
    let ahead = &ahead[..ahead.len().min(NEXT_WINDOW)];
    let night = is_night(now.hour);
    match now.wet() {
        None => match ahead.iter().find_map(|h| h.wet().map(|w| (w, h))) {
            Some((wet, h)) if !night && is_night(h.hour) => {
                (Change::Tonight { wet, at: h.hour }, *h)
            }
            Some((wet, h)) => (Change::Starts { wet, at: h.hour }, *h),
            None => (Change::Dry { night }, *now),
        },
        Some(wet) => {
            let clears = (0..ahead.len()).find(|&i| {
                ahead[i].wet().is_none() && ahead.get(i + 1).is_none_or(|n| n.wet().is_none())
            });
            match clears {
                Some(i) => (Change::Clears { at: ahead[i].hour }, ahead[i]),
                None => (Change::Lasts { wet, night }, *now),
            }
        }
    }
}

impl Change {
    /// How the source publishes it, in `weather.next`.
    pub fn key(&self) -> String {
        match self {
            Change::Starts { wet, .. } => format!("{}-from", wet.key()),
            Change::Tonight { wet, .. } => format!("{}-tonight", wet.key()),
            Change::Clears { .. } => "clearing".into(),
            Change::Lasts { wet, night: false } => format!("{}-all-day", wet.key()),
            Change::Lasts { wet, night: true } => format!("{}-all-night", wet.key()),
            Change::Dry { night: false } => "dry-all-day".into(),
            Change::Dry { night: true } => "dry-tonight".into(),
        }
    }

    /// The hour it comes at, published in `weather.next.at`.
    pub fn at(&self) -> Option<u8> {
        match self {
            Change::Starts { at, .. } | Change::Tonight { at, .. } | Change::Clears { at } => {
                Some(*at)
            }
            _ => None,
        }
    }

    /// The change a key and hour name; `None` for a key it does not know
    /// or one that needs an hour without it.
    pub fn from_key(key: &str, at: Option<u8>) -> Option<Change> {
        match key {
            "clearing" => return at.map(|at| Change::Clears { at }),
            "dry-all-day" => return Some(Change::Dry { night: false }),
            "dry-tonight" => return Some(Change::Dry { night: true }),
            _ => {}
        }
        let (wet, rest) = key.split_once('-')?;
        let wet = Wet::from_key(wet)?;
        match rest {
            "from" => at.map(|at| Change::Starts { wet, at }),
            "tonight" => Some(Change::Tonight {
                wet,
                at: at.unwrap_or(21),
            }),
            "all-day" => Some(Change::Lasts { wet, night: false }),
            "all-night" => Some(Change::Lasts { wet, night: true }),
            _ => None,
        }
    }

    /// Its sentence in two parts, the words and what they are about:
    /// ("Rain from", "15:00"), ("Snow", "tonight"), ("Dry", "all day").
    /// Every word the strip says about the forecast is here, in one place
    /// to translate.
    pub fn words(&self, clock: ClockFormat) -> (String, String) {
        match self {
            Change::Starts { wet, at } => (format!("{} from", wet.word()), hour_text(*at, clock)),
            Change::Tonight { wet, .. } => (wet.word().into(), "tonight".into()),
            Change::Clears { at } => ("Clearing by".into(), hour_text(*at, clock)),
            Change::Lasts { wet, night } => (
                wet.word().into(),
                if *night { "all night" } else { "all day" }.into(),
            ),
            Change::Dry { night } => (
                "Dry".into(),
                if *night { "tonight" } else { "all day" }.into(),
            ),
        }
    }

    /// The sentence: "Rain from 15:00", "Clearing by 6 PM".
    pub fn sentence(&self, clock: ClockFormat) -> String {
        let (head, tail) = self.words(clock);
        format!("{head} {tail}")
    }

    /// What a compact card says beside the icon: the time when the change
    /// has one, else one word ("Dry", "Rain").
    pub fn short(&self, clock: ClockFormat) -> String {
        match self {
            Change::Starts { at, .. } | Change::Tonight { at, .. } | Change::Clears { at } => {
                hour_text(*at, clock)
            }
            Change::Lasts { wet, .. } => wet.word().into(),
            Change::Dry { .. } => "Dry".into(),
        }
    }
}

/// An hour of the day as the clock setting writes it: "15:00", "3 PM".
pub fn hour_text(hour: u8, clock: ClockFormat) -> String {
    let hour = hour % 24;
    match clock {
        ClockFormat::H24 => format!("{hour:02}:00"),
        ClockFormat::H12 => {
            let h = match hour % 12 {
                0 => 12,
                h => h,
            };
            let m = if hour < 12 { "AM" } else { "PM" };
            format!("{h} {m}")
        }
    }
}

/// The widest an hour can be written in a clock format: what an hour
/// column is laid out for, whatever the hours are.
pub fn hour_widest(clock: ClockFormat) -> Vec<String> {
    match clock {
        ClockFormat::H24 => vec!["88:00".into()],
        ClockFormat::H12 => vec!["88 AM".into(), "88 PM".into()],
    }
}

/// What a compact card is laid out for: the widest time, and "Dry". A
/// wider word ("Storms", for storms all day) is laid out for when it
/// shows; with the time at 20 px in a 106 px zone it would otherwise
/// push every time above its icon.
pub fn short_widest(clock: ClockFormat) -> Vec<String> {
    let mut out = hour_widest(clock);
    out.push("Dry".into());
    out
}

const DAY_KEYS: [(&str, &str); 7] = [
    ("mon", "Mon"),
    ("tue", "Tue"),
    ("wed", "Wed"),
    ("thu", "Thu"),
    ("fri", "Fri"),
    ("sat", "Sat"),
    ("sun", "Sun"),
];

/// A date's day name key, as `weather.d1.day` carries it: "2026-11-23"
/// is `"mon"`.
pub fn day_key(date: &str) -> Option<&'static str> {
    let d: Date = date.trim().parse().ok()?;
    let i = match d.weekday() {
        Weekday::Monday => 0,
        Weekday::Tuesday => 1,
        Weekday::Wednesday => 2,
        Weekday::Thursday => 3,
        Weekday::Friday => 4,
        Weekday::Saturday => 5,
        Weekday::Sunday => 6,
    };
    Some(DAY_KEYS[i].0)
}

/// A day name key's short name on the strip: `"mon"` is "Mon".
pub fn day_name(key: &str) -> Option<&'static str> {
    DAY_KEYS.iter().find(|(k, _)| *k == key).map(|(_, n)| *n)
}

/// Every short day name, for laying day columns out for the widest.
pub fn day_names() -> impl Iterator<Item = &'static str> {
    DAY_KEYS.iter().map(|(_, n)| *n)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hours from `start`, one per character: `.` dry and clear, `c`
    /// cloudy, `r` rain, `d` drizzle, `s` snow, `t` thunder, `l` rain at a
    /// low chance (20 %).
    fn hours(start: u8, pattern: &str) -> Vec<Hour> {
        pattern
            .chars()
            .enumerate()
            .map(|(i, c)| {
                let hour = (start as usize + i) as u8 % 24;
                let (code, rain) = match c {
                    '.' => (0.0, 0.0),
                    'c' => (3.0, 10.0),
                    'r' => (63.0, 80.0),
                    'd' => (53.0, 50.0),
                    's' => (73.0, 70.0),
                    't' => (95.0, 90.0),
                    'l' => (61.0, 20.0),
                    _ => unreachable!(),
                };
                Hour {
                    hour,
                    code: Some(code),
                    rain: Some(rain),
                    day: (6..18).contains(&hour),
                }
            })
            .collect()
    }

    /// The sentence for weather now at `start` and the pattern after it.
    fn next(start: u8, pattern: &str, clock: ClockFormat) -> String {
        let all = hours(start, pattern);
        next_change(&all[0], &all[1..]).0.sentence(clock)
    }

    const H24: ClockFormat = ClockFormat::H24;

    #[test]
    fn dry_then_rain_says_when_it_starts() {
        assert_eq!(next(10, "..cccrrrr....", H24), "Rain from 15:00");
        assert_eq!(
            next(10, "..cccrrrr....", ClockFormat::H12),
            "Rain from 3 PM"
        );
        assert_eq!(next(10, ".d", H24), "Drizzle from 11:00");
        assert_eq!(next(13, "..t", H24), "Storms from 15:00");
    }

    #[test]
    fn rain_in_the_night_from_the_day_is_tonight() {
        assert_eq!(next(14, "cccccccsss", H24), "Snow tonight");
        // Already night: the hour says more than "tonight".
        assert_eq!(next(20, "...r", H24), "Rain from 23:00");
    }

    #[test]
    fn rain_now_says_when_it_clears() {
        assert_eq!(next(13, "rrrrr..cc", H24), "Clearing by 18:00");
        assert_eq!(next(13, "rrrrr..cc", ClockFormat::H12), "Clearing by 6 PM");
        // One dry hour between showers is not clearing.
        assert_eq!(next(13, "rr.rr..", H24), "Clearing by 18:00");
        // Dry at the very end of the window counts.
        assert_eq!(next(6, "rrrrrrrrrrrr.", H24), "Clearing by 18:00");
    }

    #[test]
    fn no_change_says_so() {
        assert_eq!(next(9, ".............", H24), "Dry all day");
        assert_eq!(next(19, "ccccccccccccc", H24), "Dry tonight");
        assert_eq!(next(9, "rrrrrrrrrrrrr", H24), "Rain all day");
        assert_eq!(next(22, "sssssssssssss", H24), "Snow all night");
    }

    #[test]
    fn a_low_chance_is_not_rain_and_the_window_is_twelve_hours() {
        assert_eq!(next(9, "lllll", H24), "Dry all day");
        // Rain thirteen hours away is outside the window.
        assert_eq!(next(5, ".............r", H24), "Dry tonight");
        assert_eq!(next(5, "............r", H24), "Rain from 17:00");
        // No chance given: the code alone decides.
        let mut h = hours(9, ".r");
        h[1].rain = None;
        assert_eq!(
            next_change(&h[0], &h[1..]).0.sentence(H24),
            "Rain from 10:00"
        );
    }

    #[test]
    fn the_deciding_hour_gives_the_icon() {
        let all = hours(13, "rrrrr..cc");
        let (_, at) = next_change(&all[0], &all[1..]);
        assert_eq!((at.hour, at.code), (18, Some(0.0)));
    }

    #[test]
    fn keys_round_trip() {
        let changes = [
            Change::Starts {
                wet: Wet::Rain,
                at: 15,
            },
            Change::Tonight {
                wet: Wet::Snow,
                at: 21,
            },
            Change::Clears { at: 18 },
            Change::Lasts {
                wet: Wet::Drizzle,
                night: true,
            },
            Change::Lasts {
                wet: Wet::Storms,
                night: false,
            },
            Change::Dry { night: false },
            Change::Dry { night: true },
        ];
        for c in changes {
            assert_eq!(Change::from_key(&c.key(), c.at()), Some(c), "{}", c.key());
        }
        assert_eq!(Change::from_key("rain-from", None), None);
        assert_eq!(Change::from_key("hail-from", Some(3)), None);
    }

    #[test]
    fn hours_and_days_are_written_as_the_clock_is() {
        assert_eq!(hour_text(0, H24), "00:00");
        assert_eq!(hour_text(0, ClockFormat::H12), "12 AM");
        assert_eq!(hour_text(12, ClockFormat::H12), "12 PM");
        assert_eq!(hour_text(21, ClockFormat::H12), "9 PM");
        assert_eq!(day_key("2026-11-23"), Some("mon"));
        assert_eq!(day_key("2026-11-22"), Some("sun"));
        assert_eq!(day_key("soon"), None);
        assert_eq!(day_name("thu"), Some("Thu"));
        assert_eq!(signals().len(), 4 + 5 * HOUR_SLOTS + 5 * DAY_SLOTS);
    }
}
