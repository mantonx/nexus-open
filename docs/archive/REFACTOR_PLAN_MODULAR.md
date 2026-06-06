# Modular Zone-Based Architecture Refactor

**Project:** Nexus Open - Modular Dashboard System
**Version:** 2.0.0 (Major Refactor)
**Created:** 2025-10-12
**Status:** 📋 PLANNING

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Current vs Future Architecture](#current-vs-future-architecture)
3. [Refactor Objectives](#refactor-objectives)
4. [Zone System Design](#zone-system-design)
5. [Plugin Plugin System](#plugin-plugin-system)
6. [Implementation Phases](#implementation-phases)
7. [Technical Specifications](#technical-specifications)
8. [Migration Strategy](#migration-strategy)
9. [Success Criteria](#success-criteria)
10. [Timeline & Resources](#timeline--resources)

---

## Executive Summary

### Problem Statement

The current Nexus Open architecture uses **hardcoded UI rendering** with fixed layouts and tightly coupled plugin logic. This limits:
- User customization (can't rearrange or add plugins)
- Developer extensibility (can't create third-party plugins)
- Layout flexibility (640x48 screen is underutilized)
- Visual consistency (each plugin renders differently)

### Solution: Zone-Based Modular System

Replace the monolithic rendering system with:
1. **Zone-based layout** - Configurable horizontal partitions (JSON/YAML)
2. **Plugin architecture** - Plugins as isolated RPC processes (go-plugin)
3. **Centralized rendering** - Host owns all drawing for visual consistency
4. **Hot reload** - Layout/plugin changes without restart
5. **Multi-page support** - Swipeable pages with different zone configurations

### Key Benefits

| Benefit | Impact |
|---------|--------|
| **User Experience** | Drag-and-drop zone configuration, unlimited layouts |
| **Developer Experience** | Write plugins in any language, simple RPC contract |
| **Safety** | Plugin crashes don't kill host, sandboxed execution |
| **Performance** | Independent refresh rates per zone, efficient caching |
| **Consistency** | Unified visual language, pixel-perfect typography |

---

## Current vs Future Architecture

### Current (v1.0) - Monolithic Rendering

```
┌─────────────────────────────────────────────────┐
│   Hardcoded Layout (draw.go - 416 lines)       │
│                                                 │
│   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐ │
│   │  CPU   │ │  GPU   │ │Weather │ │ Clock  │ │
│   │ Temp   │ │ Temp   │ │  Data  │ │ Time   │ │
│   └────────┘ └────────┘ └────────┘ └────────┘ │
│                                                 │
│   Each plugin renders itself                    │
│   Different fonts, styles, alignments           │
└─────────────────────────────────────────────────┘
```

**Problems:**
- ❌ Fixed 4-zone layout (160px each)
- ❌ All plugins in one binary (can't add new ones)
- ❌ Inconsistent visual styling
- ❌ No user configuration
- ❌ Restart required for any change

### Future (v2.0) - Zone-Based Plugin System

```
┌─────────────────────────────────────────────────────────────┐
│   Configuration Layer (layout.yaml)                         │
│   zones: [160, 200, 120, 160]  ← User-configurable         │
└─────────────────────────────────────────────────────────────┘
                         ↓
┌─────────────────────────────────────────────────────────────┐
│   Zone Manager (host process)                               │
│                                                              │
│   ┌──────────┐ ┌────────────┐ ┌────────┐ ┌──────────┐     │
│   │  Zone 0  │ │   Zone 1   │ │ Zone 2 │ │  Zone 3  │     │
│   │  160px   │ │   200px    │ │ 120px  │ │  160px   │     │
│   └────┬─────┘ └──────┬─────┘ └────┬───┘ └─────┬────┘     │
│        │              │             │            │          │
│        ↓ RPC          ↓ RPC         ↓ RPC        ↓ RPC     │
└─────────────────────────────────────────────────────────────┘
         │              │             │            │
    ┌────┴───┐    ┌────┴────┐   ┌────┴───┐   ┌───┴─────┐
    │Weather │    │  Media  │   │  CPU   │   │  Clock  │
    │ Plugin │    │ Plugin  │   │ Plugin │   │ Plugin  │
    │(plugin)│    │(plugin) │   │(plugin)│   │(builtin)│
    └────────┘    └─────────┘   └────────┘   └─────────┘
         ↓              ↓             ↓            ↓
    Returns         Returns       Returns      Returns
    Payload         Payload       Payload      Payload
```

**Improvements:**
- ✅ Flexible zone widths (must sum to 640px)
- ✅ Plugins as plugins (exec processes via RPC)
- ✅ Consistent rendering (host owns all drawing)
- ✅ Hot reload (config changes apply live)
- ✅ Isolated failures (plugin crash ≠ system crash)

---

## Refactor Objectives

### Primary Goals

1. **Modularity**
   - Decouple plugins from host rendering
   - Enable third-party plugin development
   - Support multiple plugin implementations (Go, Python, Rust, etc.)

2. **Configurability**
   - User-editable layout files (YAML/JSON)
   - Zone dimensions, ordering, and plugin assignments
   - Per-zone refresh rates and styling

3. **Safety**
   - Process isolation for plugins (go-plugin RPC)
   - Timeout handling (unresponsive plugins)
   - Graceful degradation (show placeholder on failure)

4. **Performance**
   - Independent refresh rates per zone (CPU: 2s, Clock: 1s, Weather: 5min)
   - Render loop decoupled from data sampling (30-60 FPS)
   - Efficient payload caching

5. **User Experience**
   - Visual consistency across all zones
   - Smooth transitions (fade/slide on data change)
   - Touch interactions (tap to cycle, swipe to page)

### Non-Goals (Future Work)

- ❌ Web-based drag-and-drop editor (v2.1+)
- ❌ Cloud plugin marketplace (v3.0+)
- ❌ Scripting language for plugins (future)
- ❌ Multi-device support (future)

---

## Zone System Design

### Screen Constraints

| Property | Value | Notes |
|----------|-------|-------|
| **Resolution** | 640×48 px | 13:1 ultra-wide ribbon |
| **Baseline Grid** | 6px | Typography alignment |
| **Safe Padding** | 4px H, 2px V | Per-zone borders |
| **Touch Targets** | 160×48 px | Full-zone taps preferred |
| **Gutters** | 1-4px | Vertical separators (optional) |

### Zone Geometry

Zones partition the 640px width horizontally. Each zone has:

```yaml
zones:
  - id: weather          # Unique identifier
    width: 160           # Pixels (required)
    x: 0                 # Offset (auto-computed if omitted)
    plugin: weather      # Plugin endpoint
    refresh_ms: 5000     # Sampling interval
    align: left          # left | center | right
    theme_override:      # Optional per-zone styling
      accent: "#00C8FF"
```

**Constraints:**
- Sum of all zone widths **must equal 640px**
- Minimum zone width: **80px** (readability)
- Maximum zones per page: **8** (practical limit)

### Example Configurations

**4 Equal Zones (Default)**
```yaml
zones:
  - { id: weather, width: 160, plugin: weather }
  - { id: cpu,     width: 160, plugin: cpu }
  - { id: gpu,     width: 160, plugin: gpu }
  - { id: clock,   width: 160, plugin: clock }
```

**Asymmetric Layout**
```yaml
zones:
  - { id: media,   width: 320, plugin: media }   # Wide media player
  - { id: network, width: 160, plugin: network }
  - { id: clock,   width: 160, plugin: clock }
```

**Single Full-Width Zone**
```yaml
zones:
  - { id: main, width: 640, plugin: dashboard }
```

### Zone Content Model

Each zone renders a **standard card** with consistent structure:

```
┌──────────────────────────┐
│ 📊 [Icon]   [Primary]    │  ← 14-16px bold, accent color
│             [Secondary]  │  ← 10px regular, muted color
│ ▁▂▃▄▅▆▇█ [Sparkline]    │  ← 2px bars, bottom-aligned
└──────────────────────────┘
```

**Layout Layers:**
1. **Icon** (optional): 12-16px, left-aligned
2. **Primary**: Main metric (e.g., "42°C", "↓58 MB/s")
3. **Secondary**: Label or context (e.g., "CPU Load 31%", "Albany ☀️")
4. **Sparkline** (optional): Normalized 0-1 values, rendered as bars

**Text Alignment:**
- `left`: Icon + text flush left (e.g., weather)
- `center`: Centered metric (e.g., temperature)
- `right`: Right-aligned value (e.g., clock)

**Severity Colors:**
| Severity | Color | Usage |
|----------|-------|-------|
| `ok` | Accent (#00C8FF) | Normal operation |
| `warn` | Yellow (#FFB020) | Warning threshold |
| `crit` | Red (#FF4444) | Critical state |

### Zone Lifecycle

```
┌─────────────────────────────────────────────────────┐
│  Initialization                                      │
│  1. Parse layout.yaml                                │
│  2. Validate zone widths (sum == 640)                │
│  3. Launch plugin plugins via go-plugin              │
│  4. Subscribe to config changes (hot reload)         │
└─────────────────────────────────────────────────────┘
                      ↓
┌─────────────────────────────────────────────────────┐
│  Sampling Loop (per zone, independent timers)        │
│  Every refresh_ms:                                   │
│    1. Call plugin.Sample() via RPC                   │
│    2. Receive Payload{Primary, Secondary, Spark...}  │
│    3. Cache payload with timestamp                   │
│    4. Trigger render update                          │
└─────────────────────────────────────────────────────┘
                      ↓
┌─────────────────────────────────────────────────────┐
│  Render Loop (30-60 FPS, decoupled from sampling)    │
│  Every frame:                                        │
│    1. Draw background + gutters                      │
│    2. For each zone:                                 │
│       - Fetch cached payload                         │
│       - Render card (icon, text, sparkline)          │
│       - Apply transitions if payload changed         │
│    3. Composite to 640x48 buffer                     │
│    4. Send to USB device                             │
└─────────────────────────────────────────────────────┘
```

**Error Handling:**
- **Plugin timeout** (>500ms): Show "—" placeholder, muted style
- **Plugin crash**: Log error, restart plugin, show error icon
- **Invalid payload**: Log warning, render last valid payload
- **Config reload**: Gracefully restart affected zones

---

## Plugin Plugin System

### Plugin Interface

Plugins expose a **minimal RPC contract** via HashiCorp go-plugin:

```go
// Plugin interface (implemented by plugins)
type Plugin interface {
    // Describe returns plugin metadata
    Describe() (Descriptor, error)

    // Sample returns current data payload
    Sample() (Payload, error)
}

// Descriptor - plugin metadata
type Descriptor struct {
    Name        string   // "CPU Temperature"
    Version     string   // "1.0.0"
    Author      string   // "Nexus Team"
    Description string   // "System CPU temperature monitor"
    Icon        string   // Icon identifier (optional)
    RefreshMs   int      // Recommended refresh interval
}

// Payload - data returned by plugin
type Payload struct {
    Title     string    `json:"title"`      // Zone title (e.g., "CPU")
    Primary   string    `json:"primary"`    // Main value (e.g., "42°C")
    Secondary string    `json:"secondary"`  // Subtext (e.g., "Load 31%")
    Spark     []float32 `json:"spark"`      // Sparkline data (0.0-1.0)
    Severity  string    `json:"severity"`   // "ok" | "warn" | "crit"
    TTLms     int       `json:"ttl_ms"`     // Cache lifetime
    Icon      string    `json:"icon"`       // Icon override (optional)
}
```

### Plugin Types

#### 1. Built-in Plugins (Native)

Compiled into the host binary for critical functionality:
- **clock**: System time/date
- **placeholder**: Error/loading state
- **debug**: Zone geometry visualization

**Pros:** No RPC overhead, always available
**Cons:** Requires recompilation to update

#### 2. External Plugins (Process-Isolated)

Launched as separate processes via go-plugin:

```yaml
zones:
  - id: weather
    plugin: exec:./plugins/weather  # Executable path
    refresh_ms: 300000              # 5 minutes
```

**Pros:** Safe isolation, any language, hot-reloadable
**Cons:** RPC overhead (~1-2ms per call)

#### 3. Script Plugins (Future)

Lightweight scripts interpreted by the host:

```yaml
zones:
  - id: custom
    plugin: script:./plugins/custom.lua
```

**Pros:** No compilation, simple for basic plugins
**Cons:** Not implemented in v2.0

### Plugin Discovery

Plugins are located via:
1. **Built-in registry**: `builtin:clock`, `builtin:placeholder`
2. **Plugin directory**: `~/.config/nexus-open/plugins/`
3. **Absolute paths**: `exec:/usr/local/bin/weather-plugin`
4. **Relative paths**: `exec:./plugins/cpu` (relative to config dir)

### Example Plugin (Go)

```go
package main

import (
    "github.com/hashicorp/go-plugin"
    "nexus/pkg/plugin"
)

type CPUModule struct{}

func (m *CPUModule) Describe() (plugin.Descriptor, error) {
    return plugin.Descriptor{
        Name:      "CPU Temperature",
        Version:   "1.0.0",
        RefreshMs: 2000,
    }, nil
}

func (m *CPUModule) Sample() (plugin.Payload, error) {
    temp := getCPUTemp() // System call

    return plugin.Payload{
        Title:     "CPU",
        Primary:   fmt.Sprintf("%.1f°C", temp),
        Secondary: "Temperature",
        Severity:  getSeverity(temp),
        TTLms:     2000,
    }, nil
}

func main() {
    plugin.Serve(&plugin.ServeConfig{
        HandshakeConfig: plugin.Handshake,
        Plugins: map[string]plugin.Plugin{
            "plugin": &plugin.Plugin{Impl: &CPUModule{}},
        },
    })
}
```

### Example Plugin (Python)

```python
#!/usr/bin/env python3
import json
import sys
import psutil

def describe():
    return {
        "name": "CPU Usage",
        "version": "1.0.0",
        "refresh_ms": 2000
    }

def sample():
    cpu_percent = psutil.cpu_percent(interval=0.1)

    return {
        "title": "CPU",
        "primary": f"{cpu_percent}%",
        "secondary": "Usage",
        "severity": "crit" if cpu_percent > 90 else "warn" if cpu_percent > 70 else "ok",
        "ttl_ms": 2000
    }

if __name__ == "__main__":
    command = sys.argv[1] if len(sys.argv) > 1 else "sample"

    if command == "describe":
        print(json.dumps(describe()))
    elif command == "sample":
        print(json.dumps(sample()))
```

### Plugin Security

**Sandboxing:**
- Process isolation (separate PID, memory space)
- Resource limits (CPU quota, memory cap)
- Network restrictions (optional firewall rules)
- No filesystem access beyond working directory

**Timeout Enforcement:**
- `Describe()`: 1 second timeout
- `Sample()`: 500ms timeout
- Automatic restart on crash (max 3 retries)
- Kill unresponsive processes (SIGKILL after 2x timeout)

---

## Implementation Phases

### Phase 1: Core Zone System (Week 1-2)

**Goal:** Build zone rendering foundation without plugins

**Tasks:**
1. **Zone Manager** (`internal/zone/manager.go`)
   - Parse layout YAML/JSON
   - Validate zone widths (sum == 640)
   - Create zone instances
   - Hot reload on config change

2. **Zone Renderer** (`internal/zone/renderer.go`)
   - Render single zone from Payload
   - Text layout engine (alignment, truncation)
   - Sparkline renderer (bars/line graphs)
   - Severity color mapping

3. **Layout Engine** (`internal/layout/engine.go`)
   - Position zones horizontally
   - Draw gutters/separators
   - Composite zones to 640x48 buffer
   - Transition effects (fade/slide)

4. **Theme System** (`internal/theme/theme.go`)
   - Global theme (bg, fg, muted, accent)
   - Per-zone overrides
   - Severity palette
   - Font loading (DejaVu Sans, Font Awesome)

5. **Testing**
   - Unit tests for zone validation
   - Renderer output tests (golden images)
   - Layout constraint tests

**Deliverables:**
- ✅ Zone system renders mock Payloads
- ✅ Layout hot reload works
- ✅ Tests passing (>70% coverage)
- ✅ Visual consistency verified

**Example Config (Mock Plugins):**
```yaml
zones:
  - id: zone1
    width: 160
    mock_payload:
      title: "CPU"
      primary: "42°C"
      secondary: "Load 31%"
      severity: "ok"
```

---

### Phase 2: Plugin Infrastructure (Week 3-4)

**Goal:** Implement go-plugin RPC system

**Tasks:**
1. **Plugin Interface** (`pkg/plugin/interface.go`)
   - Define Plugin interface
   - Descriptor and Payload types
   - Handshake config for go-plugin

2. **Plugin Host** (`internal/plugin/host.go`)
   - Launch external processes via go-plugin
   - RPC client for Describe/Sample
   - Timeout enforcement
   - Crash recovery and restart logic

3. **Built-in Plugins** (`internal/plugins/builtin/`)
   - `clock.go`: System time plugin
   - `placeholder.go`: Error/loading display
   - `debug.go`: Zone visualization

4. **Plugin Registry** (`internal/plugin/registry.go`)
   - Discover plugins (builtin, exec paths)
   - Version checking
   - Dependency validation

5. **Example External Plugins**
   - `plugins/cpu/`: Go plugin for CPU stats
   - `plugins/weather/`: Go plugin for weather
   - `plugins/hello/`: Minimal example for docs

6. **Testing**
   - Mock plugin for testing
   - Timeout/crash scenarios
   - RPC performance benchmarks

**Deliverables:**
- ✅ go-plugin integration working
- ✅ Built-in plugins functional
- ✅ Example external plugins
- ✅ Plugin tests (isolation, crashes)
- ✅ Documentation for writing plugins

---

### Phase 3: Migrate Existing Instruments (Week 5)

**Goal:** Convert current instruments to plugins

**Tasks:**
1. **CPU Temperature Plugin** (`plugins/cpu-temp/`)
   - Port from `internal/instruments/cpu_temp.go`
   - Linux/Windows/macOS support
   - Sparkline with historical temps

2. **GPU Temperature Plugin** (`plugins/gpu-temp/`)
   - Port from `internal/instruments/gpu_temp.go`
   - nvidia-smi integration
   - Threshold severity levels

3. **Network Plugin** (`plugins/network/`)
   - Port from `internal/instruments/network.go`
   - Upload/download sparklines
   - Bytes/s formatting

4. **Weather Plugin** (`plugins/weather/`)
   - Port from `internal/instruments/weather.go`
   - Open-Meteo + Nominatim geocoding
   - Icon mapping (Font Awesome)

5. **Media Plugin** (`plugins/media/`) - New
   - MPRIS D-Bus integration (Linux)
   - Now playing track/artist
   - Progress bar sparkline

6. **System Stats Plugin** (`plugins/sysinfo/`) - New
   - RAM usage
   - Disk usage
   - Uptime display

**Deliverables:**
- ✅ All v1.0 instruments as plugins
- ✅ Feature parity with current implementation
- ✅ 2+ new plugins (media, sysinfo)
- ✅ Plugin documentation
- ✅ Default layout configs

---

### Phase 4: Page & Interaction System (Week 6)

**Goal:** Multi-page layouts and touch interactions

**Tasks:**
1. **Page Manager** (`internal/page/manager.go`)
   - Support multiple layout configs (pages)
   - Page switching (button, swipe, timer)
   - Transition animations

2. **Touch Handler Refactor** (`internal/touch/handler.go`)
   - Zone-aware tap detection
   - Swipe gesture recognition
   - Long-press support (future)
   - Debouncing and multi-touch

3. **Zone Cycling** (`internal/zone/cycle.go`)
   - Per-zone plugin choices
   - Tap to cycle through plugins
   - State persistence (remember selections)

4. **Page Transition Effects**
   - Slide left/right
   - Fade in/out
   - Configurable durations

5. **Testing**
   - Touch event simulation
   - Page switching tests
   - Gesture recognition accuracy

**Deliverables:**
- ✅ Multi-page support (3+ pages)
- ✅ Swipe navigation working
- ✅ Tap to cycle zones
- ✅ Smooth transitions (60 FPS)
- ✅ Example configs for different pages

**Example Page Config:**
```yaml
pages:
  - name: "Dashboard"
    zones:
      - { id: weather, width: 160, plugin: weather }
      - { id: cpu, width: 160, plugin: cpu-temp }
      - { id: gpu, width: 160, plugin: gpu-temp }
      - { id: clock, width: 160, plugin: clock }

  - name: "Media"
    zones:
      - { id: media, width: 320, plugin: media }
      - { id: network, width: 160, plugin: network }
      - { id: clock, width: 160, plugin: clock }
```

---

### Phase 5: Configuration UI (Week 7)

**Goal:** Flutter UI for layout configuration

**Tasks:**
1. **Layout Editor Tab** (`ui/lib/src/widgets/layout/`)
   - Visual zone editor (drag widths)
   - Plugin assignment dropdowns
   - Real-time preview (simulated display)
   - Save/load layout presets

2. **Plugin Browser** (`ui/lib/src/widgets/plugins/`)
   - List available plugins
   - Show Descriptor info (name, version, author)
   - Test plugin (run Sample() manually)
   - Install/update plugins (future)

3. **Theme Editor** (`ui/lib/src/widgets/theme/`)
   - Color pickers for theme values
   - Font size sliders
   - Gutter width controls
   - Live preview

4. **Page Manager UI**
   - Add/remove pages
   - Reorder pages
   - Set default page
   - Auto-rotate timing

5. **API Extensions**
   - `GET /api/layout`: Current layout config
   - `POST /api/layout`: Update layout
   - `GET /api/plugins`: List available plugins
   - `GET /api/plugins/{id}`: Plugin descriptor
   - `POST /api/plugins/{id}/sample`: Test plugin

**Deliverables:**
- ✅ Layout editor functional
- ✅ Plugin browser working
- ✅ Theme customization UI
- ✅ API endpoints tested
- ✅ User documentation

---

### Phase 6: Polish & Documentation (Week 8)

**Goal:** Production-ready quality

**Tasks:**
1. **Performance Optimization**
   - Profile render loop (target <16ms/frame)
   - Optimize RPC calls (batching, caching)
   - Memory leak detection
   - CPU usage benchmarks

2. **Error Handling**
   - Graceful degradation (all failure modes)
   - User-friendly error messages
   - Logging improvements
   - Recovery strategies

3. **Documentation**
   - **User Guide**: Layout creation, plugin usage
   - **Developer Guide**: Writing plugins, plugin API
   - **Plugin Catalog**: Document all built-in plugins
   - **Migration Guide**: v1.0 → v2.0 upgrade path

4. **Testing**
   - Integration tests (full pipeline)
   - Visual regression tests
   - Load testing (many plugins)
   - Device compatibility

5. **Packaging Updates**
   - Update DEB/AUR/AppImage for v2.0
   - Plugin installation paths
   - Example plugin packages
   - Migration scripts

**Deliverables:**
- ✅ <16ms render time (60 FPS)
- ✅ <50MB memory usage
- ✅ Comprehensive documentation
- ✅ 75% test coverage
- ✅ v2.0 packages ready

---

## Technical Specifications

### Directory Structure

```
nexus-open/
├── cmd/
│   ├── nexus-open/           # Main host binary
│   └── plugin-tool/          # CLI for testing plugins
├── internal/
│   ├── zone/                 # Zone manager & renderer
│   │   ├── manager.go
│   │   ├── renderer.go
│   │   ├── layout.go
│   │   └── cache.go
│   ├── plugin/               # Plugin host & registry
│   │   ├── host.go
│   │   ├── registry.go
│   │   └── rpc.go
│   ├── page/                 # Page manager
│   │   ├── manager.go
│   │   └── transitions.go
│   ├── plugins/              # Built-in plugins
│   │   ├── clock.go
│   │   ├── placeholder.go
│   │   └── debug.go
│   ├── theme/                # Theme system
│   │   ├── theme.go
│   │   └── colors.go
│   └── layout/               # Layout engine
│       ├── engine.go
│       └── compositor.go
├── pkg/
│   └── plugin/               # Public plugin interface
│       ├── interface.go
│       ├── payload.go
│       └── plugin.go
├── plugins/                  # External plugins (examples)
│   ├── cpu-temp/
│   ├── gpu-temp/
│   ├── weather/
│   ├── network/
│   ├── media/
│   └── hello/                # Minimal example
├── configs/
│   ├── layouts/
│   │   ├── default.yaml
│   │   ├── media.yaml
│   │   └── minimal.yaml
│   └── themes/
│       ├── dark.yaml
│       └── light.yaml
└── docs/
    ├── MODULE_GUIDE.md       # Writing plugins
    ├── LAYOUT_GUIDE.md       # Creating layouts
    └── MIGRATION_v2.md       # v1 → v2 upgrade
```

### Configuration Schema

**Main Config** (`~/.config/nexus-open/config.yaml`)
```yaml
# Device settings
device:
  vendor_id: 0x1b1c
  product_id: 0x1b8e

# Current layout and theme
display:
  layout: ~/.config/nexus-open/layouts/default.yaml
  theme: ~/.config/nexus-open/themes/dark.yaml
  fps: 30
  default_page: 0

# Plugin paths
plugins:
  search_paths:
    - ~/.config/nexus-open/plugins
    - /usr/local/lib/nexus-open/plugins
  timeout_ms: 500

# API server
api:
  enabled: true
  port: 1985
  cors: true
```

**Layout Config** (`layouts/default.yaml`)
```yaml
name: "Default Dashboard"
version: "1.0"

# Global theme (can be overridden per-zone)
theme:
  bg: "#101010"
  fg: "#EAEAEA"
  muted: "#9AA0A6"
  accent: "#00C8FF"
  gutter_px: 2
  font_size_primary: 14
  font_size_secondary: 10

# Pages (swipeable)
pages:
  - name: "Main"
    zones:
      - id: weather
        width: 160
        plugin: builtin:placeholder  # Or exec:./plugins/weather
        refresh_ms: 300000           # 5 minutes
        align: left

      - id: cpu
        width: 160
        plugin: exec:./plugins/cpu-temp
        refresh_ms: 2000
        align: center

      - id: gpu
        width: 160
        plugin: exec:./plugins/gpu-temp
        refresh_ms: 2000
        align: center
        theme_override:
          accent: "#FF6B6B"

      - id: clock
        width: 160
        plugin: builtin:clock
        refresh_ms: 1000
        align: right

  - name: "Media"
    zones:
      - id: media
        width: 320
        plugin: exec:./plugins/media
        refresh_ms: 1000
        align: left

      - id: network
        width: 160
        plugin: exec:./plugins/network
        refresh_ms: 2000
        align: center

      - id: clock
        width: 160
        plugin: builtin:clock
        refresh_ms: 1000
        align: right

# Navigation
navigation:
  swipe_enabled: true
  auto_rotate: false
  auto_rotate_interval_s: 10
```

### Plugin Payload Specification

```go
// Payload represents data from a plugin
type Payload struct {
    // Title - Zone header (optional, often omitted for space)
    Title string `json:"title,omitempty"`

    // Primary - Main value displayed (14-16px, bold)
    // Examples: "42°C", "↓58 MB/s", "Now Playing"
    Primary string `json:"primary"`

    // Secondary - Subtext or context (10px, muted)
    // Examples: "Load 31%", "Albany ☀️", "Radiohead"
    Secondary string `json:"secondary,omitempty"`

    // Spark - Sparkline data (normalized 0.0-1.0)
    // Rendered as small bars/line at bottom of zone
    // Max 60 points (1 per second for 1-minute history)
    Spark []float32 `json:"spark,omitempty"`

    // Severity - Visual severity indicator
    // Values: "ok", "warn", "crit"
    // Affects primary text color and icon color
    Severity string `json:"severity,omitempty"`

    // TTLms - Cache lifetime in milliseconds
    // Host will re-use this payload until TTL expires
    TTLms int `json:"ttl_ms,omitempty"`

    // Icon - Icon identifier (Font Awesome name or emoji)
    // Examples: "thermometer", "cloud", "🌡️"
    Icon string `json:"icon,omitempty"`

    // Progress - Progress bar value (0.0-1.0)
    // Rendered as horizontal bar (for media playback, etc.)
    Progress float32 `json:"progress,omitempty"`
}
```

**Validation Rules:**
- `Primary` is **required** (must not be empty)
- `Severity` must be one of: `ok`, `warn`, `crit` (default: `ok`)
- `Spark` values must be in range [0.0, 1.0]
- `Spark` array length must be ≤ 60
- `Progress` must be in range [0.0, 1.0]
- `TTLms` should be ≥ 100ms (reasonable minimum)

### Rendering Specification

**Zone Render Pipeline:**

```
Input: Payload + Zone Config + Theme
  ↓
┌─────────────────────────────────────┐
│ 1. Layout Calculation                │
│    - Measure text bounds             │
│    - Calculate icon position         │
│    - Compute sparkline geometry      │
└─────────────────────────────────────┘
  ↓
┌─────────────────────────────────────┐
│ 2. Draw Background                   │
│    - Fill with theme.bg              │
│    - Apply zone padding (4px H)      │
└─────────────────────────────────────┘
  ↓
┌─────────────────────────────────────┐
│ 3. Draw Icon (if present)            │
│    - Position at left edge + padding │
│    - Color based on severity         │
│    - Size: 12-16px                   │
└─────────────────────────────────────┘
  ↓
┌─────────────────────────────────────┐
│ 4. Draw Primary Text                 │
│    - Font: DejaVu Sans Bold, 14-16px│
│    - Color: theme.accent (or severity)│
│    - Alignment: zone.align           │
│    - Truncate with "…" if overflow   │
└─────────────────────────────────────┘
  ↓
┌─────────────────────────────────────┐
│ 5. Draw Secondary Text               │
│    - Font: DejaVu Sans Regular, 10px│
│    - Color: theme.muted              │
│    - Below primary, same alignment   │
└─────────────────────────────────────┘
  ↓
┌─────────────────────────────────────┐
│ 6. Draw Sparkline (if present)       │
│    - Bottom-aligned, 2px bar height  │
│    - Color: theme.accent @ 60% alpha│
│    - Normalize to zone width - 8px   │
└─────────────────────────────────────┘
  ↓
┌─────────────────────────────────────┐
│ 7. Draw Progress Bar (if present)    │
│    - Bottom-aligned, 2px height      │
│    - Color: theme.accent             │
│    - Width: progress * (zone_width-8)│
└─────────────────────────────────────┘
  ↓
Output: Zone image buffer (width × 48px)
```

**Compositor Pipeline:**

```
Input: All zone buffers + Layout config
  ↓
┌─────────────────────────────────────┐
│ 1. Create 640×48 base canvas         │
│    - Fill with theme.bg              │
└─────────────────────────────────────┘
  ↓
┌─────────────────────────────────────┐
│ 2. Position zones horizontally       │
│    - Use zone.x offsets              │
│    - Draw vertical gutters if enabled│
└─────────────────────────────────────┘
  ↓
┌─────────────────────────────────────┐
│ 3. Composite zone buffers            │
│    - Blit each zone at computed x    │
│    - Apply transition effects        │
└─────────────────────────────────────┘
  ↓
┌─────────────────────────────────────┐
│ 4. Convert to device format          │
│    - RGB888 → RGB565 (if needed)     │
│    - Byte order conversion           │
└─────────────────────────────────────┘
  ↓
Output: 640×48×3 byte array (RGB) → USB
```

### Performance Targets

| Metric | Target | Measurement |
|--------|--------|-------------|
| **Render Time** | <16ms/frame | 60 FPS capability |
| **RPC Latency** | <2ms | go-plugin overhead |
| **Plugin Sample** | <500ms | Timeout threshold |
| **Config Reload** | <1s | Hot reload speed |
| **Memory Usage** | <75MB | Including all plugins |
| **CPU Idle** | <3% | No active updates |
| **CPU Active** | <15% | Full render + sampling |

### Font Specification

**Primary Fonts:**
- **DejaVu Sans Mono** (11px, 14px) - Metrics and code
- **DejaVu Sans** (10px, 14px, 16px) - General UI text
- **Font Awesome Free** (10px, 14px, 16px) - Icons

**Typography Rules:**
- All text rendered on **integer pixel baselines** (6px grid)
- Anti-aliasing: Grayscale (not subpixel, for consistency)
- Kerning: Enabled for proportional fonts
- Hinting: Auto-hinter for DejaVu fonts

**Icon Mapping:**
```go
var IconMap = map[string]string{
    "thermometer":  "\uf2c9",  // Temperature
    "cpu":          "\uf85a",  // Processor
    "cloud":        "\uf0c2",  // Weather
    "clock":        "\uf017",  // Time
    "network":      "\uf6ff",  // Network activity
    "warning":      "\uf071",  // Warning state
    "error":        "\uf06a",  // Error state
    "music":        "\uf001",  // Media playback
}
```

---

## Migration Strategy

### Backwards Compatibility

**v1.0 → v2.0 is a BREAKING CHANGE**

Reasons:
1. Layout system completely replaced
2. Config format changed (YAML → multi-file YAML)
3. Instruments → Plugins (different interface)
4. API endpoints extended (new routes)

**Migration Path:**

1. **Automatic Config Conversion**
   ```bash
   nexus-open migrate --from-v1 ~/.config/nexus-open/config.yaml
   ```
   - Converts v1 config to v2 layout format
   - Creates default zone layout matching v1 UI
   - Preserves theme colors and preferences

2. **Plugin Compatibility Layer** (Optional)
   - Wrapper that runs v1 instrument code as v2 plugin
   - Lower performance but avoids rewrite
   - Only for custom user instruments

3. **Side-by-Side Installation**
   - v1.x installed as `nexus-open-v1`
   - v2.x installed as `nexus-open`
   - User can test v2 before full switch

### Phased Rollout

**Alpha (Week 9):**
- Internal testing only
- Core team validates zone system
- Performance benchmarking

**Beta (Week 10-11):**
- Public beta release
- Solicit feedback on GitHub Discussions
- Bug fixes and polish

**RC (Week 12):**
- Release candidate
- Final testing on all platforms
- Documentation review

**v2.0.0 Release (Week 13):**
- Stable release
- Announce on Reddit, forums
- Update AUR, DEB, AppImage packages

### Deprecation Timeline

- **v1.0.x**: Maintenance mode (bug fixes only) until v2.1.0
- **v1.1.0**: Final v1.x release with deprecation notice
- **v2.0.0**: New default, v1.x still available
- **v3.0.0** (future): Remove v1.x compatibility layer

---

## Success Criteria

### Functional Requirements

| Requirement | Acceptance Test |
|-------------|-----------------|
| **Zone System** | User creates 5-zone layout, all zones render correctly |
| **Plugin Isolation** | Kill plugin process, host continues running |
| **Hot Reload** | Edit layout YAML, changes apply in <2s without restart |
| **Multi-Page** | Configure 3 pages, swipe navigation works |
| **Touch Interaction** | Tap zone to cycle plugins, no lag |
| **Visual Consistency** | All zones use same font/color scheme |
| **Performance** | 60 FPS render loop, <500ms plugin sampling |

### Quality Requirements

| Metric | Target | Validation |
|--------|--------|------------|
| **Test Coverage** | ≥75% | `go test -cover ./...` |
| **RPC Latency** | <2ms avg | Benchmark 1000 RPC calls |
| **Memory Usage** | <75MB | Run 8 plugins for 1 hour |
| **CPU Usage (idle)** | <3% | Monitor with no display updates |
| **Config Reload** | <1s | Measure hot reload time |
| **Plugin Crash Recovery** | <100ms | Kill plugin, measure restart |

### Documentation Requirements

| Document | Completeness |
|----------|--------------|
| **User Guide** | Layout editor, plugin browser, troubleshooting |
| **Developer Guide** | Plugin API, RPC protocol, example code |
| **Migration Guide** | v1→v2 conversion, breaking changes, FAQ |
| **Plugin Catalog** | All built-in plugins documented with examples |
| **API Reference** | OpenAPI spec for all HTTP endpoints |

### User Experience Requirements

| Criterion | Validation Method |
|-----------|-------------------|
| **Ease of Customization** | Non-technical user creates custom layout in <5 min |
| **Visual Polish** | No tearing, no flicker, smooth transitions |
| **Error Messages** | All failure modes show helpful error text |
| **Performance Feel** | No perceptible lag on interaction |
| **Plugin Discovery** | User finds and installs community plugin in <2 min |

---

## Timeline & Resources

### Development Phases

| Phase | Duration | Effort | Dependencies |
|-------|----------|--------|--------------|
| **Phase 1: Zone System** | 2 weeks | 80 hours | None |
| **Phase 2: Plugins** | 2 weeks | 80 hours | Phase 1 complete |
| **Phase 3: Migrate Plugins** | 1 week | 40 hours | Phase 2 complete |
| **Phase 4: Pages & Touch** | 1 week | 40 hours | Phase 1 complete |
| **Phase 5: Config UI** | 1 week | 40 hours | Phase 2 complete |
| **Phase 6: Polish** | 1 week | 40 hours | All phases complete |
| **Testing & Docs** | 1 week | 40 hours | Phase 6 complete |
| **Beta Testing** | 2 weeks | 20 hours | Alpha tested |

**Total Estimated Effort:** ~380 hours (9.5 weeks @ full-time, or ~19 weeks @ part-time)

### Milestones

**Week 2: Zone System Working**
- ✅ Render mock payloads in configurable zones
- ✅ Layout hot reload functional
- ✅ Tests passing (>70% coverage)

**Week 4: Plugins Functional**
- ✅ go-plugin RPC working
- ✅ 3+ example plugins (CPU, Clock, Weather)
- ✅ Plugin crash recovery tested

**Week 5: Feature Parity with v1.0**
- ✅ All v1 instruments ported to plugins
- ✅ Default layout matches v1 UI
- ✅ Performance equal or better

**Week 6: Multi-Page Navigation**
- ✅ 3+ page layouts configured
- ✅ Swipe gestures working
- ✅ Smooth transitions (60 FPS)

**Week 7: Config UI Complete**
- ✅ Layout editor in Flutter UI
- ✅ Plugin browser functional
- ✅ Theme customization working

**Week 8: Production Ready**
- ✅ All tests passing (>75% coverage)
- ✅ Documentation complete
- ✅ Performance targets met

**Week 11: Beta Release**
- ✅ Public beta deployed
- ✅ Feedback collected
- ✅ Critical bugs fixed

**Week 13: v2.0.0 Release**
- ✅ Stable release published
- ✅ Packages updated (DEB, AUR, AppImage)
- ✅ Announcement posted

### Resource Requirements

**Team:**
- 1 Backend Developer (Go) - Full-time
- 1 Frontend Developer (Flutter) - Part-time (Weeks 5-7)
- 1 QA Tester - Part-time (Weeks 8-11)
- 1 Technical Writer - Part-time (Weeks 6-8)

**Infrastructure:**
- GitHub repository (existing)
- CI/CD (GitHub Actions)
- Test devices (1-2 iCUE Nexus displays)
- Linux VMs for package testing (Debian, Arch, Fedora)

**Budget (if applicable):**
- Hardware: ~$200 (test devices)
- Cloud CI: Free (GitHub Actions)
- Domain/hosting: $0 (GitHub Pages for docs)

---

## Risks & Mitigation

### Technical Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **go-plugin complexity** | Medium | High | Prototype early, use official examples |
| **Performance regression** | Medium | Medium | Benchmark frequently, profile before release |
| **Plugin ecosystem fragmentation** | Low | Medium | Strong documentation, reference plugins |
| **Config validation complexity** | Medium | Low | Schema validation library, extensive tests |
| **Touch input bugs** | Low | Medium | Thorough gesture testing, debounce logic |

### Project Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Scope creep** | High | High | Strict phase boundaries, defer non-critical features |
| **Breaking changes backlash** | Medium | Medium | Clear migration guide, v1 compatibility mode |
| **Community adoption** | Medium | Low | Beta testing with users, gather feedback early |
| **Testing hardware availability** | Low | High | Acquire test devices early, use emulation |
| **Timeline overrun** | Medium | Medium | Buffer weeks built in, weekly progress reviews |

### User Impact Risks

| Risk | Probability | Impact | Mitigation |
|------|-------------|--------|------------|
| **Migration difficulty** | High | Medium | Automated migration tool, detailed docs |
| **Config complexity** | Medium | Medium | Presets + visual editor, not just YAML |
| **Plugin installation confusion** | Medium | Low | Plugin browser in UI, one-click install (future) |
| **Performance on old hardware** | Low | Low | Optimization passes, configurable FPS |

---

## Future Enhancements (Post-v2.0)

### v2.1: Community Features
- **Plugin Marketplace**: Discover and install community plugins
- **Cloud Sync**: Sync layouts/themes across devices
- **Layout Templates**: Share layouts via import/export

### v2.2: Advanced Layouts
- **Vertical Zones**: Support stacked zones (not just horizontal)
- **Nested Zones**: Zones within zones (grid layouts)
- **Dynamic Sizing**: Zones resize based on content

### v2.3: Developer Tools
- **Plugin SDK**: CLI tool for scaffolding plugins
- **Hot Reload**: Live code updates for plugins during development
- **Debugging UI**: Inspect plugin RPC traffic, payloads

### v3.0: Scripting & Automation
- **Lua Scripting**: Simple plugins without compilation
- **Event System**: Plugins react to device events
- **Automation Rules**: Trigger actions based on data (future)

---

## Appendix

### Glossary

| Term | Definition |
|------|------------|
| **Zone** | Horizontal partition of the 640x48 display |
| **Plugin** | Plugin that provides data via RPC (CPU, Weather, etc.) |
| **Payload** | Data structure returned by plugins (Primary, Secondary, etc.) |
| **Host** | Main nexus-open process that manages zones and rendering |
| **Plugin** | External process launched via go-plugin RPC |
| **Page** | Set of zones that can be swapped via navigation |
| **Layout** | Configuration file defining zones and their plugins |
| **Theme** | Color scheme and typography settings |

### References

- [HashiCorp go-plugin](https://github.com/hashicorp/go-plugin)
- [iCUE Nexus Specifications](https://help.corsair.com/hc/en-us/articles/4415111683597)
- [Font Awesome Icons](https://fontawesome.com/icons)
- [Open-Meteo Weather API](https://open-meteo.com/)
- [Nexus Open v1.0 PROJECT_PLAN.md](PROJECT_PLAN.md)

### Related Documents

- [MODULE_GUIDE.md](docs/MODULE_GUIDE.md) - Writing custom plugins (TBD)
- [LAYOUT_GUIDE.md](docs/LAYOUT_GUIDE.md) - Creating layouts (TBD)
- [MIGRATION_v2.md](docs/MIGRATION_v2.md) - Upgrading from v1.0 (TBD)
- [API_REFERENCE.md](docs/API_REFERENCE.md) - HTTP API specification (TBD)

---

**End of Refactor Plan**

This plan will be updated as implementation progresses. Major decisions and scope changes will be documented in git commits and GitHub issues.
