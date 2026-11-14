//! Binds a configured page to live signals, producing the formatted zones
//! the renderer draws.
//!
//! All presentation decisions live here: units, number formatting,
//! thresholds and graph scaling. The input is plain data (config, signals,
//! the current time), so every decision is unit-tested without a clock or a
//! device.

use jiff::Zoned;

use crate::config::{ClockFormat, GraphStyle, Page, Settings, Temperature, Widget};
use crate::signal::{Signals, Unit, Value};
use crate::widget::{Graph, Severity, WidgetView, Zone};

/// Shown where a reading has not arrived yet. Calm, not an error.
pub const NO_DATA: &str = "—";

/// Builds the zones for one page.
pub fn bind_page(page: &Page, signals: &Signals, settings: &Settings, now: &Zoned) -> Vec<Zone> {
    page.widgets
        .iter()
        .map(|w| Zone {
            width: w.width(),
            view: bind_widget(w, signals, settings, now),
        })
        .collect()
}

/// Whether the page shows the time, so it must redraw each minute.
pub fn shows_clock(page: &Page) -> bool {
    page.widgets
        .iter()
        .any(|w| matches!(w, Widget::Clock { .. }))
}

/// Signals a page reads; a change to any other signal needs no redraw.
pub fn signals_used(page: &Page) -> impl Iterator<Item = &str> {
    page.widgets.iter().flat_map(|w| match w {
        Widget::Clock { .. } => vec![],
        Widget::Value { signal, .. } => vec![signal.as_str()],
        Widget::Rate { down, up, .. } => vec![down.as_str(), up.as_str()],
    })
}

fn bind_widget(widget: &Widget, signals: &Signals, settings: &Settings, now: &Zoned) -> WidgetView {
    match widget {
        Widget::Clock { .. } => clock(settings.clock, now),
        Widget::Value {
            label,
            signal,
            graph,
            warn,
            crit,
            ..
        } => {
            let reading = match signals.get(signal) {
                Some(Value::Number(n, unit)) => Some((*n, *unit)),
                _ => None,
            };
            let (value, unit) = match (reading, signals.get(signal)) {
                (Some((n, u)), _) => format_number(n, u, settings.temperature),
                (None, Some(Value::Text(t))) => (t.clone(), String::new()),
                (None, _) => (NO_DATA.to_string(), String::new()),
            };
            let severity = reading.map_or(Severity::Ok, |(n, _)| severity(n, *warn, *crit));
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
            }
        }
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
        let zones = bind_page(&config.pages[0], &Signals::new(), &config.settings, &now());
        assert_eq!(zones.len(), 5);
        let WidgetView::Value { value, graph, .. } = &zones[2].view else {
            panic!("CPU is a value widget")
        };
        assert_eq!((value.as_str(), graph), (NO_DATA, &None));
    }

    #[test]
    fn page_signal_list() {
        let config = Config::default_layout();
        let used: Vec<&str> = signals_used(&config.pages[0]).collect();
        assert_eq!(
            used,
            ["weather.temp", "cpu.temp", "gpu.temp", "net.down", "net.up"]
        );
        assert!(shows_clock(&config.pages[0]));
        assert!(!shows_clock(&config.pages[2]));
    }
}
