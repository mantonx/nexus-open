package builtin

import (
	"fmt"
	"time"

	"github.com/mantonx/nexus-next/pkg/module"
)

// DebugModule displays debug information about the zone
type DebugModule struct {
	zoneID string
	width  int
}

// NewDebug creates a new debug module
func NewDebug(zoneID string, width int) *DebugModule {
	return &DebugModule{
		zoneID: zoneID,
		width:  width,
	}
}

// Describe returns module metadata
func (m *DebugModule) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "Debug",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Displays zone debug information",
		Icon:        "bug",
		RefreshMs:   1000,
	}, nil
}

// Sample returns debug payload
func (m *DebugModule) Sample() (module.Payload, error) {
	return module.Payload{
		Primary:   fmt.Sprintf("Zone: %s", m.zoneID),
		Secondary: fmt.Sprintf("%dpx wide", m.width),
		Severity:  module.SeverityOK,
		TTL:       1 * time.Second,
		Icon:      "bug",
		Timestamp: time.Now(),
	}, nil
}

// OnConfigChanged implements module.ConfigNotifier interface.
// Debug module doesn't use configuration, so this is a no-op.
func (m *DebugModule) OnConfigChanged(config map[string]interface{}) error {
	// Debug module doesn't need configuration
	return nil
}
