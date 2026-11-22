//! Fixed sample data for previews, examples and image tests.
//!
//! Not real readings. Lets the renderer be exercised the same way everywhere
//! until live signals exist.

use crate::signal::{Unit, Value};
use crate::widget::{Graph, Severity, WidgetView, Zone};

/// The System page from the current app, with representative data.
pub fn system_page() -> Vec<Zone> {
    let flat: Vec<f32> = (0..60)
        .map(|i| 0.12 + 0.03 * ((i as f32) * 0.7).sin())
        .collect();
    let spiky: Vec<f32> = (0..60)
        .map(|i| match i {
            6 => 0.85,
            7 => 0.4,
            8 => 0.2,
            _ => 0.08 + 0.02 * ((i as f32) * 1.3).sin(),
        })
        .collect();
    vec![
        Zone {
            width: 128,
            view: WidgetView::Clock {
                date: "Tue, Jun 09".into(),
                time: "4:17".into(),
                meridiem: Some("PM".into()),
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Value {
                label: "Jersey City".into(),
                value: "79".into(),
                unit: "°F".into(),
                icon: Some('\u{f185}'),
                graph: None,
                severity: Severity::Ok,
                second: None,
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Value {
                label: "CPU".into(),
                value: "28".into(),
                unit: "°C".into(),
                icon: None,
                graph: Some(Graph::Area(flat.clone())),
                severity: Severity::Ok,
                second: None,
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Value {
                label: "GPU".into(),
                value: "82".into(),
                unit: "°C".into(),
                icon: None,
                graph: Some(Graph::Area(flat)),
                severity: Severity::Warn,
                second: None,
            },
        },
        Zone {
            width: 128,
            view: WidgetView::Rate {
                label: "Net".into(),
                caption: "↓292K/s ↑309K/s".into(),
                graph: Some(Graph::Line(spiky)),
            },
        },
    ]
}

/// Plausible fake readings for every signal the default layout uses, as a
/// function of seconds since start. Drives the daemon with `--demo` until
/// real sources exist, and gives tests moving data.
pub fn readings(t: f64) -> Vec<(&'static str, Value)> {
    let wave = |period: f64, phase: f64| (t / period * std::f64::consts::TAU + phase).sin();
    // A burst every 20 s, like a download starting.
    let burst = if t % 20.0 < 3.0 { 40e6 } else { 0.0 };
    vec![
        (
            "cpu.temp",
            Value::Number(52.0 + 6.0 * wave(30.0, 0.0), Unit::Celsius),
        ),
        (
            "gpu.temp",
            Value::Number(74.0 + 8.0 * wave(45.0, 1.0), Unit::Celsius),
        ),
        ("weather.temp", Value::Number(21.0, Unit::Celsius)),
        (
            "cpu.load",
            Value::Number(30.0 + 25.0 * wave(17.0, 0.5), Unit::Percent),
        ),
        (
            "gpu.load",
            Value::Number(60.0 + 35.0 * wave(23.0, 2.0), Unit::Percent),
        ),
        (
            "mem.used",
            Value::Number(41.0 + 2.0 * wave(60.0, 0.0), Unit::Percent),
        ),
        (
            "net.down",
            Value::Number(250e3 + 150e3 * wave(7.0, 0.0) + burst, Unit::BytesPerSecond),
        ),
        (
            "net.up",
            Value::Number(40e3 + 30e3 * wave(11.0, 1.0), Unit::BytesPerSecond),
        ),
    ]
}
