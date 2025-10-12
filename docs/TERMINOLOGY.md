# Nexus Open Terminology Guide

This document defines the official terminology for Nexus Open v2.0+.

---

## Core Concepts

### Module
A **module** is a plugin that provides data to be displayed in a zone. Modules implement a simple interface with two methods: `Describe()` (metadata) and `Sample()` (current data).

**Examples:**
- Clock module (displays current time)
- CPU Temperature module (monitors CPU temp)
- Weather module (fetches weather data)
- Media module (shows now playing info)

**Types of modules:**
- **Built-in modules**: Compiled into the host binary (e.g., `builtin:clock`)
- **External modules**: Separate executables via RPC (e.g., `exec:./modules/cpu-temp`)
- **Script modules**: Interpreted scripts (future, e.g., `script:./custom.lua`)

### Zone
A **zone** is a horizontal partition of the 640x48 display that renders data from a module. Zones have configurable widths that must sum to 640 pixels.

**Attributes:**
- Width (80-640 pixels)
- Module assignment
- Refresh interval
- Text alignment (left/center/right)
- Theme overrides

### Page
A **page** is a collection of zones that can be displayed together. Users can swipe between pages or have them auto-rotate.

**Example:**
- Page 1: Weather | CPU | GPU | Clock
- Page 2: Media (wide) | Network | Clock

### Payload
A **payload** is the data structure returned by a module containing the information to display:
- `Primary`: Main value (e.g., "42°C")
- `Secondary`: Context (e.g., "CPU Load")
- `Spark`: Historical data for sparkline charts
- `Severity`: Visual indicator (ok/warn/crit)
- `Progress`: Progress bar value (0.0-1.0)

### Layout
A **layout** is a YAML configuration file that defines pages, zones, themes, and module assignments.

---

## Architecture Terms

### Host
The **host** is the main `nexus-open` process that:
- Manages zones and pages
- Launches and communicates with modules via RPC
- Owns all rendering for visual consistency
- Sends frames to the USB device

### Compositor
The **compositor** combines multiple zone images into a single 640x48 frame.

### Renderer
A **renderer** converts a module's payload into a zone image (text, sparkline, progress bar).

### RPC (Remote Procedure Call)
**RPC** is the communication mechanism between the host and external modules, implemented using HashiCorp go-plugin.

---

## Deprecated Terms (v1.0 → v2.0)

| v1.0 Term | v2.0 Term | Notes |
|-----------|-----------|-------|
| **Instrument** | **Module** | Data providers are now called modules |
| **InstrumentRegistry** | **ModuleRegistry** | Registry for available modules |
| Hardcoded layout | Zone-based layout | Configurable horizontal partitions |
| Fixed rendering | Centralized rendering | Host owns all drawing |

---

## User-Facing Terms

### Configuration UI
- "Module Browser" - List of available modules
- "Layout Editor" - Visual zone configuration
- "Theme Editor" - Color and font customization

### CLI Commands
```bash
nexus-open module list              # List available modules
nexus-open module test <name>       # Test a module's output
nexus-open layout validate <file>   # Validate layout config
nexus-open layout reload            # Hot reload layout
```

### Documentation
- "Writing Modules Guide" - How to create custom modules
- "Layout Guide" - How to configure zones and pages
- "Module API Reference" - Technical module interface docs

---

## Migration Guide (v1.0 Users)

### What Changed?
- **Instruments → Modules**: Same concept, new name and architecture
- Old: `internal/instruments/cpu_temp.go` (hardcoded in binary)
- New: `modules/cpu-temp/` (external RPC process) OR `internal/modules/builtin/cpu.go`

### Why the Change?
1. **Modularity**: External modules can be developed independently
2. **Safety**: Module crashes don't kill the host
3. **Flexibility**: Write modules in any language (Go, Python, Rust, etc.)
4. **Extensibility**: Community can create and share modules

### Compatibility
- **Breaking change**: v1.0 configs won't work with v2.0
- **Migration tool**: `nexus-open migrate --from-v1` converts old configs
- **Feature parity**: All v1.0 instruments will be available as v2.0 modules

---

## Examples

### In Code
```go
// Module interface
type Module interface {
    Describe() (Descriptor, error)
    Sample() (Payload, error)
}

// Built-in module
package builtin
type ClockModule struct{}

// External module
// modules/cpu-temp/main.go
```

### In Configuration
```yaml
pages:
  - name: "Dashboard"
    zones:
      - id: cpu
        width: 160
        module: exec:./modules/cpu-temp  # External module
        refresh_ms: 2000

      - id: clock
        width: 160
        module: builtin:clock            # Built-in module
        refresh_ms: 1000
```

### In User Documentation
> "Modules are plugins that provide data to your display. You can use built-in modules like Clock and Debug, or install community modules from the Module Browser. Advanced users can write their own modules in any language."

---

## Glossary

| Term | Definition |
|------|------------|
| **Module** | Plugin that provides data via RPC |
| **Zone** | Horizontal partition of the display |
| **Page** | Collection of zones (swipeable) |
| **Payload** | Data structure from module to zone |
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

**Last Updated:** 2025-10-12
**Version:** 2.0.0-alpha
