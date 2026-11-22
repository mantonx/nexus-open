//! The database schema, as an ordered list of migrations. A database's
//! `user_version` is the number of migrations it has had. Add new ones at
//! the end; never edit one that has shipped. docs/STORAGE.md explains the
//! design.

pub const MIGRATIONS: &[&str] = &[
    // 1: settings, layouts, pages, takeovers, widgets, actions, sources,
    // the revision counter, history.
    r#"
CREATE TABLE layout (
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE CHECK (trim(name) <> ''),
  note TEXT NOT NULL DEFAULT ''
);

CREATE TABLE settings (
  id                INTEGER PRIMARY KEY CHECK (id = 1),
  font              TEXT NOT NULL DEFAULT 'ibm-plex-sans',
  temperature       TEXT NOT NULL DEFAULT 'celsius'
                    CHECK (temperature IN ('celsius', 'fahrenheit')),
  clock             TEXT NOT NULL DEFAULT '24h' CHECK (clock IN ('24h', '12h')),
  weather_location  TEXT,
  weather_latitude  REAL CHECK (weather_latitude BETWEEN -90 AND 90),
  weather_longitude REAL CHECK (weather_longitude BETWEEN -180 AND 180),
  active_layout     INTEGER NOT NULL REFERENCES layout(id),
  note              TEXT NOT NULL DEFAULT ''
);

CREATE TABLE page (
  id        INTEGER PRIMARY KEY,
  layout_id INTEGER NOT NULL REFERENCES layout(id) ON DELETE CASCADE,
  position  INTEGER NOT NULL,
  name      TEXT NOT NULL CHECK (trim(name) <> ''),
  note      TEXT NOT NULL DEFAULT '',
  UNIQUE (layout_id, name)
);

CREATE TABLE takeover (
  id        INTEGER PRIMARY KEY,
  layout_id INTEGER NOT NULL REFERENCES layout(id) ON DELETE CASCADE,
  position  INTEGER NOT NULL,
  name      TEXT NOT NULL CHECK (trim(name) <> ''),
  seconds   REAL NOT NULL CHECK (seconds BETWEEN 0.5 AND 60),
  note      TEXT NOT NULL DEFAULT '',
  UNIQUE (layout_id, name)
);

CREATE TABLE takeover_signal (
  takeover_id INTEGER NOT NULL REFERENCES takeover(id) ON DELETE CASCADE,
  signal      TEXT NOT NULL CHECK (trim(signal) <> ''),
  position    INTEGER NOT NULL,
  PRIMARY KEY (takeover_id, signal)
) WITHOUT ROWID;

CREATE TABLE widget (
  id          INTEGER PRIMARY KEY,
  page_id     INTEGER REFERENCES page(id) ON DELETE CASCADE,
  takeover_id INTEGER REFERENCES takeover(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL,
  type        TEXT NOT NULL
              CHECK (type IN ('clock', 'value', 'rate', 'slider', 'media')),
  width       INTEGER NOT NULL CHECK (width BETWEEN 40 AND 640),
  props       TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(props)),
  note        TEXT NOT NULL DEFAULT '',
  CHECK ((page_id IS NULL) <> (takeover_id IS NULL))
);
CREATE INDEX widget_on_page     ON widget(page_id, position);
CREATE INDEX widget_on_takeover ON widget(takeover_id, position);

CREATE TABLE action (
  widget_id   INTEGER NOT NULL REFERENCES widget(id) ON DELETE CASCADE,
  gesture     TEXT NOT NULL CHECK (gesture IN ('tap', 'hold')),
  kind        TEXT NOT NULL
              CHECK (kind IN ('page', 'next-page', 'prev-page',
                              'run', 'open', 'media', 'publish')),
  target_page INTEGER REFERENCES page(id) ON DELETE RESTRICT,
  args        TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(args)),
  PRIMARY KEY (widget_id, gesture),
  CHECK ((kind = 'page') = (target_page IS NOT NULL))
) WITHOUT ROWID;
CREATE INDEX action_target ON action(target_page);

CREATE TABLE source (
  id      INTEGER PRIMARY KEY,
  name    TEXT NOT NULL UNIQUE
          CHECK (name <> '' AND name NOT GLOB '*[^A-Za-z0-9_-]*'),
  argv    TEXT NOT NULL
          CHECK (json_valid(argv) AND json_type(argv) = 'array'
                 AND json_array_length(argv) > 0),
  enabled INTEGER NOT NULL DEFAULT 1 CHECK (enabled IN (0, 1)),
  position INTEGER NOT NULL,
  note    TEXT NOT NULL DEFAULT ''
);

-- Goes up by one with every change, undo and redo included, so an edit
-- can say which state it was made against.
CREATE TABLE revision (
  id INTEGER PRIMARY KEY CHECK (id = 1),
  n  INTEGER NOT NULL
);
INSERT INTO revision VALUES (1, 0);

CREATE TABLE history (
  id     INTEGER PRIMARY KEY,
  at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ')),
  label  TEXT NOT NULL,
  before TEXT NOT NULL,
  after  TEXT NOT NULL,
  undone INTEGER NOT NULL DEFAULT 0 CHECK (undone IN (0, 1))
);
"#,
    // 2: the accent colour; takeovers over one end of the strip; the key
    // widget. Every new column is optional, so what was stored before
    // reads back the same. SQLite cannot change a CHECK in place, so the
    // widget table is rebuilt as its docs describe, with foreign keys off
    // (`migrate` does that) so its actions are not cascaded away.
    r#"
ALTER TABLE settings ADD COLUMN accent TEXT
  CHECK (accent IS NULL OR accent GLOB '#[0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f][0-9a-f]');

ALTER TABLE takeover ADD COLUMN align TEXT NOT NULL DEFAULT 'full'
  CHECK (align IN ('full', 'left', 'right'));
ALTER TABLE takeover ADD COLUMN width INTEGER CHECK (width BETWEEN 40 AND 640);

CREATE TABLE widget_2 (
  id          INTEGER PRIMARY KEY,
  page_id     INTEGER REFERENCES page(id) ON DELETE CASCADE,
  takeover_id INTEGER REFERENCES takeover(id) ON DELETE CASCADE,
  position    INTEGER NOT NULL,
  type        TEXT NOT NULL
              CHECK (type IN ('clock', 'value', 'rate', 'slider', 'media', 'key')),
  width       INTEGER NOT NULL CHECK (width BETWEEN 40 AND 640),
  props       TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(props)),
  note        TEXT NOT NULL DEFAULT '',
  CHECK ((page_id IS NULL) <> (takeover_id IS NULL))
);
INSERT INTO widget_2 (id, page_id, takeover_id, position, type, width, props, note)
  SELECT id, page_id, takeover_id, position, type, width, props, note FROM widget;
DROP TABLE widget;
ALTER TABLE widget_2 RENAME TO widget;
CREATE INDEX widget_on_page     ON widget(page_id, position);
CREATE INDEX widget_on_takeover ON widget(takeover_id, position);
"#,
];
