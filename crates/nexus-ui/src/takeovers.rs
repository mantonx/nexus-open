//! A takeover's own settings, as fields: its name, the signals whose
//! change shows it, and how long it stays. Its widgets are edited like a
//! page's, in the preview.

use nexus_core::config::{TAKEOVER_SECONDS, Takeover};

use crate::inspector::{Field, Kind, Missing, Values};

pub const NAME: &str = "takeover.name";
pub const ON_CHANGE: &str = "takeover.on_change";
pub const SECONDS: &str = "takeover.seconds";

/// What a takeover's form sets.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub name: String,
    pub on_change: Vec<String>,
    pub seconds: f64,
}

/// The values a new takeover starts from: the first free "Takeover N".
pub fn new_values(taken: &[&str]) -> Values {
    let name = (1..)
        .map(|n| format!("Takeover {n}"))
        .find(|n| !taken.contains(&n.as_str()))
        .expect("some number is free");
    Values::from([
        (NAME.into(), name),
        (ON_CHANGE.into(), String::new()),
        (SECONDS.into(), "3".into()),
    ])
}

pub fn flatten(t: &Takeover) -> Values {
    Values::from([
        (NAME.into(), t.name.clone()),
        (ON_CHANGE.into(), t.on_change.join(", ")),
        (SECONDS.into(), format_seconds(t.seconds)),
    ])
}

fn format_seconds(s: f64) -> String {
    if s.fract() == 0.0 {
        format!("{s:.0}")
    } else {
        s.to_string()
    }
}

/// Reads the form. `taken` are the other takeovers' names.
pub fn build(v: &Values, taken: &[&str]) -> Result<Settings, Missing> {
    let get = |k: &str| v.get(k).map(String::as_str).unwrap_or_default().trim();
    let name = get(NAME);
    if name.is_empty() {
        return Err(Missing::Invalid("A takeover needs a name.".into()));
    }
    if taken.contains(&name) {
        return Err(Missing::Invalid(format!(
            "There is already a takeover called “{name}”."
        )));
    }
    let seconds = get(SECONDS)
        .parse::<f64>()
        .ok()
        .filter(|s| TAKEOVER_SECONDS.contains(s))
        .ok_or_else(|| {
            Missing::Invalid(format!(
                "Seconds must be a number from {} to {}.",
                TAKEOVER_SECONDS.start(),
                TAKEOVER_SECONDS.end()
            ))
        })?;
    let on_change = signals(get(ON_CHANGE));
    if on_change.is_empty() {
        return Err(Missing::Incomplete(
            "Choose the signals whose change shows it.".into(),
        ));
    }
    Ok(Settings {
        name: name.into(),
        on_change,
        seconds,
    })
}

/// A comma-separated list of signal names, without blanks or repeats.
fn signals(list: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for s in list.split(',').map(str::trim).filter(|s| !s.is_empty()) {
        if !out.iter().any(|o| o == s) {
            out.push(s.into());
        }
    }
    out
}

/// The list with `signal` added at the end, if it is not in it already.
pub fn add_signal(list: &str, signal: &str) -> String {
    let mut all = signals(list);
    if !all.iter().any(|s| s == signal) {
        all.push(signal.into());
    }
    all.join(", ")
}

pub fn fields(v: &Values) -> Vec<Field> {
    let get = |k: &str| v.get(k).cloned().unwrap_or_default();
    vec![
        Field {
            key: NAME.into(),
            label: "Name".into(),
            kind: Kind::Text,
            value: get(NAME),
            help: String::new(),
        },
        Field {
            key: ON_CHANGE.into(),
            label: "Shows when".into(),
            kind: Kind::Signal,
            value: get(ON_CHANGE),
            help: "Signals, separated by commas: it covers the page when one changes, unless the page already shows it. Choose… adds one."
                .into(),
        },
        Field {
            key: SECONDS.into(),
            label: "Seconds".into(),
            kind: Kind::Number,
            value: get(SECONDS),
            help: format!(
                "How long it stays after the change or the last touch: {} to {}.",
                TAKEOVER_SECONDS.start(),
                TAKEOVER_SECONDS.end()
            ),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_takeover_waits_for_its_signals() {
        let mut v = new_values(&["Takeover 1"]);
        assert_eq!(v[NAME], "Takeover 2");
        assert!(matches!(build(&v, &[]), Err(Missing::Incomplete(_))));
        v.insert(ON_CHANGE.into(), add_signal("", "volume.level"));
        assert_eq!(
            build(&v, &[]),
            Ok(Settings {
                name: "Takeover 2".into(),
                on_change: vec!["volume.level".into()],
                seconds: 3.0
            })
        );
    }

    #[test]
    fn signals_are_a_list_without_blanks_or_repeats() {
        assert_eq!(add_signal("a, b", "c"), "a, b, c");
        assert_eq!(add_signal("a, b", "a"), "a, b");
        assert_eq!(add_signal(" , a,, ", "b"), "a, b");
    }

    #[test]
    fn bad_names_and_times_are_refused() {
        let mut v = new_values(&[]);
        v.insert(ON_CHANGE.into(), "media.title".into());
        v.insert(SECONDS.into(), "90".into());
        assert!(matches!(build(&v, &[]), Err(Missing::Invalid(_))));
        v.insert(SECONDS.into(), "1.5".into());
        assert!(build(&v, &[]).is_ok());
        assert!(matches!(
            build(&v, &["Takeover 1"]),
            Err(Missing::Invalid(_))
        ));
    }

    #[test]
    fn round_trips() {
        let t = Takeover {
            name: "Volume".into(),
            on_change: vec!["volume.level".into(), "volume.muted".into()],
            seconds: 2.5,
            widgets: Vec::new(),
        };
        let s = build(&flatten(&t), &[]).unwrap();
        assert_eq!(
            (s.name, s.on_change, s.seconds),
            (t.name, t.on_change, t.seconds)
        );
    }
}
