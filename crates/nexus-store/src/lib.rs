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

use nexus_core::config::{
    AlbumArt, Config, ConfigError, MediaConfig, NewsConfig, Page, Settings, Source, Takeover,
    Weather, Widget, colour_text,
};
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
    let news = read_news(conn)?;
    let media = read_media(conn)?;
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
        "SELECT id, name, seconds, note, align, width FROM takeover WHERE layout_id = ?1 ORDER BY position, id",
    )?;
    type TakeoverRow = (i64, String, f64, String, String, Option<u32>);
    let takeover_rows: Vec<TakeoverRow> = q
        .query_map([layout], |r| {
            Ok((
                r.get(0)?,
                r.get(1)?,
                r.get(2)?,
                r.get(3)?,
                r.get(4)?,
                r.get(5)?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let mut takeovers = Vec::new();
    for (id, name, seconds, note, align, width) in takeover_rows {
        let align = serde_json::from_value(Json::String(align)).map_err(corrupt)?;
        let mut q = conn.prepare(
            "SELECT signal FROM takeover_signal WHERE takeover_id = ?1 ORDER BY position",
        )?;
        let on_change = q.query_map([id], |r| r.get(0))?.collect::<Result<_, _>>()?;
        let (widgets, widget_notes) = read_widgets(conn, Owner::Takeover(id), &names)?;
        takeovers.push(Takeover {
            name,
            on_change,
            seconds,
            align,
            width,
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
        news,
        media,
        takeovers,
        sources,
    };
    config.validate()?;
    Ok((config, notes))
}

fn read_settings(conn: &Connection) -> Result<(Settings, Option<Weather>, String), StoreError> {
    let row = conn.query_row(
        "SELECT font, temperature, clock, weather_location, weather_latitude, weather_longitude, note, accent,
                night_from, night_to, idle_dim FROM settings",
        [],
        |r| {
            let night = match (
                r.get::<_, Option<String>>(8)?,
                r.get::<_, Option<String>>(9)?,
            ) {
                (Some(from), Some(to)) => json!({ "from": from, "to": to }),
                _ => Json::Null,
            };
            Ok((
                json!({
                    "font": r.get::<_, String>(0)?,
                    "temperature": r.get::<_, String>(1)?,
                    "clock": r.get::<_, String>(2)?,
                    "accent": r.get::<_, Option<String>>(7)?,
                    "night": night,
                    "idle_dim": r.get::<_, bool>(10)?,
                }),
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

/// The `[news]` section, if the settings row has one.
fn read_news(conn: &Connection) -> Result<Option<NewsConfig>, StoreError> {
    let (feeds, rotate, max_age): (Option<String>, Option<f64>, Option<f64>) = conn.query_row(
        "SELECT news_feeds, news_rotate, news_max_age FROM settings",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    let Some(feeds) = feeds else {
        return Ok(None);
    };
    let seconds = |s: Option<f64>, default| {
        s.map_or(Ok(default), |s| {
            Duration::try_from_secs_f64(s).map_err(|e| corrupt(format!("news: {e}")))
        })
    };
    let defaults = NewsConfig::default();
    Ok(Some(NewsConfig {
        feeds: serde_json::from_str(&feeds).map_err(corrupt)?,
        rotate: seconds(rotate, defaults.rotate)?,
        max_age: seconds(max_age, defaults.max_age)?,
    }))
}

fn read_media(conn: &Connection) -> Result<MediaConfig, StoreError> {
    let album_art: String = conn.query_row("SELECT album_art FROM settings", [], |r| r.get(0))?;
    Ok(MediaConfig {
        album_art: AlbumArt::from_name(&album_art)
            .ok_or_else(|| corrupt(format!("album_art \"{album_art}\"")))?,
    })
}

/// Writes the `[media]` section into the settings row.
pub(crate) fn write_media(conn: &Connection, media: &MediaConfig) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE settings SET album_art = ?1",
        [media.album_art.name()],
    )?;
    Ok(())
}

/// Writes the `[news]` section into the settings row; `None` clears it.
pub(crate) fn write_news(conn: &Connection, news: Option<&NewsConfig>) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE settings SET news_feeds = ?1, news_rotate = ?2, news_max_age = ?3",
        params![
            news.map(|n| json!(n.feeds).to_string()),
            news.map(|n| n.rotate.as_secs_f64()),
            news.map(|n| n.max_age.as_secs_f64()),
        ],
    )?;
    Ok(())
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
    if found == known {
        return Ok(());
    }
    // A migration may rebuild a table others point to, which with foreign
    // keys on would cascade deletes through them. They are switched off
    // around the migrations (it cannot be done inside a transaction), and
    // each migration must leave every reference whole before it commits.
    let keys: bool = conn.pragma_query_value(None, "foreign_keys", |r| r.get(0))?;
    conn.pragma_update(None, "foreign_keys", false)?;
    let result = (|| {
        for (i, sql) in schema::MIGRATIONS.iter().enumerate().skip(found) {
            let tx = conn.transaction()?;
            tx.execute_batch(sql)?;
            let broken: i64 =
                tx.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |r| {
                    r.get(0)
                })?;
            if broken > 0 {
                return Err(corrupt(format!(
                    "migration {} would leave {broken} broken references",
                    i + 1
                )));
            }
            tx.pragma_update(None, "user_version", (i + 1) as i64)?;
            tx.commit()?;
        }
        Ok(())
    })();
    conn.pragma_update(None, "foreign_keys", keys)?;
    result
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
    let night = config.settings.night.as_ref();
    conn.execute(
        "INSERT INTO settings (id, font, temperature, clock, weather_location, weather_latitude, weather_longitude, active_layout, note, accent,
                               night_from, night_to, idle_dim)
         VALUES (1, ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT (id) DO UPDATE SET
           font = excluded.font, temperature = excluded.temperature, clock = excluded.clock,
           weather_location = excluded.weather_location, weather_latitude = excluded.weather_latitude,
           weather_longitude = excluded.weather_longitude, active_layout = excluded.active_layout,
           note = excluded.note, accent = excluded.accent,
           night_from = excluded.night_from, night_to = excluded.night_to, idle_dim = excluded.idle_dim",
        params![
            text("font"),
            text("temperature"),
            text("clock"),
            w.and_then(|w| w.location.clone()),
            w.and_then(|w| w.latitude),
            w.and_then(|w| w.longitude),
            active,
            note,
            // Stored as hex, whatever name the layout gave it.
            config.settings.accent.map(colour_text),
            night.map(|n| n.from.clone()),
            night.map(|n| n.to.clone()),
            config.settings.idle_dim,
        ],
    )?;
    write_news(conn, config.news.as_ref())?;
    write_media(conn, &config.media)
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
    use nexus_core::config::{Action, Align};

    /// The layout these tests store: the bundled default as it was before
    /// the strip's redesign, kept so the tests do not follow the default.
    fn fixture() -> Config {
        Config::parse(include_str!("testdata/layout.toml")).unwrap()
    }

    fn with_default() -> (Store, LayoutId) {
        let mut store = Store::open_in_memory().unwrap();
        let config = fixture();
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
        assert_eq!(config, fixture());
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
        let mut other = fixture();
        other.settings.clock = nexus_core::config::ClockFormat::H12;
        other.pages.truncate(1);
        other.pages[0].widgets[0] = Widget::Clock {
            width: 107,
            icon: None,
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
            fixture().settings.clock,
            "settings are app-wide; a starter does not change them"
        );
        assert_eq!(store.layouts().unwrap().len(), 2);
    }

    #[test]
    fn invalid_layouts_are_refused_whole() {
        let mut store = Store::open_in_memory().unwrap();
        let mut config = fixture();
        config.pages[0].widgets.push(Widget::Clock {
            width: 600,
            icon: None,
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
                    icon: None,
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

    /// A snapshot as schema 1 had it: without the columns migrations 2 to
    /// 5 add.
    fn as_schema_1(snapshot: &str) -> String {
        let mut state: Map<String, Json> = serde_json::from_str(snapshot).unwrap();
        for (table, columns) in [
            (
                "settings",
                &[
                    "accent",
                    "night_from",
                    "night_to",
                    "idle_dim",
                    "news_feeds",
                    "news_rotate",
                    "news_max_age",
                    "album_art",
                ][..],
            ),
            ("takeover", &["align", "width"]),
        ] {
            for row in state[table].as_array_mut().unwrap() {
                for c in columns {
                    row.as_object_mut().unwrap().remove(*c);
                }
            }
        }
        Json::Object(state).to_string()
    }

    #[test]
    fn a_layout_stored_by_schema_1_loads_the_same_and_keeps_its_undo() {
        let path = std::env::temp_dir().join(format!("nexus-store-v1-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        // The default layout and one undoable rename, as rows of today.
        let (mut current, layout) = with_default();
        let before = history::snapshot(&current.conn).unwrap();
        let system = page_named(&current, "System");
        current
            .edit(&Edit::RenamePage {
                page: system,
                name: "Machine".into(),
            })
            .unwrap();
        let after = history::snapshot(&current.conn).unwrap();
        let renamed = current.load(layout).unwrap().0;
        {
            // A database the last release wrote, holding those rows.
            let conn = Connection::open(&path).unwrap();
            conn.execute_batch(schema::MIGRATIONS[0]).unwrap();
            conn.pragma_update(None, "user_version", 1).unwrap();
            conn.pragma_update(None, "foreign_keys", true).unwrap();
            history::restore(&conn, &as_schema_1(&after)).unwrap();
            history::record(
                &conn,
                "Rename page to Machine",
                &as_schema_1(&before),
                &as_schema_1(&after),
            )
            .unwrap();
        }
        let mut store = Store::open(&path).unwrap();
        let version: i64 = store
            .conn
            .pragma_query_value(None, "user_version", |r| r.get(0))
            .unwrap();
        assert_eq!(version as usize, schema::MIGRATIONS.len());
        assert_eq!(store.load(layout).unwrap().0, renamed, "loads identically");
        let broken: i64 = id(&store, "SELECT count(*) FROM pragma_foreign_key_check");
        assert_eq!(broken, 0);
        let keys: bool = store
            .conn
            .pragma_query_value(None, "foreign_keys", |r| r.get(0))
            .unwrap();
        assert!(keys, "foreign keys are on again");
        // The rebuilt widget table is still what actions point at.
        let actions = id(&store, "SELECT count(*) FROM action");
        assert!(actions > 0);
        let tapped = id(&store, "SELECT widget_id FROM action LIMIT 1");
        store.edit(&Edit::DeleteWidget { widget: tapped }).unwrap();
        assert!(
            id(&store, "SELECT count(*) FROM action") < actions,
            "cascaded"
        );
        store.undo().unwrap();
        // An undo step from before the migration still restores.
        assert_eq!(
            store.undo().unwrap().as_deref(),
            Some("Rename page to Machine")
        );
        assert_eq!(store.load(layout).unwrap().0, fixture());
        drop(store);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }

    /// A layout with every key migration 2 added.
    const NEW_KEYS: &str = r##"
        [settings]
        accent = "#FF8800"
        [[page]]
        name = "A"
        [[page.widget]]
        type = "value"
        width = 200
        label = "Load"
        signal = "cpu.load"
        crit = 90
        second = "mem.used"
        second_warn = 80
        second_crit = 95
        [[page.widget]]
        type = "key"
        width = 80
        icon = "play"
        label = "Play"
        tap = { media = "play-pause" }
        hold = { page = "A" }
        [[page.widget]]
        type = "key"
        width = 40
        [[takeover]]
        name = "Mode"
        on_change = ["mode"]
        seconds = 3
        align = "right"
        width = 200
        [[takeover.widget]]
        type = "key"
        width = 200
        label = "Focus"
    "##;

    /// A layout with every key the strip's redesign added.
    const DESIGN_KEYS: &str = r##"
        [settings]
        accent = "teal"
        night = { from = "22:30", to = "07:00" }
        idle_dim = false
        [media]
        album_art = "local"
        [[page]]
        name = "A"
        [[page.widget]]
        type = "value"
        width = 320
        label = "Fan"
        signal = "fan.rpm"
        icon = "fan"
        scale = [0, 3000]
        second = "fan.load"
        second_label = "duty"
        [[page.widget]]
        type = "key"
        width = 160
        icon = "browser"
        label = "Browser"
        sub = "Open the web"
        tap = { open = "https://example.org" }
        [[page.widget]]
        type = "clock"
        width = 160
        icon = "clock"
    "##;

    #[test]
    fn the_design_keys_round_trip_through_rows_and_text() {
        let config = Config::parse(DESIGN_KEYS).unwrap();
        let mut store = Store::open_in_memory().unwrap();
        let id = store
            .import("Design", &config, &Notes::default(), true)
            .unwrap();
        let (back, notes) = store.load(id).unwrap();
        assert_eq!(back, config);
        let text = text::write(&back, &notes);
        for key in [
            "accent = \"teal\"",
            "idle_dim = false",
            "album_art = \"local\"",
            "from = \"22:30\"",
            "scale = [",
            "second_label = \"duty\"",
            "sub = \"Open the web\"",
            "icon = \"clock\"",
        ] {
            assert!(text.contains(key), "{key} in {text}");
        }
        assert_eq!(text::parse(&text).unwrap().0, config);
        // Undone, the settings go back too.
        let mut settings = back.settings.clone();
        settings.night = None;
        settings.idle_dim = true;
        store
            .edit(&Edit::SetSettings {
                settings,
                weather: None,
            })
            .unwrap();
        assert_eq!(store.load(id).unwrap().0.settings.night, None);
        // Settings saved by a form that knows nothing of `[media]` keep it.
        assert_eq!(
            store.load(id).unwrap().0.media.album_art,
            nexus_core::config::AlbumArt::Local
        );
        store.undo().unwrap();
        assert_eq!(store.load(id).unwrap().0, config);
        store
            .edit(&Edit::SetMedia {
                media: MediaConfig {
                    album_art: AlbumArt::Off,
                },
            })
            .unwrap();
        assert_eq!(store.load(id).unwrap().0.media.album_art, AlbumArt::Off);
        store.undo().unwrap();
        assert_eq!(store.load(id).unwrap().0, config);
    }

    #[test]
    fn the_new_keys_round_trip_through_rows_and_text() {
        let config = Config::parse(NEW_KEYS).unwrap();
        let mut store = Store::open_in_memory().unwrap();
        let id = store
            .import("New", &config, &Notes::default(), true)
            .unwrap();
        let (back, notes) = store.load(id).unwrap();
        assert_eq!(back, config);
        let text = text::write(&back, &notes);
        assert!(text.contains("accent = \"#ff8800\""), "{text}");
        assert!(text.contains("align = \"right\"\nwidth = 200\n"), "{text}");
        assert_eq!(text::parse(&text).unwrap().0, config);
        // An old layout writes none of them.
        let old = text::write(&fixture(), &Notes::default());
        for key in [
            "\naccent =",
            "\nalign =",
            "\nsecond =",
            "\nsecond_",
            "type = \"key\"",
        ] {
            assert!(!old.contains(key), "{key} in\n{old}");
        }
    }

    #[test]
    fn edits_to_the_new_keys_undo_and_redo() {
        let config = Config::parse(NEW_KEYS).unwrap();
        let mut store = Store::open_in_memory().unwrap();
        let layout = store
            .import("New", &config, &Notes::default(), true)
            .unwrap();
        let ids = store.ids(layout).unwrap();
        let mut settings = config.settings.clone();
        settings.accent = None;
        store
            .edit(&Edit::SetSettings {
                settings,
                weather: None,
            })
            .unwrap();
        store
            .edit(&Edit::SetTakeover {
                takeover: ids.takeovers[0].id,
                name: "Mode".into(),
                on_change: vec!["mode".into()],
                seconds: 3.0,
                align: Align::Left,
                width: Some(240),
            })
            .unwrap();
        store
            .edit(&Edit::SetWidget {
                widget: ids.pages[0].widgets[1],
                value: Widget::Key {
                    width: 80,
                    icon: Some("pause".into()),
                    label: None,
                    sub: None,
                    tap: None,
                    hold: None,
                },
            })
            .unwrap();
        let edited = store.load(layout).unwrap().0;
        assert_eq!(edited.settings.accent, None);
        assert_eq!(
            (edited.takeovers[0].align, edited.takeovers[0].width),
            (Align::Left, Some(240))
        );
        assert!(
            matches!(&edited.pages[0].widgets[1], Widget::Key { icon: Some(i), .. } if i == "pause")
        );
        for _ in 0..3 {
            store.undo().unwrap();
        }
        assert_eq!(store.load(layout).unwrap().0, config, "undone exactly");
        for _ in 0..3 {
            store.redo().unwrap();
        }
        assert_eq!(store.load(layout).unwrap().0, edited, "redone exactly");
        // An edit sent without the new fields keeps the meaning it had.
        let old: Edit = serde_json::from_str(&format!(
            r#"{{"op": "set-takeover", "takeover": {}, "name": "Mode", "on_change": ["mode"], "seconds": 3}}"#,
            ids.takeovers[0].id
        ))
        .unwrap();
        store.edit(&old).unwrap();
        let t = &store.load(layout).unwrap().0.takeovers[0];
        assert_eq!((t.align, t.width), (Align::Full, None));
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
                    icon: None,
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

    /// A layout with the `[news]` section, a news widget and both news
    /// actions.
    const NEWS_KEYS: &str = r##"
        [news]
        feeds = ["https://news.example/rss.xml", "https://other.example/atom"]
        rotate = "20s"
        max_age = "36h"
        [[page]]
        name = "A"
        [[page.widget]]
        type = "news"
        width = 400
        tap = { news = "next" }
        hold = { news = "open" }
        [[page.widget]]
        type = "news"
        width = 200
    "##;

    #[test]
    fn news_round_trips_through_rows_and_text() {
        let config = Config::parse(NEWS_KEYS).unwrap();
        let news = config.news.clone().unwrap();
        assert_eq!(news.rotate, Duration::from_secs(20));
        let mut store = Store::open_in_memory().unwrap();
        let id = store
            .import("News", &config, &Notes::default(), true)
            .unwrap();
        let (back, notes) = store.load(id).unwrap();
        assert_eq!(back, config);
        let text = text::write(&back, &notes);
        for key in [
            "[news]",
            "rotate = \"20s\"",
            "max_age = \"36h\"",
            "type = \"news\"",
            "news = \"open\"",
        ] {
            assert!(text.contains(key), "{key} in\n{text}");
        }
        assert_eq!(text::parse(&text).unwrap().0, config);
        // A layout without news writes no section, and stores none.
        let (store, layout) = with_default();
        assert_eq!(store.load(layout).unwrap().0.news, None);
        assert!(!text::write(&fixture(), &Notes::default()).contains("[news]"));
        // No feeds is still a section: it says news is wanted, not set up.
        let mut empty = fixture();
        empty.news = Some(NewsConfig::default());
        let mut store = Store::open_in_memory().unwrap();
        let id = store.import("E", &empty, &Notes::default(), true).unwrap();
        assert_eq!(store.load(id).unwrap().0.news, Some(NewsConfig::default()));
    }

    #[test]
    fn news_is_its_own_edit_and_undoes() {
        let (mut store, layout) = with_default();
        let news = NewsConfig {
            feeds: vec!["https://news.example/rss.xml".into()],
            ..NewsConfig::default()
        };
        store
            .edit(&Edit::SetNews {
                news: Some(news.clone()),
            })
            .unwrap();
        assert_eq!(store.load(layout).unwrap().0.news.as_ref(), Some(&news));
        // A settings form that knows nothing of news leaves it alone.
        let mut settings = fixture().settings;
        settings.clock = nexus_core::config::ClockFormat::H12;
        store
            .edit(&Edit::SetSettings {
                settings,
                weather: None,
            })
            .unwrap();
        assert_eq!(store.load(layout).unwrap().0.news.as_ref(), Some(&news));
        store.undo().unwrap();
        assert_eq!(store.undo().unwrap().as_deref(), Some("Change news feeds"));
        assert_eq!(store.load(layout).unwrap().0.news, None);
        store.redo().unwrap();
        assert_eq!(store.load(layout).unwrap().0.news, Some(news));
        let edit: Edit = serde_json::from_str(
            r#"{"op": "set-news", "news": {"feeds": ["https://a.example/"], "rotate": 30}}"#,
        )
        .unwrap();
        let Edit::SetNews { news: Some(n) } = edit else {
            panic!("{edit:?}")
        };
        assert_eq!(
            (n.rotate, n.max_age),
            (Duration::from_secs(30), NewsConfig::default().max_age)
        );
    }

    #[test]
    fn a_schema_3_database_upgrades_and_takes_news() {
        let path = std::env::temp_dir().join(format!("nexus-store-v3-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let layout;
        {
            // What the last release wrote: schema 3, the fixture stored.
            let mut conn = Connection::open(&path).unwrap();
            for sql in &schema::MIGRATIONS[..3] {
                conn.execute_batch(sql).unwrap();
            }
            conn.pragma_update(None, "user_version", 3).unwrap();
            conn.pragma_update(None, "foreign_keys", true).unwrap();
            let tx = conn.transaction().unwrap();
            tx.execute("INSERT INTO layout (name) VALUES ('Default')", [])
                .unwrap();
            layout = tx.last_insert_rowid();
            write_layout(&tx, layout, &fixture(), &Notes::default()).unwrap();
            tx.execute(
                "INSERT INTO settings (id, active_layout) VALUES (1, ?1)",
                [layout],
            )
            .unwrap();
            tx.commit().unwrap();
        }
        let mut store = Store::open(&path).unwrap();
        let actions = id(&store, "SELECT count(*) FROM action");
        let (config, _) = store.load(layout).unwrap();
        assert_eq!(config.pages, fixture().pages, "loads identically");
        assert_eq!(config.news, None);
        assert!(actions > 0, "actions kept through the rebuild");
        assert_eq!(
            id(&store, "SELECT count(*) FROM pragma_foreign_key_check"),
            0
        );
        let ids = store.ids(layout).unwrap();
        let widget = ids.pages[0].widgets[0];
        let width = store.load(layout).unwrap().0.pages[0].widgets[0].width();
        store
            .edit(&Edit::SetWidget {
                widget,
                value: Widget::News {
                    width,
                    icon: None,
                    tap: Some(Action::News(nexus_core::config::NewsCommand::Next)),
                    hold: None,
                },
            })
            .unwrap();
        let (config, _) = store.load(layout).unwrap();
        assert!(matches!(
            config.pages[0].widgets[0],
            Widget::News {
                tap: Some(Action::News(_)),
                ..
            }
        ));
        drop(store);
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", path.display()));
        }
    }
}
