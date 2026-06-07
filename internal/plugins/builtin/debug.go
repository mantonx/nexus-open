package builtin

import (
	"fmt"
	"time"

	"github.com/mantonx/nexus-next/pkg/plugin"
)

// DebugPlugin displays debug information about the zone
type DebugPlugin struct {
	zoneID string
	width  int
}

// NewDebug creates a new debug plugin
func NewDebug(zoneID string, width int) *DebugPlugin {
	return &DebugPlugin{
		zoneID: zoneID,
		width:  width,
	}
}

// Describe returns plugin metadata
func (m *DebugPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "Debug",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Displays zone debug information",
		Icon:        "bug",
		RefreshMs:   1000,
	}, nil
}

// Sample returns debug payload
func (m *DebugPlugin) Sample() (plugin.Payload, error) {
	return plugin.Payload{
		Primary:   fmt.Sprintf("Zone: %s", m.zoneID),
		Secondary: fmt.Sprintf("%dpx wide", m.width),
		Severity:  plugin.SeverityOK,
		TTL:       1 * time.Second,
		Icon:      "bug",
		Timestamp: time.Now(),
	}, nil
}

// Configure implements plugin.Plugin. Debug has no configurable fields.
func (m *DebugPlugin) Configure(_ map[string]any) error { return nil }
