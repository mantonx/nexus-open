-- schema.sql — final DB shape after all migrations have run.
-- This file is read by sqlc to generate type-safe query code.
-- Do NOT run this directly; migrations in store.go manage schema changes.

CREATE TABLE IF NOT EXISTS schema_version (
    version INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS pages (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT    NOT NULL,
    ord  INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS zones (
    id           TEXT    PRIMARY KEY,
    page_id      INTEGER NOT NULL REFERENCES pages(id) ON DELETE CASCADE,
    ord          INTEGER NOT NULL DEFAULT 0,
    width_px     INTEGER NOT NULL CHECK(width_px >= 80 AND width_px <= 640),
    plugin       TEXT    NOT NULL DEFAULT 'builtin:placeholder',
    refresh_ms   INTEGER NOT NULL DEFAULT 2000,
    align        TEXT    NOT NULL DEFAULT 'center',
    config_json  TEXT    NOT NULL DEFAULT '{}',
    theme_json   TEXT    NOT NULL DEFAULT '{}',
    on_tap       TEXT    NOT NULL DEFAULT '',
    choices_json TEXT    NOT NULL DEFAULT '[]'
);

CREATE INDEX IF NOT EXISTS zones_page_ord ON zones(page_id, ord);

CREATE TABLE IF NOT EXISTS payload_cache (
    zone_id    TEXT    PRIMARY KEY,
    plugin_id  TEXT    NOT NULL,
    payload    TEXT    NOT NULL,
    fetched_at INTEGER NOT NULL
);
