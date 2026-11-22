//! Binds a configured page to live signals, producing the formatted zones
//! the renderer draws.
//!
//! All presentation decisions live here: units, number formatting,
//! thresholds, the fixed scale each kind of reading is drawn on, and which
//! of "no data", "not set up" and "lost" a reading is. The input is plain
//! data (config, signals, the current time), so every decision is
//! unit-tested without a clock or a device.

use jiff::{SignedDuration, Zoned};

use crate::config::{Action, ClockFormat, Settings, SliderControl, Temperature, Widget};
use crate::icons;
use crate::signal::{Signals, Unit, Value};
use crate::theme::{NET_HI, NET_LO, TEMP_HI, TEMP_LO};
use crate::widget::{Avail, Bar, Reading, Severity, Track, WidgetView, Zone};

/// Shown where a number has not arrived yet. Calm, not an error.
pub const NO_DATA: &str = "––";

/// The signals a volume source publishes: `volume.level` (percent) and
/// `volume.muted` (1 or 0).
pub const VOLUME_SIGNALS: [&str; 2] = ["volume.level", "volume.muted"];

/// The signals a media source publishes and the media widget reads.
/// `media.status` is "playing", "paused" or "stopped"; position and length
/// are in seconds, length 0 when unknown (a live stream).
pub const MEDIA_SIGNALS: [&str; 5] = [
    "media.title",
    "media.artist",
    "media.status",
    "media.position",
    "media.length",
];

/// The signals the weather source publishes. A value widget showing
/// `weather.temp` draws as the weather and reads them all: the WMO
/// condition code, its words, today's high and low (°C), and 1 by day.
pub const WEATHER_SIGNALS: [&str; 6] = [
    "weather.temp",
    "weather.code",
    "weather.condition",
    "weather.high",
    "weather.low",
    "weather.day",
];

/// The signals the news source publishes and the news widget reads: the
/// headline showing, its feed's name, the article's address, its age in
/// seconds, how many headlines are kept, and 1 while every feed is
/// failing and the headline kept is older than `max_age`.
pub const NEWS_SIGNALS: [&str; 6] = [
    "news.title",
    "news.source",
    "news.link",
    "news.age",
    "news.count",
    "news.lost",
];

/// After this long with no new reading, the weather counts as lost: the
/// source polls every 15 minutes, so three missed polls.
pub const WEATHER_STALE: f64 = 45.0 * 60.0;

/// Everything binding reads besides the widgets.
#[derive(Clone, Copy)]
pub struct Bind<'a> {
    pub signals: &'a Signals,
    pub settings: &'a Settings,
    /// The layout says where the weather is for (`[weather]`).
    pub weather: bool,
    /// The layout names at least one news feed (`[news]`).
    pub news: bool,
    /// Wall time, for the clock and for when a source was lost.
    pub now: &'a Zoned,
    /// Monotonic seconds on the clock `Signals` times readings with.
    pub mono: f64,
    /// Seconds since the track's title last started scrolling, while it
    /// may still be.
    pub marquee: Option<f32>,
    /// What opens the music player (`Config::music_action`): an idle
    /// media widget with it, or a `tap` of its own, is drawn as a key.
    pub music: Option<&'a Action>,
}

/// Builds the zones for a row of widgets: a page, or a takeover.
pub fn bind_page(widgets: &[Widget], ctx: &Bind) -> Vec<Zone> {
    widgets
        .iter()
        .map(|w| Zone {
            width: w.width(),
            view: bind_widget(w, ctx),
        })
        .collect()
}

/// Whether the row shows the time, so it must redraw each minute.
pub fn shows_clock(widgets: &[Widget]) -> bool {
    widgets.iter().any(|w| matches!(w, Widget::Clock { .. }))
}

/// Signals a row reads; a change to any other signal needs no redraw.
pub fn signals_used(widgets: &[Widget]) -> impl Iterator<Item = &str> {
    widgets.iter().flat_map(|w| match w {
        Widget::Clock { .. } => vec![],
        Widget::Media { .. } => MEDIA_SIGNALS.to_vec(),
        Widget::Slider {
            controls, signal, ..
        } => match controls {
            SliderControl::Volume => VOLUME_SIGNALS.to_vec(),
            SliderControl::Seek => MEDIA_SIGNALS.to_vec(),
            SliderControl::Run(_) => signal.iter().map(String::as_str).collect(),
        },
        Widget::Value { signal, .. } if signal == "weather.temp" => WEATHER_SIGNALS.to_vec(),
        Widget::Value { signal, second, .. } => {
            let mut used = vec![signal.as_str()];
            used.extend(second.as_deref());
            used
        }
        Widget::Rate { down, up, .. } => vec![down.as_str(), up.as_str()],
        Widget::News { .. } => NEWS_SIGNALS.to_vec(),
        Widget::Key { .. } => vec![],
    })
}

/// A headline is showing and can be opened: the source is not lost, and
/// it gave a title and a link. Only then does a news widget's own tap
/// and hold do anything, and only then is it drawn as a key.
pub fn news_live(signals: &Signals) -> bool {
    let text = |name| matches!(signals.live(name), Some(Value::Text(t)) if !t.trim().is_empty());
    let lost = matches!(signals.live("news.lost"), Some(Value::Number(n, _)) if *n != 0.0);
    !lost && text("news.title") && text("news.link")
}

/// A headline's age as the news widget says it: "now" under a minute,
/// then "12 min", "3 h", "2 d", rounded down.
pub fn news_age(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    match s {
        0..60 => "now".into(),
        60..3600 => format!("{} min", s / 60),
        3600..86_400 => format!("{} h", s / 3600),
        _ => format!("{} d", s / 86_400),
    }
}

/// The readings a row's value widgets judge against thresholds: each
/// signal with its warn and crit, the second reading included.
pub fn thresholds(
    widgets: &[Widget],
) -> impl Iterator<Item = (&Widget, &str, Option<f64>, Option<f64>)> {
    widgets.iter().flat_map(|w| match w {
        Widget::Value {
            signal,
            warn,
            crit,
            second,
            second_warn,
            second_crit,
            ..
        } => {
            let mut out = vec![(w, signal.as_str(), *warn, *crit)];
            if let Some(second) = second {
                out.push((w, second.as_str(), *second_warn, *second_crit));
            }
            out
        }
        _ => vec![],
    })
}

/// How a signal's reading stands against thresholds: `Ok` without a
/// number, and for a source that is gone (its number is old news).
pub fn judge(signals: &Signals, signal: &str, warn: Option<f64>, crit: Option<f64>) -> Severity {
    match signals.live(signal) {
        Some(Value::Number(n, _)) => severity(*n, warn, crit),
        _ => Severity::Ok,
    }
}

/// A signal's value and unit text, as a value widget shows it: "––" with
/// no reading yet.
pub fn reading_text(signals: &Signals, signal: &str, settings: &Settings) -> (String, String) {
    match signals.get(signal) {
        Some(Value::Number(n, u)) => format_number(*n, *u, settings.temperature),
        Some(Value::Text(t)) => (t.clone(), String::new()),
        None => (NO_DATA.to_string(), String::new()),
    }
}

/// A number in a signal's base unit, as its widget shows it: "92°",
/// ("62", "%").
pub fn format_number(n: f64, unit: Unit, temperature: Temperature) -> (String, String) {
    match unit {
        Unit::Celsius => (format!("{:.0}°", to_display(n, temperature)), String::new()),
        Unit::Percent => (format!("{n:.0}"), "%".into()),
        Unit::BytesPerSecond => format_rate(n),
        Unit::None => (format!("{n:.0}"), String::new()),
    }
}

fn to_display(celsius: f64, temperature: Temperature) -> f64 {
    match temperature {
        Temperature::Celsius => celsius,
        Temperature::Fahrenheit => celsius * 9.0 / 5.0 + 32.0,
    }
}

/// A byte rate in at most three figures and its unit: ("512", "B/s"),
/// ("12", "kB/s"), ("1.3", "MB/s"), ("48", "MB/s"), ("1.1", "GB/s").
/// Decimal (SI) multiples, as network tools use.
pub fn format_rate(bytes_per_second: f64) -> (String, String) {
    const UNITS: [&str; 5] = ["B/s", "kB/s", "MB/s", "GB/s", "TB/s"];
    let mut v = bytes_per_second.max(0.0);
    let mut i = 0;
    while v >= 999.5 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    // Whole kilobytes; a decimal only for a few MB or more.
    let text = if i > 1 && v < 9.95 {
        format!("{v:.1}")
    } else {
        format!("{v:.0}")
    };
    (text, UNITS[i].to_string())
}

/// The wall-clock time of monotonic moment `at`, as the clock shows it.
fn wall_time(ctx: &Bind, at: f64) -> String {
    let ago = SignedDuration::from_secs_f64((ctx.mono - at).max(0.0));
    let then = ctx.now.checked_sub(ago).unwrap_or_else(|_| ctx.now.clone());
    match ctx.settings.clock {
        ClockFormat::H24 => then.strftime("%H:%M").to_string(),
        ClockFormat::H12 => then.strftime("%-I:%M").to_string(),
    }
}

/// Whether `signal` is there to show: live, lost (with the time it was
/// last heard from), or nothing yet.
fn avail(ctx: &Bind, signal: &str) -> Avail {
    if signal.trim().is_empty() {
        return Avail::NotSetUp;
    }
    match (ctx.signals.get(signal), ctx.signals.lost_at(signal)) {
        (None, _) => Avail::NoData,
        (Some(_), Some(at)) => Avail::Lost(wall_time(ctx, at)),
        (Some(_), None) => Avail::Live,
    }
}

/// The fixed scale a reading is drawn on, in its base unit, and whether it
/// is logarithmic: temperatures share one, percentages are 0–100, rates
/// are logarithmic. A reading of any other kind has a bar only when the
/// layout gives it a `scale`.
fn scale_for(unit: Unit, custom: Option<[f64; 2]>) -> Option<(f64, f64, bool)> {
    let log = unit == Unit::BytesPerSecond;
    if let Some([lo, hi]) = custom {
        return Some((lo, hi, log && lo > 0.0));
    }
    match unit {
        Unit::Celsius => Some((TEMP_LO, TEMP_HI, false)),
        Unit::Percent => Some((0.0, 100.0, false)),
        Unit::BytesPerSecond => Some((NET_LO, NET_HI, true)),
        Unit::None => None,
    }
}

fn fraction((lo, hi, log): (f64, f64, bool), v: f64) -> f32 {
    let f = if log {
        (v.max(lo).ln() - lo.ln()) / (hi.ln() - lo.ln())
    } else {
        (v - lo) / (hi - lo)
    };
    f.clamp(0.0, 1.0) as f32
}

/// One reading, formatted, with its bar on its fixed scale.
#[allow(clippy::too_many_arguments)]
fn reading(
    ctx: &Bind,
    name: &str,
    signal: &str,
    warn: Option<f64>,
    crit: Option<f64>,
    scale: Option<[f64; 2]>,
    default_unit: Unit,
) -> Reading {
    let avail = avail(ctx, signal);
    let (value, unit) = reading_text(ctx.signals, signal, ctx.settings);
    let base_unit = match ctx.signals.get(signal) {
        Some(Value::Number(_, u)) => *u,
        Some(Value::Text(_)) => Unit::None,
        None => default_unit,
    };
    let number = match ctx.signals.get(signal) {
        Some(Value::Number(n, _)) => Some(*n),
        _ => None,
    };
    let text = matches!(ctx.signals.get(signal), Some(Value::Text(_)));
    let bar = scale_for(base_unit, scale).filter(|_| !text).map(|s| Bar {
        value: number.map(|n| fraction(s, n)),
        warn: warn.map(|w| fraction(s, w)),
        crit: crit.map(|c| fraction(s, c)),
    });
    // A hundred is three digits in every unit a value card shows (and
    // 100 °C is 212 °F): the card is laid out for that many.
    let widest = format_number(100.0, base_unit, ctx.settings.temperature);
    Reading {
        name: name.to_string(),
        value,
        unit,
        severity: judge(ctx.signals, signal, warn, crit),
        avail,
        bar,
        widest,
    }
}

/// The base unit a signal is expected in before its first reading, so a
/// card waiting for data already has the right bar.
fn expected_unit(signal: &str) -> Unit {
    match signal.split_once('.').map(|(_, w)| w) {
        Some("temp") => Unit::Celsius,
        Some("load") => Unit::Percent,
        _ if signal.starts_with("mem") => Unit::Percent,
        _ if signal.starts_with("net.") => Unit::BytesPerSecond,
        _ => Unit::None,
    }
}

/// A widget's icon: the one the layout names, if the bundled set has it,
/// else `fallback`.
fn icon_of(widget: &Widget, fallback: char) -> char {
    widget.icon().and_then(icons::by_name).unwrap_or(fallback)
}

fn bind_widget(widget: &Widget, ctx: &Bind) -> WidgetView {
    match widget {
        Widget::Clock { .. } => clock(ctx.settings.clock, ctx.now),
        Widget::Media { tap, .. } => match media(ctx.signals, ctx.marquee) {
            WidgetView::Media(None) if tap.is_some() || ctx.music.is_some() => {
                WidgetView::OpenMusic
            }
            view => view,
        },
        Widget::Slider { .. } => slider(widget, ctx.signals, None, None, None),
        Widget::Value { signal, .. } if signal == "weather.temp" => weather(widget, ctx),
        Widget::Value {
            label,
            signal,
            scale,
            warn,
            crit,
            second,
            second_label,
            second_warn,
            second_crit,
            ..
        } => {
            let first = reading(
                ctx,
                label,
                signal,
                *warn,
                *crit,
                *scale,
                expected_unit(signal),
            );
            // The limit that matters now: the critical one on a critical
            // reading, as the alarm names it; otherwise the first to come.
            let near = if first.severity == Severity::Crit {
                crit.or(*warn)
            } else {
                warn.or(*crit)
            };
            let limit = near.filter(|_| first.bar.is_some()).map(|l| {
                let unit = match ctx.signals.get(signal) {
                    Some(Value::Number(_, u)) => *u,
                    _ => expected_unit(signal),
                };
                let (v, u) = format_number(l, unit, ctx.settings.temperature);
                format!("limit {v}{u}")
            });
            let second_signal = second.as_deref();
            let second = second.as_deref().map(|s| {
                let name = second_label.clone().unwrap_or_default();
                reading(
                    ctx,
                    &name,
                    s,
                    *second_warn,
                    *second_crit,
                    None,
                    expected_unit(s),
                )
            });
            WidgetView::Value {
                icon: icon_of(widget, icons::for_signal(signal)),
                reading: first,
                limit,
                second_icon: second_signal.map_or(icons::GAUGE, icons::for_signal),
                second,
            }
        }
        Widget::Key {
            icon,
            label,
            sub,
            tap,
            hold,
            ..
        } => WidgetView::Key {
            icon: icon.as_deref().and_then(icons::by_name),
            label: label.clone(),
            sub: sub.clone(),
            set_up: tap.is_some() || hold.is_some(),
        },
        Widget::News { .. } => news(widget, ctx),
        Widget::Rate { down, up, .. } => {
            let r = |name: &str, signal: &str| {
                reading(ctx, name, signal, None, None, None, Unit::BytesPerSecond)
            };
            WidgetView::Rate {
                down: r("Down", down),
                up: r("Up", up),
            }
        }
    }
}

fn weather(widget: &Widget, ctx: &Bind) -> WidgetView {
    let s = ctx.signals;
    let number = |name| match s.get(name) {
        Some(Value::Number(n, _)) => Some(*n),
        _ => None,
    };
    let degrees = |c: f64| format!("{:.0}°", to_display(c, ctx.settings.temperature));
    let temp = number("weather.temp");
    let stale = s
        .heard_at("weather.temp")
        .filter(|at| ctx.mono - at > WEATHER_STALE);
    let avail = match (temp, s.lost_at("weather.temp").or(stale)) {
        (None, _) if !ctx.weather => Avail::NotSetUp,
        (None, _) => Avail::NoData,
        (Some(_), Some(at)) => Avail::Lost(wall_time(ctx, at)),
        (Some(_), None) => Avail::Live,
    };
    let condition = match s.get("weather.condition") {
        Some(Value::Text(t)) => t.clone(),
        _ => String::new(),
    };
    let high_low = number("weather.high")
        .zip(number("weather.low"))
        .map(|(h, l)| format!("{} / {}", degrees(h), degrees(l)));
    let day = number("weather.day").is_none_or(|d| d != 0.0);
    WidgetView::Weather {
        icon: icon_of(widget, icons::for_weather(number("weather.code"), day)),
        temp: temp.map_or(NO_DATA.to_string(), degrees),
        condition,
        high_low,
        avail,
    }
}

fn news(widget: &Widget, ctx: &Bind) -> WidgetView {
    let s = ctx.signals;
    let text = |name| match s.get(name) {
        Some(Value::Text(t)) => t.trim().to_string(),
        _ => String::new(),
    };
    let headline = text("news.title");
    let lost = matches!(s.get("news.lost"), Some(Value::Number(n, _)) if *n != 0.0)
        || s.lost_at("news.title").is_some();
    let avail = match () {
        _ if !ctx.news => Avail::NotSetUp,
        _ if headline.is_empty() => Avail::NoData,
        _ if lost => {
            let at = s.lost_at("news.title").or(s.heard_at("news.lost"));
            Avail::Lost(at.map(|at| wall_time(ctx, at)).unwrap_or_default())
        }
        _ => Avail::Live,
    };
    // The age as of when it was published, plus the time since: a lost
    // headline is not published again, and it keeps getting older.
    let age = match (s.get("news.age"), s.heard_at("news.age")) {
        (Some(Value::Number(n, _)), Some(at)) => news_age(n + (ctx.mono - at).max(0.0)),
        (Some(Value::Number(n, _)), None) => news_age(*n),
        _ => String::new(),
    };
    WidgetView::News {
        icon: icon_of(widget, icons::NEWSPAPER),
        headline,
        source: text("news.source"),
        age: if avail == Avail::NoData {
            String::new()
        } else {
            age
        },
        avail,
    }
}

/// A slider's view. `held` is the finger's position (0–1) while sliding:
/// shown instead of the live value, so the bar follows the finger at once.
/// `last_set` is what a `run` slider without a signal shows at rest.
/// `from` is the level before the change that brought a takeover up.
pub fn slider(
    widget: &Widget,
    signals: &Signals,
    held: Option<f32>,
    last_set: Option<f32>,
    from: Option<f32>,
) -> WidgetView {
    let Widget::Slider {
        label,
        controls,
        signal,
        ..
    } = widget
    else {
        unreachable!("slider() is only called for sliders")
    };
    let number = |name: &str| match signals.get(name) {
        Some(Value::Number(n, _)) => Some(*n),
        _ => None,
    };
    let percent = |f: f32| format!("{:.0}", f * 100.0);
    let none = || (NO_DATA.to_string(), String::new());
    let (fraction, (value, unit), muted, icon) = match controls {
        SliderControl::Volume => {
            let muted = held.is_none() && number("volume.muted").is_some_and(|m| m != 0.0);
            let f = held.or(number("volume.level").map(|v| (v / 100.0) as f32));
            let shown = match f {
                _ if muted => ("Muted".to_string(), String::new()),
                Some(f) => (percent(f), "%".to_string()),
                None => none(),
            };
            let icon = match f {
                _ if muted => icons::SPEAKER_X,
                Some(f) if f < 0.34 => icons::SPEAKER_LOW,
                _ => icons::SPEAKER_HIGH,
            };
            (f, shown, muted, icon)
        }
        SliderControl::Seek => {
            let length = number("media.length").filter(|l| *l > 0.0);
            let active =
                matches!(signals.get("media.status"), Some(Value::Text(s)) if s != "stopped");
            match length.filter(|_| active) {
                Some(length) => {
                    let f = held.or(number("media.position").map(|p| (p / length) as f32));
                    let at = f.map_or(0.0, |f| f64::from(f.clamp(0.0, 1.0)) * length);
                    let value = format!("{} / {}", track_time(at), track_time(length));
                    (f, (value, String::new()), false, icons::MUSIC)
                }
                None => (None, none(), false, icons::MUSIC),
            }
        }
        SliderControl::Run(_) => {
            let shown = signal
                .as_deref()
                .and_then(number)
                .map(|v| (v / 100.0) as f32);
            let f = held.or(shown).or(last_set);
            let text = f.map_or_else(none, |f| (percent(f), "%".to_string()));
            let icon = signal.as_deref().map_or(icons::ACTIVITY, icons::for_signal);
            (f, text, false, icon)
        }
    };
    WidgetView::Slider {
        icon: icon_of(widget, icon),
        label: label.clone(),
        value,
        unit,
        fraction: fraction.map(|f| f.clamp(0.0, 1.0)),
        active: held.is_some(),
        muted,
        from,
    }
}

/// "3:07", or "1:02:09" past an hour.
fn track_time(seconds: f64) -> String {
    let s = seconds.max(0.0) as u64;
    if s >= 3600 {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    } else {
        format!("{}:{:02}", s / 60, s % 60)
    }
}

/// What is playing, or `None`: no title, stopped, or the player gone.
pub fn media(signals: &Signals, marquee: Option<f32>) -> WidgetView {
    let text = |name| match signals.live(name) {
        Some(Value::Text(t)) => t.trim().to_string(),
        _ => String::new(),
    };
    let number = |name| match signals.get(name) {
        Some(Value::Number(n, _)) => Some(*n),
        _ => None,
    };
    let status = text("media.status");
    let title = text("media.title");
    if title.is_empty() || status.is_empty() || status == "stopped" {
        return WidgetView::Media(None);
    }
    WidgetView::Media(Some(Track {
        title,
        artist: text("media.artist"),
        position: number("media.position"),
        length: number("media.length").filter(|l| *l > 0.0),
        playing: status == "playing",
        marquee,
    }))
}

fn clock(format: ClockFormat, now: &Zoned) -> WidgetView {
    let f = |s: &str| now.strftime(s).to_string();
    let (time, meridiem) = match format {
        ClockFormat::H24 => (f("%H:%M"), None),
        ClockFormat::H12 => (f("%-I:%M"), Some(f("%p"))),
    };
    WidgetView::Clock {
        time,
        meridiem,
        weekday: f("%A"),
        weekday_short: f("%a"),
        date: f("%B %-d"),
        date_short: f("%b %-d"),
    }
}

fn severity(n: f64, warn: Option<f64>, crit: Option<f64>) -> Severity {
    if crit.is_some_and(|c| n >= c) {
        Severity::Crit
    } else if warn.is_some_and(|w| n >= w) {
        Severity::Warn
    } else {
        Severity::Ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    fn now() -> Zoned {
        "2026-06-09T16:17:05-04:00[America/New_York]"
            .parse()
            .unwrap()
    }

    fn ctx<'a>(s: &'a Signals, settings: &'a Settings, now: &'a Zoned) -> Bind<'a> {
        Bind {
            signals: s,
            settings,
            weather: true,
            news: true,
            now,
            mono: 0.0,
            marquee: None,
            music: None,
        }
    }

    #[test]
    fn clock_formats() {
        assert_eq!(
            clock(ClockFormat::H12, &now()),
            WidgetView::Clock {
                time: "4:17".into(),
                meridiem: Some("PM".into()),
                weekday: "Tuesday".into(),
                weekday_short: "Tue".into(),
                date: "June 9".into(),
                date_short: "Jun 9".into(),
            }
        );
        let WidgetView::Clock { time, meridiem, .. } = clock(ClockFormat::H24, &now()) else {
            unreachable!()
        };
        assert_eq!((time.as_str(), meridiem), ("16:17", None));
    }

    #[test]
    fn temperatures_convert_but_thresholds_stay_in_celsius() {
        assert_eq!(
            format_number(26.0, Unit::Celsius, Temperature::Fahrenheit),
            ("79°".into(), String::new())
        );
        assert_eq!(
            format_number(26.4, Unit::Celsius, Temperature::Celsius),
            ("26°".into(), String::new())
        );
        assert_eq!(severity(80.0, Some(75.0), Some(90.0)), Severity::Warn);
        assert_eq!(severity(90.0, Some(75.0), Some(90.0)), Severity::Crit);
        assert_eq!(severity(10.0, Some(75.0), None), Severity::Ok);
    }

    #[test]
    fn media_shows_track_times_and_pause() {
        let mut s = Signals::new();
        assert_eq!(media(&s, None), WidgetView::Media(None), "no player yet");
        s.set("media.title", Value::Text("Teardrop".into()));
        s.set("media.artist", Value::Text("Massive Attack".into()));
        s.set("media.status", Value::Text("playing".into()));
        s.set("media.position", Value::Number(75.0, Unit::None));
        s.set("media.length", Value::Number(300.0, Unit::None));
        let WidgetView::Media(Some(t)) = media(&s, None) else {
            unreachable!()
        };
        assert_eq!((t.progress(), t.playing), (Some(0.25), true));
        assert_eq!(t.artist, "Massive Attack");
        // A live stream has no length: no bar, rather than a wrong one.
        s.set("media.status", Value::Text("paused".into()));
        s.set("media.length", Value::Number(0.0, Unit::None));
        let WidgetView::Media(Some(t)) = media(&s, None) else {
            unreachable!()
        };
        assert_eq!((t.progress(), t.playing), (None, false));
        s.set("media.status", Value::Text("stopped".into()));
        assert_eq!(media(&s, None), WidgetView::Media(None));
    }

    #[test]
    fn sliders_show_live_values_or_the_finger() {
        let w = |controls: &str| {
            let c = Config::parse(&format!(
                "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"slider\"\nwidth = 200\nlabel = \"S\"\ncontrols = {controls}\n[[page.widget]]\ntype = \"clock\"\nwidth = 440\n"
            ))
            .unwrap();
            c.pages[0].widgets[0].clone()
        };
        let view = |w: &Widget, s: &Signals, held| match slider(w, s, held, None, None) {
            WidgetView::Slider {
                value,
                fraction,
                muted,
                ..
            } => (value, fraction, muted),
            _ => unreachable!(),
        };
        let mut s = Signals::new();
        let volume = w("\"volume\"");
        assert_eq!(view(&volume, &s, None), (NO_DATA.into(), None, false));
        s.set("volume.level", Value::Number(40.0, Unit::Percent));
        assert_eq!(view(&volume, &s, None), ("40".into(), Some(0.4), false));
        assert_eq!(
            view(&volume, &s, Some(0.75)),
            ("75".into(), Some(0.75), false)
        );
        s.set("volume.muted", Value::Number(1.0, Unit::None));
        assert_eq!(view(&volume, &s, None), ("Muted".into(), Some(0.4), true));
        assert!(matches!(
            slider(&volume, &s, None, None, None),
            WidgetView::Slider {
                icon: icons::SPEAKER_X,
                ..
            }
        ));

        let seek = w("\"seek\"");
        assert_eq!(view(&seek, &s, None).0, NO_DATA, "nothing playing");
        s.set("media.status", Value::Text("playing".into()));
        s.set("media.position", Value::Number(65.0, Unit::None));
        s.set("media.length", Value::Number(260.0, Unit::None));
        assert_eq!(
            view(&seek, &s, None),
            ("1:05 / 4:20".into(), Some(0.25), false)
        );
        assert_eq!(view(&seek, &s, Some(0.5)).0, "2:10 / 4:20");

        let run = w("{ run = [\"x\", \"{value}\"] }");
        assert_eq!(view(&run, &s, None).0, NO_DATA);
        assert!(matches!(
            slider(&run, &s, None, Some(0.3), None),
            WidgetView::Slider { fraction: Some(f), .. } if (f - 0.3).abs() < 1e-6
        ));
    }

    #[test]
    fn a_second_reading_is_bound_with_its_own_thresholds() {
        let c = Config::parse(
            "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"value\"\nwidth = 160\nlabel = \"GPU\"\n\
             signal = \"gpu.temp\"\ncrit = 90\nsecond = \"gpu.load\"\nsecond_warn = 70\n\
             second_label = \"load\"\n\
             [[page.widget]]\ntype = \"key\"\nwidth = 480\nicon = \"play\"\n",
        )
        .unwrap();
        let mut s = Signals::new();
        s.set("gpu.temp", Value::Number(50.0, Unit::Celsius));
        let n = now();
        let zones = bind_page(&c.pages[0].widgets, &ctx(&s, &c.settings, &n));
        let WidgetView::Value { second, .. } = &zones[0].view else {
            unreachable!()
        };
        let second = second.as_ref().unwrap();
        assert_eq!(
            (second.value.as_str(), &second.avail),
            (NO_DATA, &Avail::NoData)
        );
        assert_eq!(second.bar.unwrap().value, None, "a load bar, empty");
        s.set("gpu.load", Value::Number(75.0, Unit::Percent));
        let zones = bind_page(&c.pages[0].widgets, &ctx(&s, &c.settings, &n));
        let WidgetView::Value {
            reading,
            second,
            icon,
            limit,
            second_icon,
        } = &zones[0].view
        else {
            unreachable!()
        };
        assert_eq!(*icon, icons::GPU);
        assert_eq!(*second_icon, icons::GAUGE, "named by its own icon");
        assert_eq!(reading.severity, Severity::Ok);
        assert_eq!(reading.value, "50°");
        assert_eq!(limit.as_deref(), Some("limit 90°"));
        let second = second.as_ref().unwrap();
        assert_eq!(
            (
                second.name.as_str(),
                second.value.as_str(),
                second.unit.as_str()
            ),
            ("load", "75", "%")
        );
        assert_eq!(second.severity, Severity::Warn);
        assert_eq!(zones[0].view.severity(), Severity::Warn);
        let used: Vec<&str> = signals_used(&c.pages[0].widgets).collect();
        assert_eq!(used, ["gpu.temp", "gpu.load"]);
        assert_eq!(
            zones[1].view,
            WidgetView::Key {
                icon: Some(icons::PLAY),
                label: None,
                sub: None,
                set_up: false,
            }
        );
    }

    #[test]
    fn readings_sit_on_a_fixed_scale_per_kind() {
        let s = (TEMP_LO, TEMP_HI, false);
        assert_eq!(fraction(s, 30.0), 0.0);
        assert_eq!(fraction(s, 100.0), 1.0);
        assert!((fraction(s, 65.0) - 0.5).abs() < 1e-6);
        let net = scale_for(Unit::BytesPerSecond, None).unwrap();
        assert!(net.2, "rates are logarithmic");
        assert!((fraction(net, 1e6) - 0.5).abs() < 1e-6);
        assert_eq!(scale_for(Unit::None, None), None, "no scale, no bar");
        assert_eq!(
            scale_for(Unit::None, Some([0.0, 5000.0])),
            Some((0.0, 5000.0, false))
        );
    }

    #[test]
    fn rates_stay_short() {
        let r = |n| {
            let (v, u) = format_rate(n);
            format!("{v} {u}")
        };
        assert_eq!(r(0.0), "0 B/s");
        assert_eq!(r(999.0), "999 B/s");
        assert_eq!(r(12_000.0), "12 kB/s");
        assert_eq!(r(1_300_000.0), "1.3 MB/s");
        assert_eq!(r(48_000_000.0), "48 MB/s");
        assert_eq!(r(999_700.0), "1.0 MB/s");
        assert_eq!(r(1_100_000_000.0), "1.1 GB/s");
    }

    #[test]
    fn nothing_lost_and_not_set_up_are_told_apart() {
        let c = Config::parse(
            "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"value\"\nwidth = 320\nlabel = \"CPU\"\n\
             signal = \"cpu.temp\"\n[[page.widget]]\ntype = \"value\"\nwidth = 160\nlabel = \"X\"\n\
             signal = \"\"\n[[page.widget]]\ntype = \"value\"\nwidth = 160\nlabel = \"Weather\"\n\
             signal = \"weather.temp\"\n",
        )
        .unwrap();
        let n = now();
        let mut s = Signals::new();
        let avail = |s: &Signals, weather: bool, mono: f64| {
            let b = Bind {
                weather,
                mono,
                ..ctx(s, &c.settings, &n)
            };
            bind_page(&c.pages[0].widgets, &b)
                .into_iter()
                .map(|z| match z.view {
                    WidgetView::Value { reading, .. } => reading.avail,
                    WidgetView::Weather { avail, .. } => avail,
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>()
        };
        assert_eq!(
            avail(&s, false, 0.0),
            [Avail::NoData, Avail::NotSetUp, Avail::NotSetUp]
        );
        assert_eq!(avail(&s, true, 0.0)[2], Avail::NoData);
        s.set_at("cpu.temp", Value::Number(95.0, Unit::Celsius), 100.0);
        s.set_at("weather.temp", Value::Number(12.0, Unit::Celsius), 100.0);
        s.forget("cpu.temp", 400.0);
        // Lost 600 s before now (16:17:05): at 16:07.
        let got = avail(&s, true, 1000.0);
        assert_eq!(got[0], Avail::Lost("16:07".into()));
        assert_eq!(got[2], Avail::Live);
        assert_eq!(
            judge(&s, "cpu.temp", None, Some(90.0)),
            Severity::Ok,
            "a lost reading raises nothing"
        );
        // Over 45 minutes without a forecast: stale.
        let got = avail(&s, true, 100.0 + WEATHER_STALE + 60.0);
        assert!(matches!(&got[2], Avail::Lost(_)), "{got:?}");
    }

    #[test]
    fn page_signal_list() {
        let config = Config::default_layout();
        let used: Vec<&str> = signals_used(&config.pages[0].widgets).collect();
        assert!(used.contains(&"weather.code"), "{used:?}");
        assert!(shows_clock(&config.pages[0].widgets));
    }

    #[test]
    fn a_headlines_age_is_said_in_words() {
        assert_eq!(news_age(0.0), "now");
        assert_eq!(news_age(59.9), "now");
        assert_eq!(news_age(60.0), "1 min");
        assert_eq!(news_age(12.0 * 60.0 + 30.0), "12 min");
        assert_eq!(news_age(3599.0), "59 min");
        assert_eq!(news_age(3600.0), "1 h");
        assert_eq!(news_age(23.0 * 3600.0), "23 h");
        assert_eq!(news_age(2.0 * 86_400.0 + 5.0), "2 d");
        assert_eq!(news_age(-5.0), "now", "a clock behind the feed's");
    }

    #[test]
    fn news_is_not_set_up_waiting_live_or_lost() {
        let c = Config::parse(
            "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"news\"\nwidth = 320\n",
        )
        .unwrap();
        let n = now();
        let view = |s: &Signals, news: bool, mono: f64| {
            let b = Bind {
                news,
                mono,
                ..ctx(s, &c.settings, &n)
            };
            bind_page(&c.pages[0].widgets, &b).remove(0).view
        };
        let avail = |v: &WidgetView| match v {
            WidgetView::News { avail, .. } => avail.clone(),
            _ => unreachable!(),
        };
        let mut s = Signals::new();
        assert_eq!(avail(&view(&s, false, 0.0)), Avail::NotSetUp);
        assert_eq!(avail(&view(&s, true, 0.0)), Avail::NoData);
        assert!(!news_live(&s));
        let publish = |s: &mut Signals, title: &str, lost: f64| {
            for (name, v) in [
                ("news.title", Value::Text(title.into())),
                ("news.source", Value::Text("Gazette".into())),
                ("news.link", Value::Text("https://news.example/1".into())),
                ("news.age", Value::Number(90.0, Unit::None)),
                ("news.lost", Value::Number(lost, Unit::None)),
            ] {
                s.set_at(name, v, 100.0);
            }
        };
        publish(&mut s, "Ferry back", 0.0);
        assert_eq!(
            view(&s, true, 100.0),
            WidgetView::News {
                icon: icons::NEWSPAPER,
                headline: "Ferry back".into(),
                source: "Gazette".into(),
                age: "1 min".into(),
                avail: Avail::Live,
            }
        );
        assert!(news_live(&s));
        // Not published again for a while: it keeps getting older.
        let WidgetView::News { age, .. } = view(&s, true, 100.0 + 3600.0) else {
            unreachable!()
        };
        assert_eq!(age, "1 h");
        // Blanked: the headlines went, and the feeds work.
        publish(&mut s, "", 0.0);
        assert_eq!(avail(&view(&s, true, 100.0)), Avail::NoData);
        assert!(!news_live(&s));
        publish(&mut s, "Ferry back", 1.0);
        assert!(matches!(avail(&view(&s, true, 100.0)), Avail::Lost(_)));
        assert!(!news_live(&s), "a lost headline is not opened");
        let used: Vec<&str> = signals_used(&c.pages[0].widgets).collect();
        assert_eq!(used, NEWS_SIGNALS);
    }
}
