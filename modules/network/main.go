// network is a module that monitors network upload/download speeds
package main

import (
	"fmt"
	"sync"
	"time"

	"github.com/hashicorp/go-plugin"
	"github.com/shirou/gopsutil/net"

	"github.com/mantonx/nexus-next/pkg/module"
)

// NetworkPlugin monitors network upload/download speeds
type NetworkPlugin struct {
	history    []float32  // Sparkline data for download (last 60 samples)
	historyMu  sync.Mutex // Protect history
	maxHistory int        // Maximum history length
	lastStats  net.IOCountersStat
	lastTime   time.Time
	firstRead  bool
	format     string // "bytes" (KB/s, MB/s) or "bits" (Kbps, Mbps)
	formatMu   sync.RWMutex
	graphType  module.GraphType // Graph visualization type
	graphMu    sync.RWMutex
}

// NewNetworkPlugin creates a new network module
func NewNetworkPlugin() *NetworkPlugin {
	return &NetworkPlugin{
		history:    make([]float32, 0, 60),
		maxHistory: 60,
		firstRead:  true,
		format:     "bytes",                     // default to KB/s, MB/s
		graphType:  module.GraphTypeSparkline, // default to sparkline
	}
}

// Describe returns module metadata
func (m *NetworkPlugin) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "Network",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Monitors network upload/download speeds",
		Icon:        "network-wired",
		RefreshMs:   2000, // Update every 2 seconds
	}, nil
}

// Sample returns current network statistics
func (m *NetworkPlugin) Sample() (module.Payload, error) {
	// Get network speed
	downSpeed, upSpeed, err := m.getNetworkSpeed()
	if err != nil {
		return module.Payload{
			Primary:          "—",
			Secondary:        "No Network",
			Severity:         module.SeverityWarn,
			TTL:              2 * time.Second,
			Icon:             "network-wired",
			LineSpacing:      20,
			LabelPosition:    module.LabelPositionRight,
			LabelOffsetX:     20,
			Timestamp:        time.Now(),
		}, nil
	}

	// Update history with download speed
	m.addToHistory(downSpeed)

	// Get sparkline
	spark := m.getSparkline()

	// Get current format
	m.formatMu.RLock()
	currentFormat := m.format
	m.formatMu.RUnlock()

	// Format speeds based on configuration
	var downStr, upStr string
	if currentFormat == "bits" {
		downStr = formatSpeedBits(downSpeed)
		upStr = formatSpeedBits(upSpeed)
	} else {
		downStr = formatSpeed(downSpeed)
		upStr = formatSpeed(upSpeed)
	}

	// Get current graph type
	m.graphMu.RLock()
	currentGraphType := m.graphType
	m.graphMu.RUnlock()

	return module.Payload{
		Primary:          fmt.Sprintf("↓ %s\n↑ %s", downStr, upStr),
		Secondary:        "Network",
		Severity:         module.SeverityOK,
		Spark:            spark,
		GraphType:        currentGraphType,
		TTL:              3 * time.Second,           // Slightly longer than refresh to prevent "module slow" warnings
		Icon:             "network-wired",
		LineSpacing:      20,                        // Extra spacing for stacked network speeds
		LabelPosition:    module.LabelPositionRight, // Position label to the right
		LabelOffsetX:     20,                        // Spacing between values and label (in pixels)
		NormalizeGraph:   true,                      // Normalize to show relative bandwidth changes
		GraphBgOpacity:   0, // 0 = use renderer default
		GraphLineOpacity: 0, // 0 = use renderer default
		Timestamp:        time.Now(),
	}, nil
}

// getNetworkSpeed calculates current network speed
func (m *NetworkPlugin) getNetworkSpeed() (float64, float64, error) {
	// Get current network stats (all interfaces combined)
	stats, err := net.IOCounters(false)
	if err != nil {
		return 0, 0, fmt.Errorf("failed to get network counters: %w", err)
	}

	if len(stats) == 0 {
		return 0, 0, fmt.Errorf("no network interfaces found")
	}

	currentStats := stats[0]
	currentTime := time.Now()

	// On first read, just store the stats
	if m.firstRead {
		m.lastStats = currentStats
		m.lastTime = currentTime
		m.firstRead = false
		return 0, 0, nil
	}

	// Calculate elapsed time
	elapsed := currentTime.Sub(m.lastTime).Seconds()
	if elapsed == 0 {
		return 0, 0, nil
	}

	// Calculate bytes transferred
	bytesRecv := currentStats.BytesRecv - m.lastStats.BytesRecv
	bytesSent := currentStats.BytesSent - m.lastStats.BytesSent

	// Calculate speeds (bytes per second)
	downloadSpeed := float64(bytesRecv) / elapsed
	uploadSpeed := float64(bytesSent) / elapsed

	// Update last stats
	m.lastStats = currentStats
	m.lastTime = currentTime

	return downloadSpeed, uploadSpeed, nil
}

// formatSpeed formats bytes/sec into human-readable string (K/s, M/s, etc.)
func formatSpeed(bytesPerSec float64) string {
	const (
		KB = 1024
		MB = 1024 * KB
		GB = 1024 * MB
	)

	switch {
	case bytesPerSec >= GB:
		return fmt.Sprintf("%.1fG/s", bytesPerSec/GB)
	case bytesPerSec >= MB:
		return fmt.Sprintf("%.1fM/s", bytesPerSec/MB)
	case bytesPerSec >= KB:
		return fmt.Sprintf("%.0fK/s", bytesPerSec/KB)
	default:
		return fmt.Sprintf("%.0fB/s", bytesPerSec)
	}
}

// formatSpeedBits formats bytes/sec into bits/sec (Kb, Mb, etc.)
func formatSpeedBits(bytesPerSec float64) string {
	// Convert bytes to bits (1 byte = 8 bits)
	bitsPerSec := bytesPerSec * 8

	const (
		Kbps = 1000
		Mbps = 1000 * Kbps
		Gbps = 1000 * Mbps
	)

	switch {
	case bitsPerSec >= Gbps:
		return fmt.Sprintf("%.1fGb", bitsPerSec/Gbps)
	case bitsPerSec >= Mbps:
		return fmt.Sprintf("%.1fMb", bitsPerSec/Mbps)
	case bitsPerSec >= Kbps:
		return fmt.Sprintf("%.0fKb", bitsPerSec/Kbps)
	default:
		return fmt.Sprintf("%.0fb", bitsPerSec)
	}
}

// addToHistory adds a download speed sample to history
func (m *NetworkPlugin) addToHistory(speed float64) {
	m.historyMu.Lock()
	defer m.historyMu.Unlock()

	// Normalize to 0-1 range (0-100 MB/s for sparkline scale)
	const maxSpeedMBps = 100.0
	speedMBps := speed / (1024 * 1024)
	normalized := float32(speedMBps) / maxSpeedMBps
	if normalized > 1.0 {
		normalized = 1.0
	}
	if normalized < 0.0 {
		normalized = 0.0
	}

	m.history = append(m.history, normalized)

	// Keep only last N samples
	if len(m.history) > m.maxHistory {
		m.history = m.history[len(m.history)-m.maxHistory:]
	}
}

// getSparkline returns sparkline data
func (m *NetworkPlugin) getSparkline() []float32 {
	m.historyMu.Lock()
	defer m.historyMu.Unlock()

	// Return copy to avoid concurrent modification
	spark := make([]float32, len(m.history))
	copy(spark, m.history)
	return spark
}

// OnConfigChanged implements module.ConfigNotifier interface.
// The network module uses the "network_format" config to switch between
// bytes (KB/s, MB/s) and bits (Kbps, Mbps) display formats,
// and "graph_type" to change visualization style.
func (m *NetworkPlugin) OnConfigChanged(config map[string]interface{}) error {
	// Update network format
	m.formatMu.Lock()
	oldFormat := m.format
	if format, ok := config["network_format"].(string); ok && format != "" {
		if format == "bytes" || format == "bits" {
			m.format = format
			if m.format != oldFormat {
				// Reset history so sparkline scales correctly after format change.
				m.historyMu.Lock()
				m.history = m.history[:0]
				m.historyMu.Unlock()
			}
		}
	}
	m.formatMu.Unlock()

	// Update graph type
	m.graphMu.Lock()
	oldGraphType := m.graphType
	if graphType, ok := config["graph_type"].(string); ok && graphType != "" {
		gt := module.GraphType(graphType)
		if gt == module.GraphTypeSparkline || gt == module.GraphTypeBar || gt == module.GraphTypeArea {
			m.graphType = gt
			if m.graphType != oldGraphType {
				fmt.Printf("network: graph_type changed from %q to %q\n", oldGraphType, m.graphType)
			}
		}
	}
	m.graphMu.Unlock()

	return nil
}

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: module.Handshake,
		Plugins: map[string]plugin.Plugin{
			"plugin": &module.ExecPlugin{Impl: NewNetworkPlugin()},
		},
	})
}
