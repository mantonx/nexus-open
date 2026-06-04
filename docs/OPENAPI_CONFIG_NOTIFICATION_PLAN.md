# OpenAPI 3 + Module Config Notification System

**Status**: Planning
**Created**: 2025-10-21
**Goal**: Add OpenAPI 3 spec for Flutter/Go communication + Generic module config change notifications

## Table of Contents

1. [Overview](#overview)
2. [Current State](#current-state)
3. [Problems to Solve](#problems-to-solve)
4. [Proposed Solution](#proposed-solution)
5. [Implementation Phases](#implementation-phases)
6. [Technical Details](#technical-details)
7. [Testing Strategy](#testing-strategy)
8. [Migration Path](#migration-path)

---

## Overview

### Objectives

1. **OpenAPI 3 Integration**: Add type-safe API contract between Flutter UI and Go backend
2. **Config Notification System**: Replace file-watching with event-driven config updates for modules
3. **Developer Experience**: Auto-generate Dart models and API clients from Go code

### Key Benefits

- **Type Safety**: Compile-time errors instead of runtime errors
- **Auto-Generation**: Dart models generated from OpenAPI spec
- **Real-time Updates**: Modules receive config changes via RPC instead of polling files
- **Scalability**: Works for any number of modules
- **Documentation**: Self-documenting API

---

## Current State

### API Structure

**File**: `internal/api/handlers.go`

Current endpoints:
- `GET /api/health` - Health check
- `GET /api/config` - Get current configuration
- `POST /api/config` - Update configuration
- `POST /api/images/upload` - Upload background image
- `GET /api/images` - List images
- `DELETE /api/images` - Delete image
- `POST /api/brightness` - Set display brightness
- `GET /api/device/info` - Get device information
- `GET /api/window/state` - Get window state
- `POST /api/window/show` - Show settings window
- `POST /api/window/hide` - Hide settings window

### Current Config Update Flow

```
Flutter UI
    ↓ POST /api/config
API Handler
    ↓ Save to file (~/.config/nexus-open/config.yaml)
File System
    ↓ fsnotify detects change
Weather Module (watches file)
    ↓ Reloads config
```

### Module RPC Interface

**File**: `pkg/module/types.go`

```go
type Module interface {
    Describe() (Descriptor, error)
    Sample() (Payload, error)
}
```

**Current limitation**: Modules only respond to host requests, cannot receive push notifications

---

## Problems to Solve

### 1. API Type Safety

**Problem**: Flutter manually constructs JSON, no compile-time validation
```dart
// Current - error-prone
final response = await http.post(
  Uri.parse('$baseUrl/api/config'),
  body: jsonEncode({
    'location': location,  // typo? wrong field name?
    'unit': unit,
  }),
);
```

**Solution**: Generated type-safe client
```dart
// Desired - type-safe
await api.configApi.updateConfig(
  ConfigUpdate(
    location: location,
    unit: UnitEnum.imperial,
  ),
);
```

### 2. Config Notification

**Problem**: Each module implements its own file watching
- Duplicated code across modules
- File system polling overhead
- Delay between API update and module receiving change
- Module-specific implementation (not reusable)

**Solution**: Event-driven RPC notifications
- API broadcasts to all modules
- Real-time, no polling
- Generic implementation for all modules

### 3. Module Independence

**Problem**: Modules need to decide which config changes matter to them

**Solution**: Modules implement optional `ConfigNotifier` interface
```go
type ConfigNotifier interface {
    OnConfigChanged(config map[string]interface{}) error
}
```

---

## Proposed Solution

### Architecture Overview

```
┌──────────────────────────────────────────────────┐
│        OpenAPI 3 Specification                   │
│        (api/openapi.yaml)                        │
└────────────┬──────────────────┬──────────────────┘
             ↓                  ↓
    ┌────────────────┐   ┌─────────────────────┐
    │   Go Backend   │   │   Flutter Client    │
    │   (annotated)  │   │   (generated)       │
    └────────┬───────┘   └─────────┬───────────┘
             ↓                     ↓
    ┌────────────────────────────────────────────┐
    │         Config Update Flow                 │
    └────────────────────────────────────────────┘
             ↓
    POST /api/config (type-safe)
             ↓
    API Handler validates & saves
             ↓
    Broadcast to all modules via RPC
             ↓
    ┌────────┬────────┬──────────┬──────────┐
    ↓        ↓        ↓          ↓          ↓
Weather   CPU     GPU       Network    (future)
Module   Module  Module     Module     modules
```

### New Config Flow

```
Flutter UI
    ↓ POST /api/config (type-safe Dart model)
API Handler
    ↓ Validates (OpenAPI schema)
    ↓ Saves to file
    ↓ Broadcasts via ModuleNotifier
RPC Layer
    ↓ OnConfigChanged(config map)
Modules implementing ConfigNotifier
    ↓ Extract relevant config
    ↓ Update internal state
```

---

## Implementation Phases

### Phase 0: OpenAPI 3 Infrastructure ⚙️

**Goal**: Set up OpenAPI generation for existing API

#### Step 0.1: Install Dependencies

```bash
# Go dependencies
go get -u github.com/swaggo/swag/cmd/swag
go get -u github.com/swaggo/http-swagger

# Install swag CLI
go install github.com/swaggo/swag/cmd/swag@latest

# Flutter dependencies (in ui/)
flutter pub add dio retrofit
flutter pub add --dev retrofit_generator build_runner json_serializable
```

#### Step 0.2: Add Swagger Annotations to Existing Handlers

**File**: `internal/api/handlers.go`

Example for config endpoint:
```go
// @Summary Get configuration
// @Description Returns the current application configuration
// @Tags config
// @Produce json
// @Success 200 {object} config.Config
// @Failure 500 {object} ErrorResponse
// @Router /api/config [get]
func (s *Server) handleGetConfig(w http.ResponseWriter, r *http.Request) {
    // existing implementation
}

// @Summary Update configuration
// @Description Updates the application configuration and notifies modules
// @Tags config
// @Accept json
// @Produce json
// @Param config body config.Config true "Configuration object"
// @Success 200 {object} SuccessResponse
// @Failure 400 {object} ErrorResponse "Invalid configuration"
// @Failure 500 {object} ErrorResponse "Failed to save"
// @Router /api/config [post]
func (s *Server) handleUpdateConfig(w http.ResponseWriter, r *http.Request) {
    // existing implementation
}
```

#### Step 0.3: Add Main API Documentation

**File**: `cmd/nexus-open/main.go`

```go
// @title Nexus Open API
// @version 2.0
// @description API for Nexus Open - iCUE Nexus companion app
// @contact.name Nexus Team
// @contact.url https://github.com/mantonx/nexus-next

// @host localhost:1985
// @BasePath /

// @schemes http

package main
```

#### Step 0.4: Generate OpenAPI Spec

```bash
# From project root
swag init -g cmd/nexus-open/main.go -o api/

# This generates:
# - api/docs.go
# - api/swagger.json
// - api/swagger.yaml
```

#### Step 0.5: Serve OpenAPI Spec

**File**: `internal/api/server.go`

```go
import httpSwagger "github.com/swaggo/http-swagger"

func (s *Server) setupRoutes() {
    // Existing routes...

    // Swagger UI
    s.mux.HandleFunc("/swagger/", httpSwagger.WrapHandler)

    // OpenAPI spec JSON
    s.mux.HandleFunc("/api/openapi.json", s.handleOpenAPISpec)
}

func (s *Server) handleOpenAPISpec(w http.ResponseWriter, r *http.Request) {
    http.ServeFile(w, r, "api/swagger.json")
}
```

#### Step 0.6: Generate Flutter API Client

**Create**: `scripts/generate-flutter-api.sh`

```bash
#!/bin/bash

# Generate Dart API client from OpenAPI spec
cd "$(dirname "$0")/.."

echo "Generating Flutter API client from OpenAPI spec..."

# Ensure openapi-generator is installed
if ! command -v openapi-generator &> /dev/null; then
    echo "Error: openapi-generator not found"
    echo "Install: npm install -g @openapitools/openapi-generator-cli"
    exit 1
fi

# Generate Dart client
openapi-generator generate \
    -i api/swagger.json \
    -g dart-dio \
    -o ui/lib/generated/api \
    --additional-properties=pubName=nexus_api,pubAuthor="Nexus Team"

echo "Flutter API client generated!"
echo "Import in Dart: import 'package:nexus_api/nexus_api.dart';"
```

```bash
chmod +x scripts/generate-flutter-api.sh
```

#### Step 0.7: Update dev.sh to Auto-Generate

**File**: `dev.sh`

```bash
# Add after Flutter build, before starting backend

# Generate OpenAPI spec
echo "Generating OpenAPI spec..."
swag init -g cmd/nexus-open/main.go -o api/ -q

# Generate Flutter API client
echo "Generating Flutter API client..."
./scripts/generate-flutter-api.sh
```

---

### Phase 1: Extend Module Interface 🔌

**Goal**: Add optional config notification interface

#### Step 1.1: Add ConfigNotifier Interface

**File**: `pkg/module/types.go`

```go
// ConfigNotifier is an optional interface modules can implement
// to receive real-time configuration change notifications
type ConfigNotifier interface {
    // OnConfigChanged is called when the global configuration is updated.
    // The module should inspect the config map and update its state if relevant.
    //
    // Args:
    //   config: Full configuration as key-value map
    //
    // Returns:
    //   error if the module failed to process the config change
    OnConfigChanged(config map[string]interface{}) error
}
```

#### Step 1.2: Type Assertion Helper

**File**: `pkg/module/types.go`

```go
// SupportsConfigNotification checks if a module implements ConfigNotifier
func SupportsConfigNotification(m Module) (ConfigNotifier, bool) {
    notifier, ok := m.(ConfigNotifier)
    return notifier, ok
}
```

---

### Phase 2: Update RPC Layer 📡

**Goal**: Add RPC support for config notifications

#### Step 2.1: Add RPC Method to Client

**File**: `pkg/module/plugin.go`

```go
// OnConfigChanged sends a config change notification to the plugin
func (c *RPCClient) OnConfigChanged(config map[string]interface{}) error {
    var resp struct{} // Empty response
    err := c.client.Call("Plugin.OnConfigChanged", config, &resp)
    if err != nil && err.Error() == "rpc: can't find method Plugin.OnConfigChanged" {
        // Module doesn't implement ConfigNotifier, silently ignore
        return nil
    }
    return err
}
```

#### Step 2.2: Add RPC Method to Server

**File**: `pkg/module/plugin.go`

```go
// OnConfigChanged implements the OnConfigChanged RPC
func (s *RPCServer) OnConfigChanged(config map[string]interface{}, resp *struct{}) error {
    // Check if module implements ConfigNotifier
    if notifier, ok := s.Impl.(ConfigNotifier); ok {
        return notifier.OnConfigChanged(config)
    }
    // Module doesn't implement interface, no-op
    return nil
}
```

#### Step 2.3: Register Types for Encoding

**File**: `pkg/module/plugin.go`

```go
func init() {
    // Register types for gob encoding
    gob.Register(Descriptor{})
    gob.Register(Payload{})
    gob.Register(map[string]interface{}{}) // For config notifications
}
```

---

### Phase 3: Add Broadcast Mechanism 📢

**Goal**: Create a notifier to broadcast config changes to all modules

#### Step 3.1: Create Module Notifier

**New File**: `internal/modules/notifier.go`

```go
package modules

import (
    "fmt"
    "log/slog"
    "sync"

    "nexus-open/pkg/module"
)

// ModuleNotifier broadcasts configuration changes to all active modules
type ModuleNotifier struct {
    mu      sync.RWMutex
    modules map[string]module.Module // zone_id -> module RPC client
    logger  *slog.Logger
}

// NewModuleNotifier creates a new notifier
func NewModuleNotifier(logger *slog.Logger) *ModuleNotifier {
    return &ModuleNotifier{
        modules: make(map[string]module.Module),
        logger:  logger,
    }
}

// RegisterModule adds a module to receive config notifications
func (n *ModuleNotifier) RegisterModule(zoneID string, mod module.Module) {
    n.mu.Lock()
    defer n.mu.Unlock()
    n.modules[zoneID] = mod
    n.logger.Debug("module registered for notifications", "zone_id", zoneID)
}

// UnregisterModule removes a module from notifications
func (n *ModuleNotifier) UnregisterModule(zoneID string) {
    n.mu.Lock()
    defer n.mu.Unlock()
    delete(n.modules, zoneID)
    n.logger.Debug("module unregistered from notifications", "zone_id", zoneID)
}

// BroadcastConfigChange sends config update to all registered modules
func (n *ModuleNotifier) BroadcastConfigChange(config map[string]interface{}) {
    n.mu.RLock()
    defer n.mu.RUnlock()

    n.logger.Info("broadcasting config change to modules", "module_count", len(n.modules))

    for zoneID, mod := range n.modules {
        // Check if module implements ConfigNotifier via RPC
        if rpcClient, ok := mod.(*module.RPCClient); ok {
            if err := rpcClient.OnConfigChanged(config); err != nil {
                n.logger.Warn("failed to notify module",
                    "zone_id", zoneID,
                    "error", err)
            } else {
                n.logger.Debug("config change notification sent",
                    "zone_id", zoneID)
            }
        }
    }
}
```

#### Step 3.2: Integrate Notifier in App

**File**: `internal/app/app.go`

```go
import "nexus-open/internal/modules"

type App struct {
    // ... existing fields
    moduleNotifier *modules.ModuleNotifier
}

func New(cfg *config.Manager, logger *slog.Logger) (*App, error) {
    // ... existing initialization

    app := &App{
        // ... existing fields
        moduleNotifier: modules.NewModuleNotifier(logger),
    }

    return app, nil
}

// Expose notifier for API server
func (a *App) ModuleNotifier() *modules.ModuleNotifier {
    return a.moduleNotifier
}
```

#### Step 3.3: Register Modules with Notifier

**File**: `internal/zone/manager.go` (or wherever modules are launched)

```go
// When launching a module plugin:
func (m *Manager) launchModule(zoneID string, modulePath string) error {
    // ... existing plugin launch code

    // Get RPC client
    rpcClient := raw.(module.Module)

    // Register with notifier for config updates
    m.app.ModuleNotifier().RegisterModule(zoneID, rpcClient)

    // ... rest of launch code
}

// When stopping a module:
func (m *Manager) stopModule(zoneID string) {
    // ... existing stop code

    // Unregister from notifier
    m.app.ModuleNotifier().UnregisterModule(zoneID)
}
```

---

### Phase 4: Integrate with API Handler 🔗

**Goal**: Broadcast config changes when API receives updates

#### Step 4.1: Update API Server to Accept Notifier

**File**: `internal/api/server.go`

```go
import "nexus-open/internal/modules"

type Server struct {
    // ... existing fields
    moduleNotifier *modules.ModuleNotifier
}

func New(addr string, cfg *config.Manager, device device.Device, logger *slog.Logger, notifier *modules.ModuleNotifier) *Server {
    return &Server{
        // ... existing fields
        moduleNotifier: notifier,
    }
}
```

#### Step 4.2: Update App to Pass Notifier

**File**: `internal/app/app.go`

```go
func (a *App) Start() error {
    // ... existing code

    // Create API server with notifier
    apiServer := api.New(":1985", a.cfg, a.device, a.logger, a.moduleNotifier)

    // ... rest of start code
}
```

#### Step 4.3: Broadcast in Config Update Handler

**File**: `internal/api/handlers.go`

```go
// @Summary Update configuration
// @Description Updates the application configuration and notifies all modules
// @Tags config
// @Accept json
// @Produce json
// @Param config body config.Config true "Configuration object"
// @Success 200 {object} SuccessResponse "Configuration updated successfully"
// @Failure 400 {object} ErrorResponse "Invalid configuration"
// @Failure 500 {object} ErrorResponse "Failed to save configuration"
// @Router /api/config [post]
func (s *Server) handleUpdateConfig(w http.ResponseWriter, r *http.Request) {
    var newConfig config.Config

    if err := json.NewDecoder(r.Body).Decode(&newConfig); err != nil {
        s.respondError(w, "Invalid JSON: "+err.Error(), http.StatusBadRequest)
        return
    }

    // Validate configuration
    if err := newConfig.Validate(); err != nil {
        s.respondError(w, "Invalid configuration: "+err.Error(), http.StatusBadRequest)
        return
    }

    // Update configuration file
    if err := s.cfg.Update(newConfig); err != nil {
        s.logger.Error("failed to update config", "error", err)
        s.respondError(w, "Failed to save configuration", http.StatusInternalServerError)
        return
    }

    // Broadcast to all modules
    configMap := configToMap(newConfig)
    s.moduleNotifier.BroadcastConfigChange(configMap)

    s.logger.Info("configuration updated and broadcast to modules")
    s.respondSuccess(w, "Configuration updated successfully", nil)
}

// Helper to convert Config struct to map
func configToMap(cfg config.Config) map[string]interface{} {
    return map[string]interface{}{
        "location":   cfg.Location,
        "unit":       cfg.Unit,
        "brightness": cfg.Brightness,
        // Add other fields as needed
    }
}
```

---

### Phase 5: Update Weather Module 🌤️

**Goal**: Migrate weather module from file watching to RPC notifications

#### Step 5.1: Remove File Watching

**File**: `modules/weather/main.go`

```go
// Remove this line from NewWeatherModule():
// go wm.watchConfigChanges()  // DELETE THIS

func NewWeatherModule() *WeatherModule {
    homeDir, _ := os.UserHomeDir()
    configPath := filepath.Join(homeDir, ".config", "nexus-open", "config.yaml")

    wm := &WeatherModule{
        configPath:  configPath,
        coordsCache: make(map[string]coords),
        location:    "Jersey City, NJ",
        unit:        "imperial",
    }

    // Initial config load
    wm.loadConfig()

    return wm
}
```

#### Step 5.2: Implement ConfigNotifier Interface

**File**: `modules/weather/main.go`

```go
// OnConfigChanged implements module.ConfigNotifier
func (m *WeatherModule) OnConfigChanged(config map[string]interface{}) error {
    m.mu.Lock()
    defer m.mu.Unlock()

    changed := false

    // Extract location
    if loc, ok := config["location"].(string); ok && loc != "" {
        if m.location != loc {
            fmt.Printf("weather: location changed: %s -> %s\n", m.location, loc)
            m.location = loc
            changed = true
        }
    }

    // Extract unit
    if unit, ok := config["unit"].(string); ok && unit != "" {
        if m.unit != unit {
            fmt.Printf("weather: unit changed: %s -> %s\n", m.unit, unit)
            m.unit = unit
            changed = true
        }
    }

    // Invalidate cache if config changed
    if changed {
        m.cachedData = nil
        fmt.Println("weather: cache invalidated due to config change")
    }

    return nil
}
```

#### Step 5.3: Delete watch.go

**File**: `modules/weather/watch.go`

```bash
# Delete this file - no longer needed
rm modules/weather/watch.go
```

#### Step 5.4: Update Sample() Method

**File**: `modules/weather/main.go`

```go
// Sample returns current weather data
func (m *WeatherModule) Sample() (module.Payload, error) {
    // No longer need to call loadConfig() here - config updates via RPC

    // Check cache
    m.mu.RLock()
    if m.cachedData != nil && time.Since(m.lastUpdate) < cacheTimeout {
        data := m.cachedData
        m.mu.RUnlock()
        return m.formatPayload(data), nil
    }
    m.mu.RUnlock()

    // ... rest of existing implementation
}
```

---

### Phase 6: OpenAPI Models for Config 📝

**Goal**: Add proper OpenAPI schemas for config objects

#### Step 6.1: Add Config Struct Annotations

**File**: `internal/config/config.go`

```go
// Config represents the application configuration
// @Description Application configuration settings
type Config struct {
    // Location for weather information
    // @example "New York, NY"
    Location string `json:"location" yaml:"location" example:"Jersey City, NJ"`

    // Temperature unit (metric or imperial)
    // @example "imperial"
    Unit string `json:"unit" yaml:"unit" enums:"metric,imperial" example:"imperial"`

    // Display brightness (0-100)
    // @example 80
    Brightness int `json:"brightness,omitempty" yaml:"brightness,omitempty" minimum:"0" maximum:"100" example:"80"`
}
```

#### Step 6.2: Regenerate OpenAPI Spec

```bash
swag init -g cmd/nexus-open/main.go -o api/
./scripts/generate-flutter-api.sh
```

---

## Technical Details

### OpenAPI Tools

**Go Side**:
- **swaggo/swag**: Generates OpenAPI spec from annotations
- **swaggo/http-swagger**: Serves Swagger UI

**Flutter Side**:
- **openapi-generator**: Generates Dart client from spec
- **dio**: HTTP client used by generated code
- **retrofit**: REST API annotations (optional)

### RPC Communication

**go-plugin** uses `net/rpc` with `gob` encoding:
- Synchronous RPC calls
- Type must be registered with `gob.Register()`
- Methods must follow signature: `MethodName(args Type, resp *ResponseType) error`

### Config Serialization

```go
// Go struct -> map[string]interface{} -> gob encoding -> RPC
// RPC -> gob decoding -> map[string]interface{} -> Module extracts fields
```

---

## Testing Strategy

### Unit Tests

**Test**: `pkg/module/types_test.go`
```go
func TestConfigNotifier(t *testing.T) {
    // Test that modules implementing ConfigNotifier are detected
    // Test that modules NOT implementing it return false
}
```

**Test**: `internal/modules/notifier_test.go`
```go
func TestBroadcastConfigChange(t *testing.T) {
    // Mock module implementing ConfigNotifier
    // Register mock
    // Broadcast config
    // Verify OnConfigChanged was called with correct config
}
```

**Test**: `modules/weather/config_test.go`
```go
func TestWeatherModuleConfigNotification(t *testing.T) {
    wm := NewWeatherModule()

    // Send config change
    config := map[string]interface{}{
        "location": "San Francisco, CA",
        "unit":     "metric",
    }

    err := wm.OnConfigChanged(config)
    assert.NoError(t, err)
    assert.Equal(t, "San Francisco, CA", wm.location)
    assert.Equal(t, "metric", wm.unit)
}
```

### Integration Tests

**Test**: `test/integration/config_notification_test.go`
```go
func TestConfigUpdateFlow(t *testing.T) {
    // Start app with test config
    // Launch weather module
    // POST /api/config with new location
    // Wait briefly
    // Sample weather module
    // Verify new location is reflected
}
```

### Manual Testing

1. Start dev environment: `./dev.sh`
2. Open Swagger UI: `http://localhost:1985/swagger/`
3. Test POST `/api/config` with different locations
4. Check logs for "broadcasting config change to modules"
5. Verify weather module updates without file watching

---

## Migration Path

### Step-by-Step Rollout

1. ✅ **Phase 0**: Add OpenAPI (non-breaking, additive)
2. ✅ **Phase 1-2**: Extend module interface (non-breaking, optional)
3. ✅ **Phase 3**: Add notifier (non-breaking, infrastructure)
4. ✅ **Phase 4**: API broadcasts (works with or without modules listening)
5. ✅ **Phase 5**: Migrate weather module (one module at a time)
6. 🔄 **Future**: Migrate other modules as needed

### Backward Compatibility

- Old modules without `ConfigNotifier`: **Still work** (RPC call silently ignored)
- API still writes to file: **File-based modules still work**
- OpenAPI spec: **Does not break existing HTTP clients**

### Rollback Plan

If issues arise:
1. Revert API handler changes (remove broadcast call)
2. Weather module falls back to file watching
3. OpenAPI spec remains (doesn't affect runtime)

---

## Future Enhancements

### Phase 7: WebSocket Config Streaming (Optional)

For real-time config updates in Flutter UI:
```
Flutter UI
    ↓ WebSocket connection
API Server
    ↓ Stream config changes
Flutter UI auto-updates settings screen
```

### Phase 8: Per-Module Config (Optional)

Allow modules to declare their config schema:
```go
type ConfigurableModule interface {
    ConfigSchema() ConfigSchema
    OnConfigChanged(moduleConfig map[string]interface{}) error
}
```

### Phase 9: Config Validation (Optional)

Use OpenAPI schema to validate config in Go:
```go
import "github.com/getkin/kin-openapi/openapi3"

func ValidateConfig(config Config) error {
    // Validate against OpenAPI schema
}
```

---

## Success Criteria

✅ **OpenAPI Integration**:
- [ ] Swagger UI accessible at `/swagger/`
- [ ] All existing endpoints documented
- [ ] Flutter client can be generated from spec
- [ ] Type-safe Dart models for all API objects

✅ **Config Notification**:
- [ ] Weather module receives config changes via RPC
- [ ] No file watching in weather module
- [ ] Config updates from Flutter UI work in < 100ms
- [ ] Multiple modules can receive notifications independently

✅ **Developer Experience**:
- [ ] `dev.sh` auto-generates OpenAPI spec
- [ ] `dev.sh` auto-generates Flutter client
- [ ] New API endpoints only require swag annotations
- [ ] Module developers can opt-in to config notifications easily

✅ **Testing**:
- [ ] Unit tests for config notification system
- [ ] Integration test for full config update flow
- [ ] Manual testing shows real-time updates

---

## Timeline Estimate

- **Phase 0** (OpenAPI): 2-3 hours
- **Phase 1-2** (Module interface + RPC): 1-2 hours
- **Phase 3** (Notifier): 1-2 hours
- **Phase 4** (API integration): 1 hour
- **Phase 5** (Weather module): 1 hour
- **Testing**: 2 hours
- **Total**: ~8-12 hours

---

## Questions & Decisions

### Q1: Should we use `map[string]interface{}` or structured config?

**Decision**: Use `map[string]interface{}` for flexibility
- Allows any module to extract its relevant fields
- No coupling between module config and global config struct
- Easier to extend in the future

### Q2: Should config notification be synchronous or async?

**Decision**: Synchronous (wait for module to process)
- Simpler error handling
- Guaranteed order of updates
- Modules should process quickly (< 10ms typically)

### Q3: What if a module's OnConfigChanged fails?

**Decision**: Log warning, continue broadcasting to other modules
- One module's failure shouldn't block others
- Module will get next config on next Sample() call

### Q4: Should we keep file-based config as backup?

**Decision**: Yes, keep writing to file
- Allows modules to load initial config on startup
- Provides persistence across restarts
- Fallback if RPC notification fails

---

## References

- [Swaggo Documentation](https://github.com/swaggo/swag)
- [OpenAPI 3.0 Spec](https://swagger.io/specification/)
- [go-plugin Documentation](https://github.com/hashicorp/go-plugin)
- [Dart OpenAPI Generator](https://openapi-generator.tech/docs/generators/dart-dio)

---

**Next Steps**: Begin Phase 0 - OpenAPI Infrastructure
