//! Where Nexus Open keeps everything it is configured with: settings,
//! layouts (pages and takeovers), sources, and the undo history. One SQLite
//! database, written only by the daemon. docs/STORAGE.md has the design.
//!
//! The engine keeps working on `Config`. This crate turns a `Config` into
//! rows (`import`) and rows back into a `Config` (`load`), and `text` turns
//! either into TOML with comments kept as notes.

mod schema;
pub mod text;

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use nexus_core::config::{
    Action, Config, ConfigError, Page, Settings, Source, Takeover, Weather, Widget,
};
use rusqlite::{Connection, Transaction, params};
use serde_json::{Map, Value as Json, json};

pub use text::{Notes, RowNotes};

pub type LayoutId = i64;

#[derive(Debug)]
pub enum StoreError {
    Sqlite(rusqlite::Error),
    /// The layout breaks a rule the daemon enforces.
    Invalid(ConfigError),
    /// Stored data the current types cannot read.
    Corrupt(String),
    /// The database was made by a newer Nexus Open.
    Newer {
        found: usize,
        known: usize,
    },
    /// There is no layout yet.
    Empty,
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Sqlite(e) => write!(f, "database: {e}"),
            StoreError::Invalid(e) => write!(f, "{e}"),
            StoreError::Corrupt(m) => write!(f, "stored data cannot be read: {m}"),
            StoreError::Newer { found, known } => write!(
                f,
                "the database is from a newer Nexus Open (schema {found}; this one knows up to {known}); update Nexus Open"
            ),
            StoreError::Empty => write!(f, "there is no layout yet"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<rusqlite::Error> for StoreError {
    fn from(e: rusqlite::Error) -> Self {
        StoreError::Sqlite(e)
    }
}

impl From<ConfigError> for StoreError {
    fn from(e: ConfigError) -> Self {
        StoreError::Invalid(e)
    }
}

fn corrupt(e: impl fmt::Display) -> StoreError {
    StoreError::Corrupt(e.to_string())
}

pub struct Store {
    conn: Connection,
}

impl Store {
    pub fn open(path: &Path) -> Result<Store, StoreError> {
        Store::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Store, StoreError> {
        Store::init(Connection::open_in_memory()?)
    }

    fn init(mut conn: Connection) -> Result<Store, StoreError> {
        // SQLite leaves foreign keys off unless asked, per connection.
        conn.pragma_update(None, "foreign_keys", true)?;
        // Readers never wait on the writer, and a commit does not wait on
        // the disk (WAL with NORMAL sync: durable at the next checkpoint).
        conn.query_row("PRAGMA journal_mode = WAL", [], |_| Ok(()))?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(Duration::from_secs(2))?;
        migrate(&mut conn)?;
        Ok(Store { conn })
    }

    /// True until the first layout is imported.
    pub fn is_empty(&self) -> Result<bool, StoreError> {
        let n: i64 = self
            .conn
            .query_row("SELECT count(*) FROM layout", [], |r| r.get(0))?;
        Ok(n == 0)
    }

    /// Every layout, by name.
    pub fn layouts(&self) -> Result<Vec<(LayoutId, String)>, StoreError> {
        let mut q = self
            .conn
            .prepare("SELECT id, name FROM layout ORDER BY name")?;
        let rows = q.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<_, _>>()?)
    }

    /// The layout on the strip.
    pub fn active_layout(&self) -> Result<LayoutId, StoreError> {
        self.conn
            .query_row("SELECT active_layout FROM settings", [], |r| r.get(0))
            .map_err(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => StoreError::Empty,
                e => e.into(),
            })
    }

    /// Adds a config's pages and takeovers as a new layout named `name`.
    /// With `whole`, its settings and sources replace the stored ones and
    /// it goes on the strip; without, only the layout is added (a shared
    /// layout or a starter). The first layout ever imported always brings
    /// its settings.
    pub fn import(
        &mut self,
        name: &str,
        config: &Config,
        notes: &Notes,
        whole: bool,
    ) -> Result<LayoutId, StoreError> {
        config.validate()?;
        let tx = self.conn.transaction()?;
        tx.execute("INSERT INTO layout (name) VALUES (?1)", [name])?;
        let layout = tx.last_insert_rowid();
        write_layout(&tx, layout, config, notes)?;
        let first: i64 = tx.query_row("SELECT count(*) FROM settings", [], |r| r.get(0))?;
        if whole || first == 0 {
            write_settings(&tx, config, &notes.settings, layout)?;
        }
        if whole {
            write_sources(&tx, &config.sources, notes)?;
        }
        tx.commit()?;
        Ok(layout)
    }

    /// A layout with the settings and sources, as the engine takes it.
    pub fn load(&self, layout: LayoutId) -> Result<(Config, Notes), StoreError> {
        let (settings, weather, settings_note) = self.read_settings()?;
        let mut notes = Notes {
            settings: settings_note,
            ..Notes::default()
        };

        let mut q = self.conn.prepare(
            "SELECT id, name, note FROM page WHERE layout_id = ?1 ORDER BY position, id",
        )?;
        let page_rows: Vec<(i64, String, String)> = q
            .query_map([layout], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
            .collect::<Result<_, _>>()?;
        let names: HashMap<i64, String> = page_rows
            .iter()
            .map(|(id, name, _)| (*id, name.clone()))
            .collect();
        let mut pages = Vec::new();
        for (id, name, note) in page_rows {
            let (widgets, widget_notes) = self.read_widgets(Owner::Page(id), &names)?;
            pages.push(Page { name, widgets });
            notes.pages.push(RowNotes {
                note,
                widgets: widget_notes,
            });
        }

        let mut q = self.conn.prepare(
            "SELECT id, name, seconds, note FROM takeover WHERE layout_id = ?1 ORDER BY position, id",
        )?;
        let takeover_rows: Vec<(i64, String, f64, String)> = q
            .query_map([layout], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
            })?
            .collect::<Result<_, _>>()?;
        let mut takeovers = Vec::new();
        for (id, name, seconds, note) in takeover_rows {
            let mut q = self.conn.prepare(
                "SELECT signal FROM takeover_signal WHERE takeover_id = ?1 ORDER BY position",
            )?;
            let on_change = q.query_map([id], |r| r.get(0))?.collect::<Result<_, _>>()?;
            let (widgets, widget_notes) = self.read_widgets(Owner::Takeover(id), &names)?;
            takeovers.push(Takeover {
                name,
                on_change,
                seconds,
                widgets,
            });
            notes.takeovers.push(RowNotes {
                note,
                widgets: widget_notes,
            });
        }

        let mut q = self
            .conn
            .prepare("SELECT name, argv, enabled, note FROM source ORDER BY position, id")?;
        let source_rows: Vec<(String, String, bool, String)> = q
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
            .collect::<Result<_, _>>()?;
        let mut sources = Vec::new();
        for (name, argv, enabled, note) in source_rows {
            let run = serde_json::from_str(&argv).map_err(corrupt)?;
            sources.push(Source { name, run, enabled });
            notes.sources.push(note);
        }

        let config = Config {
            settings,
            pages,
            weather,
            takeovers,
            sources,
        };
        config.validate()?;
        Ok((config, notes))
    }

    fn read_settings(&self) -> Result<(Settings, Option<Weather>, String), StoreError> {
        let row = self.conn.query_row(
            "SELECT font, temperature, clock, weather_location, weather_latitude, weather_longitude, note FROM settings",
            [],
            |r| {
                Ok((
                    json!({ "font": r.get::<_, String>(0)?, "temperature": r.get::<_, String>(1)?, "clock": r.get::<_, String>(2)? }),
                    Weather {
                        location: r.get(3)?,
                        latitude: r.get(4)?,
                        longitude: r.get(5)?,
                    },
                    r.get::<_, String>(6)?,
                ))
            },
        );
        let (settings, weather, note) = match row {
            Err(rusqlite::Error::QueryReturnedNoRows) => return Err(StoreError::Empty),
            other => other?,
        };
        let settings = serde_json::from_value(settings).map_err(corrupt)?;
        let any = weather.location.is_some() || weather.latitude.is_some();
        Ok((settings, any.then_some(weather), note))
    }

    fn read_widgets(
        &self,
        owner: Owner,
        pages: &HashMap<i64, String>,
    ) -> Result<(Vec<Widget>, Vec<String>), StoreError> {
        let (column, id) = owner.column();
        let mut q = self.conn.prepare(&format!(
            "SELECT id, type, width, props, note FROM widget WHERE {column} = ?1 ORDER BY position, id"
        ))?;
        let rows: Vec<(i64, String, u32, String, String)> = q
            .query_map([id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
            })?
            .collect::<Result<_, _>>()?;
        let mut actions = self
            .conn
            .prepare("SELECT gesture, kind, target_page, args FROM action WHERE widget_id = ?1")?;
        let (mut widgets, mut notes) = (Vec::new(), Vec::new());
        for (id, kind, width, props, note) in rows {
            let mut fields: Map<String, Json> = serde_json::from_str(&props).map_err(corrupt)?;
            fields.insert("type".into(), kind.into());
            fields.insert("width".into(), width.into());
            let stored: Vec<(String, String, Option<i64>, String)> = actions
                .query_map([id], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                .collect::<Result<_, _>>()?;
            for (gesture, kind, target, args) in stored {
                let action = match (kind.as_str(), target) {
                    ("page", Some(page)) => {
                        let name = pages.get(&page).ok_or_else(|| {
                            corrupt(format!(
                                "a {gesture} opens page {page}, which is in another layout"
                            ))
                        })?;
                        json!({ "page": name })
                    }
                    ("next-page", _) => json!({ "page": "next" }),
                    ("prev-page", _) => json!({ "page": "prev" }),
                    _ => serde_json::from_str(&args).map_err(corrupt)?,
                };
                fields.insert(gesture, action);
            }
            widgets.push(serde_json::from_value(Json::Object(fields)).map_err(corrupt)?);
            notes.push(note);
        }
        Ok((widgets, notes))
    }
}

/// What a widget sits on.
#[derive(Clone, Copy)]
enum Owner {
    Page(i64),
    Takeover(i64),
}

impl Owner {
    fn column(self) -> (&'static str, i64) {
        match self {
            Owner::Page(id) => ("page_id", id),
            Owner::Takeover(id) => ("takeover_id", id),
        }
    }
}

fn migrate(conn: &mut Connection) -> Result<(), StoreError> {
    let found: i64 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    let found = found as usize;
    let known = schema::MIGRATIONS.len();
    if found > known {
        return Err(StoreError::Newer { found, known });
    }
    for (i, sql) in schema::MIGRATIONS.iter().enumerate().skip(found) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", (i + 1) as i64)?;
        tx.commit()?;
    }
    Ok(())
}

fn write_layout(
    tx: &Transaction,
    layout: LayoutId,
    config: &Config,
    notes: &Notes,
) -> Result<(), StoreError> {
    // Pages first, so actions can point to them by ID.
    let mut page_ids = Vec::new();
    for (i, page) in config.pages.iter().enumerate() {
        tx.execute(
            "INSERT INTO page (layout_id, position, name, note) VALUES (?1, ?2, ?3, ?4)",
            params![layout, i as i64, page.name, notes.page(i).note],
        )?;
        page_ids.push(tx.last_insert_rowid());
    }
    let by_name: HashMap<&str, i64> = config
        .pages
        .iter()
        .map(|p| p.name.as_str())
        .zip(page_ids.iter().copied())
        .collect();
    for (i, (page, id)) in config.pages.iter().zip(&page_ids).enumerate() {
        for (j, widget) in page.widgets.iter().enumerate() {
            let note = notes.page(i).widget(j);
            insert_widget(tx, Owner::Page(*id), j, widget, note, &by_name)?;
        }
    }
    for (i, takeover) in config.takeovers.iter().enumerate() {
        tx.execute(
            "INSERT INTO takeover (layout_id, position, name, seconds, note) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![layout, i as i64, takeover.name, takeover.seconds, notes.takeover(i).note],
        )?;
        let id = tx.last_insert_rowid();
        for (j, signal) in takeover.on_change.iter().enumerate() {
            tx.execute(
                "INSERT INTO takeover_signal (takeover_id, signal, position) VALUES (?1, ?2, ?3)",
                params![id, signal, j as i64],
            )?;
        }
        for (j, widget) in takeover.widgets.iter().enumerate() {
            let note = notes.takeover(i).widget(j);
            insert_widget(tx, Owner::Takeover(id), j, widget, note, &by_name)?;
        }
    }
    Ok(())
}

fn insert_widget(
    tx: &Transaction,
    owner: Owner,
    position: usize,
    widget: &Widget,
    note: &str,
    pages: &HashMap<&str, i64>,
) -> Result<(), StoreError> {
    let Json::Object(mut fields) = serde_json::to_value(widget).map_err(corrupt)? else {
        return Err(corrupt("a widget did not serialize to an object"));
    };
    fields.retain(|_, v| !v.is_null());
    let kind = fields.remove("type").unwrap_or_default();
    let width = fields.remove("width").unwrap_or_default();
    fields.remove("tap");
    fields.remove("hold");
    let (page_id, takeover_id) = match owner {
        Owner::Page(id) => (Some(id), None),
        Owner::Takeover(id) => (None, Some(id)),
    };
    tx.execute(
        "INSERT INTO widget (page_id, takeover_id, position, type, width, props, note) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            page_id,
            takeover_id,
            position as i64,
            kind.as_str(),
            width.as_i64(),
            Json::Object(fields).to_string(),
            note
        ],
    )?;
    let id = tx.last_insert_rowid();
    for (gesture, action) in [("tap", widget.tap()), ("hold", widget.hold())] {
        let Some(action) = action else { continue };
        let (kind, target, args) = match action {
            Action::Page(t) if t == "next" => ("next-page", None, json!({})),
            Action::Page(t) if t == "prev" => ("prev-page", None, json!({})),
            Action::Page(t) => {
                let page = pages.get(t.as_str()).ok_or_else(|| {
                    StoreError::Invalid(ConfigError(format!("no page called \"{t}\"")))
                })?;
                ("page", Some(*page), json!({}))
            }
            Action::Run(_) => ("run", None, json!(action)),
            Action::Open(_) => ("open", None, json!(action)),
            Action::Media(_) => ("media", None, json!(action)),
            Action::Publish { .. } => ("publish", None, json!(action)),
        };
        tx.execute(
            "INSERT INTO action (widget_id, gesture, kind, target_page, args) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, gesture, kind, target, args.to_string()],
        )?;
    }
    Ok(())
}

fn write_settings(
    tx: &Transaction,
    config: &Config,
    note: &str,
    active: LayoutId,
) -> Result<(), StoreError> {
    let s = serde_json::to_value(&config.settings).map_err(corrupt)?;
    let text = |key: &str| s[key].as_str().unwrap_or_default().to_string();
    let w = config.weather.as_ref();
    tx.execute(
        "INSERT INTO settings (id, font, temperature, clock, weather_location, weather_latitude, weather_longitude, active_layout, note)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
         ON CONFLICT (id) DO UPDATE SET
           font = excluded.font, temperature = excluded.temperature, clock = excluded.clock,
           weather_location = excluded.weather_location, weather_latitude = excluded.weather_latitude,
           weather_longitude = excluded.weather_longitude, active_layout = excluded.active_layout,
           note = excluded.note",
        params![
            text("font"),
            text("temperature"),
            text("clock"),
            w.and_then(|w| w.location.clone()),
            w.and_then(|w| w.latitude),
            w.and_then(|w| w.longitude),
            active,
            note
        ],
    )?;
    Ok(())
}

fn write_sources(tx: &Transaction, sources: &[Source], notes: &Notes) -> Result<(), StoreError> {
    tx.execute("DELETE FROM source", [])?;
    for (i, source) in sources.iter().enumerate() {
        tx.execute(
            "INSERT INTO source (name, argv, enabled, position, note) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                source.name,
                json!(source.run).to_string(),
                source.enabled,
                i as i64,
                notes.source(i)
            ],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with_default() -> (Store, LayoutId) {
        let mut store = Store::open_in_memory().unwrap();
        let config = Config::default_layout();
        let id = store
            .import("Default", &config, &Notes::default(), true)
            .unwrap();
        (store, id)
    }

    #[test]
    fn a_layout_comes_back_exactly() {
        let (store, id) = with_default();
        assert_eq!(store.active_layout().unwrap(), id);
        let (config, _) = store.load(id).unwrap();
        assert_eq!(config, Config::default_layout());
    }

    #[test]
    fn everything_round_trips_with_notes() {
        let text = r#"
            [settings]
            temperature = "fahrenheit"
            clock = "12h"

            [weather]
            latitude = 40.7
            longitude = -74.0

            # Home page.
            [[page]]
            name = "Main"

            # Goes to Other.
            [[page.widget]]
            type = "clock"
            width = 100
            tap = { page = "Other" }
            hold = { page = "next" }

            [[page.widget]]
            type = "value"
            width = 200
            label = "CPU"
            signal = "cpu.temp"
            graph = "area"
            warn = 75.0
            crit = 90.0
            tap = { run = ["htop", "-d", "10"] }
            hold = { publish = { signal = "mode", value = "focus" } }

            [[page]]
            name = "Other"

            [[page.widget]]
            type = "slider"
            width = 300
            label = "Screen"
            controls = { run = ["ddcutil", "setvcp", "10", "{value}"] }

            [[page.widget]]
            type = "media"
            width = 200
            tap = { media = "play-pause" }

            [[takeover]]
            name = "Volume"
            on_change = ["volume.muted", "volume.level"]
            seconds = 2.0

            [[takeover.widget]]
            type = "slider"
            width = 640
            label = "Volume"
            controls = "volume"

            # Disk use, every 30 s.
            [[source]]
            name = "disk"
            run = ["sh", "disk.sh"]
            enabled = false
        "#;
        let (config, notes) = text::parse(text).unwrap();
        let mut store = Store::open_in_memory().unwrap();
        let id = store.import("Mine", &config, &notes, true).unwrap();
        let (back, back_notes) = store.load(id).unwrap();
        assert_eq!(back, config);
        assert_eq!(back_notes.page(0).note, "Home page.");
        assert_eq!(back_notes.page(0).widget(0), "Goes to Other.");
        assert_eq!(back_notes.source(0), "Disk use, every 30 s.");
        assert_eq!(
            back.takeovers[0].on_change,
            ["volume.muted", "volume.level"],
            "order kept"
        );
        // And the text written from the store reads back the same.
        let written = text::write(&back, &back_notes);
        assert_eq!(text::parse(&written).unwrap(), (back, back_notes));
    }

    #[test]
    fn taps_follow_a_renamed_page_and_block_its_deletion() {
        let (store, id) = with_default();
        store
            .conn
            .execute("UPDATE page SET name = 'Time' WHERE name = 'Clock'", [])
            .unwrap();
        let (config, _) = store.load(id).unwrap();
        assert_eq!(
            config.pages[0].widgets[0].tap(),
            Some(&Action::Page("Time".into())),
            "the tap points at the page, not its old name"
        );
        let deleted = store
            .conn
            .execute("DELETE FROM page WHERE name = 'Time'", []);
        assert!(deleted.is_err(), "a page a tap opens cannot be deleted");
    }

    #[test]
    fn a_second_layout_keeps_the_settings_and_the_strip() {
        let (mut store, first) = with_default();
        let mut other = Config::default_layout();
        other.settings.clock = nexus_core::config::ClockFormat::H12;
        other.pages.truncate(1);
        other.pages[0].widgets[0] = Widget::Clock {
            width: 107,
            tap: None,
            hold: None,
        };
        let second = store
            .import("Starter", &other, &Notes::default(), false)
            .unwrap();
        assert_eq!(store.active_layout().unwrap(), first);
        let (loaded, _) = store.load(second).unwrap();
        assert_eq!(loaded.pages.len(), 1);
        assert_eq!(
            loaded.settings.clock,
            Config::default_layout().settings.clock,
            "settings are app-wide; a starter does not change them"
        );
        assert_eq!(store.layouts().unwrap().len(), 2);
    }

    #[test]
    fn invalid_layouts_are_refused_whole() {
        let mut store = Store::open_in_memory().unwrap();
        let mut config = Config::default_layout();
        config.pages[0].widgets.push(Widget::Clock {
            width: 600,
            tap: None,
            hold: None,
        });
        let e = store
            .import("Wide", &config, &Notes::default(), true)
            .unwrap_err();
        assert!(e.to_string().contains("px"), "{e}");
        assert!(store.is_empty().unwrap(), "nothing written");
    }

    #[test]
    fn a_newer_database_is_refused_and_an_older_one_upgraded() {
        let path = std::env::temp_dir().join(format!("nexus-store-test-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        drop(Store::open(&path).unwrap());
        drop(Store::open(&path).unwrap()); // already current: no migration
        let conn = Connection::open(&path).unwrap();
        conn.pragma_update(None, "user_version", 99).unwrap();
        drop(conn);
        let e = Store::open(&path).err().unwrap();
        assert!(matches!(e, StoreError::Newer { found: 99, .. }), "{e}");
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    /// A real config file, through TOML → rows → `Config` and TOML. Opt-in,
    /// since the file is personal: `NEXUS_TOML=path cargo test -p
    /// nexus-store -- --ignored`.
    #[test]
    #[ignore]
    fn a_real_file_round_trips() {
        let path = std::env::var("NEXUS_TOML").expect("set NEXUS_TOML to a config file");
        let (config, notes) = text::parse(&std::fs::read_to_string(path).unwrap()).unwrap();
        let mut store = Store::open_in_memory().unwrap();
        let id = store.import("Real", &config, &notes, true).unwrap();
        let (back, back_notes) = store.load(id).unwrap();
        assert_eq!(back, config);
        let written = text::write(&back, &back_notes);
        assert_eq!(text::parse(&written).unwrap(), (back, back_notes));
        let cleaned = notes.clone().without_lines_in(nexus_core::config::DEFAULT);
        let mut left = vec![cleaned.settings.clone()];
        for row in cleaned.pages.iter().chain(&cleaned.takeovers) {
            left.push(row.note.clone());
            left.extend(row.widgets.iter().cloned());
        }
        left.retain(|n| !n.is_empty());
        eprintln!("notes after dropping the bundled help: {left:#?}");
        let kept: usize = notes.pages.iter().map(|p| 1 + p.widgets.len()).sum();
        eprintln!(
            "{} pages, {} takeovers, {} sources; {} page/widget note slots; settings note {} lines",
            config.pages.len(),
            config.takeovers.len(),
            config.sources.len(),
            kept,
            notes.settings.lines().count()
        );
    }
}
