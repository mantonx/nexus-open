# Migration Guide: Instruments → Modules

This document outlines the migration strategy for converting v1.0 instruments to v2.0 modules.

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
internal/modules/builtin/
├── clock.go         - Built-in clock module
├── placeholder.go   - Built-in loading/error display
└── debug.go         - Built-in zone debug info

modules/
├── cpu-temp/        - External CPU temperature module
│   ├── main.go
│   └── README.md
├── gpu-temp/        - External GPU temperature module
│   ├── main.go
│   └── README.md
├── network/         - External network stats module
│   ├── main.go
│   └── README.md
└── weather/         - External weather module
    ├── main.go
    └── README.md
```

---

## Migration Strategy

### Phase 1: Core Infrastructure ✅ COMPLETE
- [x] Create `pkg/module/` interface
- [x] Create `internal/zone/` system
- [x] Define Payload and Descriptor types
- [x] Implement zone rendering

### Phase 2: Plugin System (Next)
- [ ] Implement go-plugin RPC wrapper
- [ ] Create built-in module registry
- [ ] Build 3 built-in modules (clock, placeholder, debug)
- [ ] Test RPC communication

### Phase 3: Migrate Instruments
Convert each instrument to an external module:

#### 3.1 CPU Temperature Module
**Source:** `internal/instruments/cpu_temp.go` (120 lines)
**Target:** `modules/cpu-temp/main.go`

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

#### 3.2 GPU Temperature Module
**Source:** `internal/instruments/gpu_temp.go` (95 lines)
**Target:** `modules/gpu-temp/main.go`

**Features to preserve:**
- nvidia-smi integration
- AMD GPU support (future)
- Sparkline history
- Warning thresholds (>75°C = warn, >90°C = crit)

**New capabilities:**
- Multiple GPU support
- GPU load percentage
- VRAM usage (future)

#### 3.3 Network Module
**Source:** `internal/instruments/network.go` (180 lines)
**Target:** `modules/network/main.go`

**Features to preserve:**
- psutil integration for Linux/Windows/macOS
- Upload/download rates
- Bytes/s formatting (KB/s, MB/s, GB/s)
- Sparkline for traffic history

**New capabilities:**
- Interface selection (eth0, wlan0, etc.)
- Total data transferred
- Connection count

#### 3.4 Weather Module
**Source:** `internal/instruments/weather.go` (210 lines)
**Target:** `modules/weather/main.go`

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

## Module Interface Template

Each module will implement this structure:

```go
package main

import (
    "github.com/hashicorp/go-plugin"
    "nexus-open/pkg/module"
)

type MyModule struct{
    // Module state
}

func (m *MyModule) Describe() (module.Descriptor, error) {
    return module.Descriptor{
        Name:      "Module Name",
        Version:   "1.0.0",
        Author:    "Nexus Team",
        Description: "Brief description",
        Icon:      "thermometer",
        RefreshMs: 2000,
    }, nil
}

func (m *MyModule) Sample() (module.Payload, error) {
    // Collect data
    value := collectData()

    return module.Payload{
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
        HandshakeConfig: module.Handshake,
        Plugins: map[string]plugin.Plugin{
            "module": &module.ModulePlugin{Impl: &MyModule{}},
        },
    })
}
```

---

## Compatibility Layer (Optional)

For users who need immediate v2.0 access while modules are being ported, we can provide a compatibility wrapper:

```go
// internal/compat/instrument_wrapper.go
type InstrumentWrapper struct {
    instrument instruments.Instrument
}

func (w *InstrumentWrapper) Describe() (module.Descriptor, error) {
    // Convert instrument metadata to Descriptor
}

func (w *InstrumentWrapper) Sample() (module.Payload, error) {
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
Each module will have tests for:
- Describe() returns valid metadata
- Sample() returns valid payload
- Error handling (device not found, API timeout, etc.)
- Threshold calculations
- Data formatting

### Integration Tests
- Host launches module via RPC
- Module responds within timeout
- Payload validation passes
- Module crash recovery works

### Manual Testing
```bash
# Test module standalone
./modules/cpu-temp/cpu-temp describe
./modules/cpu-temp/cpu-temp sample

# Test via host
nexus-open module test cpu-temp

# Test in layout
nexus-open --config test-layout.yaml
```

---

## Timeline

### Phase 2 (Week 3-4): Plugin Infrastructure
- Implement go-plugin RPC (2 days)
- Create built-in modules (1 day)
- Module registry and discovery (1 day)
- Testing (1 day)

### Phase 3 (Week 5): Migrate Instruments
- CPU Temperature module (1 day)
- GPU Temperature module (1 day)
- Network module (1 day)
- Weather module (1 day)
- Integration testing (1 day)

---

## Success Criteria

- [ ] All 4 instruments ported to modules
- [ ] Feature parity with v1.0
- [ ] Modules run as isolated processes
- [ ] Host survives module crashes
- [ ] Performance equal or better than v1.0
- [ ] Default layout renders correctly
- [ ] All tests passing

---

## Rollback Plan

If module migration encounters blockers:
1. Keep v1.0 instruments in place
2. Run both systems in parallel
3. Gradual cutover per module
4. v1.0 instruments can coexist with v2.0 modules

---

**Status:** Planning complete, ready for Phase 2 implementation

**Next Steps:**
1. Implement go-plugin RPC wrapper
2. Create first built-in module (clock)
3. Test end-to-end: host ↔ module ↔ zone ↔ display
