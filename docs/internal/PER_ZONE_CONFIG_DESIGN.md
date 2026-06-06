# Per-Zone Configuration Design Document

## Current State Analysis

### Configuration Architecture (As-Is)

#### 1. Global Config (`~/.config/nexus-open/config.yaml`)
**File**: [internal/config/config.go](internal/config/config.go)

**Purpose**: Application-wide settings

**Fields**:
- `location` - Weather location (e.g., "Jersey City, NJ")
- `unit` - Temperature unit (metric/imperial)
- `time_format` - Clock format (12h/24h)
- `network_format` - Network speed format (bytes/bits)
- `background_color` - Display background color
- `text_color` - Display text color
- `display` - Font settings (font_family, font_size, etc.)

**Lifecycle**:
1. Created on first app launch with defaults
2. Updated via API: `POST /api/config`
3. Broadcasts to **all plugins** via `BroadcastConfigChange()`
4. Persisted to disk automatically

**Issues**:
- Cannot have different settings per zone (e.g., CPU in °C, GPU in °F)
- All plugins receive the same config

---

#### 2. Zone Layout (`configs/layouts/multi-page.yaml`)
**File**: [internal/zone/config.go](internal/zone/config.go)

**Purpose**: Defines display layout and which plugins run in which zones

**Structure**:
```yaml
pages:
  - name: "System"
    zones:
      - id: weather
        width: 160
        plugin: exec:./plugins/weather/weather
        refresh_ms: 300000
        module_config:  # ← Recently added
          location: "Jersey City, NJ"
          unit: "imperial"
```

**Fields per Zone**:
- `id` - Unique zone identifier
- `plugin` - Plugin path (builtin:name or exec:path)
- `width` - Zone width in pixels
- `refresh_ms` - Sample interval
- `module_config` - **NEW**: Per-zone plugin configuration

**Lifecycle**:
1. Loaded on app startup
2. Hardcoded path: `configs/layouts/multi-page.yaml`
3. **No API to modify** (requires restart)
4. `module_config` sent to plugins on load

**Issues**:
- No API to update zone configs
- Must edit YAML and restart to change plugin settings
- Not user-friendly

---

#### 3. Plugin Configuration Flow

**Current Implementation**:

```
App Startup:
┌─────────────────────────────────────────┐
│ 1. Load global config.yaml              │
│ 2. Load zone layout.yaml                │
│ 3. Start plugins                        │
│ 4. Send module_config (if present)      │
└─────────────────────────────────────────┘

Runtime Config Update:
┌─────────────────────────────────────────┐
│ User: POST /api/config {"unit":"metric"}│
│ → Updates global config.yaml            │
│ → Broadcasts to ALL plugins             │
│ → Triggers immediate resample           │
└─────────────────────────────────────────┘
```

**Code Flow**:
1. API receives config update ([internal/api/handlers.go:113-133](internal/api/handlers.go#L113-L133))
2. Converts to map and broadcasts ([internal/api/handlers.go:116-131](internal/api/handlers.go#L116-L131))
3. Sampler notifies all plugins ([internal/zone/sampler.go:337-387](internal/zone/sampler.go#L337-L387))
4. Plugins receive via `OnConfigChanged()` ([pkg/plugin/interface.go](pkg/plugin/interface.go))

---

## Requirements

### User's Request
> "Per-Zone Config (New + Flexible): Need to define defaults per plugin type, API updates affect one zone"

### Functional Requirements
1. **Per-zone configuration via API**
   - Each zone can be configured independently
   - API endpoint: `POST /api/zones/{zoneID}/config`
   - Updates persist across restarts

2. **Default configurations**
   - Sensible defaults per plugin type
   - Applied when plugin first loads

3. **Real-time updates**
   - Config changes apply immediately (no restart)
   - Trigger immediate resample

4. **Persistence**
   - Zone configs saved to disk
   - Survive app restart

### Non-Functional Requirements
1. **Simplicity**: Remove complexity, not add it
2. **Consistency**: Similar API pattern to global config
3. **Performance**: No performance regression
4. **Backward compatibility**: Don't break existing plugins

---

## Design Questions

### Q1: Where to store per-zone configs?

**Option A: Extend zone layout YAML**
```yaml
zones:
  - id: cpu
    plugin: exec:./plugins/cpu-temp/cpu-temp
    module_config:
      unit: "metric"
```
- ✅ Already partially implemented
- ✅ Single source of truth
- ❌ Must parse/write YAML on every API update
- ❌ Mixing static layout with dynamic config

**Option B: Separate zone-configs.yaml**
```yaml
# zone-configs.yaml
cpu:
  unit: "metric"
gpu:
  unit: "imperial"
weather:
  location: "New York, NY"
  unit: "imperial"
```
- ✅ Clean separation of layout vs config
- ✅ Easier to manage dynamically
- ✅ Simpler API updates
- ❌ Two files to maintain

**Option C: Database/KV store**
- ❌ Too complex for this use case
- ❌ Introduces new dependency

**Recommendation**: **Option B** - Separate `zone-configs.yaml` file

---

### Q2: What about global config?

**Option A: Remove global config entirely**
- ❌ Lose UI settings (colors, fonts)
- ❌ User has to configure every zone individually

**Option B: Keep global config for non-plugin settings**
```yaml
# config.yaml (UI only)
background_color: "#000000"
text_color: "#FFFFFF"
display:
  font_family: "GoRegular"
  font_size: 11.0
```
- ✅ Keeps UI settings centralized
- ✅ Clean separation: UI config vs plugin config
- ✅ Simpler mental model

**Recommendation**: **Option B** - Keep global config for UI-only settings

---

### Q3: How to handle defaults?

**Option A: Plugin provides defaults in code**
```go
func NewCPUTempModule() *CPUTempModule {
    return &CPUTempModule{
        unit: "metric", // default
    }
}
```
- ✅ Self-contained
- ✅ No external config needed
- ❌ Hard to change defaults without code change

**Option B: Default configs in separate file**
```yaml
# default-plugin-configs.yaml
cpu-temp:
  unit: "metric"
gpu-temp:
  unit: "imperial"
weather:
  location: "Jersey City, NJ"
  unit: "imperial"
```
- ✅ Easy to customize defaults
- ✅ No code changes needed
- ❌ Another file to maintain

**Option C: Smart defaults based on plugin type + user preference**
- First load: Use plugin's hardcoded default
- User sets config: Persists to zone-configs.yaml
- Subsequent loads: Use persisted config

**Recommendation**: **Option C** - Progressive enhancement approach

---

## Proposed Solution

### Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Configuration System                      │
├─────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌──────────────────┐      ┌──────────────────────────┐    │
│  │ config.yaml      │      │ zone-configs.yaml         │    │
│  │                  │      │                           │    │
│  │ UI Settings:     │      │ Plugin Configs:           │    │
│  │ - colors         │      │   cpu:                    │    │
│  │ - fonts          │      │     unit: "metric"        │    │
│  │ - layout         │      │   gpu:                    │    │
│  │                  │      │     unit: "imperial"      │    │
│  └──────────────────┘      │   weather:                │    │
│                             │     location: "NYC"       │    │
│  API: /api/config           │     unit: "imperial"      │    │
│                             │                           │    │
│                             └──────────────────────────┘    │
│                                                               │
│                             API: /api/zones/:id/config       │
│                                                               │
└─────────────────────────────────────────────────────────────┘

Plugin Startup Flow:
1. Load zone layout (which plugins, where)
2. Check zone-configs.yaml for zone's config
3. If no config → use plugin's defaults
4. Send config to plugin via OnConfigChanged()

Runtime Update Flow:
POST /api/zones/cpu/config {"unit":"imperial"}
→ Update zone-configs.yaml
→ Broadcast to CPU plugin only
→ Trigger immediate resample
```

---

## Implementation Plan

### Phase 1: Zone Config Storage (New) ✅ Implemented

**Files to Create**:
- `internal/zoneconfig/manager.go` - Zone config manager

**Responsibilities**:
- Load `zone-configs.yaml`
- Save zone configs to disk
- Get config for specific zone
- Update config for specific zone

**Interface**:
```go
type Manager struct {
    path string
    configs map[string]map[string]interface{}
    mu sync.RWMutex
}

func NewManager(path string) (*Manager, error)
func (m *Manager) Get(zoneID string) map[string]interface{}
func (m *Manager) Set(zoneID string, config map[string]interface{}) error
func (m *Manager) GetAll() map[string]map[string]interface{}
```

---

### Phase 2: API Endpoint (New) ✅ Implemented

**Files to Modify**:
- `internal/api/server.go` - Add route
- `internal/api/handlers.go` - Add handler

**New Endpoint**:
```
POST /api/zones/:id/config
Body: {"unit": "metric", "location": "NYC"}
Response: {"status": "success", "message": "Zone config updated"}
```

**Handler Logic**:
1. Parse zone ID from URL
2. Decode JSON body
3. Update zone config file
4. Broadcast to specific plugin
5. Trigger resample

---

### Phase 3: Integration (Modify) 🚧 In Progress

**Files to Modify**:
- `internal/app/app.go` - Wire up zone config manager
- `internal/zone/sampler.go` - Load zone configs on plugin start
- `internal/api/server.go` - Pass zone config manager to API

**Changes**:
1. App creates zone config manager *(host wiring done)*
2. Sampler uses zone config on plugin load *(initial hydration implemented)*
3. API server can update zone configs *(done)*
4. Plugins consume per-zone settings without reading global config *(weather plugin migrated; remaining plugins already use notifier paths)*

---

### Phase 4: Cleanup (Remove)

**Remove from global config**:
- `location` (move to per-zone)
- `unit` (move to per-zone)
- `network_format` (move to per-zone)
- `time_format` (keep - affects clock plugin display)

**Keep in global config**:
- `background_color`
- `text_color`
- `display.*` (fonts, sizes)

---

## Migration Strategy

### Backward Compatibility

**Existing users** have:
- `config.yaml` with `location`, `unit`, etc.

**Migration approach**:
1. First launch after update: Read global config values
2. Create `zone-configs.yaml` with global values for all zones
3. Remove plugin fields from `config.yaml`
4. Future updates use per-zone API

**Migration code**:
```go
func (m *ZoneConfigManager) migrateFromGlobal(globalConfig config.Config) error {
    // If zone-configs.yaml doesn't exist, create from global
    if !fileExists(m.path) {
        // Apply global settings to all temperature plugins
        for _, zoneID := range []string{"cpu", "gpu", "weather"} {
            m.Set(zoneID, map[string]interface{}{
                "unit": globalConfig.Unit,
            })
        }
        // Apply location to weather
        m.Set("weather", map[string]interface{}{
            "location": globalConfig.Location,
            "unit": globalConfig.Unit,
        })
    }
    return nil
}
```

---

## Testing Plan

### Unit Tests
- Zone config manager load/save
- API endpoint parsing
- Config validation

### Integration Tests
1. **Test 1**: Update CPU to metric, GPU stays imperial
2. **Test 2**: Update weather location
3. **Test 3**: Restart app, configs persist
4. **Test 4**: Migration from global config

### Manual Testing
```bash
# Update CPU to Celsius
curl -X POST http://localhost:1985/api/zones/cpu/config \
  -d '{"unit":"metric"}'

# Update GPU to Fahrenheit
curl -X POST http://localhost:1985/api/zones/gpu/config \
  -d '{"unit":"imperial"}'

# Verify display shows different units
```

---

## Open Questions

1. **Q**: Should zone configs be editable via the layout API too?
   **A**: TBD - depends on if we want a zone management UI

2. **Q**: What happens if user updates layout and adds new zone?
   **A**: Use plugin defaults until user configures it

3. **Q**: Should we validate config values (e.g., unit must be metric/imperial)?
   **A**: Yes - add validation in zone config manager

4. **Q**: Error handling if plugin doesn't support ConfigNotifier?
   **A**: Return error from API, don't silently fail

---

## Success Criteria

✅ Each zone can be configured independently via API
✅ Configs persist across restarts
✅ Real-time updates (no restart needed)
✅ Backward compatible (migration works)
✅ Documentation updated
✅ Tests pass

---

## Timeline Estimate

- Phase 1 (Storage): 2-3 hours
- Phase 2 (API): 1-2 hours
- Phase 3 (Integration): 2-3 hours
- Phase 4 (Cleanup): 1 hour
- Testing: 2 hours
- Documentation: 1 hour

**Total**: ~10-12 hours

---

## Next Steps

1. Review this design doc
2. Get approval on architecture decisions
3. Implement Phase 1 (storage layer)
4. Implement Phase 2 (API endpoint)
5. Implement Phase 3 (integration)
6. Test and validate
7. Update documentation
