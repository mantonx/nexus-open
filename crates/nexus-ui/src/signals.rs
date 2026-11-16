//! The signal browser: every signal the daemon knows and its latest value,
//! as the window lists it.

use std::collections::HashMap;

use crate::inspector::{Context, Reading, show_reading};

/// One signal as listed.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub name: String,
    /// In the display units (°F here), as the inspector shows it.
    pub value: String,
}

/// The signals whose name contains `filter` (any case), by name.
pub fn rows(signals: &HashMap<String, Reading>, filter: &str, ctx: &Context) -> Vec<Row> {
    let filter = filter.trim().to_lowercase();
    let mut out: Vec<Row> = signals
        .iter()
        .filter(|(name, _)| name.to_lowercase().contains(&filter))
        .map(|(name, r)| Row {
            name: name.clone(),
            value: show_reading(r, ctx),
        })
        .collect();
    out.sort_by(|a, b| a.name.cmp(&b.name));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reading(n: f64, unit: &str) -> Reading {
        Reading {
            text: n.to_string(),
            number: Some(n),
            unit: Some(unit.into()),
        }
    }

    fn signals() -> HashMap<String, Reading> {
        HashMap::from([
            ("gpu.temp".into(), reading(50.0, "celsius")),
            ("cpu.temp".into(), reading(40.0, "celsius")),
            ("cpu.load".into(), reading(12.5, "percent")),
            (
                "media.title".into(),
                Reading {
                    text: "Song".into(),
                    ..Default::default()
                },
            ),
        ])
    }

    #[test]
    fn listed_by_name_in_display_units() {
        let ctx = Context {
            fahrenheit: true,
            ..Default::default()
        };
        let rows = rows(&signals(), "", &ctx);
        let names: Vec<&str> = rows.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, ["cpu.load", "cpu.temp", "gpu.temp", "media.title"]);
        assert_eq!(rows[1].value, "104 °F");
        assert_eq!(rows[3].value, "“Song”");
    }

    #[test]
    fn the_filter_matches_any_part_of_the_name_in_any_case() {
        let ctx = Context::default();
        let names = |f: &str| -> Vec<String> {
            rows(&signals(), f, &ctx)
                .into_iter()
                .map(|r| r.name)
                .collect()
        };
        assert_eq!(names("TEMP"), ["cpu.temp", "gpu.temp"]);
        assert_eq!(names(" cpu "), ["cpu.load", "cpu.temp"]);
        assert!(names("disk").is_empty());
    }
}
