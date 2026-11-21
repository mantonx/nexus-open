//! Layouts from the old Go and Flutter Nexus Open: its database
//! (`~/.config/nexus-open/nexus.db`), or a YAML file its Export wrote.
//! Each zone whose plugin has an equivalent becomes a widget; everything
//! else is left out and said, so nothing is lost silently.

use std::path::Path;

use nexus_core::config::{Config, GraphStyle, Page, Settings, Temperature, Weather, Widget};
use serde::Deserialize;
use serde_json::{Map, Value};

/// A layout as the old app kept it.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Legacy {
    pub name: Option<String>,
    pub pages: Vec<LegacyPage>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LegacyPage {
    pub name: String,
    pub zones: Vec<LegacyZone>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct LegacyZone {
    pub id: String,
    pub width: u32,
    /// `builtin:clock`, `exec:nexus-cpu-temp`, or a path to a program.
    pub plugin: String,
    /// `""`, `none`, `cycle` or `detail`.
    pub on_tap: String,
    /// The plugin's settings: `unit`, `graph_type`, `location`…
    pub config: Map<String, Value>,
    /// Plugins a tap cycled through.
    pub choices: Vec<String>,
    /// Colors set on this zone.
    pub themed: bool,
}

/// The new layout, and what could not be carried over.
#[derive(Clone, Debug, PartialEq)]
pub struct Conversion {
    pub config: Config,
    pub left_out: Vec<String>,
}

/// Reads the old app's database. Opened read-only: it is never changed.
pub fn read_db(path: &Path) -> Result<Legacy, String> {
    use rusqlite::{Connection, OpenFlags};
    let fail = |e: rusqlite::Error| format!("{}: {e}", path.display());
    let db = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(fail)?;
    let mut pages_query = db
        .prepare("SELECT id, name FROM pages ORDER BY ord, id")
        .map_err(|_| format!("{} is not an old Nexus Open database", path.display()))?;
    let pages: Vec<(i64, String)> = pages_query
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(fail)?
        .collect::<Result<_, _>>()
        .map_err(fail)?;
    let mut zones_query = db
        .prepare(
            "SELECT id, width_px, plugin, on_tap, config_json, choices_json, theme_json
             FROM zones WHERE page_id = ?1 ORDER BY ord, rowid",
        )
        .map_err(fail)?;
    let mut out = Legacy::default();
    for (id, name) in pages {
        let zones = zones_query
            .query_map([id], |r| {
                let width: i64 = r.get(1)?;
                let config: String = r.get(4)?;
                let choices: String = r.get(5)?;
                let theme: String = r.get(6)?;
                Ok(LegacyZone {
                    id: r.get(0)?,
                    width: u32::try_from(width).unwrap_or(0),
                    plugin: r.get(2)?,
                    on_tap: r.get(3)?,
                    config: serde_json::from_str(&config).unwrap_or_default(),
                    choices: serde_json::from_str(&choices).unwrap_or_default(),
                    themed: !matches!(theme.trim(), "" | "{}" | "null"),
                })
            })
            .map_err(fail)?
            .collect::<Result<_, _>>()
            .map_err(fail)?;
        out.pages.push(LegacyPage { name, zones });
    }
    Ok(out)
}

/// Reads a YAML layout as the old app's Export wrote it.
pub fn read_yaml(text: &str) -> Result<Legacy, String> {
    #[derive(Deserialize)]
    struct File {
        name: Option<String>,
        #[serde(default)]
        pages: Vec<YPage>,
    }
    #[derive(Deserialize)]
    struct YPage {
        name: String,
        #[serde(default)]
        zones: Vec<YZone>,
    }
    #[derive(Deserialize)]
    struct YZone {
        #[serde(default)]
        id: String,
        width: u32,
        #[serde(default)]
        plugin: String,
        #[serde(default)]
        on_tap: String,
        #[serde(default)]
        plugin_config: Option<Map<String, Value>>,
        #[serde(default)]
        choices: Vec<String>,
        #[serde(default)]
        theme_override: Option<serde_yaml_ng::Value>,
    }
    let file: File =
        serde_yaml_ng::from_str(text).map_err(|e| format!("not an old Nexus Open layout: {e}"))?;
    Ok(Legacy {
        name: file.name.filter(|n| !n.trim().is_empty()),
        pages: file
            .pages
            .into_iter()
            .map(|p| LegacyPage {
                name: p.name,
                zones: p
                    .zones
                    .into_iter()
                    .map(|z| LegacyZone {
                        id: z.id,
                        width: z.width,
                        plugin: z.plugin,
                        on_tap: z.on_tap,
                        config: z.plugin_config.unwrap_or_default(),
                        choices: z.choices,
                        themed: z.theme_override.is_some(),
                    })
                    .collect(),
            })
            .collect(),
    })
}

/// The old plugin a zone ran, by its short name: `clock`, `cpu-temp`…
/// `exec:nexus-cpu-temp`, `exec:./plugins/cpu-temp/cpu-temp` and
/// `builtin:clock` all name theirs.
fn plugin_name(plugin: &str) -> &str {
    let bare = plugin
        .strip_prefix("builtin:")
        .or_else(|| plugin.strip_prefix("exec:"))
        .unwrap_or(plugin);
    let file = bare.rsplit(['/', '\\']).next().unwrap_or(bare);
    file.strip_prefix("nexus-").unwrap_or(file)
}

/// The old plugins' graph styles, as the new ones: a sparkline is a line,
/// and a bar graph is closest to a filled area.
fn graph(config: &Map<String, Value>) -> GraphStyle {
    match config.get("graph_type").and_then(Value::as_str) {
        Some("area" | "bar") => GraphStyle::Area,
        _ => GraphStyle::Line,
    }
}

/// A value widget with the thresholds the old plugin colored at.
fn value(width: u32, label: &str, signal: &str, graph: GraphStyle, warn: f64, crit: f64) -> Widget {
    Widget::Value {
        width,
        label: label.into(),
        signal: signal.into(),
        graph: Some(graph),
        warn: Some(warn),
        crit: Some(crit),
        tap: None,
        hold: None,
    }
}

/// The widget for one old zone, or why there is none.
fn widget(zone: &LegacyZone) -> Result<Widget, String> {
    let width = zone.width;
    let c = &zone.config;
    Ok(match plugin_name(&zone.plugin) {
        "clock" => Widget::Clock {
            width,
            tap: None,
            hold: None,
        },
        "media" => Widget::Media {
            width,
            tap: None,
            hold: None,
        },
        "weather" => Widget::Value {
            width,
            label: "Weather".into(),
            signal: "weather.temp".into(),
            graph: None,
            warn: None,
            crit: None,
            tap: None,
            hold: None,
        },
        // The old temperature plugins colored at 75 and 90 °C.
        "cpu-temp" => value(width, "CPU", "cpu.temp", graph(c), 75.0, 90.0),
        "gpu-temp" => value(width, "GPU", "gpu.temp", graph(c), 75.0, 90.0),
        // The old load plugins colored at 70 and 90 %.
        "cpu-load" => value(width, "CPU load", "cpu.load", graph(c), 70.0, 90.0),
        "gpu-load" if c.get("source").and_then(Value::as_str) == Some("memory") => {
            return Err("it showed VRAM use, which nothing publishes yet".into());
        }
        "gpu-load" => value(width, "GPU load", "gpu.load", graph(c), 70.0, 90.0),
        "network" => Widget::Rate {
            width,
            label: "Net".into(),
            down: "net.down".into(),
            up: "net.up".into(),
            tap: None,
            hold: None,
        },
        "placeholder" | "" => return Err("an empty placeholder".into()),
        other => {
            return Err(format!(
                "the plugin `{other}` has no equivalent; an external source can publish \
                 its readings (docs/EXTERNAL_SOURCES.md)"
            ));
        }
    })
}

/// Converts an old layout. Fails only if no page has anything to show.
pub fn convert(legacy: &Legacy) -> Result<Conversion, String> {
    let mut left_out = Vec::new();
    let mut settings = Settings::default();
    let mut weather = None;
    let mut themed = false;
    let mut pages: Vec<Page> = Vec::new();
    for page in &legacy.pages {
        let mut widgets = Vec::new();
        for zone in &page.zones {
            let at = format!("page \"{}\", zone `{}`", page.name, zone.id);
            match widget(zone) {
                Ok(w) => widgets.push(w),
                Err(why) => {
                    left_out.push(format!("{at}: left out: {why}."));
                    continue;
                }
            }
            let c = &zone.config;
            if c.get("unit").and_then(Value::as_str) == Some("imperial") {
                settings.temperature = Temperature::Fahrenheit;
            }
            let name = plugin_name(&zone.plugin);
            if name == "gpu-load" && c.get("show_vram").and_then(Value::as_bool) == Some(true) {
                left_out.push(format!("{at}: its VRAM line is not shown."));
            }
            if name == "cpu-load"
                && let Some(cores) = c
                    .get("source")
                    .filter(|v| !matches!(v.as_str(), Some("all" | "")))
            {
                left_out.push(format!(
                    "{at}: it showed only {cores}; it now shows the load of every core together."
                ));
            }
            if c.get("network_format").and_then(Value::as_str) == Some("bits") {
                left_out.push(format!("{at}: shown in bytes per second, not bits."));
            }
            if name == "weather"
                && weather.is_none()
                && let Some(place) = c.get("location").and_then(Value::as_str)
                && !place.trim().is_empty()
            {
                weather = Some(Weather {
                    location: Some(place.trim().into()),
                    latitude: None,
                    longitude: None,
                });
            }
            match zone.on_tap.as_str() {
                "cycle" => left_out.push(format!(
                    "{at}: a tap cycled through plugins; give the widget a tap action instead."
                )),
                "detail" => left_out.push(format!(
                    "{at}: a tap showed details; give the widget a tap action instead."
                )),
                _ => {}
            }
            themed |= zone.themed;
        }
        if widgets.is_empty() {
            left_out.push(format!(
                "page \"{}\": left out: nothing on it converts.",
                page.name
            ));
            continue;
        }
        // Names must be unique now; the old app allowed repeats.
        let mut name = page.name.trim().to_string();
        if name.is_empty() {
            name = "Page".into();
        }
        let base = name.clone();
        let mut n = 2;
        while pages.iter().any(|p| p.name == name) {
            name = format!("{base} {n}");
            n += 1;
        }
        pages.push(Page { name, widgets });
    }
    if themed {
        left_out.push("Colors set on zones: the new app has one theme for every widget.".into());
    }
    if pages.is_empty() {
        return Err("nothing in that layout converts".into());
    }
    let config = Config {
        settings,
        pages,
        weather,
        takeovers: Vec::new(),
        sources: Vec::new(),
    };
    config.validate().map_err(|e| e.to_string())?;
    Ok(Conversion { config, left_out })
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = r##"
name: "Desk"
version: "1.0"
theme:
  bg: "#000000"
pages:
  - name: "Main"
    zones:
      - id: clock
        width: 160
        plugin: builtin:clock
      - id: cpu
        width: 160
        plugin: exec:nexus-cpu-temp
        plugin_config: { unit: imperial, graph_type: bar }
      - id: net
        width: 160
        plugin: exec:./plugins/network/network
        plugin_config: { network_format: bits }
        on_tap: cycle
        theme_override: { accent: "#38C878" }
      - id: mine
        width: 160
        plugin: exec:/home/someone/bin/stocks
  - name: "Main"
    zones:
      - id: w
        width: 320
        plugin: exec:nexus-weather
        plugin_config: { location: "Lisbon", unit: metric }
      - id: load
        width: 160
        plugin: exec:nexus-gpu-load
        plugin_config: { show_vram: true }
      - id: vram
        width: 80
        plugin: exec:nexus-gpu-load
        plugin_config: { source: memory }
      - id: cores
        width: 80
        plugin: exec:nexus-cpu-load
        plugin_config: { source: [cpu0, cpu1] }
  - name: "Empty"
    zones:
      - id: p
        width: 640
        plugin: builtin:placeholder
"##;

    #[test]
    fn an_old_layout_converts_and_says_what_it_left_out() {
        let legacy = read_yaml(OLD).unwrap();
        assert_eq!(legacy.name.as_deref(), Some("Desk"));
        let c = convert(&legacy).unwrap();
        let names: Vec<&str> = c.config.pages.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Main", "Main 2"]);
        let main = &c.config.pages[0].widgets;
        assert_eq!(main.len(), 3, "the unknown plugin is left out");
        assert!(matches!(main[0], Widget::Clock { width: 160, .. }));
        assert_eq!(
            main[1],
            value(160, "CPU", "cpu.temp", GraphStyle::Area, 75.0, 90.0)
        );
        assert!(matches!(&main[2], Widget::Rate { down, .. } if down == "net.down"));
        assert_eq!(c.config.settings.temperature, Temperature::Fahrenheit);
        assert_eq!(
            c.config
                .weather
                .as_ref()
                .and_then(|w| w.location.as_deref()),
            Some("Lisbon")
        );
        let said = c.left_out.join("\n");
        for expected in [
            "`stocks` has no equivalent",
            "bytes per second",
            "cycled through plugins",
            "page \"Empty\": left out",
            "one theme",
            "zone `vram`: left out: it showed VRAM",
            "zone `load`: its VRAM line",
            "zone `cores`: it showed only",
        ] {
            assert!(said.contains(expected), "missing {expected:?} in:\n{said}");
        }
    }

    #[test]
    fn plugin_names_are_found_in_every_old_spelling() {
        for p in [
            "exec:nexus-cpu-temp",
            "exec:./plugins/cpu-temp/cpu-temp",
            "exec:C:\\nexus\\nexus-cpu-temp",
            "cpu-temp",
        ] {
            assert_eq!(plugin_name(p), "cpu-temp", "{p}");
        }
        assert_eq!(plugin_name("builtin:clock"), "clock");
    }

    #[test]
    fn a_layout_with_nothing_to_convert_is_refused() {
        let legacy = read_yaml("pages:\n  - name: A\n    zones:\n      - {id: p, width: 640, plugin: builtin:placeholder}\n").unwrap();
        assert!(convert(&legacy).unwrap_err().contains("nothing"));
        assert!(read_yaml("just: [text").is_err());
    }

    #[test]
    fn the_database_is_read_in_page_and_zone_order() {
        let dir = std::env::temp_dir().join(format!("nexus-legacy-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("nexus.db");
        let _ = std::fs::remove_file(&path);
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch(
            "CREATE TABLE pages (id INTEGER PRIMARY KEY, name TEXT, ord INTEGER);
             CREATE TABLE zones (id TEXT PRIMARY KEY, page_id INTEGER, ord INTEGER,
               width_px INTEGER, plugin TEXT, refresh_ms INTEGER DEFAULT 2000,
               align TEXT DEFAULT 'center', config_json TEXT DEFAULT '{}',
               theme_json TEXT DEFAULT '{}', on_tap TEXT DEFAULT '',
               choices_json TEXT DEFAULT '[]');
             INSERT INTO pages VALUES (1, 'Second', 1), (2, 'First', 0);
             INSERT INTO zones (id, page_id, ord, width_px, plugin, config_json, choices_json)
               VALUES ('b', 2, 1, 320, 'exec:nexus-gpu-temp', 'null', 'null'),
                      ('a', 2, 0, 320, 'builtin:clock', '{\"clock_face\":\"digital\"}', '[]'),
                      ('c', 1, 0, 640, 'exec:nexus-media', '{}', '[]');",
        )
        .unwrap();
        drop(db);
        let legacy = read_db(&path).unwrap();
        let order: Vec<(&str, Vec<&str>)> = legacy
            .pages
            .iter()
            .map(|p| {
                (
                    p.name.as_str(),
                    p.zones.iter().map(|z| z.id.as_str()).collect(),
                )
            })
            .collect();
        assert_eq!(order, [("First", vec!["a", "b"]), ("Second", vec!["c"])]);
        assert!(convert(&legacy).unwrap().left_out.is_empty());
        assert!(read_db(&dir.join("missing.db")).is_err());
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
