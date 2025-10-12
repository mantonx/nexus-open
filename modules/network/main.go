// network is a module that monitors network upload/download speeds
package main

import (
	"fmt"
	"sync"
	"time"

	"github.com/hashicorp/go-plugin"
	"github.com/shirou/gopsutil/net"

	"nexus-open/pkg/module"
)

// NetworkModule monitors network upload/download speeds
type NetworkModule struct {
	history    []float32  // Sparkline data for download (last 60 samples)
	historyMu  sync.Mutex // Protect history
	maxHistory int        // Maximum history length
	lastStats  net.IOCountersStat
	lastTime   time.Time
	firstRead  bool
}

// NewNetworkModule creates a new network module
func NewNetworkModule() *NetworkModule {
	return &NetworkModule{
		history:    make([]float32, 0, 60),
		maxHistory: 60,
		firstRead:  true,
	}
}

// Describe returns module metadata
func (m *NetworkModule) Describe() (module.Descriptor, error) {
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
func (m *NetworkModule) Sample() (module.Payload, error) {
	// Get network speed
	downSpeed, upSpeed, err := m.getNetworkSpeed()
	if err != nil {
		return module.Payload{
			Primary:   "—",
			Secondary: "No Network",
			Severity:  module.SeverityWarn,
			TTL:       2 * time.Second,
			Timestamp: time.Now(),
		}, nil
	}

	// Update history with download speed
	m.addToHistory(downSpeed)

	// Get sparkline
	spark := m.getSparkline()

	// Format speeds
	downStr := formatSpeed(downSpeed)
	upStr := formatSpeed(upSpeed)

	return module.Payload{
		Primary:   fmt.Sprintf("↓%s ↑%s", downStr, upStr),
		Secondary: "Network",
		Severity:  module.SeverityOK,
		Spark:     spark,
		TTL:       2 * time.Second,
		Icon:      "network-wired",
		Timestamp: time.Now(),
	}, nil
}

// getNetworkSpeed calculates current network speed
func (m *NetworkModule) getNetworkSpeed() (float64, float64, error) {
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

// formatSpeed formats bytes/sec into human-readable string
func formatSpeed(bytesPerSec float64) string {
	const (
		KB = 1024
		MB = 1024 * KB
		GB = 1024 * MB
	)

	switch {
	case bytesPerSec >= GB:
		return fmt.Sprintf("%.1f GB/s", bytesPerSec/GB)
	case bytesPerSec >= MB:
		return fmt.Sprintf("%.1f MB/s", bytesPerSec/MB)
	case bytesPerSec >= KB:
		return fmt.Sprintf("%.1f KB/s", bytesPerSec/KB)
	default:
		return fmt.Sprintf("%.0f B/s", bytesPerSec)
	}
}

// addToHistory adds a download speed sample to history
func (m *NetworkModule) addToHistory(speed float64) {
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
func (m *NetworkModule) getSparkline() []float32 {
	m.historyMu.Lock()
	defer m.historyMu.Unlock()

	// Return copy to avoid concurrent modification
	spark := make([]float32, len(m.history))
	copy(spark, m.history)
	return spark
}

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: module.Handshake,
		Plugins: map[string]plugin.Plugin{
			"module": &module.ModulePlugin{Impl: NewNetworkModule()},
		},
	})
}
