//! Edits: every change the settings window or `nexusctl` can make, by row
//! ID. Each runs inside the store's write transaction, which checks every
//! layout afterwards and keeps the change only if all are still valid.

use std::collections::HashMap;

use nexus_core::config::{
    Action, Align, Config, ConfigError, Settings, Source, Takeover, Weather, Widget,
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};

use crate::{LayoutId, Notes, StoreError, corrupt, load};

/// What a widget sits on, by ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Row {
    Page(i64),
    Takeover(i64),
}

/// Anything that can carry a note.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Item {
    Settings,
    Layout(LayoutId),
    Page(i64),
    Widget(i64),
    Takeover(i64),
    Source(i64),
}

/// One change. Sent as JSON: `{"op": "rename-page", "page": 3, "name":
/// "Time"}`. A missing `layout` means the one on the strip.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case", deny_unknown_fields)]
pub enum Edit {
    AddPage {
        #[serde(default)]
        layout: Option<LayoutId>,
        name: String,
        #[serde(default)]
        at: Option<usize>,
    },
    RenamePage {
        page: i64,
        name: String,
    },
    MovePage {
        page: i64,
        to: usize,
    },
    DeletePage {
        page: i64,
    },
    AddWidget {
        on: Row,
        #[serde(default)]
        at: Option<usize>,
        widget: Widget,
    },
    /// Replaces a widget's type, width, fields and actions; its ID, place
    /// and note stay.
    SetWidget {
        widget: i64,
        value: Widget,
    },
    /// To another place on its row, or onto another row of the same layout.
    MoveWidget {
        widget: i64,
        #[serde(default)]
        on: Option<Row>,
        to: usize,
    },
    DeleteWidget {
        widget: i64,
    },
    AddTakeover {
        #[serde(default)]
        layout: Option<LayoutId>,
        takeover: Takeover,
    },
    /// Everything but the widgets, which are edited as widgets.
    SetTakeover {
        takeover: i64,
        name: String,
        on_change: Vec<String>,
        seconds: f64,
        /// Missing: full, as before these existed.
        #[serde(default)]
        align: Align,
        #[serde(default)]
        width: Option<u32>,
    },
    DeleteTakeover {
        takeover: i64,
    },
    AddSource {
        source: Source,
    },
    SetSource {
        source: i64,
        value: Source,
    },
    DeleteSource {
        source: i64,
    },
    SetSettings {
        settings: Settings,
        #[serde(default)]
        weather: Option<Weather>,
    },
    SetNote {
        item: Item,
        note: String,
    },
    /// A new layout: empty but for one page, or a copy of another.
    AddLayout {
        name: String,
        #[serde(default)]
        copy_of: Option<LayoutId>,
    },
    RenameLayout {
        layout: LayoutId,
        name: String,
    },
    DeleteLayout {
        layout: LayoutId,
    },
    /// Puts a layout on the strip.
    Activate {
        layout: LayoutId,
    },
    /// The whole text of `nexusctl edit`: a layout with the settings and
    /// sources, applied as changes so that matching rows keep their IDs.
    /// Pages, takeovers and sources match by name, widgets by position.
    Replace {
        #[serde(default)]
        layout: Option<LayoutId>,
        config: Box<Config>,
        notes: Notes,
    },
    /// Several edits as one step, undone together.
    Batch {
        label: String,
        edits: Vec<Edit>,
    },
}

impl Edit {
    /// What the undo menu calls it.
    pub fn label(&self) -> String {
        match self {
            Edit::AddPage { name, .. } => format!("Add page {name}"),
            Edit::RenamePage { name, .. } => format!("Rename page to {name}"),
            Edit::MovePage { .. } => "Move page".into(),
            Edit::DeletePage { .. } => "Delete page".into(),
            Edit::AddWidget { .. } => "Add widget".into(),
            Edit::SetWidget { .. } => "Change widget".into(),
            Edit::MoveWidget { .. } => "Move widget".into(),
            Edit::DeleteWidget { .. } => "Delete widget".into(),
            Edit::AddTakeover { takeover, .. } => format!("Add takeover {}", takeover.name),
            Edit::SetTakeover { name, .. } => format!("Change takeover {name}"),
            Edit::DeleteTakeover { .. } => "Delete takeover".into(),
            Edit::AddSource { source } => format!("Add source {}", source.name),
            Edit::SetSource { value, .. } => format!("Change source {}", value.name),
            Edit::DeleteSource { .. } => "Delete source".into(),
            Edit::SetSettings { .. } => "Change settings".into(),
            Edit::SetNote { .. } => "Change note".into(),
            Edit::AddLayout { name, .. } => format!("Add layout {name}"),
            Edit::RenameLayout { name, .. } => format!("Rename layout to {name}"),
            Edit::DeleteLayout { .. } => "Delete layout".into(),
            Edit::Activate { .. } => "Switch layout".into(),
            Edit::Replace { .. } => "Edit as text".into(),
            Edit::Batch { label, .. } => label.clone(),
        }
    }
}

fn missing(what: &str, id: i64) -> StoreError {
    StoreError::Invalid(ConfigError(format!("there is no {what} {id}")))
}

fn refuse(message: String) -> StoreError {
    StoreError::Invalid(ConfigError(message))
}

pub(crate) fn apply(conn: &Connection, edit: &Edit) -> Result<(), StoreError> {
    match edit {
        Edit::AddPage { layout, name, at } => {
            let layout = layout_or_active(conn, *layout)?;
            conn.execute(
                "INSERT INTO page (layout_id, position, name) VALUES (?1, -1, ?2)",
                params![layout, name],
            )?;
            let id = conn.last_insert_rowid();
            place(conn, "page", "layout_id", layout, Some((id, *at)))?;
        }
        Edit::RenamePage { page, name } => {
            changed(
                conn.execute(
                    "UPDATE page SET name = ?2 WHERE id = ?1",
                    params![page, name],
                )?,
                "page",
                *page,
            )?;
        }
        Edit::MovePage { page, to } => {
            let layout = parent(conn, "page", "layout_id", *page)?;
            place(conn, "page", "layout_id", layout, Some((*page, Some(*to))))?;
        }
        Edit::DeletePage { page } => delete_page(conn, *page)?,
        Edit::AddWidget { on, at, widget } => {
            let pages = page_ids(conn, row_layout(conn, *on)?)?;
            let id = put_widget(conn, None, *on, -1, widget, "", &pages)?;
            let (column, owner) = owner_column(*on);
            place(conn, "widget", column, owner, Some((id, *at)))?;
        }
        Edit::SetWidget { widget, value } => {
            let (on, position, note): (Row, i64, String) = widget_row(conn, *widget)?;
            let pages = page_ids(conn, row_layout(conn, on)?)?;
            put_widget(conn, Some(*widget), on, position, value, &note, &pages)?;
        }
        Edit::MoveWidget { widget, on, to } => {
            let (from, _, _) = widget_row(conn, *widget)?;
            let on = on.unwrap_or(from);
            if on != from {
                if row_layout(conn, on)? != row_layout(conn, from)? {
                    return Err(refuse(
                        "a widget moves only within its layout; copy the layout instead".into(),
                    ));
                }
                let (page, takeover) = owner_ids(on);
                conn.execute(
                    "UPDATE widget SET page_id = ?2, takeover_id = ?3 WHERE id = ?1",
                    params![widget, page, takeover],
                )?;
                let (column, owner) = owner_column(from);
                place(conn, "widget", column, owner, None)?;
            }
            let (column, owner) = owner_column(on);
            place(conn, "widget", column, owner, Some((*widget, Some(*to))))?;
        }
        Edit::DeleteWidget { widget } => {
            let (on, _, _) = widget_row(conn, *widget)?;
            conn.execute("DELETE FROM widget WHERE id = ?1", [widget])?;
            let (column, owner) = owner_column(on);
            place(conn, "widget", column, owner, None)?;
        }
        Edit::AddTakeover { layout, takeover } => {
            let layout = layout_or_active(conn, *layout)?;
            let id = put_takeover(conn, None, layout, takeover, "")?;
            let pages = page_ids(conn, layout)?;
            for (i, widget) in takeover.widgets.iter().enumerate() {
                put_widget(conn, None, Row::Takeover(id), i as i64, widget, "", &pages)?;
            }
            place(conn, "takeover", "layout_id", layout, Some((id, None)))?;
        }
        Edit::SetTakeover {
            takeover,
            name,
            on_change,
            seconds,
            align,
            width,
        } => {
            changed(
                conn.execute(
                    "UPDATE takeover SET name = ?2, seconds = ?3, align = ?4, width = ?5 WHERE id = ?1",
                    params![takeover, name, seconds, align.id(), width],
                )?,
                "takeover",
                *takeover,
            )?;
            put_signals(conn, *takeover, on_change)?;
        }
        Edit::DeleteTakeover { takeover } => {
            let layout = parent(conn, "takeover", "layout_id", *takeover)?;
            conn.execute("DELETE FROM takeover WHERE id = ?1", [takeover])?;
            place(conn, "takeover", "layout_id", layout, None)?;
        }
        Edit::AddSource { source } => {
            put_source(conn, None, source, -1, "")?;
            let id = conn.last_insert_rowid();
            place(conn, "source", "1", 1, Some((id, None)))?;
        }
        Edit::SetSource { source, value } => {
            let (position, note): (i64, String) = conn
                .query_row(
                    "SELECT position, note FROM source WHERE id = ?1",
                    [source],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .optional()?
                .ok_or_else(|| missing("source", *source))?;
            put_source(conn, Some(*source), value, position, &note)?;
        }
        Edit::DeleteSource { source } => {
            changed(
                conn.execute("DELETE FROM source WHERE id = ?1", [source])?,
                "source",
                *source,
            )?;
            place(conn, "source", "1", 1, None)?;
        }
        Edit::SetSettings { settings, weather } => {
            let s = serde_json::to_value(settings).map_err(corrupt)?;
            let text = |key: &str| s[key].as_str().unwrap_or_default().to_string();
            let w = weather.as_ref();
            conn.execute(
                "UPDATE settings SET font = ?1, temperature = ?2, clock = ?3,
                   weather_location = ?4, weather_latitude = ?5, weather_longitude = ?6,
                   accent = ?7",
                params![
                    text("font"),
                    text("temperature"),
                    text("clock"),
                    w.and_then(|w| w.location.clone()),
                    w.and_then(|w| w.latitude),
                    w.and_then(|w| w.longitude),
                    s["accent"].as_str(),
                ],
            )?;
        }
        Edit::SetNote { item, note } => {
            let (table, id) = match item {
                Item::Settings => ("settings", 1),
                Item::Layout(id) => ("layout", *id),
                Item::Page(id) => ("page", *id),
                Item::Widget(id) => ("widget", *id),
                Item::Takeover(id) => ("takeover", *id),
                Item::Source(id) => ("source", *id),
            };
            changed(
                conn.execute(
                    &format!("UPDATE {table} SET note = ?2 WHERE id = ?1"),
                    params![id, note],
                )?,
                table,
                id,
            )?;
        }
        Edit::AddLayout { name, copy_of } => {
            conn.execute("INSERT INTO layout (name) VALUES (?1)", [name])?;
            let layout = conn.last_insert_rowid();
            match copy_of {
                Some(from) => {
                    let (config, notes) = load(conn, *from)?;
                    crate::write_layout(conn, layout, &config, &notes)?;
                }
                // A layout needs a page; start with an empty one.
                None => {
                    conn.execute(
                        "INSERT INTO page (layout_id, position, name) VALUES (?1, 0, 'Page 1')",
                        [layout],
                    )?;
                }
            }
        }
        Edit::RenameLayout { layout, name } => {
            changed(
                conn.execute(
                    "UPDATE layout SET name = ?2 WHERE id = ?1",
                    params![layout, name],
                )?,
                "layout",
                *layout,
            )?;
        }
        Edit::DeleteLayout { layout } => {
            if active(conn)? == *layout {
                return Err(refuse(
                    "the layout on the strip cannot be deleted; switch to another first".into(),
                ));
            }
            // Its actions first: they point at its pages, which refuse to go
            // while anything points at them.
            conn.execute(
                &format!("DELETE FROM action WHERE widget_id IN ({LAYOUT_WIDGETS})"),
                [layout],
            )?;
            changed(
                conn.execute("DELETE FROM layout WHERE id = ?1", [layout])?,
                "layout",
                *layout,
            )?;
        }
        Edit::Activate { layout } => {
            parent(conn, "layout", "id", *layout)?;
            conn.execute("UPDATE settings SET active_layout = ?1", [layout])?;
        }
        Edit::Replace {
            layout,
            config,
            notes,
        } => {
            let layout = layout_or_active(conn, *layout)?;
            replace(conn, layout, config, notes)?;
        }
        Edit::Batch { edits, .. } => {
            for edit in edits {
                apply(conn, edit)?;
            }
        }
    }
    Ok(())
}

/// Every widget of the layout `?1`, on its pages and its takeovers.
const LAYOUT_WIDGETS: &str = "SELECT w.id FROM widget w
     LEFT JOIN page p ON w.page_id = p.id
     LEFT JOIN takeover t ON w.takeover_id = t.id
     WHERE p.layout_id = ?1 OR t.layout_id = ?1";

fn changed(rows: usize, what: &str, id: i64) -> Result<(), StoreError> {
    if rows == 0 {
        return Err(missing(what, id));
    }
    Ok(())
}

pub(crate) fn active(conn: &Connection) -> Result<LayoutId, StoreError> {
    conn.query_row("SELECT active_layout FROM settings", [], |r| r.get(0))
        .optional()?
        .ok_or(StoreError::Empty)
}

fn layout_or_active(conn: &Connection, layout: Option<LayoutId>) -> Result<LayoutId, StoreError> {
    match layout {
        Some(id) => parent(conn, "layout", "id", id),
        None => active(conn),
    }
}

/// `column` of row `id` in `table`, which must exist.
fn parent(conn: &Connection, table: &str, column: &str, id: i64) -> Result<i64, StoreError> {
    conn.query_row(
        &format!("SELECT {column} FROM {table} WHERE id = ?1"),
        [id],
        |r| r.get(0),
    )
    .optional()?
    .ok_or_else(|| missing(table, id))
}

fn owner_column(on: Row) -> (&'static str, i64) {
    match on {
        Row::Page(id) => ("page_id", id),
        Row::Takeover(id) => ("takeover_id", id),
    }
}

fn owner_ids(on: Row) -> (Option<i64>, Option<i64>) {
    match on {
        Row::Page(id) => (Some(id), None),
        Row::Takeover(id) => (None, Some(id)),
    }
}

fn row_layout(conn: &Connection, on: Row) -> Result<LayoutId, StoreError> {
    match on {
        Row::Page(id) => parent(conn, "page", "layout_id", id),
        Row::Takeover(id) => parent(conn, "takeover", "layout_id", id),
    }
}

/// Where a widget is: its row, position and note.
fn widget_row(conn: &Connection, widget: i64) -> Result<(Row, i64, String), StoreError> {
    let (page, takeover, position, note): (Option<i64>, Option<i64>, i64, String) = conn
        .query_row(
            "SELECT page_id, takeover_id, position, note FROM widget WHERE id = ?1",
            [widget],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?
        .ok_or_else(|| missing("widget", widget))?;
    let on = match (page, takeover) {
        (Some(p), _) => Row::Page(p),
        (_, Some(t)) => Row::Takeover(t),
        _ => return Err(corrupt(format!("widget {widget} is on nothing"))),
    };
    Ok((on, position, note))
}

/// Numbers the rows of `table` where `scope` = `id` as 0, 1, 2… in their
/// current order, after putting `moving` at index `to` (the end if `None`).
fn place(
    conn: &Connection,
    table: &str,
    scope: &str,
    id: i64,
    moving: Option<(i64, Option<usize>)>,
) -> Result<(), StoreError> {
    let skip = moving.map_or(0, |(m, _)| m);
    let mut q = conn.prepare(&format!(
        "SELECT id FROM {table} WHERE {scope} = ?1 AND id <> ?2 ORDER BY position, id"
    ))?;
    let mut ids: Vec<i64> = q
        .query_map(params![id, skip], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    if let Some((m, to)) = moving {
        let at = to.unwrap_or(ids.len()).min(ids.len());
        ids.insert(at, m);
    }
    for (i, row) in ids.iter().enumerate() {
        conn.execute(
            &format!("UPDATE {table} SET position = ?2 WHERE id = ?1"),
            params![row, i as i64],
        )?;
    }
    Ok(())
}

pub(crate) fn page_ids(
    conn: &Connection,
    layout: LayoutId,
) -> Result<HashMap<String, i64>, StoreError> {
    let mut q = conn.prepare("SELECT name, id FROM page WHERE layout_id = ?1")?;
    let ids = q
        .query_map([layout], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    Ok(ids)
}

/// Writes a widget as a new row, or over row `id`, with its actions.
pub(crate) fn put_widget(
    conn: &Connection,
    id: Option<i64>,
    on: Row,
    position: i64,
    widget: &Widget,
    note: &str,
    pages: &HashMap<String, i64>,
) -> Result<i64, StoreError> {
    let Json::Object(mut fields) = serde_json::to_value(widget).map_err(corrupt)? else {
        return Err(corrupt("a widget did not serialize to an object"));
    };
    fields.retain(|_, v| !v.is_null());
    let kind = fields.remove("type").unwrap_or_default();
    let width = fields.remove("width").unwrap_or_default();
    fields.remove("tap");
    fields.remove("hold");
    let props = Json::Object(fields).to_string();
    let (page, takeover) = owner_ids(on);
    let id = match id {
        Some(id) => {
            conn.execute(
                "UPDATE widget SET page_id = ?2, takeover_id = ?3, position = ?4, type = ?5,
                   width = ?6, props = ?7, note = ?8 WHERE id = ?1",
                params![
                    id,
                    page,
                    takeover,
                    position,
                    kind.as_str(),
                    width.as_i64(),
                    props,
                    note
                ],
            )?;
            conn.execute("DELETE FROM action WHERE widget_id = ?1", [id])?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO widget (page_id, takeover_id, position, type, width, props, note)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    page,
                    takeover,
                    position,
                    kind.as_str(),
                    width.as_i64(),
                    props,
                    note
                ],
            )?;
            conn.last_insert_rowid()
        }
    };
    for (gesture, action) in [("tap", widget.tap()), ("hold", widget.hold())] {
        let Some(action) = action else { continue };
        let (kind, target, args) = match action {
            Action::Page(t) if t == "next" => ("next-page", None, json!({})),
            Action::Page(t) if t == "prev" => ("prev-page", None, json!({})),
            Action::Page(t) => {
                let page = pages.get(t).ok_or_else(|| {
                    refuse(format!(
                        "no page called \"{t}\"; use a page name, \"next\" or \"prev\""
                    ))
                })?;
                ("page", Some(*page), json!({}))
            }
            Action::Run(_) => ("run", None, json!(action)),
            Action::Open(_) => ("open", None, json!(action)),
            Action::Media(_) => ("media", None, json!(action)),
            Action::Publish { .. } => ("publish", None, json!(action)),
        };
        conn.execute(
            "INSERT INTO action (widget_id, gesture, kind, target_page, args) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, gesture, kind, target, args.to_string()],
        )?;
    }
    Ok(id)
}

pub(crate) fn put_takeover(
    conn: &Connection,
    id: Option<i64>,
    layout: LayoutId,
    takeover: &Takeover,
    note: &str,
) -> Result<i64, StoreError> {
    let id = match id {
        Some(id) => {
            conn.execute(
                "UPDATE takeover SET name = ?2, seconds = ?3, note = ?4, align = ?5, width = ?6 WHERE id = ?1",
                params![
                    id,
                    takeover.name,
                    takeover.seconds,
                    note,
                    takeover.align.id(),
                    takeover.width
                ],
            )?;
            id
        }
        None => {
            conn.execute(
                "INSERT INTO takeover (layout_id, position, name, seconds, note, align, width)
                 VALUES (?1, -1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    layout,
                    takeover.name,
                    takeover.seconds,
                    note,
                    takeover.align.id(),
                    takeover.width
                ],
            )?;
            conn.last_insert_rowid()
        }
    };
    put_signals(conn, id, &takeover.on_change)?;
    Ok(id)
}

fn put_signals(conn: &Connection, takeover: i64, signals: &[String]) -> Result<(), StoreError> {
    conn.execute(
        "DELETE FROM takeover_signal WHERE takeover_id = ?1",
        [takeover],
    )?;
    for (i, signal) in signals.iter().enumerate() {
        conn.execute(
            "INSERT INTO takeover_signal (takeover_id, signal, position) VALUES (?1, ?2, ?3)",
            params![takeover, signal, i as i64],
        )?;
    }
    Ok(())
}

pub(crate) fn put_source(
    conn: &Connection,
    id: Option<i64>,
    source: &Source,
    position: i64,
    note: &str,
) -> Result<(), StoreError> {
    let argv = json!(source.run).to_string();
    match id {
        Some(id) => conn.execute(
            "UPDATE source SET name = ?2, argv = ?3, enabled = ?4, position = ?5, note = ?6 WHERE id = ?1",
            params![id, source.name, argv, source.enabled, position, note],
        )?,
        None => conn.execute(
            "INSERT INTO source (name, argv, enabled, position, note) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![source.name, argv, source.enabled, position, note],
        )?,
    };
    Ok(())
}

/// Deletes a page, unless a tap or hold elsewhere opens it: that would
/// leave the action pointing at nothing, so the person is told which.
fn delete_page(conn: &Connection, page: i64) -> Result<(), StoreError> {
    let (layout, name): (LayoutId, String) = conn
        .query_row(
            "SELECT layout_id, name FROM page WHERE id = ?1",
            [page],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or_else(|| missing("page", page))?;
    let user: Option<(String, String, Option<String>, Option<String>)> = conn
        .query_row(
            "SELECT a.gesture, w.type, p.name, t.name FROM action a
               JOIN widget w ON a.widget_id = w.id
               LEFT JOIN page p ON w.page_id = p.id
               LEFT JOIN takeover t ON w.takeover_id = t.id
             WHERE a.target_page = ?1 AND (w.page_id IS NULL OR w.page_id <> ?1)
             LIMIT 1",
            [page],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
        )
        .optional()?;
    if let Some((gesture, widget, on_page, on_takeover)) = user {
        let place = match (on_page, on_takeover) {
            (Some(p), _) => format!("page \"{p}\""),
            (_, Some(t)) => format!("takeover \"{t}\""),
            _ => "another row".into(),
        };
        let gesture = if gesture == "hold" {
            "long press"
        } else {
            "tap"
        };
        return Err(refuse(format!(
            "The {gesture} on the {widget} widget on {place} opens page \"{name}\". Change that {gesture} first."
        )));
    }
    // Its own widgets' actions may point at itself.
    conn.execute(
        "DELETE FROM action WHERE widget_id IN (SELECT id FROM widget WHERE page_id = ?1)",
        [page],
    )?;
    conn.execute("DELETE FROM page WHERE id = ?1", [page])?;
    place(conn, "page", "layout_id", layout, None)
}

/// `nexusctl edit`: makes the layout, settings and sources match `config`,
/// keeping the IDs of rows that match.
fn replace(
    conn: &Connection,
    layout: LayoutId,
    config: &Config,
    notes: &Notes,
) -> Result<(), StoreError> {
    config.validate()?;
    // Actions are written again for every widget, after the pages exist.
    conn.execute(
        &format!("DELETE FROM action WHERE widget_id IN ({LAYOUT_WIDGETS})"),
        [layout],
    )?;

    let mut old = page_ids(conn, layout)?;
    let mut page_rows = Vec::new();
    for (i, page) in config.pages.iter().enumerate() {
        let note = &notes.page(i).note;
        let id = match old.remove(&page.name) {
            Some(id) => {
                conn.execute(
                    "UPDATE page SET position = ?2, note = ?3 WHERE id = ?1",
                    params![id, i as i64, note],
                )?;
                id
            }
            None => {
                conn.execute(
                    "INSERT INTO page (layout_id, position, name, note) VALUES (?1, ?2, ?3, ?4)",
                    params![layout, i as i64, page.name, note],
                )?;
                conn.last_insert_rowid()
            }
        };
        page_rows.push(id);
    }
    for id in old.into_values() {
        conn.execute("DELETE FROM page WHERE id = ?1", [id])?;
    }
    let pages = page_ids(conn, layout)?;
    for (i, (page, id)) in config.pages.iter().zip(&page_rows).enumerate() {
        sync_widgets(
            conn,
            Row::Page(*id),
            &page.widgets,
            &notes.page(i).widgets,
            &pages,
        )?;
    }

    let mut q = conn.prepare("SELECT name, id FROM takeover WHERE layout_id = ?1")?;
    let mut old: HashMap<String, i64> = q
        .query_map([layout], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (i, takeover) in config.takeovers.iter().enumerate() {
        let note = &notes.takeover(i).note;
        let id = put_takeover(conn, old.remove(&takeover.name), layout, takeover, note)?;
        conn.execute(
            "UPDATE takeover SET position = ?2 WHERE id = ?1",
            params![id, i as i64],
        )?;
        sync_widgets(
            conn,
            Row::Takeover(id),
            &takeover.widgets,
            &notes.takeover(i).widgets,
            &pages,
        )?;
    }
    for id in old.into_values() {
        conn.execute("DELETE FROM takeover WHERE id = ?1", [id])?;
    }

    crate::write_settings(conn, config, &notes.settings, active(conn)?)?;

    let mut q = conn.prepare("SELECT name, id FROM source")?;
    let mut old: HashMap<String, i64> = q
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?
        .collect::<Result<_, _>>()?;
    for (i, source) in config.sources.iter().enumerate() {
        put_source(
            conn,
            old.remove(&source.name),
            source,
            i as i64,
            notes.source(i),
        )?;
    }
    for id in old.into_values() {
        conn.execute("DELETE FROM source WHERE id = ?1", [id])?;
    }
    Ok(())
}

/// Makes a row's widgets match `widgets`, position by position: existing
/// rows are written over, extra ones added or removed.
fn sync_widgets(
    conn: &Connection,
    on: Row,
    widgets: &[Widget],
    notes: &[String],
    pages: &HashMap<String, i64>,
) -> Result<(), StoreError> {
    let (column, owner) = owner_column(on);
    let mut q = conn.prepare(&format!(
        "SELECT id FROM widget WHERE {column} = ?1 ORDER BY position, id"
    ))?;
    let old: Vec<i64> = q
        .query_map([owner], |r| r.get(0))?
        .collect::<Result<_, _>>()?;
    for (i, widget) in widgets.iter().enumerate() {
        let note = notes.get(i).map_or("", String::as_str);
        put_widget(conn, old.get(i).copied(), on, i as i64, widget, note, pages)?;
    }
    for id in old.iter().skip(widgets.len()) {
        conn.execute("DELETE FROM widget WHERE id = ?1", [id])?;
    }
    Ok(())
}
