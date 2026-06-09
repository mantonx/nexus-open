# Nexus Open Terminology

## Core Concepts

**Plugin** — A data provider that supplies content for a zone. Implements three methods: `Describe()` (metadata + config schema), `Configure()` (apply user settings), and `Sample()` (current data snapshot).

- *Built-in plugin* — compiled into the host binary (e.g. `builtin:clock`)
- *External plugin* — a separate Go binary launched as a subprocess over net/RPC (e.g. `exec:./plugins/cpu-temp`)

**Zone** — A horizontal partition of the 640×48 display. Each zone is assigned a plugin, a width (must sum to 640 across a page), and optional config overrides.

**Page** — A collection of zones shown together. Users swipe between pages.

**Payload** — The data struct returned by `Plugin.Sample()`:

- `Primary` — main value (e.g. `"42°C"`)
- `Secondary` — context label (e.g. `"CPU Temp"`)
- `Spark` — `[]float32` history for sparkline charts
- `Caption` — small annotation line (e.g. `"↓222K ↑221K"`)
- `Severity` — visual state: `ok` / `warn` / `crit`
- `Progress` — progress bar fill (0.0–1.0)
- `RawFrame` — pre-rendered RGBA pixels (bypasses renderer, e.g. analog clock)

**Layout** — A YAML file under `configs/layouts/` that defines pages, zones, themes, and plugin assignments. The committed layout is stored in SQLite; YAML is the source-of-truth format for shipping defaults.

**Host** — The main `nexus-open` process. Owns rendering, plugin lifecycle, USB writes, and the API server.

**Renderer** — Converts a `Payload` into a zone image (text, sparkline, progress bar, etc.).

**Compositor** — Assembles individual zone images into a single 640×48 frame.

---

## Configuration Layers

Three distinct layers, each with its own storage and lifecycle:

| Layer | API endpoint | Storage | Edited by |
| --- | --- | --- | --- |
| Global config | `GET/POST /api/config` | SQLite `settings` table | Flutter Settings → Global tab |
| Committed layout | `GET /api/layout` | SQLite `pages` + `zones` | Confirmed draft, or initial YAML import |
| Draft layout | `GET/PUT /api/layout/draft` | In-memory (`DraftManager`) | Flutter Editor tab; discarded on idle/disconnect |

**Draft flow:**

1. Flutter opens Editor → `GET /api/layout/draft` creates an in-memory draft from the committed layout.
2. Zone add/remove/patch calls update the draft and immediately reload the hardware display for live preview.
3. **Confirm** → `POST /api/layout/commit` persists the draft to SQLite, discards draft.
4. **Discard** (or navigating away / WebSocket close) → `POST /api/layout/discard` reloads committed layout.

**Plugin config vs zone config:**

- `POST /api/zones/:id/config` stores per-zone plugin parameters (e.g. temperature units, city name). Writes directly; no draft involved.
- When building a new zone through the Editor, plugin config is part of the draft zone's `plugin_config` field and is persisted on commit.
