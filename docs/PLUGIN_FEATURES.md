# Plugin Features & Patterns

This document describes common patterns and features implemented in built-in plugins.

---

## Blinking Elements

### Overview
Some plugins benefit from blinking/animated elements for visual feedback (e.g., clock colon, loading indicators).

### Implementation Pattern: Toggle State

**Approach:** Plugin maintains internal toggle state and alternates output.

**Example: Clock Plugin with Blinking Colon**

```go
type ClockModule struct {
    showColon bool  // Toggle state
}

func (m *ClockModule) Sample() (plugin.Payload, error) {
    now := time.Now()

    // Toggle every call
    m.showColon = !m.showColon

    var timeStr string
    if m.showColon {
        timeStr = now.Format("15:04")  // "15:04"
    } else {
        timeStr = now.Format("15 04")  // "15 04" (space instead)
    }

    return plugin.Payload{
        Primary: timeStr,
        // ...
    }, nil
}
```

**Configuration:**
```yaml
zones:
  - id: clock
    plugin: builtin:clock
    refresh_ms: 500  # Half-second for classic blink effect
```

**Timing:**
- `refresh_ms: 500` → Toggles every 500ms → 1Hz blink rate
- `refresh_ms: 1000` → Toggles every 1s → Slower blink

**Why This Works:**
- ✅ Simple: Just a boolean toggle
- ✅ Deterministic: Always alternates
- ✅ Configurable: Change blink rate via `refresh_ms`
- ✅ No renderer changes: Renderer is stateless

---

## Refresh Rate Guidelines

| Plugin Type | Refresh Rate | Rationale |
|-------------|--------------|-----------|
| **Clock** | 500ms | Blinking colon + subsecond accuracy |
| **CPU/GPU Temp** | 1000-2000ms | Metrics don't change faster |
| **Network** | 1000-2000ms | 1-second averages |
| **Weather** | 300000ms (5min) | API rate limits |
| **Placeholder** | 5000ms | Static content |
| **Debug** | 1000ms | Development only |

**Performance Note:** Built-in plugins have zero RPC overhead, so 500ms refresh is free.

---

## State Management

### Stateless Plugins (Preferred)
```go
func (m *SimpleModule) Sample() (plugin.Payload, error) {
    // Fetch data
    value := getCurrentValue()

    // Return immediately
    return plugin.Payload{Primary: value}, nil
}
```

**Pros:**
- No concurrency concerns
- Predictable
- Easy to test

### Stateful Plugins (When Needed)
```go
type StatefulModule struct {
    history []float32  // Sparkline data
    mu      sync.Mutex // Protect state
}

func (m *StatefulModule) Sample() (plugin.Payload, error) {
    m.mu.Lock()
    defer m.mu.Unlock()

    value := getCurrentValue()
    m.history = append(m.history, value)
    if len(m.history) > 60 {
        m.history = m.history[1:]
    }

    return plugin.Payload{
        Primary: formatValue(value),
        Spark:   m.history,
    }, nil
}
```

**When to use:**
- Historical data (sparklines)
- Calculated averages
- Toggle states (blink)
- Rate limiting

**Best practices:**
- Always use `sync.Mutex` for thread safety
- Limit state size (e.g., max 60 sparkline points)
- Document state in comments

---

## Sparkline Patterns

### Fixed-Size Circular Buffer
```go
type Plugin struct {
    history []float32
    maxSize int
}

func (m *Plugin) addSample(value float32) {
    m.history = append(m.history, value)
    if len(m.history) > m.maxSize {
        m.history = m.history[1:] // Remove oldest
    }
}
```

### Normalized Values
```go
func normalize(value, min, max float32) float32 {
    if max == min {
        return 0.5
    }
    normalized := (value - min) / (max - min)
    if normalized < 0 {
        return 0
    }
    if normalized > 1 {
        return 1
    }
    return normalized
}

// Example: CPU temp 0-100°C
sparkValue := normalize(temp, 0, 100)
```

---

## Severity Thresholds

### Pattern: Multi-Level Thresholds
```go
func getSeverity(temp float32) plugin.Severity {
    switch {
    case temp >= 85:
        return plugin.SeverityCrit  // Red
    case temp >= 70:
        return plugin.SeverityWarn  // Yellow
    default:
        return plugin.SeverityOK    // Accent color
    }
}
```

### Configurable Thresholds (Future)
```go
type ThresholdConfig struct {
    WarnAbove float32
    CritAbove float32
}

func (m *Plugin) getSeverity(value float32) plugin.Severity {
    if value >= m.config.CritAbove {
        return plugin.SeverityCrit
    }
    if value >= m.config.WarnAbove {
        return plugin.SeverityWarn
    }
    return plugin.SeverityOK
}
```

---

## Error Handling

### Graceful Degradation
```go
func (m *Plugin) Sample() (plugin.Payload, error) {
    value, err := fetchData()
    if err != nil {
        // Return placeholder instead of error
        return plugin.Payload{
            Primary:   "—",
            Secondary: "Unavailable",
            Severity:  plugin.SeverityWarn,
        }, nil
    }

    return plugin.Payload{
        Primary: formatValue(value),
    }, nil
}
```

### When to Return Errors
```go
// Fatal errors only (can't recover)
if device == nil {
    return plugin.Payload{}, fmt.Errorf("device not initialized")
}

// Transient errors: degrade gracefully
if err := fetchAPI(); err != nil {
    return plugin.Payload{
        Primary: "—",
        Secondary: "API Error",
    }, nil  // Return payload, not error
}
```

---

## Plugin Lifecycle

### Built-in Plugin Instantiation
```go
// Registry creates new instance per zone
registry.registerBuiltin("clock", func() plugin.Plugin {
    return builtin.NewClock()  // Fresh instance
})
```

**Implications:**
- Each zone gets independent plugin instance
- State is per-zone (e.g., separate blink toggles)
- No shared state between zones

### External Plugin Lifecycle
```go
// Plugin host launches one process per zone ID
host.LaunchModule(ctx, "zone-cpu", "exec:./plugins/cpu-temp")
host.LaunchModule(ctx, "zone-gpu", "exec:./plugins/gpu-temp")
```

**Implications:**
- One process per zone
- Process persists across samples
- State maintained between calls

---

## Testing Patterns

### Unit Test Example
```go
func TestClockBlink(t *testing.T) {
    clock := builtin.NewClock()

    // Sample twice
    p1, _ := clock.Sample()
    p2, _ := clock.Sample()

    // Should toggle
    if p1.Primary == p2.Primary {
        t.Error("Clock should toggle colon")
    }

    // One should have ":", one should have " "
    hasColon := strings.Contains(p1.Primary, ":")
    hasSpace := strings.Contains(p2.Primary, " ")

    if !hasColon && !hasSpace {
        t.Error("Clock should alternate colon and space")
    }
}
```

---

## Future Patterns

### Animations (Future Enhancement)
```go
type Payload struct {
    // ...
    Animation *AnimationConfig  // Future
}

type AnimationConfig struct {
    Type       string   // "blink", "scroll", "fade"
    IntervalMs int
    Targets    []string // What to animate
}
```

### Configuration (Future)
```go
type Plugin interface {
    Describe() (Descriptor, error)
    Sample() (Payload, error)
    Configure(config map[string]interface{}) error  // Future
}
```

---

**Last Updated:** 2025-10-12
**Version:** 2.0.0-alpha
