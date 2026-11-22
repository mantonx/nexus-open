//! A takeover's own settings, as fields: its name, the signals whose
//! change shows it, and how long it stays. Its widgets are edited like a
//! page's, in the preview.

use nexus_core::config::{Align, MIN_WIDGET_WIDTH, TAKEOVER_SECONDS, Takeover};

use crate::inspector::{Field, Kind, Missing, Values};

pub const NAME: &str = "takeover.name";
pub const ON_CHANGE: &str = "takeover.on_change";
pub const SECONDS: &str = "takeover.seconds";
pub const ALIGN: &str = "takeover.align";
pub const WIDTH: &str = "takeover.width";

/// What a takeover's form sets.
#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    pub name: String,
    pub on_change: Vec<String>,
    pub seconds: f64,
    pub align: Align,
    pub width: Option<u32>,
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
        (ALIGN.into(), Align::Full.id().into()),
        (WIDTH.into(), String::new()),
    ])
}

pub fn flatten(t: &Takeover) -> Values {
    Values::from([
        (NAME.into(), t.name.clone()),
        (ON_CHANGE.into(), t.on_change.join(", ")),
        (SECONDS.into(), format_seconds(t.seconds)),
        (ALIGN.into(), t.align.id().into()),
        (WIDTH.into(), t.width.map(width_text).unwrap_or_default()),
    ])
}

/// Where a takeover can sit, in the form's words, by `Align::id`.
const COVERS: [(&str, &str); 3] = [
    ("full", "Whole strip"),
    ("left", "Left end"),
    ("right", "Right end"),
];

/// How much of the strip a partial takeover covers: in words, so no
/// pixel number is needed, and every one lands on a card edge.
const WIDTHS: [(&str, u32); 5] = [
    ("A quarter", 160),
    ("A third", 213),
    ("Half", 320),
    ("Two thirds", 427),
    ("Three quarters", 480),
];

/// A width in words when it is one of the offered ones, else in pixels.
fn width_text(w: u32) -> String {
    WIDTHS
        .iter()
        .find(|(_, px)| *px == w)
        .map_or_else(|| format!("{w} px"), |(name, _)| (*name).to_string())
}

/// A width from its words, "320 px" or "320".
fn width_value(text: &str) -> Option<u32> {
    if let Some((_, px)) = WIDTHS.iter().find(|(name, _)| *name == text) {
        return Some(*px);
    }
    text.trim_end_matches("px").trim().parse().ok()
}

/// The takeover's signals, in order.
pub fn triggers(v: &Values) -> Vec<String> {
    signals(v.get(ON_CHANGE).map(String::as_str).unwrap_or_default())
}

/// The list without `signal`.
pub fn remove_signal(list: &str, signal: &str) -> String {
    signals(list)
        .into_iter()
        .filter(|s| s != signal)
        .collect::<Vec<_>>()
        .join(", ")
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
            "Choose a reading: when it changes, the takeover shows.".into(),
        ));
    }
    let align = match get(ALIGN) {
        "" | "full" | "Whole strip" => Align::Full,
        "left" | "Left end" => Align::Left,
        "right" | "Right end" => Align::Right,
        _ => {
            return Err(Missing::Invalid(
                "It covers the whole strip, or its left or right end.".into(),
            ));
        }
    };
    let width = match (align, get(WIDTH)) {
        (Align::Full, _) => None,
        (_, "") => {
            return Err(Missing::Incomplete(
                "Choose how much of the strip it covers.".into(),
            ));
        }
        (_, w) => Some(
            width_value(w)
                .filter(|w| (MIN_WIDGET_WIDTH..=nexus_core::WIDTH).contains(w))
                .ok_or_else(|| {
                    Missing::Invalid(format!(
                        "The width must be {MIN_WIDGET_WIDTH} to {} pixels.",
                        nexus_core::WIDTH
                    ))
                })?,
        ),
    };
    Ok(Settings {
        name: name.into(),
        on_change,
        seconds,
        align,
        width,
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
    let align = get(ALIGN);
    let covers = COVERS
        .iter()
        .find(|(id, name)| *id == align || *name == align)
        .map_or("Whole strip", |(_, name)| name);
    let mut out = vec![
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
            kind: Kind::Triggers,
            value: get(ON_CHANGE),
            help: "It covers the page when one of these readings changes, unless the page already shows it.".into(),
        },
        Field {
            key: SECONDS.into(),
            label: "Stays for".into(),
            kind: Kind::Number,
            value: get(SECONDS),
            help: format!(
                "Seconds after the change or the last touch on it: {} to {}.",
                TAKEOVER_SECONDS.start(),
                TAKEOVER_SECONDS.end()
            ),
        },
        Field {
            key: ALIGN.into(),
            label: "Covers".into(),
            kind: Kind::Segmented(COVERS.iter().map(|(_, n)| n.to_string()).collect()),
            value: covers.into(),
            help: "One end leaves the rest of the page in view beside it.".into(),
        },
    ];
    if covers != "Whole strip" {
        let current = get(WIDTH);
        let mut options: Vec<String> = WIDTHS.iter().map(|(n, _)| n.to_string()).collect();
        if !current.is_empty() && !options.contains(&current) {
            options.push(current.clone());
        }
        out.push(Field {
            key: WIDTH.into(),
            label: "How much".into(),
            kind: Kind::Choice(options),
            value: current,
            help: "Its widgets share this much of the strip.".into(),
        });
    }
    out
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
                seconds: 3.0,
                align: Align::Full,
                width: None,
            })
        );
    }

    #[test]
    fn a_partial_takeover_needs_a_width() {
        let mut v = new_values(&[]);
        v.insert(ON_CHANGE.into(), "mode".into());
        v.insert(ALIGN.into(), "right".into());
        assert!(matches!(build(&v, &[]), Err(Missing::Incomplete(_))));
        v.insert(WIDTH.into(), "20".into());
        assert!(matches!(build(&v, &[]), Err(Missing::Invalid(_))));
        v.insert(WIDTH.into(), "200".into());
        let s = build(&v, &[]).unwrap();
        assert_eq!((s.align, s.width), (Align::Right, Some(200)));
        v.insert(ALIGN.into(), "middle".into());
        assert!(matches!(build(&v, &[]), Err(Missing::Invalid(_))));
    }

    #[test]
    fn widths_and_ends_are_chosen_in_words() {
        let mut v = new_values(&[]);
        v.insert(ON_CHANGE.into(), "volume.level".into());
        v.insert(ALIGN.into(), "Right end".into());
        v.insert(WIDTH.into(), "Half".into());
        let s = build(&v, &[]).unwrap();
        assert_eq!((s.align, s.width), (Align::Right, Some(320)));
        assert_eq!(width_text(213), "A third");
        assert_eq!(width_text(250), "250 px");
        assert_eq!(width_value("250 px"), Some(250));
        // A width not offered is still shown, as itself.
        v.insert(WIDTH.into(), "250 px".into());
        let f = fields(&v);
        let width = f.iter().find(|f| f.key == WIDTH).unwrap();
        assert!(matches!(&width.kind, Kind::Choice(o) if o.contains(&"250 px".to_string())));
        // A whole-strip takeover has no width to choose.
        v.insert(ALIGN.into(), "full".into());
        assert!(!fields(&v).iter().any(|f| f.key == WIDTH));
    }

    #[test]
    fn triggers_are_added_and_removed_one_at_a_time() {
        assert_eq!(remove_signal("a, b, c", "b"), "a, c");
        assert_eq!(remove_signal("a", "a"), "");
        let mut v = new_values(&[]);
        v.insert(ON_CHANGE.into(), "media.title, volume.level".into());
        assert_eq!(triggers(&v), ["media.title", "volume.level"]);
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
            align: Align::Left,
            width: Some(320),
            widgets: Vec::new(),
        };
        let s = build(&flatten(&t), &[]).unwrap();
        assert_eq!(
            (s.name, s.on_change, s.seconds, s.align, s.width),
            (t.name, t.on_change, t.seconds, t.align, t.width)
        );
    }
}
