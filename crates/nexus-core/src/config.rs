//! The config file: settings plus the pages shown on the strip.
//!
//! One human-editable TOML file. Loading is two steps: parse (syntax and
//! types, with the line and column of any mistake) and validate (rules the
//! types cannot express, such as widths that must fit the strip). A config
//! that fails either step is rejected whole; the caller keeps the last good
//! one, so a typo never blanks the strip.

use std::collections::HashSet;
use std::fmt;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::WIDTH;
use crate::fonts::FontChoice;
use crate::theme::Rgb;

/// The layout used when there is no config file yet.
pub const DEFAULT: &str = include_str!("../assets/default.toml");

/// Layouts to start from, by name: their pages and takeovers. Any
/// settings in them are ignored when one is added.
pub const STARTERS: [(&str, &str); 4] = [
    ("Everything", DEFAULT),
    ("Monitor", include_str!("../assets/starters/monitor.toml")),
    ("Music", include_str!("../assets/starters/music.toml")),
    ("Minimal", include_str!("../assets/starters/minimal.toml")),
];

/// Narrowest widget that still fits a label and a value.
pub const MIN_WIDGET_WIDTH: u32 = 40;

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    #[serde(default)]
    pub settings: Settings,
    // Defaulted so an empty file reaches `validate`, whose message says
    // what to add, instead of serde's "missing field `page`".
    #[serde(rename = "page", default)]
    pub pages: Vec<Page>,
    /// Where the weather is for. Without it there is no weather reading.
    pub weather: Option<Weather>,
    /// Rows that briefly cover the strip when a signal changes.
    #[serde(rename = "takeover", default)]
    pub takeovers: Vec<Takeover>,
    /// Programs that publish signals.
    #[serde(rename = "source", default)]
    pub sources: Vec<Source>,
}

/// A program the daemon runs that publishes signals: one JSON object per
/// line on its standard output. See docs/EXTERNAL_SOURCES.md.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// Letters, digits, `-` and `_`: it names the program in logs and, on
    /// Linux, its systemd unit.
    pub name: String,
    /// The program and its arguments. No shell, as for a `run` action.
    pub run: Vec<String>,
    /// Off keeps the source in the config without running it.
    #[serde(default = "yes", skip_serializing_if = "is_yes")]
    pub enabled: bool,
}

fn yes() -> bool {
    true
}

fn is_yes(b: &bool) -> bool {
    *b
}

/// A row of widgets that covers the strip, or one end of it, for a few
/// seconds when one of its signals changes: the new track, the new volume.
/// A touch on it keeps it up; a swipe or the timer puts the page back.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Takeover {
    pub name: String,
    /// Signals whose change shows it. Not when a signal first appears, and
    /// not while the page on the strip already shows that signal.
    pub on_change: Vec<String>,
    /// How long it stays after the change or the last touch on it.
    pub seconds: f64,
    /// Which part of the strip it covers. Beside a partial one the page
    /// stays on the strip and works as usual.
    #[serde(default, skip_serializing_if = "Align::is_full")]
    pub align: Align,
    /// Its width in pixels, for `left` and `right`; a full one has none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(rename = "widget", default)]
    pub widgets: Vec<Widget>,
}

/// Where a takeover sits on the strip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Align {
    #[default]
    Full,
    Left,
    Right,
}

impl Align {
    pub fn is_full(&self) -> bool {
        *self == Align::Full
    }

    /// How a layout writes it: `full`, `left` or `right`.
    pub fn id(self) -> &'static str {
        match self {
            Align::Full => "full",
            Align::Left => "left",
            Align::Right => "right",
        }
    }
}

impl Takeover {
    /// The pixels it covers: its first column and its width. A partial one
    /// without a width that fits (which `validate` refuses) counts as full.
    pub fn span(&self) -> (u32, u32) {
        match (self.align, self.width) {
            (Align::Left, Some(w)) if w <= WIDTH => (0, w),
            (Align::Right, Some(w)) if w <= WIDTH => (WIDTH - w, w),
            _ => (0, WIDTH),
        }
    }
}

/// Bounds for a takeover's `seconds`.
pub const TAKEOVER_SECONDS: std::ops::RangeInclusive<f64> = 0.5..=60.0;

/// The `[weather]` section: a place name to look up, or coordinates.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Weather {
    /// "City" or "City, region, country". Looked up once per change.
    pub location: Option<String>,
    /// Coordinates in degrees. When given, the location is not looked up.
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Settings {
    #[serde(deserialize_with = "font_from_id", serialize_with = "font_to_id")]
    pub font: FontChoice,
    pub temperature: Temperature,
    pub clock: ClockFormat,
    /// The user's accent colour, written `"#rrggbb"`. `None`: the default.
    #[serde(
        deserialize_with = "accent_from_text",
        serialize_with = "accent_to_text",
        skip_serializing_if = "Option::is_none"
    )]
    pub accent: Option<Rgb>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Temperature {
    #[default]
    Celsius,
    Fahrenheit,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize)]
pub enum ClockFormat {
    #[default]
    #[serde(rename = "24h")]
    H24,
    #[serde(rename = "12h")]
    H12,
}

#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Page {
    pub name: String,
    #[serde(rename = "widget", default)]
    pub widgets: Vec<Widget>,
}

/// One zone on a page. `type` picks the variant.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "lowercase", deny_unknown_fields)]
pub enum Widget {
    /// Date, time and AM/PM from the system clock.
    Clock {
        width: u32,
        #[serde(default)]
        tap: Option<Action>,
        #[serde(default)]
        hold: Option<Action>,
    },
    /// One reading, such as a temperature, with optional graph and
    /// thresholds.
    Value {
        width: u32,
        label: String,
        signal: String,
        #[serde(default)]
        graph: Option<GraphStyle>,
        /// Thresholds in the signal's base unit (°C for temperatures), so
        /// they mean the same thing whichever display unit is chosen.
        #[serde(default)]
        warn: Option<f64>,
        #[serde(default)]
        crit: Option<f64>,
        /// A second signal, shown beside the first, with its own
        /// thresholds in its own base unit.
        #[serde(default)]
        second: Option<String>,
        #[serde(default)]
        second_warn: Option<f64>,
        #[serde(default)]
        second_crit: Option<f64>,
        #[serde(default)]
        tap: Option<Action>,
        #[serde(default)]
        hold: Option<Action>,
    },
    /// Download and upload rates with a graph of the download rate.
    Rate {
        width: u32,
        label: String,
        down: String,
        up: String,
        #[serde(default)]
        tap: Option<Action>,
        #[serde(default)]
        hold: Option<Action>,
    },
    /// A bar to drag along: volume, position in the track, or anything a
    /// command can set. A touch that lands on it belongs to it, so pages
    /// cannot be swiped from a slider.
    Slider {
        width: u32,
        label: String,
        controls: SliderControl,
        /// For `controls = { run = … }`: a 0–100 signal for the bar to
        /// show. Without one, it shows the last value set.
        #[serde(default)]
        signal: Option<String>,
        #[serde(default)]
        tap: Option<Action>,
        #[serde(default)]
        hold: Option<Action>,
    },
    /// What is playing: title, artist and progress, from the `media.*`
    /// signals.
    Media {
        width: u32,
        #[serde(default)]
        tap: Option<Action>,
        #[serde(default)]
        hold: Option<Action>,
    },
    /// A control to press: an icon and a label. It shows nothing live.
    Key {
        width: u32,
        /// The icon's name. Free-form for now: no icon set is chosen yet.
        #[serde(default)]
        icon: Option<String>,
        #[serde(default)]
        label: Option<String>,
        #[serde(default)]
        tap: Option<Action>,
        #[serde(default)]
        hold: Option<Action>,
    },
}

impl Widget {
    pub fn width(&self) -> u32 {
        match self {
            Widget::Clock { width, .. }
            | Widget::Slider { width, .. }
            | Widget::Media { width, .. }
            | Widget::Value { width, .. }
            | Widget::Rate { width, .. }
            | Widget::Key { width, .. } => *width,
        }
    }

    /// The same widget at another width.
    pub fn with_width(mut self, new: u32) -> Widget {
        match &mut self {
            Widget::Clock { width, .. }
            | Widget::Slider { width, .. }
            | Widget::Media { width, .. }
            | Widget::Value { width, .. }
            | Widget::Rate { width, .. }
            | Widget::Key { width, .. } => *width = new,
        }
        self
    }

    /// What a tap does, if anything.
    pub fn tap(&self) -> Option<&Action> {
        match self {
            Widget::Clock { tap, .. }
            | Widget::Slider { tap, .. }
            | Widget::Media { tap, .. }
            | Widget::Value { tap, .. }
            | Widget::Rate { tap, .. }
            | Widget::Key { tap, .. } => tap.as_ref(),
        }
    }

    /// What a long press does, if anything.
    pub fn hold(&self) -> Option<&Action> {
        match self {
            Widget::Clock { hold, .. }
            | Widget::Slider { hold, .. }
            | Widget::Media { hold, .. }
            | Widget::Value { hold, .. }
            | Widget::Rate { hold, .. }
            | Widget::Key { hold, .. } => hold.as_ref(),
        }
    }
}

/// What a tap or a long press on a widget does. Written as a one-key table:
/// `tap = { page = "Clock" }`, `hold = { media = "next" }`.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum Action {
    /// Show a page: "next", "prev", or a page's name.
    Page(String),
    /// Start a program: the program, then its arguments. There is no shell,
    /// so nothing in it is expanded or interpreted; write
    /// `["sh", "-c", "…"]` to get one.
    Run(Vec<String>),
    /// Open a URL or a file in the desktop's default app.
    Open(String),
    /// Control the player the media widget shows.
    Media(MediaCommand),
    /// Set a signal to a value: a number if it reads as one, else text.
    Publish { signal: String, value: String },
}

/// What a slider sets.
#[derive(Clone, Debug, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase", deny_unknown_fields)]
pub enum SliderControl {
    /// The default audio output's volume, 0–100%, set as the finger moves.
    Volume,
    /// The position in the track the media widget shows, set on release.
    Seek,
    /// A program and its arguments, run as the finger moves with `{value}`
    /// replaced by 0–100. Runs are never queued: a slow command is next run
    /// with the latest value only.
    Run(Vec<String>),
}

/// Stands for the slider's value, 0–100, in a `run` slider's arguments.
pub const SLIDER_VALUE: &str = "{value}";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MediaCommand {
    PlayPause,
    Next,
    Previous,
    Stop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GraphStyle {
    Area,
    Line,
}

fn font_to_id<S: Serializer>(font: &FontChoice, s: S) -> Result<S::Ok, S::Error> {
    s.serialize_str(font.id())
}

fn font_from_id<'de, D: Deserializer<'de>>(d: D) -> Result<FontChoice, D::Error> {
    let id = String::deserialize(d)?;
    id.parse().map_err(serde::de::Error::custom)
}

/// Reads an accent colour written `"#rrggbb"`, in either case. The error
/// names the key and shows the form.
pub fn parse_colour(text: &str) -> Result<Rgb, String> {
    let hex = text
        .strip_prefix('#')
        .filter(|h| h.len() == 6 && h.chars().all(|c| c.is_ascii_hexdigit()));
    match hex.and_then(|h| u32::from_str_radix(h, 16).ok()) {
        Some(v) => Ok(Rgb::hex(v)),
        None => Err(format!(
            "accent must be a colour written \"#rrggbb\", as in accent = \"#5aa0e0\"; \"{text}\" is not one"
        )),
    }
}

/// A colour as a layout writes it: `#5aa0e0`.
pub fn colour_text(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

fn accent_to_text<S: Serializer>(accent: &Option<Rgb>, s: S) -> Result<S::Ok, S::Error> {
    match accent {
        Some(c) => s.serialize_str(&colour_text(*c)),
        None => s.serialize_none(),
    }
}

fn accent_from_text<'de, D: Deserializer<'de>>(d: D) -> Result<Option<Rgb>, D::Error> {
    match Option::<String>::deserialize(d)? {
        Some(text) => parse_colour(&text)
            .map(Some)
            .map_err(serde::de::Error::custom),
        None => Ok(None),
    }
}

/// Why a config was rejected. The message names the place and the fix.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfigError(pub String);

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

impl Config {
    /// Parses and validates a config file's text.
    pub fn parse(text: &str) -> Result<Config, ConfigError> {
        let config: Config = toml::from_str(text).map_err(|e| ConfigError(e.to_string()))?;
        config.validate()?;
        Ok(config)
    }

    /// The built-in layout.
    pub fn default_layout() -> Config {
        Config::parse(DEFAULT).expect("bundled default.toml must be valid")
    }

    /// Index of the page with this name.
    pub fn page_index(&self, name: &str) -> Option<usize> {
        self.pages.iter().position(|p| p.name == name)
    }

    /// Checks the rules the types cannot express. `parse` runs it; a
    /// config built another way (from the store) runs it itself.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if self.pages.is_empty() {
            return Err(ConfigError(
                "no pages; add at least one [[page]] with a name".into(),
            ));
        }
        let mut names = HashSet::new();
        for page in &self.pages {
            let at = format!("page \"{}\"", page.name);
            if page.name.trim().is_empty() {
                return Err(ConfigError("a page has an empty name".into()));
            }
            if !names.insert(page.name.as_str()) {
                return Err(ConfigError(format!(
                    "{at} appears twice; page names must be unique"
                )));
            }
            self.check_widgets(&at, &page.widgets, WIDTH)?;
            let sliders = page
                .widgets
                .iter()
                .filter(|w| matches!(w, Widget::Slider { .. }));
            let only_sliders = !page.widgets.is_empty() && sliders.count() == page.widgets.len();
            if self.pages.len() > 1 && only_sliders {
                return Err(ConfigError(format!(
                    "{at} has only sliders, so it could not be swiped away; add another widget"
                )));
            }
        }
        let mut names = HashSet::new();
        for t in &self.takeovers {
            let at = format!("takeover \"{}\"", t.name);
            let err = |m: &str| Err(ConfigError(format!("{at}: {m}")));
            if t.name.trim().is_empty() {
                return Err(ConfigError("a takeover has an empty name".into()));
            }
            if !names.insert(t.name.as_str()) {
                return err("appears twice; takeover names must be unique");
            }
            if t.on_change.is_empty() || t.on_change.iter().any(|s| s.trim().is_empty()) {
                return err("on_change needs signal names, as in on_change = [\"media.title\"]");
            }
            if !TAKEOVER_SECONDS.contains(&t.seconds) {
                return err(&format!(
                    "seconds is {}; it must be {} to {}",
                    t.seconds,
                    TAKEOVER_SECONDS.start(),
                    TAKEOVER_SECONDS.end()
                ));
            }
            if t.widgets.is_empty() {
                return err("it has no widgets; add a [[takeover.widget]]");
            }
            let room = match (t.align, t.width) {
                (Align::Full, None) => WIDTH,
                (Align::Full, Some(_)) => {
                    return err(
                        "width is only for a takeover with align = \"left\" or \"right\"; a full one is as wide as the strip",
                    );
                }
                (align, None) => {
                    return err(&format!(
                        "align = \"{}\" needs a width in pixels, as in width = 320",
                        align.id()
                    ));
                }
                (_, Some(w)) if !(MIN_WIDGET_WIDTH..=WIDTH).contains(&w) => {
                    return err(&format!(
                        "width is {w}; it must be {MIN_WIDGET_WIDTH} to {WIDTH} px"
                    ));
                }
                (_, Some(w)) => w,
            };
            self.check_widgets(&at, &t.widgets, room)?;
        }
        let mut names = HashSet::new();
        for source in &self.sources {
            let at = format!("source \"{}\"", source.name);
            let fits = |c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_';
            if source.name.is_empty() || !source.name.chars().all(fits) {
                return Err(ConfigError(format!(
                    "{at}: a source name is letters, digits, - and _ only"
                )));
            }
            if !names.insert(source.name.as_str()) {
                return Err(ConfigError(format!(
                    "{at} appears twice; source names must be unique"
                )));
            }
            if source.run.first().is_none_or(|p| p.trim().is_empty()) {
                return Err(ConfigError(format!(
                    "{at}: run needs a program, as in run = [\"python3\", \"stars.py\"]"
                )));
            }
        }
        if let Some(w) = &self.weather {
            w.validate()?;
        }
        Ok(())
    }
}

impl Config {
    /// The rules every row of widgets follows, page or takeover. `at` names
    /// the row in messages; `room` is how wide the row is.
    fn check_widgets(&self, at: &str, widgets: &[Widget], room: u32) -> Result<(), ConfigError> {
        let at_widget = |i: usize| format!("{at}, widget {}", i + 1);
        for (i, w) in widgets.iter().enumerate() {
            if w.width() < MIN_WIDGET_WIDTH {
                return Err(ConfigError(format!(
                    "{}: width {} is too narrow; the minimum is {MIN_WIDGET_WIDTH}",
                    at_widget(i),
                    w.width()
                )));
            }
        }
        let total: u32 = widgets.iter().map(Widget::width).sum();
        if total > room {
            let what = if room == WIDTH { "the strip" } else { "it" };
            return Err(ConfigError(format!(
                "{at}: widget widths add up to {total} px, but {what} is {room} px wide"
            )));
        }
        for (i, w) in widgets.iter().enumerate() {
            if let Widget::Slider {
                controls,
                tap,
                hold,
                ..
            } = w
            {
                if tap.is_some() || hold.is_some() {
                    return Err(ConfigError(format!(
                        "{}: a slider uses touches itself, so it cannot have tap or hold",
                        at_widget(i)
                    )));
                }
                if let SliderControl::Run(argv) = controls
                    && (argv.is_empty() || !argv.iter().any(|a| a.contains(SLIDER_VALUE)))
                {
                    return Err(ConfigError(format!(
                        "{}: a run slider needs a program and {SLIDER_VALUE} in its arguments",
                        at_widget(i)
                    )));
                }
            }
            if let Widget::Value {
                second,
                second_warn,
                second_crit,
                ..
            } = w
            {
                match second {
                    Some(s) if s.trim().is_empty() => {
                        return Err(ConfigError(format!(
                            "{}: second is empty; name a signal, or leave second out",
                            at_widget(i)
                        )));
                    }
                    None if second_warn.is_some() || second_crit.is_some() => {
                        return Err(ConfigError(format!(
                            "{}: second_warn and second_crit need a second signal",
                            at_widget(i)
                        )));
                    }
                    _ => {}
                }
            }
            for (gesture, action) in [("tap", w.tap()), ("hold", w.hold())] {
                if let Some(action) = action {
                    self.check_action(action)
                        .map_err(|m| ConfigError(format!("{}, {gesture}: {m}", at_widget(i))))?;
                }
            }
        }
        Ok(())
    }

    fn check_action(&self, action: &Action) -> Result<(), String> {
        match action {
            Action::Page(target) => {
                let known =
                    matches!(target.as_str(), "next" | "prev") || self.page_index(target).is_some();
                if !known {
                    return Err(format!(
                        "no page called \"{target}\"; use a page name, \"next\" or \"prev\""
                    ));
                }
            }
            Action::Run(argv) if argv.first().is_none_or(|p| p.trim().is_empty()) => {
                return Err("run needs a program, as in run = [\"pavucontrol\"]".into());
            }
            Action::Open(target) if target.trim().is_empty() => {
                return Err("open needs a URL or a file".into());
            }
            Action::Publish { signal, .. } if signal.trim().is_empty() => {
                return Err("publish needs a signal name".into());
            }
            _ => {}
        }
        Ok(())
    }
}

impl Weather {
    fn validate(&self) -> Result<(), ConfigError> {
        let err = |m: &str| Err(ConfigError(format!("[weather]: {m}")));
        match (self.latitude, self.longitude) {
            (Some(lat), Some(lon)) => {
                if !(-90.0..=90.0).contains(&lat) || !(-180.0..=180.0).contains(&lon) {
                    return err("latitude must be -90 to 90 and longitude -180 to 180");
                }
            }
            (None, None) => {
                if self.location.as_deref().is_none_or(|l| l.trim().is_empty()) {
                    return err("give a location, or a latitude and a longitude");
                }
            }
            _ => return err("give both latitude and longitude, or neither"),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {

    #[test]
    fn every_starter_is_valid_and_fills_its_pages() {
        for (name, text) in super::STARTERS {
            let c = Config::parse(text).unwrap_or_else(|e| panic!("{name}: {e}"));
            for p in &c.pages {
                let width: u32 = p.widgets.iter().map(|w| w.width()).sum();
                assert_eq!(width, 640, "{name}, page {}", p.name);
            }
        }
    }

    use super::*;

    #[test]
    fn actions_parse_and_are_checked() {
        let page = |tap: &str| {
            Config::parse(&format!(
                "[[page]]\nname = \"Main\"\n[[page.widget]]\ntype = \"clock\"\nwidth = 100\n{tap}\n"
            ))
        };
        let tap = |src: &str| page(src).unwrap().pages[0].widgets[0].tap().cloned();
        assert_eq!(
            tap("tap = { page = \"next\" }"),
            Some(Action::Page("next".into()))
        );
        assert_eq!(
            tap("tap = { run = [\"pavucontrol\", \"-t\", \"3\"] }"),
            Some(Action::Run(vec![
                "pavucontrol".into(),
                "-t".into(),
                "3".into()
            ]))
        );
        assert_eq!(
            tap("tap = { media = \"play-pause\" }"),
            Some(Action::Media(MediaCommand::PlayPause))
        );
        let held = page("hold = { open = \"https://example.org\" }").unwrap();
        assert_eq!(
            held.pages[0].widgets[0].hold(),
            Some(&Action::Open("https://example.org".into()))
        );
        for (bad, says) in [
            ("tap = { page = \"Nope\" }", "no page called \"Nope\""),
            ("tap = { run = [] }", "run needs a program"),
            ("tap = { media = \"louder\" }", "unknown variant"),
            ("tap = { page = \"next\", run = [\"x\"] }", ""),
        ] {
            let e = page(bad).unwrap_err().0;
            assert!(e.contains(says), "{bad}: {e}");
        }
    }

    #[test]
    fn sliders_parse_and_are_checked() {
        let parse = |widget: &str, more: &str| {
            Config::parse(&format!(
                "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"slider\"\nwidth = 200\nlabel = \"S\"\n{widget}\n{more}\n[[page]]\nname = \"B\"\n"
            ))
        };
        let clock = "[[page.widget]]\ntype = \"clock\"\nwidth = 100\n";
        let controls = |src: &str| match &parse(src, clock).unwrap().pages[0].widgets[0] {
            Widget::Slider { controls, .. } => controls.clone(),
            _ => unreachable!(),
        };
        assert_eq!(controls("controls = \"volume\""), SliderControl::Volume);
        assert_eq!(controls("controls = \"seek\""), SliderControl::Seek);
        assert_eq!(
            controls("controls = { run = [\"ddcutil\", \"setvcp\", \"10\", \"{value}\"] }"),
            SliderControl::Run(vec![
                "ddcutil".into(),
                "setvcp".into(),
                "10".into(),
                "{value}".into()
            ])
        );
        for (widget, more, says) in [
            ("controls = \"volume\"", "", "only sliders"),
            (
                "controls = \"volume\"\ntap = { page = \"B\" }",
                clock,
                "cannot have tap",
            ),
            ("controls = { run = [\"ddcutil\"] }", clock, "{value}"),
        ] {
            let e = parse(widget, more).unwrap_err().0;
            assert!(e.contains(says), "{widget}: {e}");
        }
    }

    #[test]
    fn weather_needs_a_place() {
        let page = "\n[[page]]\nname = \"P\"\n";
        let ok = |w: &str| Config::parse(&format!("[weather]\n{w}{page}"));
        assert_eq!(
            ok("location = \"Jersey City\"\n")
                .unwrap()
                .weather
                .unwrap()
                .location
                .as_deref(),
            Some("Jersey City")
        );
        assert!(ok("latitude = 40.7\nlongitude = -74.0\n").is_ok());
        for bad in ["", "latitude = 40.7\n", "latitude = 91\nlongitude = 0\n"] {
            let e = ok(bad).unwrap_err().0;
            assert!(e.starts_with("[weather]"), "{bad:?} gave {e}");
        }
        assert_eq!(Config::parse(page).unwrap().weather, None);
    }

    #[test]
    fn default_layout_is_valid() {
        let c = Config::default_layout();
        assert!(!c.pages.is_empty());
        assert_eq!(c.pages[0].name, "System");
    }

    #[test]
    fn settings_parse() {
        let c = Config::parse(
            r#"
            [settings]
            font = "lato"
            temperature = "fahrenheit"
            clock = "12h"
            [[page]]
            name = "A"
            "#,
        )
        .unwrap();
        assert_eq!(c.settings.font, FontChoice::Lato);
        assert_eq!(c.settings.temperature, Temperature::Fahrenheit);
        assert_eq!(c.settings.clock, ClockFormat::H12);
    }

    #[test]
    fn the_accent_is_a_hex_colour_or_absent() {
        let with = |accent: &str| {
            Config::parse(&format!(
                "[settings]\naccent = {accent}\n[[page]]\nname = \"A\"\n"
            ))
        };
        assert_eq!(
            with("\"#5AA0e0\"").unwrap().settings.accent,
            Some(Rgb::hex(0x5aa0e0))
        );
        assert_eq!(Config::parse(PAGE).unwrap().settings.accent, None);
        for bad in [
            "\"blue\"",
            "\"#12345\"",
            "\"#1234567\"",
            "\"#gggggg\"",
            "\"5aa0e0\"",
            "3",
        ] {
            let e = with(bad).unwrap_err().0;
            assert!(e.contains("accent"), "{bad}: names the key: {e}");
        }
        let e = with("\"blue\"").unwrap_err().0;
        assert!(e.contains("#rrggbb") && e.contains("\"blue\""), "{e}");
        assert_eq!(colour_text(Rgb::hex(0x0a0b0c)), "#0a0b0c");
    }

    const PAGE: &str = "[[page]]\nname = \"A\"\n";

    #[test]
    fn keys_parse_with_their_actions() {
        let c = Config::parse(&format!(
            "{PAGE}[[page.widget]]\ntype = \"key\"\nwidth = 60\nicon = \"play\"\nlabel = \"Play\"\n\
             tap = {{ media = \"play-pause\" }}\nhold = {{ media = \"next\" }}\n\
             [[page.widget]]\ntype = \"key\"\nwidth = 40\n"
        ))
        .unwrap();
        let Widget::Key { icon, label, .. } = &c.pages[0].widgets[0] else {
            panic!("{:?}", c.pages[0].widgets[0])
        };
        assert_eq!(
            (icon.as_deref(), label.as_deref()),
            (Some("play"), Some("Play"))
        );
        assert_eq!(
            c.pages[0].widgets[0].tap(),
            Some(&Action::Media(MediaCommand::PlayPause))
        );
        assert_eq!(
            c.pages[0].widgets[0].hold(),
            Some(&Action::Media(MediaCommand::Next))
        );
        assert_eq!(c.pages[0].widgets[1].tap(), None, "both optional");
        let narrow = err(&format!(
            "{PAGE}[[page.widget]]\ntype = \"key\"\nwidth = 30\n"
        ));
        assert!(narrow.contains("too narrow"), "{narrow}");
        let typo = err(&format!(
            "{PAGE}[[page.widget]]\ntype = \"key\"\nwidth = 60\nicn = \"x\"\n"
        ));
        assert!(typo.contains("icn"), "{typo}");
    }

    #[test]
    fn a_value_may_have_a_second_reading() {
        let value = |more: &str| {
            Config::parse(&format!(
                "{PAGE}[[page.widget]]\ntype = \"value\"\nwidth = 160\nlabel = \"Load\"\nsignal = \"cpu.load\"\n{more}"
            ))
        };
        let c = value("second = \"mem.used\"\nsecond_warn = 80\nsecond_crit = 95\n").unwrap();
        let Widget::Value {
            second,
            second_warn,
            second_crit,
            ..
        } = &c.pages[0].widgets[0]
        else {
            unreachable!()
        };
        assert_eq!(
            (second.as_deref(), *second_warn, *second_crit),
            (Some("mem.used"), Some(80.0), Some(95.0))
        );
        for (more, says) in [
            ("second_crit = 95\n", "need a second signal"),
            ("second = \" \"\n", "second is empty"),
        ] {
            let e = value(more).unwrap_err().0;
            assert!(e.contains(says), "{more}: {e}");
        }
    }

    #[test]
    fn a_takeover_may_cover_one_end_of_the_strip() {
        let t = |keys: &str, width: u32| {
            Config::parse(&format!(
                "{PAGE}[[takeover]]\nname = \"T\"\non_change = [\"x\"]\nseconds = 2\n{keys}\
                 [[takeover.widget]]\ntype = \"clock\"\nwidth = {width}\n"
            ))
        };
        let full = t("", 640).unwrap();
        assert_eq!(
            (full.takeovers[0].align, full.takeovers[0].width),
            (Align::Full, None),
            "full by default"
        );
        assert_eq!(full.takeovers[0].span(), (0, 640));
        let right = t("align = \"right\"\nwidth = 200\n", 200).unwrap();
        assert_eq!(right.takeovers[0].span(), (440, 200));
        let left = t("align = \"left\"\nwidth = 120\n", 100).unwrap();
        assert_eq!(left.takeovers[0].span(), (0, 120));
        for (keys, width, says) in [
            ("align = \"left\"\n", 100, "needs a width"),
            ("width = 200\n", 200, "width is only for"),
            ("align = \"right\"\nwidth = 30\n", 30, "must be 40 to 640"),
            (
                "align = \"right\"\nwidth = 200\n",
                300,
                "300 px, but it is 200 px wide",
            ),
            ("align = \"middle\"\nwidth = 200\n", 200, "unknown variant"),
        ] {
            let e = t(keys, width).unwrap_err().0;
            assert!(e.contains(says), "{keys}: {e}");
        }
    }

    fn err(text: &str) -> String {
        Config::parse(text).unwrap_err().0
    }

    #[test]
    fn mistakes_are_named() {
        let e = err("[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"clok\"\nwidth = 100\n");
        assert!(e.contains("clok"), "{e}");
        assert!(e.contains("line 4"), "location given: {e}");

        let e = err("[settings]\nfont = \"comic-sans\"\n[[page]]\nname = \"A\"\n");
        assert!(e.contains("ibm-plex-sans"), "lists the choices: {e}");

        let e = err("[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"clock\"\nwidht = 100\n");
        assert!(e.contains("widht"), "typos are not ignored: {e}");
    }

    #[test]
    fn rules_the_types_cannot_express() {
        assert!(err("").contains("no pages"));
        assert!(err("[[page]]\nname = \"A\"\n[[page]]\nname = \"A\"\n").contains("twice"));
        let wide = "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"clock\"\nwidth = 400\n\
                    [[page.widget]]\ntype = \"clock\"\nwidth = 400\n";
        assert!(err(wide).contains("800 px"));
        let narrow = "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"clock\"\nwidth = 10\n";
        assert!(err(narrow).contains("too narrow"));
    }

    #[test]
    fn takeovers_are_checked() {
        let t = |body: &str| {
            Config::parse(&format!(
                "[[page]]\nname = \"A\"\n[[takeover]]\nname = \"T\"\n{body}"
            ))
        };
        let slider = "[[takeover.widget]]\ntype = \"slider\"\nwidth = 640\nlabel = \"V\"\ncontrols = \"volume\"\n";
        let ok = t(&format!(
            "on_change = [\"volume.level\"]\nseconds = 2\n{slider}"
        ))
        .unwrap();
        assert_eq!(ok.takeovers[0].on_change, ["volume.level"]);
        for (body, says) in [
            (
                format!("on_change = []\nseconds = 2\n{slider}"),
                "on_change needs",
            ),
            (
                format!("on_change = [\"x\"]\nseconds = 0\n{slider}"),
                "seconds is 0",
            ),
            ("on_change = [\"x\"]\nseconds = 2\n".into(), "no widgets"),
            (
                format!("on_change = [\"x\"]\nseconds = 2\n{slider}{slider}"),
                "takeover \"T\": widget widths add up to 1280",
            ),
            (
                format!(
                    "on_change = [\"x\"]\nseconds = 2\n{slider}[[takeover]]\nname = \"T\"\non_change = [\"y\"]\nseconds = 2\n{slider}"
                ),
                "appears twice",
            ),
        ] {
            let e = t(&body).unwrap_err().0;
            assert!(e.contains(says), "{body}: {e}");
        }
    }

    #[test]
    fn sources_are_checked() {
        let src = |body: &str| Config::parse(&format!("[[page]]\nname = \"A\"\n{body}"));
        let ok = src("[[source]]\nname = \"disk\"\nrun = [\"sh\", \"disk.sh\"]\n").unwrap();
        assert_eq!(ok.sources[0].run, ["sh", "disk.sh"]);
        for (body, says) in [
            (
                "[[source]]\nname = \"my disk\"\nrun = [\"x\"]\n",
                "letters, digits",
            ),
            (
                "[[source]]\nname = \"d\"\nrun = []\n",
                "run needs a program",
            ),
            (
                "[[source]]\nname = \"d\"\nrun = [\"x\"]\n[[source]]\nname = \"d\"\nrun = [\"y\"]\n",
                "appears twice",
            ),
        ] {
            let e = src(body).unwrap_err().0;
            assert!(e.contains(says), "{body}: {e}");
        }
    }

    /// docs/LAYOUT.md's examples are real: each complete one parses.
    #[test]
    fn the_layout_reference_examples_parse() {
        // A Windows checkout may have CRLF line endings.
        let page = include_str!("../../../docs/LAYOUT.md").replace("\r\n", "\n");
        let mut checked = 0;
        for block in page.split("```toml\n").skip(1) {
            let text = &block[..block.find("```").expect("closed block")];
            if text.contains("[[page]]") {
                Config::parse(text).unwrap_or_else(|e| panic!("{e}\n{text}"));
                checked += 1;
            }
        }
        assert!(checked > 0, "the reference has a whole layout");
    }
}
