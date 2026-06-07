# Nexus Open Terminology Guide

This document defines the official terminology for Nexus Open v2.0+.

---

## Core Concepts

### Plugin
A **plugin** is a data provider that supplies content to be displayed in a zone. Plugins implement a three-method interface: `Describe()` (metadata + config schema), `Configure()` (apply user settings), and `Sample()` (current data).

**Examples:**
- Clock plugin (displays current time)
- CPU Temperature plugin (monitors CPU temp)
- Weather plugin (fetches weather data)
- Media plugin (shows now playing info)

**Types of plugins:**
- **Built-in plugins**: Compiled into the host binary (e.g., `builtin:clock`)
- **External plugins**: Separate executables via RPC (e.g., `exec:./plugins/cpu-temp`)
- **Script plugins**: Interpreted scripts (future, e.g., `script:./custom.lua`)

### Zone
A **zone** is a horizontal partition of the 640x48 display that renders data from a plugin. Zones have configurable widths that must sum to 640 pixels.

**Attributes:**
- Width (80-640 pixels)
- Plugin assignment
- Refresh interval
- Text alignment (left/center/right)
- Theme overrides

### Page
A **page** is a collection of zones that can be displayed together. Users can swipe between pages or have them auto-rotate.

**Example:**
- Page 1: Weather | CPU | GPU | Clock
- Page 2: Media (wide) | Network | Clock

### Payload
A **payload** is the data structure returned by a plugin containing the information to display:
- `Primary`: Main value (e.g., "42°C")
- `Secondary`: Context (e.g., "CPU Load")
- `Spark`: Historical data for sparkline charts
- `Severity`: Visual indicator (ok/warn/crit)
- `Progress`: Progress bar value (0.0-1.0)

### Layout
A **layout** is a YAML configuration file that defines pages, zones, themes, and plugin assignments.

---

## Architecture Terms

### Host
The **host** is the main `nexus-open` process that:
- Manages zones and pages
- Launches and communicates with plugins via RPC
- Owns all rendering for visual consistency
- Sends frames to the USB device

### Compositor
The **compositor** combines multiple zone images into a single 640x48 frame.

### Renderer
A **renderer** converts a plugin's payload into a zone image (text, sparkline, progress bar).

### RPC (Remote Procedure Call)
**RPC** is the communication mechanism between the host and external plugins, implemented using HashiCorp go-plugin.

---

## Deprecated Terms (v1.0 → v2.0)

| v1.0 Term | v2.0 Term | Notes |
|-----------|-----------|-------|
| **Instrument** | **Plugin** | Data providers are now called plugins |
| **InstrumentRegistry** | **ModuleRegistry** | Registry for available plugins |
| Hardcoded layout | Zone-based layout | Configurable horizontal partitions |
| Fixed rendering | Centralized rendering | Host owns all drawing |

---

## User-Facing Terms

### Configuration UI
- "Plugin Browser" - List of available plugins
- "Layout Editor" - Visual zone configuration
- "Theme Editor" - Color and font customization

### CLI Commands
```bash
nexus-open plugin list              # List available plugins
nexus-open plugin test <name>       # Test a plugin's output
nexus-open layout validate <file>   # Validate layout config
nexus-open layout reload            # Hot reload layout
```

### Documentation
- "Writing Plugins Guide" - How to create custom plugins
- "Layout Guide" - How to configure zones and pages
- "Plugin API Reference" - Technical plugin interface docs

---

## Migration Guide (v1.0 Users)

### What Changed?
- **Instruments → Plugins**: Same concept, new name and architecture
- Old: `internal/instruments/cpu_temp.go` (hardcoded in binary)
- New: `plugins/cpu-temp/` (external RPC process) OR `internal/plugins/builtin/cpu.go`

### Why the Change?
1. **Modularity**: External plugins can be developed independently
2. **Safety**: Plugin crashes don't kill the host
3. **Flexibility**: Write plugins in any language (Go, Python, Rust, etc.)
4. **Extensibility**: Community can create and share plugins

### Compatibility
- **Breaking change**: v1.0 configs won't work with v2.0
- **Migration tool**: `nexus-open migrate --from-v1` converts old configs
- **Feature parity**: All v1.0 instruments will be available as v2.0 plugins

---

## Examples

### In Code
```go
// Plugin interface
type Plugin interface {
    Describe() (Descriptor, error)
    Configure(cfg map[string]any) error
    Sample() (Payload, error)
}

// Built-in plugin
package builtin
type ClockModule struct{}

// External plugin
// plugins/cpu-temp/main.go
```

### In Configuration
```yaml
pages:
  - name: "Dashboard"
    zones:
      - id: cpu
        width: 160
        plugin: exec:./plugins/cpu-temp  # External plugin
        refresh_ms: 2000

      - id: clock
        width: 160
        plugin: builtin:clock            # Built-in plugin
        refresh_ms: 1000
```

### In User Documentation
> "Plugins supply data to your display. You can use built-in plugins like Clock and Debug, or install community plugins from the Plugin Browser. Advanced users can write their own plugins in any language."

---

## Config Surface Hierarchy

Nexus Open has three distinct configuration layers, each with its own storage and lifecycle:

| Layer | API | Storage | Who Edits |
|-------|-----|---------|-----------|
| **Global config** | `GET/POST /api/config` | SQLite `settings` table | Flutter settings → Global tab |
| **Layout (committed)** | `GET /api/layout` | SQLite `pages`+`zones` tables | Committed by draft confirm |
| **Draft layout** | `GET/PUT /api/layout/draft` | In-memory (`DraftManager`) | Flutter Editor tab; auto-discards on idle/disconnect |

### How the draft flow works

1. Flutter opens the Editor tab → `GET /api/layout/draft` creates an in-memory draft from the committed layout.
2. Zone add/remove/patch calls update the draft and immediately reload the hardware display for live preview.
3. Clicking **Confirm** calls `POST /api/layout/commit`, which persists the draft to SQLite and discards the draft.
4. Clicking **Discard** (or navigating away / closing the WebSocket) calls `POST /api/layout/discard`, which reloads the committed layout onto the hardware.

### Plugin config vs zone config

- **Plugin config** (`/api/zones/:id/config`) stores per-zone plugin parameters (e.g., temperature units, city name). These write directly; no draft is involved.
- **Zone config** (the draft zone's `plugin_config` field) stores the same data when building a new zone through the Editor. On commit, plugin config is persisted alongside the zone.

---

## Glossary

| Term | Definition |
|------|------------|
| **Plugin** | Plugin that provides data via RPC |
| **Zone** | Horizontal partition of the display |
| **Page** | Collection of zones (swipeable) |
| **Payload** | Data structure from plugin to zone |
| **Layout** | Configuration file (YAML) |
| **Host** | Main nexus-open process |
| **Compositor** | Combines zones into frame |
| **Renderer** | Converts payload to image |
| **RPC** | Inter-process communication |
| **Built-in** | Compiled into host binary |
| **External** | Separate executable process |
| **Sparkline** | Small inline chart |
| **Severity** | Visual state (ok/warn/crit) |

---

**Last Updated:** 2026-06-07
**Version:** 2.0.0
