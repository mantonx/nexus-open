# Migration Guide: Instruments → Plugins

This document outlines the migration strategy for converting v1.0 instruments to v2.0 plugins.

---

## Overview

**v1.0 Architecture (Current):**
```
internal/instruments/
├── cpu_temp.go      - CPU temperature collector
├── gpu_temp.go      - GPU temperature collector
├── network.go       - Network statistics
├── weather.go       - Weather data fetcher
└── monitor.go       - Instrument registry/aggregator
```

**v2.0 Architecture (Target):**
```
internal/plugins/builtin/
├── clock.go         - Built-in clock plugin
├── placeholder.go   - Built-in loading/error display
└── debug.go         - Built-in zone debug info

plugins/
├── cpu-temp/        - External CPU temperature plugin
│   ├── main.go
│   └── README.md
├── gpu-temp/        - External GPU temperature plugin
│   ├── main.go
│   └── README.md
├── network/         - External network stats plugin
│   ├── main.go
│   └── README.md
└── weather/         - External weather plugin
    ├── main.go
    └── README.md
```

---

## Migration Strategy

### Phase 1: Core Infrastructure ✅ COMPLETE
- [x] Create `pkg/plugin/` interface
- [x] Create `internal/zone/` system
- [x] Define Payload and Descriptor types
- [x] Implement zone rendering

### Phase 2: Plugin System (Next)
- [ ] Implement go-plugin RPC wrapper
- [ ] Create built-in plugin registry
- [ ] Build 3 built-in plugins (clock, placeholder, debug)
- [ ] Test RPC communication

### Phase 3: Migrate Instruments
Convert each instrument to an external plugin:

#### 3.1 CPU Temperature Plugin
**Source:** `internal/instruments/cpu_temp.go` (120 lines)
**Target:** `plugins/cpu-temp/main.go`

**Features to preserve:**
- Linux: Read from `/sys/class/thermal/`
- Windows: WMI queries (if supported)
- macOS: `osx-cpu-temp` command
- Sparkline history (last 60 samples)
- Warning thresholds (>70°C = warn, >85°C = crit)

**New capabilities:**
- Configurable thresholds
- Per-core temperature display (future)
- JSON output for testing

#### 3.2 GPU Temperature Plugin
**Source:** `internal/instruments/gpu_temp.go` (95 lines)
**Target:** `plugins/gpu-temp/main.go`

**Features to preserve:**
- nvidia-smi integration
- AMD GPU support (future)
- Sparkline history
- Warning thresholds (>75°C = warn, >90°C = crit)

**New capabilities:**
- Multiple GPU support
- GPU load percentage
- VRAM usage (future)

#### 3.3 Network Plugin
**Source:** `internal/instruments/network.go` (180 lines)
**Target:** `plugins/network/main.go`

**Features to preserve:**
- psutil integration for Linux/Windows/macOS
- Upload/download rates
- Bytes/s formatting (KB/s, MB/s, GB/s)
- Sparkline for traffic history

**New capabilities:**
- Interface selection (eth0, wlan0, etc.)
- Total data transferred
- Connection count

#### 3.4 Weather Plugin
**Source:** `internal/instruments/weather.go` (210 lines)
**Target:** `plugins/weather/main.go`

**Features to preserve:**
- Open-Meteo API integration
- Nominatim geocoding for location names
- Temperature, humidity, conditions
- Weather icons (Font Awesome mapping)

**New capabilities:**
- Hourly forecast
- Multiple locations
- Unit preferences (°F/°C)
- Configurable update interval

---

## Plugin Interface Template

Each plugin will implement this structure:

```go
package main

import (
    "github.com/hashicorp/go-plugin"
    "nexus-open/pkg/plugin"
)

type MyModule struct{
    // Plugin state
}

func (m *MyModule) Describe() (plugin.Descriptor, error) {
    return plugin.Descriptor{
        Name:      "Plugin Name",
        Version:   "1.0.0",
        Author:    "Nexus Team",
        Description: "Brief description",
        Icon:      "thermometer",
        RefreshMs: 2000,
    }, nil
}

func (m *MyModule) Sample() (plugin.Payload, error) {
    // Collect data
    value := collectData()

    return plugin.Payload{
        Primary:   formatValue(value),
        Secondary: "Context",
        Severity:  getSeverity(value),
        Spark:     getHistory(),
        TTL:       2 * time.Second,
        Timestamp: time.Now(),
    }, nil
}

func main() {
    plugin.Serve(&plugin.ServeConfig{
        HandshakeConfig: plugin.Handshake,
        Plugins: map[string]plugin.Plugin{
            "plugin": &plugin.ModulePlugin{Impl: &MyModule{}},
        },
    })
}
```

---

## Compatibility Layer (Optional)

For users who need immediate v2.0 access while plugins are being ported, we can provide a compatibility wrapper:

```go
// internal/compat/instrument_wrapper.go
type InstrumentWrapper struct {
    instrument instruments.Instrument
}

func (w *InstrumentWrapper) Describe() (plugin.Descriptor, error) {
    // Convert instrument metadata to Descriptor
}

func (w *InstrumentWrapper) Sample() (plugin.Payload, error) {
    // Call old instrument interface
    // Convert to Payload format
}
```

**Pros:**
- Immediate v2.0 functionality
- Gradual migration possible

**Cons:**
- Technical debt
- No process isolation
- Performance overhead

**Decision:** Skip compatibility layer, do clean migration in Phase 3.

---

## Testing Strategy

### Unit Tests
Each plugin will have tests for:
- Describe() returns valid metadata
- Sample() returns valid payload
- Error handling (device not found, API timeout, etc.)
- Threshold calculations
- Data formatting

### Integration Tests
- Host launches plugin via RPC
- Plugin responds within timeout
- Payload validation passes
- Plugin crash recovery works

### Manual Testing
```bash
# Test plugin standalone
./plugins/cpu-temp/cpu-temp describe
./plugins/cpu-temp/cpu-temp sample

# Test via host
nexus-open plugin test cpu-temp

# Test in layout
nexus-open --config test-layout.yaml
```

---

## Timeline

### Phase 2 (Week 3-4): Plugin Infrastructure
- Implement go-plugin RPC (2 days)
- Create built-in plugins (1 day)
- Plugin registry and discovery (1 day)
- Testing (1 day)

### Phase 3 (Week 5): Migrate Instruments
- CPU Temperature plugin (1 day)
- GPU Temperature plugin (1 day)
- Network plugin (1 day)
- Weather plugin (1 day)
- Integration testing (1 day)

---

## Success Criteria

- [ ] All 4 instruments ported to plugins
- [ ] Feature parity with v1.0
- [ ] Plugins run as isolated processes
- [ ] Host survives plugin crashes
- [ ] Performance equal or better than v1.0
- [ ] Default layout renders correctly
- [ ] All tests passing

---

## Rollback Plan

If plugin migration encounters blockers:
1. Keep v1.0 instruments in place
2. Run both systems in parallel
3. Gradual cutover per plugin
4. v1.0 instruments can coexist with v2.0 plugins

---

**Status:** Planning complete, ready for Phase 2 implementation

**Next Steps:**
1. Implement go-plugin RPC wrapper
2. Create first built-in plugin (clock)
3. Test end-to-end: host ↔ plugin ↔ zone ↔ display
