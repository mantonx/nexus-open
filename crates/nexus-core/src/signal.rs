//! Signals: named live values that sources publish and widgets show.
//!
//! A source publishes raw readings in fixed base units (degrees Celsius,
//! bytes per second, percent). How a value looks on the strip (°F or °C,
//! "292K/s", colour) is decided by the widget and the settings, never by the
//! source. That keeps sources simple and makes every display choice a
//! setting.

use std::collections::{HashMap, VecDeque};

/// Samples of history kept per numeric signal, oldest first. One graph
/// spans this many samples.
pub const HISTORY: usize = 60;

/// The base unit a number is published in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Unit {
    /// A plain number.
    None,
    /// Degrees Celsius. Shown in °C or °F depending on the settings.
    Celsius,
    /// 0–100.
    Percent,
    /// Bytes per second.
    BytesPerSecond,
}

/// One reading.
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Number(f64, Unit),
    Text(String),
}

#[derive(Debug)]
struct Entry {
    value: Value,
    history: VecDeque<f32>,
}

/// The latest value of every signal, plus recent history for numbers.
#[derive(Debug, Default)]
pub struct Signals {
    entries: HashMap<String, Entry>,
}

impl Signals {
    pub fn new() -> Self {
        Self::default()
    }

    /// Records a reading. Numbers are also appended to the signal's history.
    pub fn set(&mut self, name: &str, value: Value) {
        let entry = self
            .entries
            .entry(name.to_string())
            .or_insert_with(|| Entry {
                value: value.clone(),
                history: VecDeque::with_capacity(HISTORY),
            });
        if let Value::Number(n, _) = value {
            if entry.history.len() == HISTORY {
                entry.history.pop_front();
            }
            entry.history.push_back(n as f32);
        }
        entry.value = value;
    }

    /// The latest reading, if the signal has ever been published.
    pub fn get(&self, name: &str) -> Option<&Value> {
        self.entries.get(name).map(|e| &e.value)
    }

    /// Recent numeric readings, oldest first. Empty for unknown or text
    /// signals.
    pub fn history(&self, name: &str) -> Vec<f32> {
        self.entries
            .get(name)
            .map(|e| e.history.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Every signal name, sorted, for `nexusctl` and the signal browser.
    pub fn names(&self) -> Vec<&str> {
        let mut names: Vec<&str> = self.entries.keys().map(String::as_str).collect();
        names.sort_unstable();
        names
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_latest_value_and_bounded_history() {
        let mut s = Signals::new();
        for i in 0..(HISTORY + 5) {
            s.set("cpu.temp", Value::Number(i as f64, Unit::Celsius));
        }
        assert_eq!(
            s.get("cpu.temp"),
            Some(&Value::Number((HISTORY + 4) as f64, Unit::Celsius))
        );
        let h = s.history("cpu.temp");
        assert_eq!(h.len(), HISTORY);
        assert_eq!(h[0], 5.0, "oldest samples are dropped first");
    }

    #[test]
    fn text_has_no_history_and_unknown_is_none() {
        let mut s = Signals::new();
        s.set("media.title", Value::Text("Song".into()));
        assert!(s.history("media.title").is_empty());
        assert_eq!(s.get("nope"), None);
        assert_eq!(s.names(), ["media.title"]);
    }
}
