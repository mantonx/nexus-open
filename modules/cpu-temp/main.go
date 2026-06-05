// cpu-temp is a module that monitors CPU temperature
// Supports Linux, Windows, and macOS
package main

import (
	"fmt"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/hashicorp/go-plugin"

	"github.com/mantonx/nexus-next/pkg/module"
)

// CPUTempPlugin monitors CPU temperature
type CPUTempPlugin struct {
	history    []float32  // Sparkline data (last 60 samples)
	historyMu  sync.Mutex // Protect history
	maxHistory int        // Maximum history length
	unit       string     // "metric" (Celsius) or "imperial" (Fahrenheit)
	unitMu     sync.RWMutex
	graphType  module.GraphType // Graph visualization type
	graphMu    sync.RWMutex
}

// NewCPUTempPlugin creates a new CPU temperature module
func NewCPUTempPlugin() *CPUTempPlugin {
	return &CPUTempPlugin{
		history:    make([]float32, 0, 60),
		maxHistory: 60,
		unit:       "metric",                    // default to Celsius
		graphType:  module.GraphTypeSparkline, // default to sparkline
	}
}

// Describe returns module metadata
func (m *CPUTempPlugin) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "CPU Temperature",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Monitors CPU temperature (Linux, Windows, macOS)",
		Icon:        "cpu",
		RefreshMs:   2000, // Update every 2 seconds
	}, nil
}

// Sample returns current CPU temperature
func (m *CPUTempPlugin) Sample() (module.Payload, error) {
	// Get CPU temperature
	temp, err := m.getCPUTemp()
	if err != nil {
		return module.Payload{
			Primary:   "—",
			Secondary: "No CPU",
			Severity:  module.SeverityWarn,
			TTL:       2 * time.Second,
			Timestamp: time.Now(),
		}, nil
	}

	// Update history
	m.addToHistory(temp)

	// Get sparkline (normalized to 0-100°C range)
	spark := m.getSparkline()

	// Determine severity (always based on Celsius thresholds)
	severity := m.getSeverity(temp)

	// Get current unit and format temperature
	m.unitMu.RLock()
	currentUnit := m.unit
	m.unitMu.RUnlock()

	var tempStr string
	if currentUnit == "imperial" {
		fahrenheit := (temp * 9 / 5) + 32
		tempStr = fmt.Sprintf("%.0f°F", fahrenheit)
	} else {
		tempStr = fmt.Sprintf("%.0f°C", temp)
	}

	// Get current graph type
	m.graphMu.RLock()
	currentGraphType := m.graphType
	m.graphMu.RUnlock()

	return module.Payload{
		Primary:          tempStr,
		Secondary:        "CPU",
		Severity:         severity,
		Spark:            spark,
		GraphType:        currentGraphType,
		TTL:              2 * time.Second,
		Icon:             "cpu",
		GraphBgOpacity:   0,
		GraphLineOpacity: 0,
		Timestamp:        time.Now(),
	}, nil
}

// getCPUTemp reads CPU temperature based on OS
func (m *CPUTempPlugin) getCPUTemp() (float64, error) {
	switch runtime.GOOS {
	case "linux":
		return m.readLinuxTemp()
	case "windows":
		return m.readWindowsTemp()
	case "darwin":
		return m.readMacTemp()
	default:
		return 0, fmt.Errorf("unsupported operating system: %s", runtime.GOOS)
	}
}

// readLinuxTemp reads CPU temperature from sysfs
func (m *CPUTempPlugin) readLinuxTemp() (float64, error) {
	// Try thermal_zone0 first (most common)
	data, err := os.ReadFile("/sys/class/thermal/thermal_zone0/temp")
	if err != nil {
		// Fallback: try hwmon (some systems)
		data, err = os.ReadFile("/sys/class/hwmon/hwmon0/temp1_input")
		if err != nil {
			return 0, fmt.Errorf("failed to read temperature: %w", err)
		}
	}

	temp, err := strconv.ParseFloat(strings.TrimSpace(string(data)), 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %w", err)
	}

	// Convert millidegrees to degrees
	return temp / 1000.0, nil
}

// readWindowsTemp reads CPU temperature on Windows
func (m *CPUTempPlugin) readWindowsTemp() (float64, error) {
	// Use WMIC to query thermal zone temperature
	cmd := exec.Command("wmic", "/namespace:\\\\root\\wmi", "PATH",
		"MSAcpi_ThermalZoneTemperature", "GET", "CurrentTemperature", "/value")
	out, err := cmd.Output()
	if err != nil {
		return 0, fmt.Errorf("failed to execute wmic: %w", err)
	}

	parts := strings.Split(string(out), "=")
	if len(parts) != 2 {
		return 0, fmt.Errorf("invalid wmic output format")
	}

	temp, err := strconv.ParseFloat(strings.TrimSpace(parts[1]), 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %w", err)
	}

	// Convert from decikelvin to Celsius
	// WMIC returns temperature in tenths of Kelvin
	return (temp / 10.0) - 273.15, nil
}

// readMacTemp reads CPU temperature on macOS
func (m *CPUTempPlugin) readMacTemp() (float64, error) {
	// Try sysctl first
	cmd := exec.Command("sysctl", "-n", "machdep.xcpm.cpu_thermal_level")
	out, err := cmd.Output()
	if err != nil {
		// Fallback: try osx-cpu-temp if installed
		cmd = exec.Command("osx-cpu-temp")
		out, err = cmd.Output()
		if err != nil {
			return 0, fmt.Errorf("failed to read CPU temperature: %w", err)
		}
		// Parse "50.5°C" format
		tempStr := strings.TrimSpace(string(out))
		tempStr = strings.TrimSuffix(tempStr, "°C")
		temp, err := strconv.ParseFloat(tempStr, 64)
		if err != nil {
			return 0, fmt.Errorf("failed to parse temperature: %w", err)
		}
		return temp, nil
	}

	// Parse sysctl output (thermal level, not actual temp)
	// This is a thermal pressure level (0-100), not temperature
	level, err := strconv.ParseFloat(strings.TrimSpace(string(out)), 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %w", err)
	}

	// Estimate temperature from thermal level (rough approximation)
	// 0 = ~40°C, 100 = ~100°C
	return 40.0 + (level * 0.6), nil
}

// addToHistory adds a temperature sample to history
func (m *CPUTempPlugin) addToHistory(temp float64) {
	m.historyMu.Lock()
	defer m.historyMu.Unlock()

	m.history = append(m.history, float32(temp))

	// Keep only last N samples
	if len(m.history) > m.maxHistory {
		m.history = m.history[len(m.history)-m.maxHistory:]
	}
}

// getSparkline returns sparkline data normalised to a stable range.
// Uses the 10th/90th percentile of history as the scale bounds so a single
// spike doesn't compress all other values to a flatline.
func (m *CPUTempPlugin) getSparkline() []float32 {
	m.historyMu.Lock()
	defer m.historyMu.Unlock()

	if len(m.history) == 0 {
		return nil
	}

	mn, mx := percentileRange(m.history, 5, 95)
	rng := mx - mn
	if rng < 2.0 {
		mid := (mn + mx) / 2
		mn = mid - 1.0
		mx = mid + 1.0
		rng = 2.0
	}

	spark := make([]float32, len(m.history))
	for i, v := range m.history {
		s := (v - mn) / rng
		if s < 0 { s = 0 }
		if s > 1 { s = 1 }
		spark[i] = s
	}
	return spark
}

// percentileRange returns the lo-th and hi-th percentile values from data.
func percentileRange(data []float32, loPct, hiPct int) (float32, float32) {
	sorted := make([]float32, len(data))
	copy(sorted, data)
	// Simple insertion sort — history is at most 60 elements.
	for i := 1; i < len(sorted); i++ {
		for j := i; j > 0 && sorted[j] < sorted[j-1]; j-- {
			sorted[j], sorted[j-1] = sorted[j-1], sorted[j]
		}
	}
	loIdx := loPct * (len(sorted) - 1) / 100
	hiIdx := hiPct * (len(sorted) - 1) / 100
	return sorted[loIdx], sorted[hiIdx]
}

// getSeverity returns severity based on temperature
func (m *CPUTempPlugin) getSeverity(temp float64) module.Severity {
	switch {
	case temp >= 90:
		return module.SeverityCrit // Critical: ≥90°C
	case temp >= 75:
		return module.SeverityWarn // Warning: ≥75°C
	default:
		return module.SeverityOK // OK: <75°C
	}
}

// OnConfigChanged implements module.ConfigNotifier interface.
// The CPU temp module uses the "unit" config to switch between Celsius and Fahrenheit,
// and "graph_type" to change visualization style.
func (m *CPUTempPlugin) OnConfigChanged(config map[string]interface{}) error {
	// Update unit
	m.unitMu.Lock()
	oldUnit := m.unit
	if unit, ok := config["unit"].(string); ok && unit != "" {
		if unit == "metric" || unit == "imperial" {
			m.unit = unit
			if m.unit != oldUnit {
				fmt.Printf("cpu-temp: unit changed from %q to %q\n", oldUnit, m.unit)
			}
		}
	}
	m.unitMu.Unlock()

	// Update graph type
	m.graphMu.Lock()
	oldGraphType := m.graphType
	if graphType, ok := config["graph_type"].(string); ok && graphType != "" {
		gt := module.GraphType(graphType)
		if gt == module.GraphTypeSparkline || gt == module.GraphTypeBar || gt == module.GraphTypeArea {
			m.graphType = gt
			if m.graphType != oldGraphType {
				fmt.Printf("cpu-temp: graph_type changed from %q to %q\n", oldGraphType, m.graphType)
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
			"plugin": &module.ExecPlugin{Impl: NewCPUTempPlugin()},
		},
	})
}
