# Multi-Page Configuration Analysis

## The Problem

Multi-page layouts can have **multiple zones running the same module**:

```yaml
pages:
  - name: "System"
    zones:
      - id: cpu           # Small CPU widget
        width: 160
        module: exec:./modules/cpu-temp/cpu-temp

  - name: "Performance"
    zones:
      - id: cpu-main      # Large CPU widget
        width: 240
        module: exec:./modules/cpu-temp/cpu-temp
```

**Question**: Should `cpu` and `cpu-main` share config or have independent configs?

---

## Three Approaches

### Option A: Per-Zone Config (Fully Independent)

**Each zone has its own config**, even if running the same module.

**Config storage**:
```yaml
# zone-configs.yaml
cpu:
  unit: "metric"      # Page 1: CPU shows Celsius
cpu-main:
  unit: "imperial"    # Page 2: CPU shows Fahrenheit
```

**API**:
```bash
POST /api/zones/cpu/config {"unit":"metric"}
POST /api/zones/cpu-main/config {"unit":"imperial"}
```

**Pros**:
- ✅ Maximum flexibility - each zone is truly independent
- ✅ Simple implementation - zone ID is primary key
- ✅ No special handling for multi-page

**Cons**:
- ❌ Confusing UX - same module shows different values on different pages
- ❌ User must configure same module multiple times
- ❌ Config duplication

**Use case**:
- You want Page 1 to show Celsius for quick glance
- You want Page 2 to show Fahrenheit for detailed analysis

**User experience**:
```
User: "Why does my CPU temp change when I swipe pages?"
User: "I have to set the weather location twice?"
```

---

### Option B: Per-Module Config (Fully Shared)

**All zones running the same module share one config**.

**Config storage**:
```yaml
# zone-configs.yaml
_module_configs:
  "exec:./modules/cpu-temp/cpu-temp":
    unit: "metric"     # Applies to ALL CPU zones
  "exec:./modules/gpu-temp/gpu-temp":
    unit: "imperial"   # Applies to ALL GPU zones
```

**API**:
```bash
POST /api/modules/cpu-temp/config {"unit":"metric"}
# Affects both 'cpu' and 'cpu-main' zones
```

**Pros**:
- ✅ Simple mental model - one config per module
- ✅ No duplication
- ✅ Consistent across all pages

**Cons**:
- ❌ No per-zone flexibility
- ❌ Module path as config key is awkward
- ❌ What about built-in modules? `builtin:clock` vs `builtin:clock24`?

**Use case**:
- You want all CPU displays to show metric everywhere
- You want all GPU displays to show imperial everywhere

**User experience**:
```
User: "I set CPU to Celsius, why is the big CPU widget also Celsius?"
Answer: "They're the same module, they share config"
```

---

### Option C: Hybrid (Module Defaults + Zone Overrides) **[RECOMMENDED]**

**Shared by default, override per-zone when needed**.

**Config storage**:
```yaml
# zone-configs.yaml
_module_defaults:
  "exec:./modules/cpu-temp/cpu-temp":
    unit: "metric"     # Default for all CPU zones

_zone_overrides:
  cpu-main:
    unit: "imperial"   # Override only for 'cpu-main'
```

**API**:
```bash
# Set default for ALL CPU zones
POST /api/modules/cpu-temp/config {"unit":"metric"}

# Override specific zone
POST /api/zones/cpu-main/config {"unit":"imperial"}
```

**Config resolution order**:
1. Check `_zone_overrides[zoneID]`
2. Fall back to `_module_defaults[modulePath]`
3. Fall back to module's hardcoded default

**Pros**:
- ✅ Best of both worlds
- ✅ Simple common case (one config, applies everywhere)
- ✅ Flexible edge cases (override specific zones)
- ✅ Clear hierarchy and fallback chain

**Cons**:
- ⚠️ More complex implementation
- ⚠️ Two API endpoints (`/modules/:name/config` and `/zones/:id/config`)
- ⚠️ Need to explain hierarchy to users

**Use case**:
- Default: All temps in metric
- Override: Main performance page in imperial for detail work

**User experience**:
```
User: "Set all temps to Celsius" → POST /api/modules/cpu-temp/config
User: "Make the big CPU widget Fahrenheit" → POST /api/zones/cpu-main/config
```

---

## Comparison Table

| Feature | Option A (Per-Zone) | Option B (Per-Module) | Option C (Hybrid) |
|---------|---------------------|----------------------|-------------------|
| **Flexibility** | ⭐⭐⭐ | ⭐ | ⭐⭐⭐ |
| **Simplicity** | ⭐⭐ | ⭐⭐⭐ | ⭐⭐ |
| **No duplication** | ❌ | ✅ | ✅ |
| **Common use case** | ❌ Complex | ✅ Simple | ✅ Simple |
| **Edge cases** | ✅ Natural | ❌ Impossible | ✅ Supported |
| **Implementation** | ⭐⭐⭐ Simple | ⭐⭐ Medium | ⭐ Complex |

---

## Recommendation: **Option C (Hybrid)**

### Rationale

1. **Common case is simple**: Most users want consistent config
   - Set CPU to metric once → applies everywhere
   - No need to configure every zone

2. **Edge cases are possible**: Power users get flexibility
   - Override specific zones when needed
   - Clear API for both levels

3. **Future-proof**: Supports advanced use cases
   - Performance page in different units
   - Different weather locations per page
   - Detailed vs summary views

### Implementation Details

**File structure**:
```yaml
# ~/.config/nexus-open/zone-configs.yaml
_module_defaults:
  "exec:./modules/cpu-temp/cpu-temp":
    unit: "metric"
  "exec:./modules/weather/weather":
    location: "Jersey City, NJ"
    unit: "imperial"

_zone_overrides:
  cpu-main:
    unit: "imperial"  # Override for performance page
  weather-west:
    location: "San Francisco, CA"  # Different location
```

**Go code**:
```go
func (m *ZoneConfigManager) Get(zoneID, modulePath string) map[string]interface{} {
    // 1. Check zone override
    if override, exists := m.zoneOverrides[zoneID]; exists {
        return override
    }

    // 2. Check module default
    if defaults, exists := m.moduleDefaults[modulePath]; exists {
        return defaults
    }

    // 3. No config
    return nil
}
```

**API endpoints**:
```
POST /api/modules/:moduleName/config     # Set module default
GET  /api/modules/:moduleName/config     # Get module default

POST /api/zones/:zoneID/config           # Set zone override
GET  /api/zones/:zoneID/config           # Get effective config (with fallback)
DELETE /api/zones/:zoneID/config         # Remove override (fall back to module default)
```

---

## Examples

### Example 1: Simple Case (No Overrides)

**Goal**: All temperature modules show Celsius

```bash
# Set module defaults
POST /api/modules/cpu-temp/config {"unit":"metric"}
POST /api/modules/gpu-temp/config {"unit":"metric"}

# Result: ALL zones running these modules show Celsius
# - cpu (Page 1) → 28°C
# - cpu-main (Page 2) → 28°C
# - gpu (Page 1) → 52°C
# - gpu-main (Page 2) → 52°C
```

**Config file**:
```yaml
_module_defaults:
  "exec:./modules/cpu-temp/cpu-temp":
    unit: "metric"
  "exec:./modules/gpu-temp/gpu-temp":
    unit: "metric"
```

---

### Example 2: Per-Page Override

**Goal**:
- Page 1 (quick view) → Celsius
- Page 2 (performance detail) → Fahrenheit

```bash
# Set module defaults (Celsius)
POST /api/modules/cpu-temp/config {"unit":"metric"}
POST /api/modules/gpu-temp/config {"unit":"metric"}

# Override performance page zones
POST /api/zones/cpu-main/config {"unit":"imperial"}
POST /api/zones/gpu-main/config {"unit":"imperial"}

# Result:
# Page 1: cpu=28°C, gpu=52°C
# Page 2: cpu-main=82°F, gpu-main=126°F
```

**Config file**:
```yaml
_module_defaults:
  "exec:./modules/cpu-temp/cpu-temp":
    unit: "metric"
  "exec:./modules/gpu-temp/gpu-temp":
    unit: "metric"

_zone_overrides:
  cpu-main:
    unit: "imperial"
  gpu-main:
    unit: "imperial"
```

---

### Example 3: Different Locations

**Goal**: Multiple weather widgets for different cities

```bash
# Default location
POST /api/modules/weather/config {"location":"New York, NY","unit":"imperial"}

# Override for west coast widget
POST /api/zones/weather-west/config {"location":"San Francisco, CA"}

# Result:
# weather → New York, NY
# weather-west → San Francisco, CA
# Both use imperial units (inherited from module default)
```

---

## API Specification

### Module Config Endpoints

#### Set Module Default Config
```
POST /api/modules/:moduleName/config
Content-Type: application/json

{
  "unit": "metric",
  "location": "Jersey City, NJ"
}

Response: 200 OK
{
  "status": "success",
  "message": "Module config updated",
  "affected_zones": ["cpu", "cpu-main"]
}
```

#### Get Module Default Config
```
GET /api/modules/:moduleName/config

Response: 200 OK
{
  "unit": "metric",
  "location": "Jersey City, NJ"
}
```

---

### Zone Config Endpoints

#### Set Zone Override
```
POST /api/zones/:zoneID/config
Content-Type: application/json

{
  "unit": "imperial"
}

Response: 200 OK
{
  "status": "success",
  "message": "Zone config override set"
}
```

#### Get Effective Zone Config (with fallback)
```
GET /api/zones/:zoneID/config

Response: 200 OK
{
  "config": {
    "unit": "imperial"  # Could be from override or module default
  },
  "source": "zone_override"  # or "module_default" or "module_hardcoded"
}
```

#### Remove Zone Override
```
DELETE /api/zones/:zoneID/config

Response: 200 OK
{
  "status": "success",
  "message": "Zone override removed, falling back to module default"
}
```

---

## Migration Path

### For Existing Users

**Current state**: Global config with `unit: "imperial"`

**Migration logic**:
```go
func Migrate() {
    globalConfig := loadGlobalConfig()

    // Create module defaults from global config
    for _, modulePath := range getTemperatureModules() {
        setModuleDefault(modulePath, map[string]interface{}{
            "unit": globalConfig.Unit,
        })
    }

    // Weather gets both location and unit
    setModuleDefault("exec:./modules/weather/weather", map[string]interface{}{
        "location": globalConfig.Location,
        "unit": globalConfig.Unit,
    })
}
```

**Result**: Seamless upgrade, all zones continue working as before

---

## Summary

**Option C (Hybrid)** provides:
- ✅ Simple default behavior (set once, applies everywhere)
- ✅ Advanced customization when needed (per-zone overrides)
- ✅ Clear API design (two levels of configuration)
- ✅ Intuitive fallback hierarchy
- ✅ Future-proof for complex use cases

**Trade-off**: Slightly more complex implementation, but much better UX.
