//! Binds a configured page to live signals, producing the formatted zones
//! the renderer draws.
//!
//! All presentation decisions live here: units, number formatting,
//! thresholds and graph scaling. The input is plain data (config, signals,
//! the current time), so every decision is unit-tested without a clock or a
//! device.

use jiff::Zoned;

use crate::config::{ClockFormat, GraphStyle, Settings, SliderControl, Temperature, Widget};
use crate::signal::{Signals, Unit, Value};
use crate::widget::{Graph, Reading, Severity, Track, WidgetView, Zone};

/// Shown where a reading has not arrived yet. Calm, not an error.
pub const NO_DATA: &str = "—";

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

/// Builds the zones for a row of widgets: a page, or a takeover.
pub fn bind_page(
    widgets: &[Widget],
    signals: &Signals,
    settings: &Settings,
    now: &Zoned,
) -> Vec<Zone> {
    widgets
        .iter()
        .map(|w| Zone {
            width: w.width(),
            view: bind_widget(w, signals, settings, now),
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
        Widget::Value { signal, second, .. } => {
            let mut used = vec![signal.as_str()];
            used.extend(second.as_deref());
            used
        }
        Widget::Rate { down, up, .. } => vec![down.as_str(), up.as_str()],
        Widget::Key { .. } => vec![],
    })
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

/// How a signal's reading stands against thresholds; `Ok` without a number.
pub fn judge(signals: &Signals, signal: &str, warn: Option<f64>, crit: Option<f64>) -> Severity {
    match signals.get(signal) {
        Some(Value::Number(n, _)) => severity(*n, warn, crit),
        _ => Severity::Ok,
    }
}

/// A signal's value and unit text, as a value widget shows it: "—" with
/// no reading yet.
pub fn reading_text(signals: &Signals, signal: &str, settings: &Settings) -> (String, String) {
    match signals.get(signal) {
        Some(Value::Number(n, u)) => format_number(*n, *u, settings.temperature),
        Some(Value::Text(t)) => (t.clone(), String::new()),
        None => (NO_DATA.to_string(), String::new()),
    }
}

fn bind_widget(widget: &Widget, signals: &Signals, settings: &Settings, now: &Zoned) -> WidgetView {
    match widget {
        Widget::Clock { .. } => clock(settings.clock, now),
        Widget::Media { .. } => media(signals),
        Widget::Slider { .. } => slider(widget, signals, None, None),
        Widget::Value {
            label,
            signal,
            graph,
            warn,
            crit,
            second,
            second_warn,
            second_crit,
            ..
        } => {
            let reading = match signals.get(signal) {
                Some(Value::Number(n, unit)) => Some((*n, *unit)),
                _ => None,
            };
            let (value, unit) = reading_text(signals, signal, settings);
            let severity = judge(signals, signal, *warn, *crit);
            let second = second.as_deref().map(|s| {
                let (value, unit) = reading_text(signals, s, settings);
                Reading {
                    value,
                    unit,
                    severity: judge(signals, s, *second_warn, *second_crit),
                }
            });
            let graph = graph.and_then(|style| {
                let unit = reading.map_or(Unit::None, |(_, u)| u);
                let points = scale(&signals.history(signal), unit);
                (!points.is_empty()).then_some(match style {
                    GraphStyle::Area => Graph::Area(points),
                    GraphStyle::Line => Graph::Line(points),
                })
            });
            WidgetView::Value {
                label: label.clone(),
                value,
                unit,
                icon: None,
                graph,
                severity,
                second,
            }
        }
        Widget::Key { icon, label, .. } => WidgetView::Key {
            icon: icon.clone(),
            label: label.clone(),
        },
        Widget::Rate {
            label, down, up, ..
        } => {
            let rate = |name: &str| match signals.get(name) {
                Some(Value::Number(n, _)) => format_rate(*n),
                _ => NO_DATA.to_string(),
            };
            let history = scale(&signals.history(down), Unit::BytesPerSecond);
            WidgetView::Rate {
                label: label.clone(),
                caption: format!("↓{} ↑{}", rate(down), rate(up)),
                graph: (!history.is_empty()).then_some(Graph::Line(history)),
            }
        }
    }
}

/// A slider's view. `held` is the finger's position (0–1) while sliding:
/// shown instead of the live value, so the bar follows the finger at once.
/// `last_set` is what a `run` slider without a signal shows at rest.
pub fn slider(
    widget: &Widget,
    signals: &Signals,
    held: Option<f32>,
    last_set: Option<f32>,
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
    let percent = |f: f32| format!("{:.0}%", f * 100.0);
    let (fraction, value, muted) = match controls {
        SliderControl::Volume => {
            let muted = held.is_none() && number("volume.muted").is_some_and(|m| m != 0.0);
            let f = held.or(number("volume.level").map(|v| (v / 100.0) as f32));
            let value = match f {
                _ if muted => "Muted".to_string(),
                Some(f) => percent(f),
                None => NO_DATA.to_string(),
            };
            (f, value, muted)
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
                    (f, value, false)
                }
                None => (None, NO_DATA.to_string(), false),
            }
        }
        SliderControl::Run(_) => {
            let shown = signal
                .as_deref()
                .and_then(number)
                .map(|v| (v / 100.0) as f32);
            let f = held.or(shown).or(last_set);
            (f, f.map_or(NO_DATA.to_string(), percent), false)
        }
    };
    WidgetView::Slider {
        label: label.clone(),
        value,
        fraction: fraction.map(|f| f.clamp(0.0, 1.0)),
        active: held.is_some(),
        muted,
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

fn media(signals: &Signals) -> WidgetView {
    let text = |name| match signals.get(name) {
        Some(Value::Text(t)) => t.trim().to_string(),
        _ => String::new(),
    };
    let number = |name| match signals.get(name) {
        Some(Value::Number(n, _)) => *n,
        _ => 0.0,
    };
    let status = text("media.status");
    let title = text("media.title");
    if title.is_empty() || status.is_empty() || status == "stopped" {
        return WidgetView::Media(None);
    }
    let length = number("media.length");
    let progress =
        (length > 0.0).then(|| (number("media.position") / length).clamp(0.0, 1.0) as f32);
    WidgetView::Media(Some(Track {
        title,
        artist: text("media.artist"),
        progress,
        playing: status == "playing",
    }))
}

fn clock(format: ClockFormat, now: &Zoned) -> WidgetView {
    let date = now.strftime("%a, %b %d").to_string();
    match format {
        ClockFormat::H24 => WidgetView::Clock {
            date,
            time: now.strftime("%H:%M").to_string(),
            meridiem: None,
        },
        ClockFormat::H12 => WidgetView::Clock {
            date,
            time: now.strftime("%-I:%M").to_string(),
            meridiem: Some(now.strftime("%p").to_string()),
        },
    }
}

/// Value and unit text for a number in its base unit.
fn format_number(n: f64, unit: Unit, temperature: Temperature) -> (String, String) {
    match unit {
        Unit::Celsius => match temperature {
            Temperature::Celsius => (format!("{n:.0}"), "°C".into()),
            Temperature::Fahrenheit => (format!("{:.0}", n * 9.0 / 5.0 + 32.0), "°F".into()),
        },
        Unit::Percent => (format!("{n:.0}"), "%".into()),
        Unit::BytesPerSecond => (format_rate(n), String::new()),
        Unit::None => (format!("{n:.0}"), String::new()),
    }
}

/// A byte rate in at most four characters plus "/s": "512B/s", "4.2K/s",
/// "292K/s", "1.1G/s". Decimal (SI) multiples, as network tools use.
pub fn format_rate(bytes_per_second: f64) -> String {
    const PREFIXES: [&str; 5] = ["B", "K", "M", "G", "T"];
    let mut v = bytes_per_second.max(0.0);
    let mut i = 0;
    while v >= 999.5 && i < PREFIXES.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i > 0 && v < 9.95 {
        format!("{v:.1}{}/s", PREFIXES[i])
    } else {
        format!("{v:.0}{}/s", PREFIXES[i])
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

/// Maps history to 0.0–1.0 for drawing.
///
/// Percentages use their fixed 0–100 range. Rates start at zero and scale to
/// the window's peak. Anything else (temperatures) zooms to the window's own
/// range, but never to less than `MIN_SPAN` units, so sensor noise of a
/// degree stays a flat line instead of filling the zone.
fn scale(history: &[f32], unit: Unit) -> Vec<f32> {
    const MIN_SPAN: f32 = 10.0;
    if history.is_empty() {
        return Vec::new();
    }
    let max = history.iter().copied().fold(f32::MIN, f32::max);
    let min = history.iter().copied().fold(f32::MAX, f32::min);
    let (lo, hi) = match unit {
        Unit::Percent => (0.0, 100.0),
        Unit::BytesPerSecond => (0.0, max.max(1.0)),
        Unit::Celsius | Unit::None => {
            let pad = (MIN_SPAN - (max - min)).max(0.0) / 2.0;
            (min - pad, max + pad)
        }
    };
    history
        .iter()
        .map(|&v| ((v - lo) / (hi - lo)).clamp(0.0, 1.0))
        .collect()
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

    #[test]
    fn clock_formats() {
        assert_eq!(
            clock(ClockFormat::H12, &now()),
            WidgetView::Clock {
                date: "Tue, Jun 09".into(),
                time: "4:17".into(),
                meridiem: Some("PM".into()),
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
            ("79".into(), "°F".into())
        );
        assert_eq!(
            format_number(26.4, Unit::Celsius, Temperature::Celsius),
            ("26".into(), "°C".into())
        );
        assert_eq!(severity(80.0, Some(75.0), Some(90.0)), Severity::Warn);
        assert_eq!(severity(90.0, Some(75.0), Some(90.0)), Severity::Crit);
        assert_eq!(severity(10.0, Some(75.0), None), Severity::Ok);
    }

    #[test]
    fn media_shows_track_progress_and_pause() {
        let mut s = Signals::new();
        assert_eq!(media(&s), WidgetView::Media(None), "no player yet");
        s.set("media.title", Value::Text("Teardrop".into()));
        s.set("media.artist", Value::Text("Massive Attack".into()));
        s.set("media.status", Value::Text("playing".into()));
        s.set("media.position", Value::Number(75.0, Unit::None));
        s.set("media.length", Value::Number(300.0, Unit::None));
        assert_eq!(
            media(&s),
            WidgetView::Media(Some(Track {
                title: "Teardrop".into(),
                artist: "Massive Attack".into(),
                progress: Some(0.25),
                playing: true,
            }))
        );
        // A live stream has no length: no bar, rather than a wrong one.
        s.set("media.status", Value::Text("paused".into()));
        s.set("media.length", Value::Number(0.0, Unit::None));
        let WidgetView::Media(Some(t)) = media(&s) else {
            unreachable!()
        };
        assert_eq!((t.progress, t.playing), (None, false));
        s.set("media.status", Value::Text("stopped".into()));
        assert_eq!(media(&s), WidgetView::Media(None));
    }

    #[test]
    fn sliders_show_live_values_or_the_finger() {
        let w = |controls: &str| {
            let c = Config::parse(&format!(
                "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"slider\"\nwidth = 200\nlabel = \"S\"\ncontrols = {controls}\n[[page.widget]]\ntype = \"clock\"\nwidth = 100\n"
            ))
            .unwrap();
            c.pages[0].widgets[0].clone()
        };
        let view = |w: &Widget, s: &Signals, held| match slider(w, s, held, None) {
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
        assert_eq!(view(&volume, &s, None), ("40%".into(), Some(0.4), false));
        assert_eq!(
            view(&volume, &s, Some(0.75)),
            ("75%".into(), Some(0.75), false)
        );
        s.set("volume.muted", Value::Number(1.0, Unit::None));
        assert_eq!(view(&volume, &s, None), ("Muted".into(), Some(0.4), true));

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
            slider(&run, &s, None, Some(0.3)),
            WidgetView::Slider { fraction: Some(f), .. } if (f - 0.3).abs() < 1e-6
        ));
    }

    #[test]
    fn a_second_reading_is_bound_with_its_own_thresholds() {
        let c = Config::parse(
            "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"value\"\nwidth = 160\nlabel = \"GPU\"\n\
             signal = \"gpu.temp\"\ncrit = 90\nsecond = \"gpu.load\"\nsecond_warn = 70\n\
             [[page.widget]]\ntype = \"key\"\nwidth = 60\nicon = \"play\"\n",
        )
        .unwrap();
        let mut s = Signals::new();
        s.set("gpu.temp", Value::Number(50.0, Unit::Celsius));
        let zones = bind_page(&c.pages[0].widgets, &s, &c.settings, &now());
        let WidgetView::Value { second, .. } = &zones[0].view else {
            unreachable!()
        };
        assert_eq!(
            second,
            &Some(Reading {
                value: NO_DATA.into(),
                unit: String::new(),
                severity: Severity::Ok
            })
        );
        s.set("gpu.load", Value::Number(75.0, Unit::Percent));
        let zones = bind_page(&c.pages[0].widgets, &s, &c.settings, &now());
        let WidgetView::Value {
            severity, second, ..
        } = &zones[0].view
        else {
            unreachable!()
        };
        assert_eq!(*severity, Severity::Ok);
        assert_eq!(
            second,
            &Some(Reading {
                value: "75".into(),
                unit: "%".into(),
                severity: Severity::Warn
            })
        );
        let used: Vec<&str> = signals_used(&c.pages[0].widgets).collect();
        assert_eq!(used, ["gpu.temp", "gpu.load"]);
        assert_eq!(
            zones[1].view,
            WidgetView::Key {
                icon: Some("play".into()),
                label: None
            }
        );
    }

    #[test]
    fn rates_stay_short() {
        assert_eq!(format_rate(0.0), "0B/s");
        assert_eq!(format_rate(512.0), "512B/s");
        assert_eq!(format_rate(999.0), "999B/s");
        assert_eq!(format_rate(4_200.0), "4.2K/s");
        assert_eq!(format_rate(292_000.0), "292K/s");
        assert_eq!(format_rate(999_700.0), "1.0M/s");
        assert_eq!(format_rate(1_100_000_000.0), "1.1G/s");
    }

    #[test]
    fn graphs_scale_by_unit() {
        assert_eq!(scale(&[0.0, 50.0, 100.0], Unit::Percent), [0.0, 0.5, 1.0]);
        assert_eq!(
            scale(&[0.0, 500.0, 1000.0], Unit::BytesPerSecond),
            [0.0, 0.5, 1.0]
        );
        // 40–41 °C is noise: drawn near the middle, not edge to edge.
        let t = scale(&[40.0, 41.0], Unit::Celsius);
        assert!(t[0] > 0.4 && t[1] < 0.6, "{t:?}");
    }

    #[test]
    fn missing_signals_show_a_calm_placeholder() {
        let config = Config::default_layout();
        let zones = bind_page(
            &config.pages[0].widgets,
            &Signals::new(),
            &config.settings,
            &now(),
        );
        assert_eq!(zones.len(), 6);
        assert_eq!(zones[1].view, WidgetView::Media(None), "nothing playing");
        let WidgetView::Value { value, graph, .. } = &zones[3].view else {
            panic!("CPU is a value widget")
        };
        assert_eq!((value.as_str(), graph), (NO_DATA, &None));
    }

    #[test]
    fn page_signal_list() {
        let config = Config::default_layout();
        let used: Vec<&str> = signals_used(&config.pages[0].widgets).collect();
        let mut expected = MEDIA_SIGNALS.to_vec();
        expected.extend(["weather.temp", "cpu.temp", "gpu.temp", "net.down", "net.up"]);
        assert_eq!(used, expected);
        assert!(shows_clock(&config.pages[0].widgets));
        assert!(!shows_clock(&config.pages[2].widgets));
    }
}
