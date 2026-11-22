//! The inspector: the selected widget as a list of fields, and back.
//!
//! A widget is flattened into named values (`label`, `signal`, `tap`,
//! `tap.page`, …) in the words the window shows. The window draws whatever
//! fields `fields` lists, so a new widget type or setting is added here
//! and nowhere else. A change sets one value and rebuilds the widget with
//! `build`. When the widget cannot be built yet (a tap set to "Run a
//! command" with no command), the values that differ are kept as a draft
//! and nothing is sent: the strip never runs a half-written action.

use std::collections::{BTreeMap, HashMap};

use nexus_core::config::{Action, GraphStyle, MediaCommand, SLIDER_VALUE, SliderControl, Widget};

/// A widget's values by key, in the window's words.
pub type Values = BTreeMap<String, String>;

/// The latest value of a signal, as the daemon reported it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Reading {
    pub text: String,
    pub number: Option<f64>,
    pub unit: Option<String>,
}

/// What fields and values depend on besides the widget.
#[derive(Clone, Debug, Default)]
pub struct Context {
    /// The layout's page names, for "Show a page".
    pub pages: Vec<String>,
    /// Temperatures shown and typed in °F.
    pub fahrenheit: bool,
    pub signals: HashMap<String, Reading>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Kind {
    Text,
    Number,
    Choice(Vec<String>),
    /// Several lines of free text.
    Note,
    /// A signal's name, chosen from the picker.
    Signal,
    /// A few short options side by side.
    Segmented(Vec<String>),
    /// The signals that set off a takeover, as chips.
    Triggers,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    pub key: String,
    pub label: String,
    pub kind: Kind,
    pub value: String,
    /// Under the field: what it does, or what it currently reads.
    pub help: String,
}

/// Why `build` could not make a widget.
#[derive(Clone, Debug, PartialEq)]
pub enum Missing {
    /// Something still has to be filled in; keep it as a draft.
    Incomplete(String),
    /// A value that can never work, such as letters in a number.
    Invalid(String),
}

/// What a change to one field comes to.
#[derive(Clone, Debug, PartialEq)]
// One per edit, short-lived: not worth boxing the widget.
#[allow(clippy::large_enum_variant)]
pub enum Outcome {
    /// Send this widget.
    Send(Widget),
    /// Keep these values as a draft; the text says what is missing.
    Draft(Values, String),
    Invalid(String),
    /// The widget would come out the same.
    Unchanged,
}

const CLOCK: &str = "Clock";
const READING: &str = "Reading";
const RATE: &str = "Network rate";
const SLIDER: &str = "Slider";
const MEDIA: &str = "Now playing";
const KEY: &str = "Key";

const NO_GRAPH: &str = "None";
const AREA: &str = "Area";
const LINE: &str = "Line";

const VOLUME: &str = "Volume";
const SEEK: &str = "Track position";
const COMMAND: &str = "A command";

const NOTHING: &str = "Nothing";
const SHOW_PAGE: &str = "Show a page";
const NEXT_PAGE: &str = "Next page";
const PREV_PAGE: &str = "Previous page";
const RUN: &str = "Run a command";
const OPEN: &str = "Open a file or URL";
const PLAY_PAUSE: &str = "Play or pause";
const NEXT_TRACK: &str = "Next track";
const PREV_TRACK: &str = "Previous track";
const STOP: &str = "Stop playback";
const PUBLISH: &str = "Set a signal";
/// Grouped by what they act on: music, pages, the computer.
const ACTIONS: [&str; 11] = [
    NOTHING, PLAY_PAUSE, NEXT_TRACK, PREV_TRACK, STOP, SHOW_PAGE, NEXT_PAGE, PREV_PAGE, OPEN, RUN,
    PUBLISH,
];

/// The widget as values.
pub fn flatten(widget: &Widget, ctx: &Context) -> Values {
    let mut v = Values::new();
    let mut put = |k: &str, s: &str| {
        v.insert(k.to_string(), s.to_string());
    };
    put("width", &widget.width().to_string());
    // Any widget may name its icon. Not every type has a field for it yet;
    // the value is carried so that saving the form keeps it.
    put("icon", widget.icon().unwrap_or_default());
    match widget {
        Widget::Clock { .. } => put("type", CLOCK),
        Widget::Media { .. } => put("type", MEDIA),
        Widget::Key { label, sub, .. } => {
            put("type", KEY);
            put("label", label.as_deref().unwrap_or_default());
            put("sub", sub.as_deref().unwrap_or_default());
        }
        Widget::Value {
            label,
            signal,
            graph,
            scale,
            second_label,
            warn,
            crit,
            second,
            second_warn,
            second_crit,
            ..
        } => {
            put("type", READING);
            // Carried, like the icon: "0 5000", or empty.
            put(
                "scale",
                &scale.map_or(String::new(), |[lo, hi]| format!("{lo} {hi}")),
            );
            put("second_label", second_label.as_deref().unwrap_or_default());
            put("label", label);
            put("signal", signal);
            put(
                "graph",
                match graph {
                    None => NO_GRAPH,
                    Some(GraphStyle::Area) => AREA,
                    Some(GraphStyle::Line) => LINE,
                },
            );
            let shown = |t: &Option<f64>| {
                t.map(|c| number(threshold_out(c, signal, ctx)))
                    .unwrap_or_default()
            };
            put("warn", &shown(warn));
            put("crit", &shown(crit));
            let second = second.as_deref().unwrap_or_default();
            let shown = |t: &Option<f64>| {
                t.map(|c| number(threshold_out(c, second, ctx)))
                    .unwrap_or_default()
            };
            put("second", second);
            put("second_warn", &shown(second_warn));
            put("second_crit", &shown(second_crit));
        }
        Widget::Rate {
            label, down, up, ..
        } => {
            put("type", RATE);
            put("label", label);
            put("down", down);
            put("up", up);
        }
        Widget::Slider {
            label,
            controls,
            signal,
            ..
        } => {
            put("type", SLIDER);
            put("label", label);
            match controls {
                SliderControl::Volume => put("controls", VOLUME),
                SliderControl::Seek => put("controls", SEEK),
                SliderControl::Run(argv) => {
                    put("controls", COMMAND);
                    put("command", &join(argv));
                }
            }
            put("signal", signal.as_deref().unwrap_or_default());
        }
    }
    flatten_action(&mut v, "tap", widget.tap());
    flatten_action(&mut v, "hold", widget.hold());
    v
}

fn flatten_action(v: &mut Values, prefix: &str, action: Option<&Action>) {
    let mut put = |k: &str, s: &str| {
        let key = if k.is_empty() {
            prefix.to_string()
        } else {
            format!("{prefix}.{k}")
        };
        v.insert(key, s.to_string());
    };
    match action {
        None => put("", NOTHING),
        Some(Action::Page(p)) if p == "next" => put("", NEXT_PAGE),
        Some(Action::Page(p)) if p == "prev" => put("", PREV_PAGE),
        Some(Action::Page(p)) => {
            put("", SHOW_PAGE);
            put("page", p);
        }
        Some(Action::Run(argv)) => {
            put("", RUN);
            put("run", &join(argv));
        }
        Some(Action::Open(target)) => {
            put("", OPEN);
            put("open", target);
        }
        Some(Action::Media(m)) => put(
            "",
            match m {
                MediaCommand::PlayPause => PLAY_PAUSE,
                MediaCommand::Next => NEXT_TRACK,
                MediaCommand::Previous => PREV_TRACK,
                MediaCommand::Stop => STOP,
            },
        ),
        Some(Action::Publish { signal, value }) => {
            put("", PUBLISH);
            put("signal", signal);
            put("value", value);
        }
    }
}

/// The widget the values describe. A value a type needs but the values
/// lack (after a change of type) takes a working default.
pub fn build(v: &Values, ctx: &Context) -> Result<Widget, Missing> {
    let get = |k: &str| v.get(k).map_or("", String::as_str).trim();
    let or = |k: &str, default: &str| {
        let s = get(k);
        if s.is_empty() {
            default.to_string()
        } else {
            s.to_string()
        }
    };
    let width: u32 = get("width")
        .parse()
        .map_err(|_| Missing::Invalid("The width must be a whole number of pixels.".into()))?;
    let tap = build_action(v, "tap", ctx)?;
    let hold = build_action(v, "hold", ctx)?;
    let text = |k: &str| Some(get(k).to_string()).filter(|s| !s.is_empty());
    let icon = text("icon");
    Ok(match get("type") {
        CLOCK => Widget::Clock {
            width,
            icon,
            tap,
            hold,
        },
        MEDIA => Widget::Media {
            width,
            icon,
            tap,
            hold,
        },
        KEY => Widget::Key {
            width,
            icon,
            label: text("label"),
            sub: text("sub"),
            tap,
            hold,
        },
        READING => {
            let signal = or("signal", "cpu.temp");
            let second = Some(get("second").to_string()).filter(|s| !s.is_empty());
            let threshold = |k: &str, of: &str, name: &str| -> Result<Option<f64>, Missing> {
                match get(k) {
                    "" => Ok(None),
                    s => s
                        .parse::<f64>()
                        .map(|n| Some(threshold_in(n, of, ctx)))
                        .map_err(|_| Missing::Invalid(format!("{name} must be a number."))),
                }
            };
            let of = second.as_deref().unwrap_or_default();
            let (second_warn, second_crit) = (
                threshold("second_warn", of, "Second warn at")?,
                threshold("second_crit", of, "Second critical at")?,
            );
            if second.is_none() && (second_warn.is_some() || second_crit.is_some()) {
                return Err(Missing::Incomplete(
                    "Choose the second signal its thresholds are for.".into(),
                ));
            }
            let scale = match get("scale")
                .split_whitespace()
                .map(str::parse::<f64>)
                .collect::<Result<Vec<_>, _>>()
            {
                Ok(v) if v.is_empty() => None,
                Ok(v) if v.len() == 2 && v[0] < v[1] => Some([v[0], v[1]]),
                _ => {
                    return Err(Missing::Invalid(
                        "The scale is two numbers, the lowest first.".into(),
                    ));
                }
            };
            Widget::Value {
                second,
                second_warn,
                second_crit,
                second_label: text("second_label"),
                scale,
                icon,
                width,
                label: or("label", "CPU"),
                graph: match get("graph") {
                    AREA => Some(GraphStyle::Area),
                    LINE => Some(GraphStyle::Line),
                    _ => None,
                },
                warn: threshold("warn", &signal, "Warn at")?,
                crit: threshold("crit", &signal, "Critical at")?,
                signal,
                tap,
                hold,
            }
        }
        RATE => Widget::Rate {
            width,
            icon,
            label: or("label", "Net"),
            down: or("down", "net.down"),
            up: or("up", "net.up"),
            tap,
            hold,
        },
        SLIDER => {
            let controls = match get("controls") {
                SEEK => SliderControl::Seek,
                COMMAND => {
                    let argv = split(get("command"))?;
                    if argv.is_empty() {
                        return Err(Missing::Incomplete(format!(
                            "Write the command the slider runs, with {SLIDER_VALUE} where its 0–100 goes."
                        )));
                    }
                    if !argv.iter().any(|a| a.contains(SLIDER_VALUE)) {
                        return Err(Missing::Incomplete(format!(
                            "Put {SLIDER_VALUE} in the command where the slider's 0–100 goes."
                        )));
                    }
                    SliderControl::Run(argv)
                }
                _ => SliderControl::Volume,
            };
            let signal = matches!(controls, SliderControl::Run(_))
                .then(|| get("signal").to_string())
                .filter(|s| !s.is_empty());
            Widget::Slider {
                width,
                label: or("label", "Volume"),
                controls,
                signal,
                icon,
                tap,
                hold,
            }
        }
        other => return Err(Missing::Invalid(format!("No widget type \"{other}\"."))),
    })
}

fn build_action(v: &Values, prefix: &str, ctx: &Context) -> Result<Option<Action>, Missing> {
    let get = |k: &str| {
        v.get(&format!("{prefix}.{k}"))
            .map_or("", String::as_str)
            .trim()
            .to_string()
    };
    let gesture = if prefix == "tap" { "tap" } else { "long press" };
    Ok(Some(match v.get(prefix).map_or(NOTHING, String::as_str) {
        NEXT_PAGE => Action::Page("next".into()),
        PREV_PAGE => Action::Page("prev".into()),
        SHOW_PAGE => {
            let page = get("page");
            if !ctx.pages.contains(&page) {
                return Err(Missing::Incomplete(format!(
                    "Pick the page a {gesture} shows."
                )));
            }
            Action::Page(page)
        }
        RUN => {
            let argv = split(&get("run"))?;
            if argv.is_empty() {
                return Err(Missing::Incomplete(format!(
                    "Write the command a {gesture} runs."
                )));
            }
            Action::Run(argv)
        }
        OPEN => {
            let target = get("open");
            if target.is_empty() {
                return Err(Missing::Incomplete(format!(
                    "Write the file or URL a {gesture} opens."
                )));
            }
            Action::Open(target)
        }
        PLAY_PAUSE => Action::Media(MediaCommand::PlayPause),
        NEXT_TRACK => Action::Media(MediaCommand::Next),
        PREV_TRACK => Action::Media(MediaCommand::Previous),
        STOP => Action::Media(MediaCommand::Stop),
        PUBLISH => {
            let signal = get("signal");
            if signal.is_empty() {
                return Err(Missing::Incomplete(format!(
                    "Name the signal a {gesture} sets."
                )));
            }
            Action::Publish {
                signal,
                value: get("value"),
            }
        }
        _ => return Ok(None),
    }))
}

/// The fields to show for these values.
pub fn fields(v: &Values, ctx: &Context) -> Vec<Field> {
    let get = |k: &str| v.get(k).cloned().unwrap_or_default();
    let mut out = Vec::new();
    let mut add = |key: &str, label: &str, kind: Kind, help: String| {
        out.push(Field {
            key: key.into(),
            label: label.into(),
            kind,
            value: get(key),
            help,
        });
    };
    let choice = |options: &[&str]| Kind::Choice(options.iter().map(|s| s.to_string()).collect());

    let segmented =
        |options: &[&str]| Kind::Segmented(options.iter().map(|s| s.to_string()).collect());
    // What a widget is comes from the gallery it was added from; to have
    // another kind, add that one. So there is no type field here.
    let ty = get("type");
    add(
        "width",
        "Width",
        Kind::Number,
        "In strip pixels; dragging the grip on the strip does the same. The widget on its right gives or takes the difference.".into(),
    );
    match ty.as_str() {
        READING => {
            add("label", "Label", Kind::Text, String::new());
            add(
                "signal",
                "Signal",
                Kind::Signal,
                reading_help(&get("signal"), ctx),
            );
            add(
                "graph",
                "Graph",
                segmented(&[NO_GRAPH, AREA, LINE]),
                String::new(),
            );
            let unit = threshold_unit(&get("signal"), ctx);
            add(
                "warn",
                "Warn at",
                Kind::Number,
                format!("Shown amber from this value{unit}. Empty: never."),
            );
            add(
                "crit",
                "Critical at",
                Kind::Number,
                format!("Shown red from this value{unit}. Empty: never."),
            );
            add(
                "second",
                "Second signal",
                Kind::Signal,
                if get("second").trim().is_empty() {
                    "Optional: a second reading shown beside the first.".into()
                } else {
                    reading_help(&get("second"), ctx)
                },
            );
            if !get("second").trim().is_empty() {
                let unit = threshold_unit(&get("second"), ctx);
                add(
                    "second_warn",
                    "Second warn at",
                    Kind::Number,
                    format!("For the second reading{unit}. Empty: never."),
                );
                add(
                    "second_crit",
                    "Second critical at",
                    Kind::Number,
                    format!("For the second reading{unit}. Empty: never."),
                );
            }
        }
        KEY => {
            add(
                "icon",
                "Icon",
                Kind::Text,
                "The icon's name, as in: play.".into(),
            );
            add("label", "Label", Kind::Text, String::new());
        }
        RATE => {
            add("label", "Label", Kind::Text, String::new());
            add(
                "down",
                "Download signal",
                Kind::Signal,
                reading_help(&get("down"), ctx),
            );
            add(
                "up",
                "Upload signal",
                Kind::Signal,
                reading_help(&get("up"), ctx),
            );
        }
        SLIDER => {
            add("label", "Label", Kind::Text, String::new());
            add(
                "controls",
                "Sets",
                segmented(&[VOLUME, SEEK, COMMAND]),
                String::new(),
            );
            if get("controls") == COMMAND {
                add(
                    "command",
                    "Command",
                    Kind::Text,
                    format!(
                        "Run as the finger moves, with {SLIDER_VALUE} as 0–100, as in: brightnessctl set {SLIDER_VALUE}%. No shell: write sh -c '…' for one."
                    ),
                );
                add(
                    "signal",
                    "Shows signal",
                    Kind::Signal,
                    "Optional: a 0–100 signal for the bar to follow.".into(),
                );
            }
        }
        _ => {}
    }
    for (prefix, label) in [("tap", "Tap"), ("hold", "Long press")] {
        let action = get(prefix);
        add(prefix, label, choice(&ACTIONS), String::new());
        let key = |k: &str| format!("{prefix}.{k}");
        match action.as_str() {
            SHOW_PAGE => {
                let pages: Vec<&str> = ctx.pages.iter().map(String::as_str).collect();
                add(&key("page"), "Page", choice(&pages), String::new());
            }
            RUN => add(
                &key("run"),
                "Command",
                Kind::Text,
                "For example: notify-send hello. No shell: write sh -c '…' for one.".into(),
            ),
            OPEN => add(
                &key("open"),
                "File or URL",
                Kind::Text,
                "Opened in its usual app, as in: https://example.com".into(),
            ),
            PUBLISH => {
                add(
                    &key("signal"),
                    "Signal",
                    Kind::Signal,
                    "Any name, as in: mode. Widgets reading it change at once.".into(),
                );
                add(&key("value"), "Value", Kind::Text, String::new());
            }
            _ => {}
        }
    }
    out
}

/// One field changed: what to do about it. `drafts` are values not yet
/// sent, laid over the widget's own.
pub fn apply(widget: &Widget, drafts: &Values, key: &str, value: &str, ctx: &Context) -> Outcome {
    let saved = flatten(widget, ctx);
    let mut v = saved.clone();
    v.extend(drafts.clone());
    // Thresholds are shown in the signal's unit (°F for a temperature
    // here). A new signal re-shows them in its own unit, so the stored
    // values carry over rather than the numbers on screen.
    let thresholds = match key {
        "signal" => Some(["warn", "crit"]),
        "second" => Some(["second_warn", "second_crit"]),
        _ => None,
    };
    if let Some(thresholds) = thresholds {
        let old = v.get(key).cloned().unwrap_or_default();
        for k in thresholds {
            if let Some(n) = v.get(k).and_then(|t| t.trim().parse::<f64>().ok()) {
                let stored = threshold_in(n, &old, ctx);
                v.insert(k.into(), number(threshold_out(stored, value, ctx)));
            }
        }
    }
    v.insert(key.into(), value.into());
    match build(&v, ctx) {
        Ok(w) if &w == widget => Outcome::Unchanged,
        Ok(w) => Outcome::Send(w),
        Err(Missing::Incomplete(why)) => {
            let draft = v
                .into_iter()
                .filter(|(k, val)| saved.get(k) != Some(val))
                .collect();
            Outcome::Draft(draft, why)
        }
        Err(Missing::Invalid(why)) => Outcome::Invalid(why),
    }
}

/// "Now 52 °C", or that nothing has published it.
fn reading_help(signal: &str, ctx: &Context) -> String {
    if signal.trim().is_empty() {
        return String::new();
    }
    match ctx.signals.get(signal.trim()) {
        None => "Nothing has published this signal yet.".into(),
        Some(r) => format!("Now {}", show_reading(r, ctx)),
    }
}

/// A reading in the units the strip shows it in.
pub fn show_reading(r: &Reading, ctx: &Context) -> String {
    let Some(n) = r.number else {
        return format!("“{}”", r.text);
    };
    match r.unit.as_deref() {
        Some("celsius") if ctx.fahrenheit => format!("{} °F", number(n * 9.0 / 5.0 + 32.0)),
        Some("celsius") => format!("{} °C", number(n)),
        Some("percent") => format!("{}%", number(n)),
        Some("bytes-per-second") => rate(n),
        _ => number(n),
    }
}

fn rate(bytes: f64) -> String {
    let units = ["B/s", "KB/s", "MB/s", "GB/s"];
    let mut n = bytes;
    let mut i = 0;
    while n >= 1000.0 && i + 1 < units.len() {
        n /= 1000.0;
        i += 1;
    }
    format!("{} {}", number(n), units[i])
}

fn is_temperature(signal: &str, ctx: &Context) -> bool {
    ctx.signals
        .get(signal.trim())
        .is_some_and(|r| r.unit.as_deref() == Some("celsius"))
}

fn threshold_unit(signal: &str, ctx: &Context) -> &'static str {
    match (is_temperature(signal, ctx), ctx.fahrenheit) {
        (true, true) => ", in °F",
        (true, false) => ", in °C",
        _ => "",
    }
}

/// A stored threshold (°C for temperatures) as shown.
fn threshold_out(stored: f64, signal: &str, ctx: &Context) -> f64 {
    if ctx.fahrenheit && is_temperature(signal, ctx) {
        stored * 9.0 / 5.0 + 32.0
    } else {
        stored
    }
}

/// A typed threshold as stored, rounded to a tenth of a degree so 80 °F
/// does not come back as 79.99999.
fn threshold_in(typed: f64, signal: &str, ctx: &Context) -> f64 {
    if ctx.fahrenheit && is_temperature(signal, ctx) {
        ((typed - 32.0) * 5.0 / 9.0 * 10.0).round() / 10.0
    } else {
        typed
    }
}

/// A number without needless decimals: 52, 52.5.
fn number(n: f64) -> String {
    let tenths = (n * 10.0).round() / 10.0;
    if tenths.fract() == 0.0 {
        format!("{tenths:.0}")
    } else {
        format!("{tenths:.1}")
    }
}

fn join(argv: &[String]) -> String {
    shlex::try_join(argv.iter().map(String::as_str)).unwrap_or_else(|_| argv.join(" "))
}

fn split(command: &str) -> Result<Vec<String>, Missing> {
    shlex::split(command).ok_or_else(|| {
        Missing::Invalid("The command has an unclosed quote or a stray backslash.".into())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::config::Config;

    fn ctx() -> Context {
        let mut signals = HashMap::new();
        signals.insert(
            "cpu.temp".into(),
            Reading {
                text: "52".into(),
                number: Some(52.0),
                unit: Some("celsius".into()),
            },
        );
        Context {
            pages: ["System", "Clock", "Home", "Music", "Launch", "Desk"]
                .map(String::from)
                .to_vec(),
            fahrenheit: true,
            signals,
        }
    }

    #[test]
    fn every_default_widget_round_trips() {
        let ctx = ctx();
        let config = Config::default_layout();
        let rows = config.pages.iter().map(|p| &p.widgets);
        let rows = rows.chain(config.takeovers.iter().map(|t| &t.widgets));
        for widgets in rows {
            for w in widgets {
                assert_eq!(build(&flatten(w, &ctx), &ctx).as_ref(), Ok(w));
            }
        }
    }

    #[test]
    fn thresholds_are_typed_in_the_shown_unit() {
        let ctx = ctx();
        let w = Widget::Value {
            width: 100,
            label: "CPU".into(),
            signal: "cpu.temp".into(),
            graph: None,
            icon: None,
            scale: None,
            second_label: None,
            warn: Some(80.0),
            crit: None,
            second: None,
            second_warn: None,
            second_crit: None,
            tap: None,
            hold: None,
        };
        assert_eq!(flatten(&w, &ctx)["warn"], "176");
        let Outcome::Send(Widget::Value { crit, .. }) =
            apply(&w, &Values::new(), "crit", "194", &ctx)
        else {
            panic!("not sent");
        };
        assert_eq!(crit, Some(90.0));
        // A signal that is not a temperature is left alone.
        let Outcome::Send(Widget::Value { crit, .. }) =
            apply(&w, &Values::new(), "signal", "gpu.load", &ctx)
        else {
            panic!("not sent");
        };
        assert_eq!(crit, None);
    }

    #[test]
    fn a_new_signal_keeps_the_stored_thresholds() {
        let ctx = ctx();
        let w = Widget::Value {
            width: 100,
            label: "CPU".into(),
            signal: "cpu.temp".into(),
            graph: None,
            icon: None,
            scale: None,
            second_label: None,
            warn: Some(75.0),
            crit: Some(90.0),
            second: None,
            second_warn: None,
            second_crit: None,
            tap: None,
            hold: None,
        };
        // Shown as 167 and 194 °F; a signal that is not a temperature
        // must not store those numbers.
        let Outcome::Send(Widget::Value { warn, crit, .. }) =
            apply(&w, &Values::new(), "signal", "gpu.load", &ctx)
        else {
            panic!("not sent");
        };
        assert_eq!((warn, crit), (Some(75.0), Some(90.0)));
    }

    #[test]
    fn an_unfinished_action_is_a_draft() {
        let ctx = ctx();
        let w = Widget::Clock {
            width: 100,
            icon: None,
            tap: None,
            hold: None,
        };
        let Outcome::Draft(draft, why) = apply(&w, &Values::new(), "tap", RUN, &ctx) else {
            panic!("sent a tap with no command");
        };
        assert!(why.contains("command"), "{why}");
        assert_eq!(draft.get("tap").map(String::as_str), Some(RUN));
        // The draft's fields show the command box.
        let mut shown = flatten(&w, &ctx);
        shown.extend(draft.clone());
        assert!(fields(&shown, &ctx).iter().any(|f| f.key == "tap.run"));
        // Filling it in sends the action, quoted words kept together.
        let Outcome::Send(sent) = apply(&w, &draft, "tap.run", "notify-send 'hi there'", &ctx)
        else {
            panic!("not sent");
        };
        assert_eq!(
            sent.tap(),
            Some(&Action::Run(vec!["notify-send".into(), "hi there".into()]))
        );
    }

    #[test]
    fn a_new_type_starts_working() {
        let ctx = ctx();
        let w = Widget::Clock {
            width: 120,
            icon: None,
            tap: Some(Action::Page("next".into())),
            hold: None,
        };
        let Outcome::Send(sent) = apply(&w, &Values::new(), "type", READING, &ctx) else {
            panic!("not sent");
        };
        assert!(matches!(&sent, Widget::Value { signal, width: 120, .. } if signal == "cpu.temp"));
        assert_eq!(sent.tap(), w.tap(), "actions survive a change of type");
    }

    #[test]
    fn bad_values_say_why() {
        let ctx = ctx();
        let w = Widget::Media {
            width: 100,
            icon: None,
            tap: None,
            hold: None,
        };
        assert!(matches!(
            apply(&w, &Values::new(), "width", "wide", &ctx),
            Outcome::Invalid(_)
        ));
        assert!(matches!(
            apply(&w, &Values::new(), "tap.run", "x", &ctx),
            Outcome::Unchanged
        ));
        let mut draft = Values::new();
        draft.insert("tap".into(), RUN.into());
        assert!(matches!(
            apply(&w, &draft, "tap.run", "echo 'oops", &ctx),
            Outcome::Invalid(_)
        ));
        assert_eq!(
            apply(&w, &Values::new(), "type", MEDIA, &ctx),
            Outcome::Unchanged
        );
    }

    #[test]
    fn keys_and_second_readings_are_edited_like_any_field() {
        let ctx = ctx();
        let key = Widget::Key {
            width: 80,
            icon: Some("play".into()),
            label: None,
            sub: None,
            tap: Some(Action::Media(MediaCommand::PlayPause)),
            hold: None,
        };
        assert_eq!(build(&flatten(&key, &ctx), &ctx).as_ref(), Ok(&key));
        let keys: Vec<String> = fields(&flatten(&key, &ctx), &ctx)
            .into_iter()
            .map(|f| f.key)
            .collect();
        assert!(keys.contains(&"icon".into()) && keys.contains(&"label".into()));
        let Outcome::Send(Widget::Key { label, .. }) =
            apply(&key, &Values::new(), "label", "Play", &ctx)
        else {
            panic!("not sent");
        };
        assert_eq!(label.as_deref(), Some("Play"));

        let w = Widget::Media {
            width: 120,
            icon: None,
            tap: None,
            hold: None,
        };
        let Outcome::Send(value) = apply(&w, &Values::new(), "type", READING, &ctx) else {
            panic!("not sent");
        };
        let Outcome::Send(with) = apply(&value, &Values::new(), "second", "cpu.temp", &ctx) else {
            panic!("not sent");
        };
        // The second reading is a temperature: typed in °F, stored in °C.
        let Outcome::Send(Widget::Value {
            second,
            second_crit,
            ..
        }) = apply(&with, &Values::new(), "second_crit", "194", &ctx)
        else {
            panic!("not sent");
        };
        assert_eq!(
            (second.as_deref(), second_crit),
            (Some("cpu.temp"), Some(90.0))
        );
    }

    #[test]
    fn readings_show_in_the_strips_units() {
        let ctx = ctx();
        assert_eq!(show_reading(&ctx.signals["cpu.temp"], &ctx), "125.6 °F");
        let r = Reading {
            text: String::new(),
            number: Some(2_500_000.0),
            unit: Some("bytes-per-second".into()),
        };
        assert_eq!(show_reading(&r, &ctx), "2.5 MB/s");
    }
}
