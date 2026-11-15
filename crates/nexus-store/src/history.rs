//! Undo and redo. Every change stores the whole state before and after it,
//! as JSON rows with their IDs; undo puts "before" back and redo "after".
//! The state is a few kilobytes, so this is simpler and sturdier than
//! recording and inverting each change, and it cannot fail halfway.

use rusqlite::types::Value as Sql;
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Map, Value as Json};

use crate::{StoreError, corrupt};

/// Changes kept for undo.
pub const KEPT: i64 = 200;

/// The tables a snapshot holds, parents before children: restored in this
/// order and cleared in reverse, so foreign keys hold at every step.
const TABLES: [&str; 8] = [
    "layout",
    "settings",
    "page",
    "takeover",
    "takeover_signal",
    "widget",
    "action",
    "source",
];

/// Everything but the history and the revision, as JSON.
pub fn snapshot(conn: &Connection) -> Result<String, StoreError> {
    let mut state = Map::new();
    for table in TABLES {
        let mut q = conn.prepare(&format!("SELECT * FROM {table} ORDER BY 1, 2"))?;
        let columns: Vec<String> = q.column_names().map(str::to_string).collect();
        let rows = q
            .query_map([], |r| {
                let mut row = Map::new();
                for (i, column) in columns.iter().enumerate() {
                    let value = match r.get::<_, Sql>(i)? {
                        Sql::Null => Json::Null,
                        Sql::Integer(n) => n.into(),
                        Sql::Real(x) => x.into(),
                        Sql::Text(t) => t.into(),
                        Sql::Blob(_) => Json::Null, // no table stores blobs
                    };
                    row.insert(column.clone(), value);
                }
                Ok(Json::Object(row))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        state.insert(table.into(), Json::Array(rows));
    }
    Ok(Json::Object(state).to_string())
}

/// Replaces everything a snapshot holds with its rows.
pub fn restore(conn: &Connection, snapshot: &str) -> Result<(), StoreError> {
    let state: Map<String, Json> = serde_json::from_str(snapshot).map_err(corrupt)?;
    for table in TABLES.iter().rev() {
        conn.execute(&format!("DELETE FROM {table}"), [])?;
    }
    for table in TABLES {
        let Some(Json::Array(rows)) = state.get(table) else {
            return Err(corrupt(format!("a snapshot has no {table} rows")));
        };
        for row in rows {
            let Json::Object(row) = row else {
                return Err(corrupt("a snapshot row is not an object"));
            };
            let columns: Vec<&str> = row.keys().map(String::as_str).collect();
            let marks = vec!["?"; columns.len()].join(", ");
            let values: Vec<Sql> = row
                .values()
                .map(|v| match v {
                    Json::Null => Ok(Sql::Null),
                    Json::Number(n) => Ok(n
                        .as_i64()
                        .map(Sql::Integer)
                        .unwrap_or_else(|| Sql::Real(n.as_f64().unwrap_or_default()))),
                    Json::String(t) => Ok(Sql::Text(t.clone())),
                    _ => Err(corrupt("a snapshot value is not a column value")),
                })
                .collect::<Result<_, _>>()?;
            conn.execute(
                &format!(
                    "INSERT INTO {table} ({}) VALUES ({marks})",
                    columns.join(", ")
                ),
                rusqlite::params_from_iter(values),
            )?;
        }
    }
    Ok(())
}

/// Records a change. Anything undone is dropped: a new change starts a new
/// branch, as in every editor.
pub fn record(conn: &Connection, label: &str, before: &str, after: &str) -> Result<(), StoreError> {
    conn.execute("DELETE FROM history WHERE undone = 1", [])?;
    conn.execute(
        "INSERT INTO history (label, before, after) VALUES (?1, ?2, ?3)",
        params![label, before, after],
    )?;
    conn.execute(
        "DELETE FROM history WHERE id <= (SELECT max(id) FROM history) - ?1",
        [KEPT],
    )?;
    Ok(())
}

/// The change undo would take back: its ID, label and state before it.
pub fn last_done(conn: &Connection) -> Result<Option<(i64, String, String)>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT id, label, before FROM history WHERE undone = 0 ORDER BY id DESC LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
}

/// The change redo would make again: its ID, label and state after it.
pub fn first_undone(conn: &Connection) -> Result<Option<(i64, String, String)>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT id, label, after FROM history WHERE undone = 1 ORDER BY id LIMIT 1",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .optional()?)
}

pub fn mark(conn: &Connection, id: i64, undone: bool) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE history SET undone = ?2 WHERE id = ?1",
        params![id, undone],
    )?;
    Ok(())
}
