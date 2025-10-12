package plugin

import (
	"context"
	"fmt"
	"log/slog"
	"strings"

	"nexus-open/internal/modules/builtin"
	"nexus-open/pkg/module"
)

// Registry manages module discovery and instantiation
type Registry struct {
	logger      *slog.Logger
	pluginHost  *Host
	builtinMods map[string]func() module.Module
}

// NewRegistry creates a new module registry
func NewRegistry(logger *slog.Logger) *Registry {
	r := &Registry{
		logger:      logger,
		pluginHost:  NewHost(logger),
		builtinMods: make(map[string]func() module.Module),
	}

	// Register built-in modules
	r.registerBuiltin("clock", func() module.Module {
		return builtin.NewClock()
	})
	r.registerBuiltin("placeholder", func() module.Module {
		return builtin.NewPlaceholder("Loading...")
	})
	r.registerBuiltin("debug", func() module.Module {
		return builtin.NewDebug("unknown", 160)
	})

	logger.Info("module registry initialized", "builtin_modules", len(r.builtinMods))

	return r
}

// registerBuiltin registers a built-in module factory
func (r *Registry) registerBuiltin(name string, factory func() module.Module) {
	r.builtinMods[name] = factory
	r.logger.Debug("registered built-in module", "name", name)
}

// GetModule retrieves or launches a module by endpoint
// Endpoints:
//   - builtin:name  → Built-in module
//   - exec:path     → External module via plugin
func (r *Registry) GetModule(ctx context.Context, zoneID, endpoint string) (module.Module, error) {
	parts := strings.SplitN(endpoint, ":", 2)
	if len(parts) != 2 {
		return nil, fmt.Errorf("invalid module endpoint: %s (expected type:value)", endpoint)
	}

	moduleType := parts[0]
	moduleValue := parts[1]

	switch moduleType {
	case "builtin":
		return r.getBuiltin(moduleValue)

	case "exec":
		return r.getExternal(ctx, zoneID, moduleValue)

	default:
		return nil, fmt.Errorf("unknown module type: %s (expected builtin or exec)", moduleType)
	}
}

// getBuiltin returns a built-in module instance
func (r *Registry) getBuiltin(name string) (module.Module, error) {
	factory, ok := r.builtinMods[name]
	if !ok {
		return nil, fmt.Errorf("built-in module not found: %s", name)
	}

	mod := factory()
	r.logger.Debug("created built-in module", "name", name)

	return mod, nil
}

// getExternal launches or retrieves an external module
func (r *Registry) getExternal(ctx context.Context, zoneID, path string) (module.Module, error) {
	// Try to get existing module
	mod, err := r.pluginHost.GetModule(zoneID)
	if err == nil {
		r.logger.Debug("reusing external module", "zone_id", zoneID, "path", path)
		return mod, nil
	}

	// Launch new plugin
	mod, err = r.pluginHost.LaunchModule(ctx, zoneID, path)
	if err != nil {
		return nil, fmt.Errorf("failed to launch module: %w", err)
	}

	r.logger.Debug("launched external module", "zone_id", zoneID, "path", path)

	return mod, nil
}

// ListBuiltin returns names of all built-in modules
func (r *Registry) ListBuiltin() []string {
	names := make([]string, 0, len(r.builtinMods))
	for name := range r.builtinMods {
		names = append(names, name)
	}
	return names
}

// Shutdown cleans up all resources
func (r *Registry) Shutdown() {
	r.logger.Info("shutting down module registry")
	r.pluginHost.KillAll()
}
