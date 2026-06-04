# Zone Naming Convention

## The Problem

Current zone naming is inconsistent and doesn't reflect page context:

```yaml
pages:
  - name: "System"
    zones:
      - id: cpu           # Which page? Not clear from name
      - id: gpu
      - id: network

  - name: "Performance"
    zones:
      - id: cpu-main      # What does "-main" mean?
      - id: gpu-main
      - id: network-main
```

**Issues**:
1. Zone ID doesn't indicate which page it's on
2. `-main` suffix is arbitrary and unclear
3. Hard to reference zones in API calls
4. Confusing for users

---

## Proposed Convention

### Format: `{page}.{module}` or `{page}.{module}.{variant}`

**Examples**:
```yaml
pages:
  - name: "System"
    zones:
      - id: system.cpu           # Clear: CPU on System page
      - id: system.gpu
      - id: system.network
      - id: system.weather

  - name: "Performance"
    zones:
      - id: performance.cpu      # Clear: CPU on Performance page
      - id: performance.gpu
      - id: performance.network

  - name: "Clock"
    zones:
      - id: clock.time           # Clear: Time widget on Clock page
```

**With variants** (if multiple instances on same page):
```yaml
pages:
  - name: "Dashboard"
    zones:
      - id: dashboard.weather.home    # Weather for home location
      - id: dashboard.weather.work    # Weather for work location
      - id: dashboard.cpu.summary     # CPU summary widget
      - id: dashboard.cpu.detailed    # CPU detailed widget
```

---

## Benefits

### 1. **Self-documenting**
```bash
# OLD (confusing)
POST /api/zones/cpu-main/config

# NEW (clear)
POST /api/zones/performance.cpu/config
```

### 2. **Easy to list zones by page**
```bash
# Get all zones on performance page
GET /api/zones?page=performance
→ ["performance.cpu", "performance.gpu", "performance.network"]
```

### 3. **Module defaults make sense**
```bash
# Set default for all CPU zones
POST /api/modules/cpu-temp/config {"unit":"metric"}
# Affects: system.cpu, performance.cpu, dashboard.cpu

# Override performance page only
POST /api/zones/performance.cpu/config {"unit":"imperial"}
```

### 4. **Clear hierarchy**
```
Page → Zone → Module
performance → performance.cpu → exec:./modules/cpu-temp/cpu-temp
```

---

## Migration

### Old → New Mapping

| Old ID | New ID | Page |
|--------|--------|------|
| `cpu` | `system.cpu` | System |
| `gpu` | `system.gpu` | System |
| `network` | `system.network` | System |
| `weather` | `system.weather` | System |
| `cpu-main` | `performance.cpu` | Performance |
| `gpu-main` | `performance.gpu` | Performance |
| `network-main` | `performance.network` | Performance |
| `clock` | `clock.time` | Clock |

### Updated Layout File

```yaml
name: "Multi-Page Dashboard"
version: "1.0"

pages:
  - name: "System"
    zones:
      - id: system.weather
        width: 160
        module: exec:./modules/weather/weather
        refresh_ms: 300000

      - id: system.cpu
        width: 160
        module: exec:./modules/cpu-temp/cpu-temp
        refresh_ms: 2000

      - id: system.gpu
        width: 160
        module: exec:./modules/gpu-temp/gpu-temp
        refresh_ms: 2000

      - id: system.network
        width: 160
        module: exec:./modules/network/network
        refresh_ms: 2000

  - name: "Performance"
    zones:
      - id: performance.cpu
        width: 240
        module: exec:./modules/cpu-temp/cpu-temp
        refresh_ms: 1000

      - id: performance.gpu
        width: 240
        module: exec:./modules/gpu-temp/gpu-temp
        refresh_ms: 1000

      - id: performance.network
        width: 160
        module: exec:./modules/network/network
        refresh_ms: 1000

  - name: "Clock"
    zones:
      - id: clock.time
        width: 640
        module: builtin:clock
        refresh_ms: 1000
```

---

## Naming Rules

### 1. **Page Prefix**
- Use lowercase page name (or short alias)
- Examples: `system`, `performance`, `clock`, `media`

### 2. **Module Name**
- Use the logical module name (not the full path)
- Examples: `cpu`, `gpu`, `weather`, `network`, `time`

### 3. **Optional Variant**
- Only when multiple instances of same module on same page
- Use descriptive variants: `summary`, `detailed`, `home`, `work`

### 4. **Separator**
- Use dot (`.`) as separator for clarity
- Easy to parse: `id.split('.')` → `["system", "cpu"]`

### 5. **Case**
- All lowercase
- No spaces (use hyphen if needed: `new-york`)

---

## API Impact

### Before (Confusing)
```bash
# Which CPU? Not clear from name
POST /api/zones/cpu/config {"unit":"metric"}
POST /api/zones/cpu-main/config {"unit":"imperial"}
```

### After (Clear)
```bash
# Clear which zone on which page
POST /api/zones/system.cpu/config {"unit":"metric"}
POST /api/zones/performance.cpu/config {"unit":"imperial"}

# Set default for ALL CPU zones
POST /api/modules/cpu-temp/config {"unit":"metric"}
```

---

## Examples

### Example 1: Multi-Location Weather

```yaml
pages:
  - name: "Weather"
    zones:
      - id: weather.home
        width: 320
        module: exec:./modules/weather/weather
        # Module default: Jersey City, NJ

      - id: weather.office
        width: 320
        module: exec:./modules/weather/weather
        # Override: New York, NY
```

```bash
# Set module default (used by weather.home)
POST /api/modules/weather/config {
  "location": "Jersey City, NJ",
  "unit": "imperial"
}

# Override for office widget
POST /api/zones/weather.office/config {
  "location": "New York, NY"
  # Inherits unit:"imperial" from module default
}
```

---

### Example 2: Dashboard with Multiple CPU Widgets

```yaml
pages:
  - name: "Dashboard"
    zones:
      - id: dashboard.cpu.temp
        width: 160
        module: exec:./modules/cpu-temp/cpu-temp

      - id: dashboard.cpu.load
        width: 160
        module: exec:./modules/cpu-load/cpu-load

      - id: dashboard.cpu.graph
        width: 320
        module: exec:./modules/cpu-graph/cpu-graph
```

```bash
# Different modules, same prefix - clear hierarchy
POST /api/zones/dashboard.cpu.temp/config {"unit":"metric"}
POST /api/zones/dashboard.cpu.load/config {"cores":"all"}
POST /api/zones/dashboard.cpu.graph/config {"history_seconds":60}
```

---

## Summary

**New Convention**: `{page}.{module}.{variant?}`

**Benefits**:
- ✅ Self-documenting zone IDs
- ✅ Clear page context
- ✅ Easy to parse and query
- ✅ Works well with hybrid config (module defaults + zone overrides)
- ✅ Extensible for variants

**Migration**: Simple rename in layout file, backward compatible with zone config manager

**Recommendation**: Adopt this convention for all new layouts and migrate existing ones.
