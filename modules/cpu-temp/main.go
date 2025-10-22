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

	"nexus-open/pkg/module"
)

// CPUTempModule monitors CPU temperature
type CPUTempModule struct {
	history    []float32  // Sparkline data (last 60 samples)
	historyMu  sync.Mutex // Protect history
	maxHistory int        // Maximum history length
	unit       string     // "metric" (Celsius) or "imperial" (Fahrenheit)
	unitMu     sync.RWMutex
	graphType  module.GraphType // Graph visualization type
	graphMu    sync.RWMutex
}

// NewCPUTempModule creates a new CPU temperature module
func NewCPUTempModule() *CPUTempModule {
	return &CPUTempModule{
		history:    make([]float32, 0, 60),
		maxHistory: 60,
		unit:       "metric",                    // default to Celsius
		graphType:  module.GraphTypeSparkline, // default to sparkline
	}
}

// Describe returns module metadata
func (m *CPUTempModule) Describe() (module.Descriptor, error) {
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
func (m *CPUTempModule) Sample() (module.Payload, error) {
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
		Primary:   tempStr,
		Secondary: "CPU Temp",
		Severity:  severity,
		Spark:     spark,
		GraphType: currentGraphType,
		TTL:       2 * time.Second,
		Icon:      "cpu",
		Timestamp: time.Now(),
	}, nil
}

// getCPUTemp reads CPU temperature based on OS
func (m *CPUTempModule) getCPUTemp() (float64, error) {
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
func (m *CPUTempModule) readLinuxTemp() (float64, error) {
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
func (m *CPUTempModule) readWindowsTemp() (float64, error) {
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
func (m *CPUTempModule) readMacTemp() (float64, error) {
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
func (m *CPUTempModule) addToHistory(temp float64) {
	m.historyMu.Lock()
	defer m.historyMu.Unlock()

	// Normalize to 0-1 range (0-100°C)
	normalized := float32(temp) / 100.0
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
func (m *CPUTempModule) getSparkline() []float32 {
	m.historyMu.Lock()
	defer m.historyMu.Unlock()

	// Return copy to avoid concurrent modification
	spark := make([]float32, len(m.history))
	copy(spark, m.history)
	return spark
}

// getSeverity returns severity based on temperature
func (m *CPUTempModule) getSeverity(temp float64) module.Severity {
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
func (m *CPUTempModule) OnConfigChanged(config map[string]interface{}) error {
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
			"module": &module.ModulePlugin{Impl: NewCPUTempModule()},
		},
	})
}
