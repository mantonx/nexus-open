//! What "Add" offers: every kind of widget, ready to use, at a sensible
//! size, in three groups. The gallery shows each one rendered (see
//! `thumbs`); choosing one inserts it and makes room (`geometry::make_room`).

use nexus_core::config::{Action, ForecastShow, MediaCommand, SliderControl, Widget};

/// A group in the gallery.
pub const GROUPS: [&str; 3] = ["Readings", "Controls", "Time and weather"];

/// One thing to add.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    pub name: &'static str,
    /// One line under its picture: what it shows or does.
    pub about: &'static str,
    /// Index into `GROUPS`.
    pub group: usize,
    /// The widget at the width it would like.
    pub widget: Widget,
}

/// A reading at `width`: its signal, thresholds and an optional second
/// signal beside it.
fn value(
    width: u32,
    (label, signal): (&str, &str),
    warn: Option<f64>,
    crit: Option<f64>,
    second: Option<&str>,
) -> Widget {
    Widget::Value {
        width,
        label: label.into(),
        signal: signal.into(),
        graph: None,
        warn,
        crit,
        second: second.map(Into::into),
        second_warn: None,
        second_crit: None,
        second_label: None,
        scale: None,
        icon: None,
        tap: None,
        hold: None,
    }
}

/// Everything the gallery offers, in order.
pub fn presets() -> Vec<Preset> {
    vec![
        Preset {
            name: "CPU temperature",
            about: "How hot the processor is, amber from 75 °C",
            group: 0,
            widget: value(
                196,
                ("CPU", "cpu.temp"),
                Some(75.0),
                Some(90.0),
                Some("cpu.load"),
            ),
        },
        Preset {
            name: "GPU temperature",
            about: "How hot the graphics card is",
            group: 0,
            widget: value(
                196,
                ("GPU", "gpu.temp"),
                Some(80.0),
                Some(90.0),
                Some("gpu.load"),
            ),
        },
        Preset {
            name: "CPU load",
            about: "How busy the processor is",
            group: 0,
            widget: value(124, ("Load", "cpu.load"), None, None, None),
        },
        Preset {
            name: "Memory",
            about: "How much memory is in use",
            group: 0,
            widget: value(124, ("Memory", "mem.used"), Some(85.0), Some(95.0), None),
        },
        Preset {
            name: "Network",
            about: "Download and upload speed",
            group: 0,
            widget: Widget::Rate {
                width: 160,
                icon: None,
                label: "Network".into(),
                down: "net.down".into(),
                up: "net.up".into(),
                tap: None,
                hold: None,
            },
        },
        Preset {
            name: "Volume slider",
            about: "Drag along it to set the volume",
            group: 1,
            widget: Widget::Slider {
                width: 208,
                icon: None,
                label: "Volume".into(),
                controls: SliderControl::Volume,
                signal: None,
                tap: None,
                hold: None,
            },
        },
        Preset {
            name: "Now playing",
            about: "The track, with a tap to play or pause",
            group: 1,
            widget: Widget::Media {
                width: 240,
                icon: None,
                tap: Some(Action::Media(MediaCommand::PlayPause)),
                hold: Some(Action::Media(MediaCommand::Next)),
            },
        },
        Preset {
            name: "Launch key",
            about: "Opens a website or app with a tap",
            group: 1,
            widget: Widget::Key {
                width: 124,
                sub: None,
                icon: Some("globe-simple".into()),
                label: Some("Browser".into()),
                tap: Some(Action::Open("https://".into())),
                hold: None,
            },
        },
        Preset {
            name: "Next page key",
            about: "Shows the next page with a tap",
            group: 1,
            widget: Widget::Key {
                width: 96,
                sub: None,
                icon: Some("arrow-right".into()),
                label: Some("Next".into()),
                tap: Some(Action::Page("next".into())),
                hold: None,
            },
        },
        Preset {
            name: "Clock",
            about: "The time, and the date when there is room",
            group: 2,
            widget: Widget::Clock {
                width: 192,
                icon: None,
                tap: None,
                hold: None,
            },
        },
        Preset {
            name: "Weather",
            about: "The temperature outside",
            group: 2,
            widget: value(120, ("Weather", "weather.temp"), None, None, None),
        },
        Preset {
            name: "Weather forecast",
            about: "The weather to come: the next change, hours or days",
            group: 2,
            widget: Widget::Forecast {
                width: 200,
                show: ForecastShow::Next,
                icon: None,
                tap: None,
                hold: None,
            },
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::config::{Config, MIN_WIDGET_WIDTH, Page};

    #[test]
    fn every_preset_makes_a_valid_layout_on_its_own() {
        for p in presets() {
            assert!(p.group < GROUPS.len(), "{}", p.name);
            assert!(p.widget.width() >= MIN_WIDGET_WIDTH, "{}", p.name);
            let mut c = Config::default_layout();
            c.pages = vec![Page {
                name: "Test".into(),
                widgets: vec![p.widget.clone()],
            }];
            c.takeovers.clear();
            c.validate().unwrap_or_else(|e| panic!("{}: {e}", p.name));
        }
    }

    #[test]
    fn every_group_has_something() {
        let all = presets();
        for (g, name) in GROUPS.iter().enumerate() {
            assert!(all.iter().any(|p| p.group == g), "{name}");
        }
    }
}
