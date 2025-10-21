// Package module defines the interface and types for Nexus Open modules.
// Modules are plugins that provide data to zones via RPC.
package module

import "time"

// Descriptor contains metadata about a module.
type Descriptor struct {
	Name        string `json:"name"`         // Human-readable name (e.g., "CPU Temperature")
	Version     string `json:"version"`      // Semantic version (e.g., "1.0.0")
	Author      string `json:"author"`       // Author name or organization
	Description string `json:"description"`  // Brief description of functionality
	Icon        string `json:"icon"`         // Default icon identifier (Font Awesome or emoji)
	RefreshMs   int    `json:"refresh_ms"`   // Recommended refresh interval in milliseconds
}

// Payload represents data returned by a module to be rendered in a zone.
type Payload struct {
	// Title - Optional zone header (usually omitted for space)
	Title string `json:"title,omitempty"`

	// Primary - Main value displayed (14-16px bold)
	// Examples: "42°C", "↓58 MB/s", "Now Playing"
	Primary string `json:"primary"`

	// Secondary - Subtext or context (10px, muted color)
	// Examples: "Load 31%", "Albany ☀️", "Radiohead"
	Secondary string `json:"secondary,omitempty"`

	// Spark - Sparkline data (normalized 0.0-1.0, max 60 points)
	// Rendered as small bars/line at bottom of zone
	Spark []float32 `json:"spark,omitempty"`

	// Severity - Visual severity indicator: "ok", "warn", "crit"
	// Affects primary text color
	Severity Severity `json:"severity,omitempty"`

	// TTL - Cache lifetime
	// Host will re-use this payload until TTL expires
	TTL time.Duration `json:"ttl,omitempty"`

	// Icon - Icon override (Font Awesome name or emoji)
	Icon string `json:"icon,omitempty"`

	// Progress - Progress bar value (0.0-1.0)
	// Rendered as horizontal bar (for media playback, etc.)
	Progress float32 `json:"progress,omitempty"`

	// Timestamp - When this payload was generated
	Timestamp time.Time `json:"timestamp,omitempty"`
}

// Severity levels for visual indication
type Severity string

const (
	SeverityOK   Severity = "ok"   // Normal operation (accent color)
	SeverityWarn Severity = "warn" // Warning threshold (yellow/orange)
	SeverityCrit Severity = "crit" // Critical state (red)
)

// Validate checks if the payload meets requirements
func (p *Payload) Validate() error {
	if p.Primary == "" {
		return ErrEmptyPrimary
	}

	if p.Severity != "" && p.Severity != SeverityOK && p.Severity != SeverityWarn && p.Severity != SeverityCrit {
		return ErrInvalidSeverity
	}

	if len(p.Spark) > 60 {
		return ErrSparkTooLong
	}

	for i, v := range p.Spark {
		if v < 0.0 || v > 1.0 {
			return &ErrSparkOutOfRange{Index: i, Value: v}
		}
	}

	if p.Progress < 0.0 || p.Progress > 1.0 {
		return ErrProgressOutOfRange
	}

	return nil
}

// IsExpired checks if the payload has exceeded its TTL
func (p *Payload) IsExpired() bool {
	if p.TTL == 0 {
		return false // No TTL means never expires
	}
	return time.Since(p.Timestamp) > p.TTL
}

// Module is the interface that all modules must implement.
// This will be used with go-plugin RPC in Phase 2.
type Module interface {
	// Describe returns module metadata
	Describe() (Descriptor, error)

	// Sample returns current data payload
	Sample() (Payload, error)
}

// ConfigNotifier is an optional interface modules can implement
// to receive real-time configuration change notifications.
//
// When implemented, the host will call OnConfigChanged whenever
// the global configuration is updated via the API, allowing modules
// to react to config changes without polling or file watching.
type ConfigNotifier interface {
	// OnConfigChanged is called when the global configuration is updated.
	// The module should inspect the config map and update its state if relevant.
	//
	// Args:
	//   config: Full configuration as key-value map (e.g., location, unit, time_format)
	//
	// Returns:
	//   error if the module failed to process the config change
	OnConfigChanged(config map[string]interface{}) error
}

// SupportsConfigNotification checks if a module implements ConfigNotifier.
// This allows the host to conditionally broadcast config changes only to
// modules that can handle them.
//
// Example:
//
//	if notifier, ok := module.SupportsConfigNotification(m); ok {
//	    notifier.OnConfigChanged(configMap)
//	}
func SupportsConfigNotification(m Module) (ConfigNotifier, bool) {
	notifier, ok := m.(ConfigNotifier)
	return notifier, ok
}
