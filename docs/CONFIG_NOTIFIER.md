# ConfigNotifier System

## Overview

The ConfigNotifier system enables plugins to receive **immediate real-time configuration updates** via RPC, eliminating the need for file watching or polling. When a user updates configuration through the API, plugins are notified instantly and can update their display within seconds.

## How It Works

### Architecture

1. **User updates config** via API (`POST /api/config`)
2. **API handler saves config** to file
3. **API handler broadcasts config** to all loaded plugins via `BroadcastConfigChange()`
4. **Plugins receive notification** through their `OnConfigChanged()` method
5. **Host triggers immediate sample** via trigger channels
6. **Display updates immediately** (typically 2-3 seconds)

### Key Components

```
┌─────────────┐
│   API       │ POST /api/config
│  Handler    │────────────────┐
└─────────────┘                │
                               ▼
                    ┌──────────────────┐
                    │ BroadcastConfig  │
                    │    Change()      │
                    └──────────────────┘
                               │
                ┌──────────────┼──────────────┐
                ▼              ▼              ▼
         ┌──────────┐   ┌──────────┐   ┌──────────┐
         │ Weather  │   │ Network  │   │  Clock   │
         │ Plugin   │   │ Plugin   │   │  Plugin  │
         └──────────┘   └──────────┘   └──────────┘
              │               │               │
              └───────────────┴───────────────┘
                             │
                    OnConfigChanged(config)
                             │
                             ▼
                    Update internal state
                    Clear cache / Fetch new data
                             │
                             ▼
                    ┌─────────────────┐
                    │ Trigger Channel │
                    │ Signals Sample  │
                    └─────────────────┘
                             │
                             ▼
                    Sample() returns fresh data
                             │
                             ▼
                    Display updates immediately!
```

## Implementation Guide

### 1. Add Config Field to Config Struct

**File**: `internal/config/config.go`

```go
type Config struct {
    // ... existing fields ...

    // openapi:description Network speed display format (bytes for KB/s, MB/s or bits for Kbps, Mbps)
    // openapi:enum bytes bits
    // openapi:example bytes
    NetworkFormat string `mapstructure:"network_format" json:"network_format"`
}
```

**CRITICAL**: You MUST add OpenAPI annotations for the API documentation to work correctly.

### 2. Add Config Field to API Broadcast Map

**File**: `internal/api/handlers.go` (in `handleConfigUpdate`)

```go
configMap := map[string]interface{}{
    "location":         newConfig.Location,
    "time_format":      newConfig.TimeFormat,
    "unit":             newConfig.Unit,
    "network_format":   newConfig.NetworkFormat,  // ← ADD THIS!
    // ... other fields ...
}
s.broadcaster.BroadcastConfigChange(configMap)
```

**CRITICAL**: If you forget this step, plugins will never receive the config value! The broadcast map must include ALL fields that plugins need.

### 3. Implement OnConfigChanged in Your Plugin

**File**: `plugins/your-plugin/main.go`

```go
// OnConfigChanged implements plugin.ConfigNotifier interface.
// The plugin uses the "your_config_field" config.
func (m *YourModule) OnConfigChanged(config map[string]interface{}) error {
    m.mu.Lock()
    defer m.mu.Unlock()

    oldValue := m.configValue

    // Extract config value
    if value, ok := config["your_config_field"].(string); ok && value != "" {
        m.configValue = value

        if m.configValue != oldValue {
            // Clear any cached data so next Sample() fetches fresh data
            m.cachedData = nil
            m.lastUpdate = time.Time{}

            fmt.Printf("your-plugin: config changed from %q to %q\n", oldValue, m.configValue)
        }
    }

    return nil
}
```

**Key Points**:
- Lock your mutex to protect internal state
- Check if the config field actually changed before doing work
- Clear caches so the next `Sample()` call gets fresh data
- Return `nil` on success

### 4. The Trigger Channel System (Already Implemented)

The host automatically triggers immediate `Sample()` calls after broadcasting config changes:

**File**: `internal/zone/sampler.go`

```go
func (s *Sampler) BroadcastConfigChange(config map[string]interface{}) {
    // ... notify all plugins ...

    // Trigger immediate resampling for plugins that were notified
    for _, zoneID := range zonesToResample {
        if triggerCh, ok := s.triggerChannels[zoneID]; ok {
            select {
            case triggerCh <- struct{}{}:
                s.logger.Debug("triggered immediate resample", "zone_id", zoneID)
            default:
                // Channel already has pending trigger, skip
            }
        }
    }
}
```

You don't need to modify this - it's already built in!

## Common Patterns

### Pattern 1: Simple Value Update (Weather Location)

```go
func (m *WeatherModule) OnConfigChanged(config map[string]interface{}) error {
    m.mu.Lock()
    defer m.mu.Unlock()

    oldLocation := m.location
    oldUnit := m.unit

    if location, ok := config["location"].(string); ok && location != "" {
        m.location = location
    }

    if unit, ok := config["unit"].(string); ok && unit != "" {
        m.unit = unit
    }

    if m.location != oldLocation || m.unit != oldUnit {
        m.cachedData = nil
        m.lastUpdate = time.Time{}
        fmt.Printf("weather: config updated - location=%q unit=%q\n",
            m.location, m.unit)
    }

    return nil
}
```

### Pattern 2: Format Switch (Network Bytes/Bits)

```go
func (m *NetworkModule) OnConfigChanged(config map[string]interface{}) error {
    m.formatMu.Lock()
    defer m.formatMu.Unlock()

    oldFormat := m.format

    if format, ok := config["network_format"].(string); ok && format != "" {
        if format == "bytes" || format == "bits" {
            m.format = format
            if m.format != oldFormat {
                fmt.Printf("network: format changed from %q to %q\n", oldFormat, m.format)
            }
        }
    }

    return nil
}
```

### Pattern 3: No-Op Implementation (Plugin Doesn't Use Config)

```go
func (m *CPUTempModule) OnConfigChanged(config map[string]interface{}) error {
    // CPU temperature plugin doesn't need configuration
    return nil
}
```

**IMPORTANT**: Even if your plugin doesn't use config, you MUST implement this method to prevent RPC errors!

## Troubleshooting

### Problem: Plugin doesn't receive config updates

**Checklist**:
1. ✅ Did you add the field to `internal/config/config.go`?
2. ✅ Did you add the field to the broadcast map in `internal/api/handlers.go`?
3. ✅ Did you implement `OnConfigChanged()` in your plugin?
4. ✅ Did you rebuild your plugin binary?
5. ✅ Did you restart the dev server to pick up the new binary?

**Debug**: Add logging to see what config keys are received:
```go
func (m *YourModule) OnConfigChanged(config map[string]interface{}) error {
    fmt.Printf("plugin: OnConfigChanged called with keys: %v\n", getKeys(config))
    // ... rest of implementation ...
}
```

### Problem: Display doesn't update immediately

**Cause**: Plugin's `OnConfigChanged` is being called, but the display shows old data.

**Solution**: Make sure you're clearing cached data:
```go
if configChanged {
    m.cachedData = nil      // Clear cache
    m.lastUpdate = time.Time{}  // Reset timestamp
}
```

The trigger channel system will call `Sample()` immediately, which will fetch fresh data.

### Problem: Gob encoding errors

**Error**: `gob: type not registered for interface: map[string]interface {}`

**Cause**: Missing gob type registration.

**Solution**: Already fixed in `pkg/plugin/plugin.go`:
```go
func init() {
    gob.Register(map[string]interface{}{})
}
```

## Performance Considerations

### Immediate Updates Are Fast

- **Weather location changes**: ~2-3 seconds (includes API geocoding + weather fetch)
- **Network format changes**: ~instant (no external API calls)
- **Clock format changes**: ~instant

### Why It's Fast

1. **No polling**: Plugins don't check files every N seconds
2. **Direct RPC**: Immediate notification via go-plugin RPC
3. **Trigger channels**: Host immediately calls `Sample()` after notification
4. **Smart caching**: Plugins cache data and only refetch when config changes

### Avoid These Anti-Patterns

❌ **DON'T** fetch data in a goroutine in `OnConfigChanged`:
```go
// BAD - Can cause deadlocks!
func (m *Plugin) OnConfigChanged(config map[string]interface{}) error {
    m.mu.Lock()
    defer m.mu.Unlock()

    go func() {
        m.mu.Lock()  // DEADLOCK!
        m.fetchData()
        m.mu.Unlock()
    }()
    return nil
}
```

✅ **DO** just clear cache and let the trigger system call `Sample()`:
```go
// GOOD - Simple and safe
func (m *Plugin) OnConfigChanged(config map[string]interface{}) error {
    m.mu.Lock()
    defer m.mu.Unlock()

    m.cachedData = nil  // Clear cache
    return nil
    // Host will immediately call Sample() which will fetch fresh data
}
```

## Examples in Codebase

- **Weather Plugin**: [`plugins/weather/main.go:139-165`](plugins/weather/main.go#L139-L165)
- **Network Plugin**: [`plugins/network/main.go:220-242`](plugins/network/main.go#L220-L242)
- **Clock Plugin**: [`internal/plugins/builtin/clock.go`](internal/plugins/builtin/clock.go)
- **Sampler (Trigger System)**: [`internal/zone/sampler.go:322-372`](internal/zone/sampler.go#L322-L372)

## Per-Plugin Configuration

In addition to global configuration (via API), plugins can receive **per-zone configuration** from the zone layout YAML file. This allows each zone to configure its plugin independently.

### How Per-Plugin Config Works

1. **Define config in zone layout** (`configs/layouts/*.yaml`):
```yaml
pages:
  - name: "System"
    zones:
      - id: cpu
        width: 160
        plugin: exec:./plugins/cpu-temp/cpu-temp
        refresh_ms: 2000
        module_config:
          unit: "metric"  # CPU shows Celsius

      - id: gpu
        width: 160
        plugin: exec:./plugins/gpu-temp/gpu-temp
        refresh_ms: 2000
        module_config:
          unit: "imperial"  # GPU shows Fahrenheit
```

2. **Sampler sends config on plugin load** (`internal/zone/sampler.go:127-140`):
```go
// Send plugin-specific config if available
if zoneConfig.ModuleConfig != nil && len(zoneConfig.ModuleConfig) > 0 {
    if notifier, ok := plugin.SupportsConfigNotification(mod); ok {
        if err := notifier.OnConfigChanged(zoneConfig.ModuleConfig); err != nil {
            // handle error
        }
    }
}
```

3. **Plugin receives config** - Same `OnConfigChanged()` method as global config!

### Example: CPU in Celsius, GPU in Fahrenheit

```yaml
zones:
  - id: cpu
    plugin: exec:./plugins/cpu-temp/cpu-temp
    module_config:
      unit: "metric"  # 28°C

  - id: gpu
    plugin: exec:./plugins/gpu-temp/gpu-temp
    module_config:
      unit: "imperial"  # 127°F
```

Result:
- CPU zone displays: `28°C`
- GPU zone displays: `127°F`

Both plugins use the same code - they just receive different config values!

### Per-Plugin vs Global Config

| Feature | Per-Plugin Config | Global Config |
|---------|------------------|---------------|
| **Defined in** | Zone layout YAML | Global config file |
| **Scope** | Single zone/plugin | All plugins |
| **Updated via** | Restart required | API (real-time) |
| **Use case** | Different settings per zone | User preferences |
| **Example** | CPU in °C, GPU in °F | All temps in °F |

**Best Practice**: Use per-plugin config for zone-specific settings, global config for user preferences.

## Summary

The ConfigNotifier system provides **immediate real-time configuration updates** with minimal code:

1. Add config field to struct
2. Add field to broadcast map
3. Implement `OnConfigChanged()` to update internal state
4. Clear cache - let the trigger system do the rest!

Updates appear on the display in **seconds**, not minutes. No polling, no file watching, just instant RPC notifications.

**Per-plugin configuration** enables fine-grained control - each zone can configure its plugin independently via the zone layout YAML.
