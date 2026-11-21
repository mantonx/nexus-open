//! Where Nexus Open keeps everything it is configured with: settings,
//! layouts (pages and takeovers), sources, and the undo history. One SQLite
//! database, written only by the daemon. docs/STORAGE.md has the design.
//!
//! The engine keeps working on `Config`. This crate turns a `Config` into
//! rows (`import`) and rows back into a `Config` (`load`), and `text` turns
//! either into TOML with comments kept as notes.

pub mod edit;
mod history;
pub mod legacy;
mod schema;
pub mod text;

use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::time::Duration;

use nexus_core::config::{Config, ConfigError, Page, Settings, Source, Takeover, Weather, Widget};
use rusqlite::{Connection, Transaction, params};
use serde_json::{Map, Value as Json, json};

pub use edit::{Edit, Item, Row};
pub use text::{Notes, RowNotes};

pub type LayoutId = i64;

/// The IDs behind a loaded layout, in the same order as its `Config`:
/// `pages[i]` is the page `config.pages[i]`, and so on. Edits name things
/// by these IDs.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct Ids {
    pub pages: Vec<RowIds>,
    pub takeovers: Vec<RowIds>,
}

/// A page's or takeover's ID and its widgets' IDs, left to right.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct RowIds {
    pub id: i64,
    pub widgets: Vec<i64>,
}

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

    /// The layout with this name.
    pub fn layout_named(&self, name: &str) -> Result<Option<LayoutId>, StoreError> {
        Ok(self
            .layouts()?
            .into_iter()
            .find_map(|(id, n)| (n == name).then_some(id)))
    }

    /// A layout's name.
    pub fn layout_name(&self, layout: LayoutId) -> Result<String, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT name FROM layout WHERE id = ?1", [layout], |r| {
                r.get(0)
            })?)
    }

    /// Writes a consistent copy of the whole database to `to`, which must
    /// not exist. Safe while the database is in use.
    pub fn backup(&self, to: &Path) -> Result<(), StoreError> {
        self.conn
            .execute("VACUUM INTO ?1", [to.to_string_lossy()])?;
        Ok(())
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
        self.write(&format!("Add layout {name}"), |tx| {
            tx.execute("INSERT INTO layout (name) VALUES (?1)", [name])?;
            let layout = tx.last_insert_rowid();
            write_layout(tx, layout, config, notes)?;
            let first: i64 = tx.query_row("SELECT count(*) FROM settings", [], |r| r.get(0))?;
            if whole || first == 0 {
                write_settings(tx, config, &notes.settings, layout)?;
            }
            if whole {
                write_sources(tx, &config.sources, notes)?;
            }
            Ok(layout)
        })
    }

    /// Makes one change, or none: it is refused whole if it would leave any
    /// layout invalid, with the reason. Returns the new revision.
    pub fn edit(&mut self, edit: &Edit) -> Result<i64, StoreError> {
        self.write(&edit.label(), |tx| edit::apply(tx, edit))?;
        self.revision()
    }

    /// Takes back the last change. Its label, or `None` with nothing to undo.
    pub fn undo(&mut self) -> Result<Option<String>, StoreError> {
        self.step(true)
    }

    /// Makes the last undone change again.
    pub fn redo(&mut self) -> Result<Option<String>, StoreError> {
        self.step(false)
    }

    /// What undo and redo would do, for the menu.
    pub fn undo_label(&self) -> Result<Option<String>, StoreError> {
        Ok(history::last_done(&self.conn)?.map(|(_, label, _)| label))
    }

    pub fn redo_label(&self) -> Result<Option<String>, StoreError> {
        Ok(history::first_undone(&self.conn)?.map(|(_, label, _)| label))
    }

    /// Goes up by one with every change, undo and redo included.
    pub fn revision(&self) -> Result<i64, StoreError> {
        Ok(self
            .conn
            .query_row("SELECT n FROM revision", [], |r| r.get(0))?)
    }

    fn step(&mut self, back: bool) -> Result<Option<String>, StoreError> {
        let tx = self.conn.transaction()?;
        let entry = if back {
            history::last_done(&tx)?
        } else {
            history::first_undone(&tx)?
        };
        let Some((id, label, state)) = entry else {
            return Ok(None);
        };
        history::restore(&tx, &state)?;
        history::mark(&tx, id, back)?;
        bump(&tx)?;
        tx.commit()?;
        Ok(Some(label))
    }

    /// Runs a change in a transaction and keeps it only if every layout is
    /// still valid afterwards. A change that changes nothing is not
    /// recorded; the first layout ever written is not undoable, since
    /// undoing it would leave nothing to show.
    fn write<T>(
        &mut self,
        label: &str,
        change: impl FnOnce(&Transaction) -> Result<T, StoreError>,
    ) -> Result<T, StoreError> {
        let tx = self.conn.transaction()?;
        let had_layout: bool =
            tx.query_row("SELECT EXISTS (SELECT 1 FROM layout)", [], |r| r.get(0))?;
        let before = history::snapshot(&tx)?;
        let out = change(&tx)?;
        let after = history::snapshot(&tx)?;
        if before != after {
            let mut q = tx.prepare("SELECT id FROM layout")?;
            let layouts: Vec<LayoutId> =
                q.query_map([], |r| r.get(0))?.collect::<Result<_, _>>()?;
            drop(q);
            for layout in layouts {
                load(&tx, layout)?;
            }
            if had_layout {
                history::record(&tx, label, &before, &after)?;
            }
            bump(&tx)?;
            tx.commit()?;
        }
        Ok(out)
    }

    /// The IDs behind `load(layout)`, in the same order.
    pub fn ids(&self, layout: LayoutId) -> Result<Ids, StoreError> {
        let rows = |table: &str, owner: &str| -> Result<Vec<RowIds>, StoreError> {
            let mut q = self.conn.prepare(&format!(
                "SELECT id FROM {table} WHERE layout_id = ?1 ORDER BY position, id"
            ))?;
            let ids: Vec<i64> = q
                .query_map([layout], |r| r.get(0))?
                .collect::<Result<_, _>>()?;
            let mut q = self.conn.prepare(&format!(
                "SELECT id FROM widget WHERE {owner} = ?1 ORDER BY position, id"
            ))?;
            ids.into_iter()
                .map(|id| {
                    let widgets = q.query_map([id], |r| r.get(0))?.collect::<Result<_, _>>()?;
                    Ok(RowIds { id, widgets })
                })
                .collect()
        };
        Ok(Ids {
            pages: rows("page", "page_id")?,
            takeovers: rows("takeover", "takeover_id")?,
        })
    }

    /// A layout with the settings and sources, as the engine takes it.
    pub fn load(&self, layout: LayoutId) -> Result<(Config, Notes), StoreError> {
        load(&self.conn, layout)
    }
}

/// A layout with the settings and sources, as the engine takes it. Also
/// the check that a write left every layout valid.
pub(crate) fn load(conn: &Connection, layout: LayoutId) -> Result<(Config, Notes), StoreError> {
    let (settings, weather, settings_note) = read_settings(conn)?;
    let mut notes = Notes {
        settings: settings_note,
        ..Notes::default()
    };

    let mut q =
        conn.prepare("SELECT id, name, note FROM page WHERE layout_id = ?1 ORDER BY position, id")?;
    let page_rows: Vec<(i64, String, String)> = q
        .query_map([layout], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))?
        .collect::<Result<_, _>>()?;
    let names: HashMap<i64, String> = page_rows
        .iter()
        .map(|(id, name, _)| (*id, name.clone()))
        .collect();
    let mut pages = Vec::new();
    for (id, name, note) in page_rows {
        let (widgets, widget_notes) = read_widgets(conn, Owner::Page(id), &names)?;
        pages.push(Page { name, widgets });
        notes.pages.push(RowNotes {
            note,
            widgets: widget_notes,
        });
    }

    let mut q = conn.prepare(
        "SELECT id, name, seconds, note FROM takeover WHERE layout_id = ?1 ORDER BY position, id",
    )?;
    let takeover_rows: Vec<(i64, String, f64, String)> = q
        .query_map([layout], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut takeovers = Vec::new();
    for (id, name, seconds, note) in takeover_rows {
        let mut q = conn.prepare(
            "SELECT signal FROM takeover_signal WHERE takeover_id = ?1 ORDER BY position",
        )?;
        let on_change = q.query_map([id], |r| r.get(0))?.collect::<Result<_, _>>()?;
        let (widgets, widget_notes) = read_widgets(conn, Owner::Takeover(id), &names)?;
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

    let mut q =
        conn.prepare("SELECT name, argv, enabled, note FROM source ORDER BY position, id")?;
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

fn read_settings(conn: &Connection) -> Result<(Settings, Option<Weather>, String), StoreError> {
    let row = conn.query_row(
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
    conn: &Connection,
    owner: Owner,
    pages: &HashMap<i64, String>,
) -> Result<(Vec<Widget>, Vec<String>), StoreError> {
    let (column, id) = owner.column();
    let mut q = conn.prepare(&format!(
        "SELECT id, type, width, props, note FROM widget WHERE {column} = ?1 ORDER BY position, id"
    ))?;
    let rows: Vec<(i64, String, u32, String, String)> = q
        .query_map([id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?
        .collect::<Result<_, _>>()?;
    let mut actions =
        conn.prepare("SELECT gesture, kind, target_page, args FROM action WHERE widget_id = ?1")?;
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

fn bump(conn: &Connection) -> Result<(), StoreError> {
    conn.execute("UPDATE revision SET n = n + 1", [])?;
    Ok(())
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

pub(crate) fn write_layout(
    conn: &Connection,
    layout: LayoutId,
    config: &Config,
    notes: &Notes,
) -> Result<(), StoreError> {
    // Pages first, so actions can point to them by ID.
    let mut page_rows = Vec::new();
    for (i, page) in config.pages.iter().enumerate() {
        conn.execute(
            "INSERT INTO page (layout_id, position, name, note) VALUES (?1, ?2, ?3, ?4)",
            params![layout, i as i64, page.name, notes.page(i).note],
        )?;
        page_rows.push(conn.last_insert_rowid());
    }
    let pages = edit::page_ids(conn, layout)?;
    for (i, (page, id)) in config.pages.iter().zip(&page_rows).enumerate() {
        for (j, widget) in page.widgets.iter().enumerate() {
            let note = notes.page(i).widget(j);
            edit::put_widget(conn, None, Row::Page(*id), j as i64, widget, note, &pages)?;
        }
    }
    for (i, takeover) in config.takeovers.iter().enumerate() {
        let id = edit::put_takeover(conn, None, layout, takeover, &notes.takeover(i).note)?;
        conn.execute(
            "UPDATE takeover SET position = ?2 WHERE id = ?1",
            params![id, i as i64],
        )?;
        for (j, widget) in takeover.widgets.iter().enumerate() {
            let note = notes.takeover(i).widget(j);
            edit::put_widget(
                conn,
                None,
                Row::Takeover(id),
                j as i64,
                widget,
                note,
                &pages,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn write_settings(
    conn: &Connection,
    config: &Config,
    note: &str,
    active: LayoutId,
) -> Result<(), StoreError> {
    let s = serde_json::to_value(&config.settings).map_err(corrupt)?;
    let text = |key: &str| s[key].as_str().unwrap_or_default().to_string();
    let w = config.weather.as_ref();
    conn.execute(
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

fn write_sources(conn: &Connection, sources: &[Source], notes: &Notes) -> Result<(), StoreError> {
    conn.execute("DELETE FROM source", [])?;
    for (i, source) in sources.iter().enumerate() {
        edit::put_source(conn, None, source, i as i64, notes.source(i))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::config::Action;

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
    fn ids_line_up_with_the_loaded_layout() {
        let (mut store, id) = with_default();
        let (config, _) = store.load(id).unwrap();
        let ids = store.ids(id).unwrap();
        assert_eq!(ids.pages.len(), config.pages.len());
        assert_eq!(ids.takeovers.len(), config.takeovers.len());
        for (row, page) in ids.pages.iter().zip(&config.pages) {
            assert_eq!(row.widgets.len(), page.widgets.len());
        }
        // Moving the first widget to the end moves its ID with it.
        let first = &ids.pages[0];
        let moved = first.widgets[0];
        store
            .edit(&Edit::MoveWidget {
                widget: moved,
                on: None,
                to: first.widgets.len() - 1,
            })
            .unwrap();
        let (after, _) = store.load(id).unwrap();
        let ids_after = store.ids(id).unwrap();
        assert_eq!(ids_after.pages[0].widgets.last(), Some(&moved));
        assert_eq!(
            after.pages[0].widgets.last(),
            config.pages[0].widgets.first()
        );
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

    fn id(store: &Store, sql: &str) -> i64 {
        store.conn.query_row(sql, [], |r| r.get(0)).unwrap()
    }

    fn page_named(store: &Store, name: &str) -> i64 {
        id(store, &format!("SELECT id FROM page WHERE name = '{name}'"))
    }

    #[test]
    fn edits_apply_and_undo_redo_restore_exactly() {
        let (mut store, layout) = with_default();
        assert_eq!(
            store.undo_label().unwrap(),
            None,
            "the first import is not undoable"
        );
        let original = store.load(layout).unwrap();
        let r0 = store.revision().unwrap();

        let r1 = store
            .edit(&Edit::AddPage {
                layout: None,
                name: "Extra".into(),
                at: Some(1),
            })
            .unwrap();
        assert_eq!(r1, r0 + 1);
        let extra = page_named(&store, "Extra");
        store
            .edit(&Edit::AddWidget {
                on: Row::Page(extra),
                at: None,
                widget: Widget::Clock {
                    width: 200,
                    tap: Some(Action::Page("System".into())),
                    hold: None,
                },
            })
            .unwrap();
        let (config, _) = store.load(layout).unwrap();
        assert_eq!(config.pages[1].name, "Extra", "placed where asked");
        assert_eq!(config.pages[1].widgets.len(), 1);
        let after_two = config;

        assert_eq!(store.undo().unwrap().as_deref(), Some("Add widget"));
        assert_eq!(store.undo().unwrap().as_deref(), Some("Add page Extra"));
        assert_eq!(store.undo().unwrap(), None);
        assert_eq!(store.load(layout).unwrap(), original);
        assert_eq!(
            store.redo_label().unwrap().as_deref(),
            Some("Add page Extra")
        );
        store.redo().unwrap();
        store.redo().unwrap();
        assert_eq!(store.load(layout).unwrap().0, after_two);
        assert_eq!(
            store.revision().unwrap(),
            r0 + 6,
            "undo and redo count as changes"
        );
    }

    #[test]
    fn a_refused_edit_changes_nothing() {
        let (mut store, layout) = with_default();
        let before = store.load(layout).unwrap();
        let r = store.revision().unwrap();
        let system = page_named(&store, "System");
        let e = store
            .edit(&Edit::AddWidget {
                on: Row::Page(system),
                at: None,
                widget: Widget::Clock {
                    width: 100,
                    tap: None,
                    hold: None,
                },
            })
            .unwrap_err();
        assert!(e.to_string().contains("add up to"), "{e}");
        assert_eq!(store.load(layout).unwrap(), before);
        assert_eq!(store.revision().unwrap(), r);
        assert_eq!(store.undo_label().unwrap(), None);
    }

    #[test]
    fn a_page_a_tap_opens_cannot_be_deleted_and_the_reason_says_which() {
        let (mut store, layout) = with_default();
        let clock = page_named(&store, "Clock");
        let e = store.edit(&Edit::DeletePage { page: clock }).unwrap_err();
        assert!(
            e.to_string()
                .contains("The tap on the clock widget on page \"System\" opens page \"Clock\""),
            "{e}"
        );
        // Load is unchanged; a page nothing opens goes, and the rest close up.
        let load = page_named(&store, "Load");
        store.edit(&Edit::DeletePage { page: load }).unwrap();
        let names: Vec<String> = store
            .load(layout)
            .unwrap()
            .0
            .pages
            .into_iter()
            .map(|p| p.name)
            .collect();
        assert!(!names.contains(&"Load".to_string()), "{names:?}");
        let positions: Vec<i64> = {
            let mut q = store
                .conn
                .prepare("SELECT position FROM page ORDER BY position")
                .unwrap();
            q.query_map([], |r| r.get(0))
                .unwrap()
                .map(Result::unwrap)
                .collect()
        };
        assert_eq!(positions, (0..names.len() as i64).collect::<Vec<_>>());
    }

    #[test]
    fn resizing_an_edge_is_one_step() {
        let (mut store, layout) = with_default();
        let mut q = store
            .conn
            .prepare("SELECT w.id FROM widget w JOIN page p ON w.page_id = p.id WHERE p.name = 'System' ORDER BY w.position")
            .unwrap();
        let ids: Vec<i64> = q
            .query_map([], |r| r.get(0))
            .unwrap()
            .map(Result::unwrap)
            .collect();
        drop(q);
        let (config, _) = store.load(layout).unwrap();
        let resize = |w: &Widget, by: i32| {
            let mut w = w.clone();
            match &mut w {
                Widget::Clock { width, .. } | Widget::Media { width, .. } => {
                    *width = (*width as i32 + by) as u32
                }
                _ => unreachable!(),
            }
            w
        };
        let widgets = &config.pages[0].widgets;
        store
            .edit(&Edit::Batch {
                label: "Resize clock".into(),
                edits: vec![
                    Edit::SetWidget {
                        widget: ids[1],
                        value: resize(&widgets[1], -20),
                    },
                    Edit::SetWidget {
                        widget: ids[0],
                        value: resize(&widgets[0], 20),
                    },
                ],
            })
            .unwrap();
        let (after, _) = store.load(layout).unwrap();
        assert_eq!(after.pages[0].widgets[0].width(), widgets[0].width() + 20);
        assert_eq!(store.undo().unwrap().as_deref(), Some("Resize clock"));
        assert_eq!(store.load(layout).unwrap().0, config);
    }

    #[test]
    fn a_text_edit_keeps_the_ids_of_what_matches() {
        let (mut store, layout) = with_default();
        let system = page_named(&store, "System");
        let first_widget = id(
            &store,
            &format!("SELECT id FROM widget WHERE page_id = {system} AND position = 0"),
        );
        let (config, notes) = store.load(layout).unwrap();
        let mut text = text::write(&config, &notes);
        text = text.replace("name = \"Load\"", "name = \"Usage\"");
        text = text.replace(
            "[[page]]\nname = \"System\"",
            "# My main page.\n[[page]]\nname = \"System\"",
        );
        let (edited, edited_notes) = text::parse(&text).unwrap();
        store
            .edit(&Edit::Replace {
                layout: None,
                config: Box::new(edited.clone()),
                notes: edited_notes,
            })
            .unwrap();
        let (back, back_notes) = store.load(layout).unwrap();
        assert_eq!(back, edited);
        assert_eq!(back_notes.page(0).note, "My main page.");
        assert_eq!(
            page_named(&store, "System"),
            system,
            "matched by name: same ID"
        );
        assert_eq!(
            id(
                &store,
                &format!("SELECT id FROM widget WHERE page_id = {system} AND position = 0")
            ),
            first_widget,
            "matched by position: same ID"
        );
        assert_eq!(store.undo().unwrap().as_deref(), Some("Edit as text"));
        assert_eq!(store.load(layout).unwrap().0, config);
    }

    #[test]
    fn a_new_edit_after_undo_drops_the_redo_and_history_is_capped() {
        let (mut store, _) = with_default();
        let note = |n: usize| Edit::SetNote {
            item: Item::Settings,
            note: format!("note {n}"),
        };
        store.edit(&note(1)).unwrap();
        store.edit(&note(2)).unwrap();
        store.undo().unwrap();
        store.edit(&note(3)).unwrap();
        assert_eq!(store.redo_label().unwrap(), None);
        for n in 0..(history::KEPT as usize + 10) {
            store.edit(&note(n + 10)).unwrap();
        }
        let kept = id(&store, "SELECT count(*) FROM history");
        assert_eq!(kept, history::KEPT);
        // A change that changes nothing is not a step.
        let r = store.revision().unwrap();
        store.edit(&note(history::KEPT as usize + 19)).unwrap(); // the last one again
        assert_eq!(store.revision().unwrap(), r);
    }

    #[test]
    fn edits_are_json_with_an_op() {
        let edit: Edit =
            serde_json::from_str(r#"{"op": "rename-page", "page": 3, "name": "Time"}"#).unwrap();
        assert_eq!(
            edit,
            Edit::RenamePage {
                page: 3,
                name: "Time".into()
            }
        );
        let edit: Edit = serde_json::from_str(
            r#"{"op": "move-widget", "widget": 9, "on": {"takeover": 2}, "to": 0}"#,
        )
        .unwrap();
        assert_eq!(
            edit,
            Edit::MoveWidget {
                widget: 9,
                on: Some(Row::Takeover(2)),
                to: 0
            }
        );
        assert!(serde_json::from_str::<Edit>(r#"{"op": "rename-page", "page": 3}"#).is_err());
    }

    #[test]
    fn a_backup_opens_with_everything_in_it() {
        let (store, layout) = with_default();
        let path =
            std::env::temp_dir().join(format!("nexus-store-backup-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        store.backup(&path).unwrap();
        let copy = Store::open(&path).unwrap();
        assert_eq!(copy.load(layout).unwrap(), store.load(layout).unwrap());
        drop(copy);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
}
