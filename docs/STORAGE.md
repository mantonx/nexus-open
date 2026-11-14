# Storage: SQLite as the source of truth

Status: **agreed** 2026-11-14 (Matt: hand-editing must be better than a
lossy export and import; several layouts; undo history in the database). Replaces `nexus.toml` as the place
settings, layouts, takeovers and sources live.

## Goals

- One store for everything the app is configured with. The daemon, the
  settings window and `nexusctl` all see the same state.
- Edits from the UI apply to the strip immediately, can be undone and redone,
  and can never leave the app in a state the daemon rejects.
- A broken or missing database never blanks the strip.
- Layouts can still be shared, imported, exported and shipped as starter
  layouts.

- Editing by hand stays first-class: as TOML in your own editor, with your
  comments kept.

Not goals: editing the database file itself, or syncing it between machines.

## Decisions

### 1. The daemon is the only writer

The daemon opens the database and is the only process that writes to it. The
settings window and `nexusctl` send edits over the control socket. They never
open the database.

Why:

- **Validation in one place.** An edit is applied inside a transaction, the
  resulting layout is checked by the same Rust rules the engine relies on, and
  the transaction commits only if they pass.
- **Instant apply.** The daemon that commits the edit hands the new layout to
  the engine at once. There is no file watching and no reload latency.
- **No cross-process notification problem.** SQLite cannot tell one process
  that another changed the database. With one writer, nothing needs telling:
  the daemon pushes each new revision to connected windows.
- **One undo history**, whoever made the edit.

The cost is that edits need the daemon running. It is a user service that
starts with the session, so this is rare. While it is unreachable, the
window keeps the edits made since (shown as pending) and sends them when it
reconnects. Edits refer to rows by ID, so they replay cleanly unless they
conflict, and a conflict is shown, not lost.

### 2. Tables for structure, JSON for each widget type's fields

Pages, widgets, takeovers, actions and sources are real tables with foreign
keys. A widget's type-specific fields (label, signal, graph, thresholds,
slider controls) are a JSON column, checked by the Rust types on every write.

Why not:

- **A table per widget type:** every new widget type would need a schema
  migration.
- **Key-value rows for every field:** hard to query, and no shape at all.
- **The whole layout as one JSON blob:** loses what the database is for: IDs,
  constraints, and references that survive renames.

### 3. Actions point to pages by ID

`tap = { page = "Clock" }` becomes a row whose `target_page` is the Clock
page's ID. Renaming a page can no longer break a tap. Deleting a page that a
tap points to is refused ("Clock is opened by the tap on System › clock;
change that first"), and the UI offers to remove those actions too.

### 4. Undo stores before-and-after snapshots

Each edit records the state before and after it, as JSON with row IDs, plus a
label for the menu ("Resize CPU"). Undo restores "before" in a transaction;
redo restores "after". A new edit drops anything undone. At about 5 KB each,
the last 200 edits take 1 MB.

The history is stored in the database, so undo survives a restart.

The alternative, SQLite's session extension (invertible changesets), records
less but adds a build feature and conflict handling. Snapshots are simpler
and cannot fail halfway.

### 5. TOML stays, as the exchange format

`nexus.toml`'s format, already reviewed and signed off, becomes the import and
export format:

- `nexusctl export > layout.toml` and `nexusctl import layout.toml`, plus the
  same in the UI.
- Starter layouts and the first-run default are bundled TOML files.
- The docs' examples stay valid.
- `nexusctl edit` uses it for editing by hand (decision 7).

The existing parser and validator stay as they are. Export needs `Serialize`
on the config types.

### 6. Where the file lives

| Platform | Path |
|---|---|
| Linux | `~/.local/share/nexus-open/nexusd.db` (`$XDG_DATA_HOME`) |
| macOS | `~/Library/Application Support/nexus-open/nexusd.db` |
| Windows | `%APPDATA%\nexus-open\nexusd.db` |

The data directory, because it is no longer a file meant for editing. Named
`nexusd.db`, because the Go app already uses `nexus.db` in both the config
and data folders. The two apps must never open each other's file.

The systemd unit gets `ReadWritePaths=%h/.local/share/nexus-open`.

### 7. Editing by hand: `nexusctl edit`, with comments kept as notes

`nexusctl edit` works like `git commit` or `kubectl edit`. It writes what is
on the strip as TOML to a temporary file and opens it in `$VISUAL` or
`$EDITOR`. When the editor closes:

- **Unchanged:** nothing happens.
- **Valid:** the daemon applies it as one edit, undoable like any other, and
  the strip updates.
- **Invalid:** the file reopens with the reason at the top as a comment, and
  your text as you left it, until it is fixed or emptied (which cancels).
- **Changed meanwhile** (in the window, or by another `nexusctl`): refused
  rather than overwriting, and the file reopens with a note saying so. Every
  state has a revision number, and the edit names the revision it started
  from.

The file holds the settings, the sources, and the layout on the strip.
`nexusctl edit --layout NAME` edits another layout.

**Comments are kept as notes.** Layouts, pages, widgets, takeovers and
sources each have a `note`. The comment lines directly above a `[settings]`,
`[[page]]`, `[[page.widget]]`, `[[takeover]]`, `[[takeover.widget]]` or
`[[source]]` line are that item's note. The next edit or export writes them
back in the same place, and the settings window shows and edits them in the
inspector. So a comment survives edits made in the window, which a text file
edited by both a person and a program cannot promise.

- Lines starting with `##` are help that the export writes at the top of the
  file (what the keys mean). They are never stored.
- A comment anywhere else (at the end of a line, or between keys) cannot be
  attached to anything, so it is not kept. `nexusctl edit` says so and names
  the line, rather than dropping it silently.

**Row IDs survive a hand edit.** The edited text is applied as changes, not
as a replacement: pages, takeovers and sources are matched by name, and
widgets by position within their page. Only what changed gets new rows, so
the window's selection and the undo history stay meaningful.

`nexusctl export` and `import` use the same format, notes included, for
sharing layouts.

## Schema (version 1)

The migrations in `crates/nexus-store/src/schema.rs` are authoritative;
this copy is for reading.

```sql
PRAGMA user_version = 1;

-- App-wide settings: one row.
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

-- A set of pages and takeovers. One is on the strip; others are starter
-- layouts, imports, or saved alternatives.
CREATE TABLE layout (
  id   INTEGER PRIMARY KEY,
  name TEXT NOT NULL UNIQUE CHECK (trim(name) <> ''),
  note TEXT NOT NULL DEFAULT ''
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

-- A widget sits on a page or on a takeover, never both.
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

-- At most one tap and one hold per widget.
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

-- Programs that publish signals. App-wide: they feed every layout.
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

CREATE TABLE history (
  id     INTEGER PRIMARY KEY,
  at     TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ')),
  label  TEXT NOT NULL,
  before TEXT NOT NULL,
  after  TEXT NOT NULL,
  undone INTEGER NOT NULL DEFAULT 0 CHECK (undone IN (0, 1))
);
```

Notes:

- **Positions have no unique constraint.** SQLite checks uniqueness row by
  row, so reordering in one `UPDATE` would collide partway through. The store
  numbers positions 0, 1, 2, … inside every transaction that touches them.
- **Rules that span rows stay in Rust:** widths fitting 640 px, a page of only
  sliders, a run slider needing `{value}`, a takeover having widgets. The
  constraints above are a second line of defence, not the rulebook.
- Every connection sets `foreign_keys = ON` (SQLite defaults it off),
  `journal_mode = WAL`, `synchronous = NORMAL` and `busy_timeout = 2000`.

## How an edit flows

```
UI / nexusctl ── edit {op} ──► daemon
                                 BEGIN
                                 apply op to rows
                                 load rows → Config
                                 Config::validate ── fails ──► ROLLBACK, reply with the reason
                                 record history (before, after, label)
                                 COMMIT
                                 engine.handle(Input::Config)   ← strip updates
                                 push revision N+1 to every watcher
```

The engine keeps working on the `Config` struct it has today. Loading from
rows replaces parsing TOML, and nothing in the engine, layout or gesture code
changes. Page actions are turned back into names when the `Config` is built.

### Edits

A typed list, sent as JSON. Every edit names rows by ID:

- **Pages:** add, rename, move, delete.
- **Widgets:** add, change fields, move (within a row or to another), resize
  an edge (the left widget grows by what the right one gives up, so the total
  stays the same), delete.
- **Actions:** set or clear a widget's tap or hold.
- **Takeovers:** add, change name, seconds or signals, delete.
- **Sources:** add, change, enable or disable, delete.
- **Settings:** change any field.
- **Layouts:** switch, duplicate, rename, delete, import.
- **Batch:** several edits under one label and one undo step, for UI gestures
  that do more than one thing.

### Control socket additions

| Request | Reply |
|---|---|
| `state` | everything, with IDs and the revision number |
| `edit {json}` | the new revision, or why it was refused |
| `undo` / `redo` | the new revision, or "nothing to undo" |
| `watch` | a stream: a line for every new revision |
| `export` | the active layout as TOML |
| `import {toml}` | a new layout (`nexusctl` reads the file and sends its text, so the sandboxed daemon never needs to read your files) |

`nexusctl reload` re-reads the database. `nexusctl source add NAME -- PROGRAM
ARGS…` covers the common command-line edit, so the external sources doc keeps
a one-line way to add one.

## Lifecycle

- **First start, no database:** create it. If `nexus.toml` exists, import it
  as the layout "Imported", then rename the file `nexus.toml.imported`
  (renamed, never deleted). Otherwise import the bundled default.
- **Schema upgrades:** `PRAGMA user_version` and an ordered list of
  migrations, each in its own transaction, with a backup taken first. A
  database from a newer version is opened read-only, and the window says
  which version is needed.
- **Backups:** `VACUUM INTO backups/nexusd-YYYY-MM-DD.db` on the first edit
  of each day; the last 7 are kept. Also before every migration.
- **A database that will not open, or fails `PRAGMA quick_check`:** the strip
  shows the bundled default, so it is never blank. The file is moved aside to
  `nexusd.db.broken-DATE` and never overwritten. `nexusctl doctor` and the
  window report it and offer the newest backup.
- **Concurrency:** one connection, owned by the daemon thread. An edit is a
  few small rows, and with WAL and `synchronous = NORMAL` a commit does not
  wait on the disk, so it takes well under a millisecond. Measured during
  the build, not assumed.

## Crates

- `rusqlite` 0.42 with `bundled`, which compiles SQLite into the binary: the
  same version on every platform, nothing to install on Windows or macOS,
  and about 1 MB larger. Distribution packages that prefer the system SQLite
  can turn `bundled` off.
- A new crate, `nexus-store`: schema, migrations, rows ↔ `Config`, edits,
  history, import and export, backups. It is tested against in-memory
  databases, with no daemon.

## Build order

1. `nexus-store`, on its own, with tests: the default layout and Matt's file
   round-trip (TOML → rows → `Config` and TOML, with nothing lost); every
   edit either commits a valid layout or changes nothing; undo and redo
   restore exactly; deleting a page a tap points to is refused.
2. The daemon switches to the store: first-run import, the socket requests,
   `nexusctl` export, import, undo and redo, and the systemd path. Deployed on
   Matt's machine, his `nexus.toml` imported and checked page by page on the
   strip.
3. The settings window edits through the socket (Phase 4 continues from
   here).

## Decided with Matt

1. Hand-editing must be better than a lossy export and import: decision 7.
2. Several layouts from the start.
3. Undo history lives in the database and survives restarts.
