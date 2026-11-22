//! Fixed sample data for previews, examples and image tests.
//!
//! Not real readings. Lets the renderer be exercised the same way everywhere
//! until live signals exist.

use jiff::Zoned;

use crate::config::Config;
use crate::layout::{Bind, bind_page};
use crate::signal::{Signals, Unit, Value};
use crate::widget::Zone;

/// The moment sample frames are drawn at: a Sunday morning.
pub fn sample_time() -> Zoned {
    "2026-11-22T10:53:00-05:00[America/New_York]"
        .parse()
        .expect("a valid time")
}

/// The signals of `readings(0)`, stored.
pub fn sample_signals() -> Signals {
    let mut s = Signals::new();
    for (name, value) in readings(0.0) {
        s.set(name, value);
    }
    s
}

/// The default layout's System page, bound to `readings(0)`.
pub fn system_page() -> Vec<Zone> {
    let config = Config::default_layout();
    let page = config
        .page_index("System")
        .map_or(&config.pages[0], |i| &config.pages[i]);
    let signals = sample_signals();
    let now = sample_time();
    let ctx = Bind {
        signals: &signals,
        settings: &config.settings,
        weather: true,
        now: &now,
        mono: 0.0,
        marquee: None,
    };
    bind_page(&page.widgets, &ctx)
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
        ("weather.temp", Value::Number(14.0, Unit::Celsius)),
        ("weather.code", Value::Number(3.0, Unit::None)),
        ("weather.condition", Value::Text("Cloudy".into())),
        ("weather.high", Value::Number(16.0, Unit::Celsius)),
        ("weather.low", Value::Number(9.0, Unit::Celsius)),
        ("weather.day", Value::Number(1.0, Unit::None)),
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
