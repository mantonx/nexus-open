# Module Features & Patterns

This document describes common patterns and features implemented in built-in modules.

---

## Blinking Elements

### Overview
Some modules benefit from blinking/animated elements for visual feedback (e.g., clock colon, loading indicators).

### Implementation Pattern: Toggle State

**Approach:** Module maintains internal toggle state and alternates output.

**Example: Clock Module with Blinking Colon**

```go
type ClockModule struct {
    showColon bool  // Toggle state
}

func (m *ClockModule) Sample() (module.Payload, error) {
    now := time.Now()

    // Toggle every call
    m.showColon = !m.showColon

    var timeStr string
    if m.showColon {
        timeStr = now.Format("15:04")  // "15:04"
    } else {
        timeStr = now.Format("15 04")  // "15 04" (space instead)
    }

    return module.Payload{
        Primary: timeStr,
        // ...
    }, nil
}
```

**Configuration:**
```yaml
zones:
  - id: clock
    module: builtin:clock
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

| Module Type | Refresh Rate | Rationale |
|-------------|--------------|-----------|
| **Clock** | 500ms | Blinking colon + subsecond accuracy |
| **CPU/GPU Temp** | 1000-2000ms | Metrics don't change faster |
| **Network** | 1000-2000ms | 1-second averages |
| **Weather** | 300000ms (5min) | API rate limits |
| **Placeholder** | 5000ms | Static content |
| **Debug** | 1000ms | Development only |

**Performance Note:** Built-in modules have zero RPC overhead, so 500ms refresh is free.

---

## State Management

### Stateless Modules (Preferred)
```go
func (m *SimpleModule) Sample() (module.Payload, error) {
    // Fetch data
    value := getCurrentValue()

    // Return immediately
    return module.Payload{Primary: value}, nil
}
```

**Pros:**
- No concurrency concerns
- Predictable
- Easy to test

### Stateful Modules (When Needed)
```go
type StatefulModule struct {
    history []float32  // Sparkline data
    mu      sync.Mutex // Protect state
}

func (m *StatefulModule) Sample() (module.Payload, error) {
    m.mu.Lock()
    defer m.mu.Unlock()

    value := getCurrentValue()
    m.history = append(m.history, value)
    if len(m.history) > 60 {
        m.history = m.history[1:]
    }

    return module.Payload{
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
type Module struct {
    history []float32
    maxSize int
}

func (m *Module) addSample(value float32) {
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
func getSeverity(temp float32) module.Severity {
    switch {
    case temp >= 85:
        return module.SeverityCrit  // Red
    case temp >= 70:
        return module.SeverityWarn  // Yellow
    default:
        return module.SeverityOK    // Accent color
    }
}
```

### Configurable Thresholds (Future)
```go
type ThresholdConfig struct {
    WarnAbove float32
    CritAbove float32
}

func (m *Module) getSeverity(value float32) module.Severity {
    if value >= m.config.CritAbove {
        return module.SeverityCrit
    }
    if value >= m.config.WarnAbove {
        return module.SeverityWarn
    }
    return module.SeverityOK
}
```

---

## Error Handling

### Graceful Degradation
```go
func (m *Module) Sample() (module.Payload, error) {
    value, err := fetchData()
    if err != nil {
        // Return placeholder instead of error
        return module.Payload{
            Primary:   "—",
            Secondary: "Unavailable",
            Severity:  module.SeverityWarn,
        }, nil
    }

    return module.Payload{
        Primary: formatValue(value),
    }, nil
}
```

### When to Return Errors
```go
// Fatal errors only (can't recover)
if device == nil {
    return module.Payload{}, fmt.Errorf("device not initialized")
}

// Transient errors: degrade gracefully
if err := fetchAPI(); err != nil {
    return module.Payload{
        Primary: "—",
        Secondary: "API Error",
    }, nil  // Return payload, not error
}
```

---

## Module Lifecycle

### Built-in Module Instantiation
```go
// Registry creates new instance per zone
registry.registerBuiltin("clock", func() module.Module {
    return builtin.NewClock()  // Fresh instance
})
```

**Implications:**
- Each zone gets independent module instance
- State is per-zone (e.g., separate blink toggles)
- No shared state between zones

### External Module Lifecycle
```go
// Plugin host launches one process per zone ID
host.LaunchModule(ctx, "zone-cpu", "exec:./modules/cpu-temp")
host.LaunchModule(ctx, "zone-gpu", "exec:./modules/gpu-temp")
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
type Module interface {
    Describe() (Descriptor, error)
    Sample() (Payload, error)
    Configure(config map[string]interface{}) error  // Future
}
```

---

**Last Updated:** 2025-10-12
**Version:** 2.0.0-alpha
