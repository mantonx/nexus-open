// gpu-temp is a module that monitors GPU temperature
// Supports NVIDIA (nvidia-smi), AMD (rocm-smi), and Intel GPUs
package main

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"strconv"
	"strings"
	"sync"
	"time"

	"github.com/hashicorp/go-plugin"

	"nexus-open/pkg/module"
)

// GPUTempModule monitors GPU temperature
type GPUTempModule struct {
	history    []float32  // Sparkline data (last 60 samples)
	historyMu  sync.Mutex // Protect history
	maxHistory int        // Maximum history length
	vendor     string     // Detected GPU vendor (nvidia/amd/intel)
	vendorMu   sync.Mutex // Protect vendor detection
	unit       string     // "metric" (Celsius) or "imperial" (Fahrenheit)
	unitMu     sync.RWMutex
	graphType  module.GraphType // Graph visualization type
	graphMu    sync.RWMutex
}

// NewGPUTempModule creates a new GPU temperature module
func NewGPUTempModule() *GPUTempModule {
	return &GPUTempModule{
		history:    make([]float32, 0, 60),
		maxHistory: 60,
		graphType:  module.GraphTypeSparkline, // default to sparkline
		unit:       "metric", // default to Celsius
	}
}

// Describe returns module metadata
func (m *GPUTempModule) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "GPU Temperature",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Monitors GPU temperature (NVIDIA, AMD, Intel)",
		Icon:        "microchip",
		RefreshMs:   2000, // Update every 2 seconds
	}, nil
}

// Sample returns current GPU temperature
func (m *GPUTempModule) Sample() (module.Payload, error) {
	// Get GPU temperature
	temp, err := m.getGPUTemp()
	if err != nil {
		return module.Payload{
			Primary:   "—",
			Secondary: "No GPU",
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
		Secondary: "GPU Temp",
		Severity:  severity,
		Spark:     spark,
		GraphType: currentGraphType,
		TTL:       2 * time.Second,
		Icon:      "microchip",
		Timestamp: time.Now(),
	}, nil
}

// getGPUTemp queries for GPU temperature using multiple detection methods
func (m *GPUTempModule) getGPUTemp() (float64, error) {
	// Try NVIDIA first
	if temp, err := m.getNVIDIATemp(); err == nil {
		m.setVendor("nvidia")
		return temp, nil
	}

	// Try AMD
	if temp, err := m.getAMDTemp(); err == nil {
		m.setVendor("amd")
		return temp, nil
	}

	// Try Intel
	if temp, err := m.getIntelTemp(); err == nil {
		m.setVendor("intel")
		return temp, nil
	}

	// Try generic sysfs (fallback for any GPU)
	if temp, err := m.getSysfsTemp(); err == nil {
		m.setVendor("generic")
		return temp, nil
	}

	return 0, fmt.Errorf("no GPU found or supported")
}

// getNVIDIATemp queries nvidia-smi for NVIDIA GPU temperature
func (m *GPUTempModule) getNVIDIATemp() (float64, error) {
	cmd := exec.Command("nvidia-smi", "--query-gpu=temperature.gpu", "--format=csv,noheader,nounits")
	output, err := cmd.Output()
	if err != nil {
		return 0, fmt.Errorf("nvidia-smi failed: %w", err)
	}

	tempStr := strings.TrimSpace(string(output))
	lines := strings.Split(tempStr, "\n")
	if len(lines) == 0 {
		return 0, fmt.Errorf("no NVIDIA GPU found")
	}

	temp, err := strconv.ParseFloat(lines[0], 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %w", err)
	}

	return temp, nil
}

// getAMDTemp queries rocm-smi for AMD GPU temperature
func (m *GPUTempModule) getAMDTemp() (float64, error) {
	// Try rocm-smi first (AMD ROCm stack)
	cmd := exec.Command("rocm-smi", "--showtemp", "--csv")
	output, err := cmd.Output()
	if err == nil {
		// Parse CSV output: card0,Temperature (Sensor edge) (C),52.0
		lines := strings.Split(strings.TrimSpace(string(output)), "\n")
		for _, line := range lines {
			if strings.Contains(line, "card") && strings.Contains(line, "Temperature") {
				fields := strings.Split(line, ",")
				if len(fields) >= 3 {
					temp, err := strconv.ParseFloat(strings.TrimSpace(fields[2]), 64)
					if err == nil {
						return temp, nil
					}
				}
			}
		}
	}

	// Fallback: Try AMD sysfs path
	// /sys/class/drm/card*/device/hwmon/hwmon*/temp1_input (in millidegrees)
	return m.findAMDSysfsTemp()
}

// getIntelTemp queries for Intel GPU temperature
func (m *GPUTempModule) getIntelTemp() (float64, error) {
	// Intel GPUs typically expose temperature via sysfs
	// /sys/class/drm/card*/device/hwmon/hwmon*/temp1_input
	return m.findIntelSysfsTemp()
}

// getSysfsTemp is a generic fallback that searches sysfs for any GPU temp
func (m *GPUTempModule) getSysfsTemp() (float64, error) {
	// Search /sys/class/drm/card*/device/hwmon/hwmon*/temp*_input
	pattern := "/sys/class/drm/card*/device/hwmon/hwmon*/temp1_input"
	matches, err := filepath.Glob(pattern)
	if err != nil || len(matches) == 0 {
		return 0, fmt.Errorf("no sysfs temperature found")
	}

	// Read first match
	data, err := os.ReadFile(matches[0])
	if err != nil {
		return 0, fmt.Errorf("failed to read sysfs temp: %w", err)
	}

	// Parse temperature (in millidegrees Celsius)
	tempStr := strings.TrimSpace(string(data))
	milliTemp, err := strconv.ParseInt(tempStr, 10, 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse sysfs temp: %w", err)
	}

	// Convert millidegrees to degrees
	return float64(milliTemp) / 1000.0, nil
}

// findAMDSysfsTemp searches for AMD GPU temperature in sysfs
func (m *GPUTempModule) findAMDSysfsTemp() (float64, error) {
	// AMD GPUs appear as /sys/class/drm/card*/device/hwmon/hwmon*/temp1_input
	// Look for cards with AMD vendor ID (1002) or amdgpu driver
	cards, err := filepath.Glob("/sys/class/drm/card[0-9]")
	if err != nil {
		return 0, err
	}

	for _, card := range cards {
		// Check if this is an AMD GPU
		vendorPath := filepath.Join(card, "device", "vendor")
		vendorData, err := os.ReadFile(vendorPath)
		if err != nil {
			continue
		}
		vendor := strings.TrimSpace(string(vendorData))

		// AMD vendor ID is 0x1002
		if !strings.Contains(vendor, "0x1002") {
			continue
		}

		// Find temperature file
		pattern := filepath.Join(card, "device", "hwmon", "hwmon*", "temp1_input")
		matches, err := filepath.Glob(pattern)
		if err != nil || len(matches) == 0 {
			continue
		}

		// Read temperature
		data, err := os.ReadFile(matches[0])
		if err != nil {
			continue
		}

		tempStr := strings.TrimSpace(string(data))
		milliTemp, err := strconv.ParseInt(tempStr, 10, 64)
		if err != nil {
			continue
		}

		return float64(milliTemp) / 1000.0, nil
	}

	return 0, fmt.Errorf("no AMD GPU found")
}

// findIntelSysfsTemp searches for Intel GPU temperature in sysfs
func (m *GPUTempModule) findIntelSysfsTemp() (float64, error) {
	// Intel GPUs appear as /sys/class/drm/card*/device/hwmon/hwmon*/temp1_input
	// Look for cards with Intel vendor ID (8086) or i915 driver
	cards, err := filepath.Glob("/sys/class/drm/card[0-9]")
	if err != nil {
		return 0, err
	}

	for _, card := range cards {
		// Check if this is an Intel GPU
		vendorPath := filepath.Join(card, "device", "vendor")
		vendorData, err := os.ReadFile(vendorPath)
		if err != nil {
			continue
		}
		vendor := strings.TrimSpace(string(vendorData))

		// Intel vendor ID is 0x8086
		if !strings.Contains(vendor, "0x8086") {
			continue
		}

		// Find temperature file
		pattern := filepath.Join(card, "device", "hwmon", "hwmon*", "temp1_input")
		matches, err := filepath.Glob(pattern)
		if err != nil || len(matches) == 0 {
			continue
		}

		// Read temperature
		data, err := os.ReadFile(matches[0])
		if err != nil {
			continue
		}

		tempStr := strings.TrimSpace(string(data))
		milliTemp, err := strconv.ParseInt(tempStr, 10, 64)
		if err != nil {
			continue
		}

		return float64(milliTemp) / 1000.0, nil
	}

	return 0, fmt.Errorf("no Intel GPU found")
}

// setVendor sets the detected GPU vendor (thread-safe)
func (m *GPUTempModule) setVendor(vendor string) {
	m.vendorMu.Lock()
	defer m.vendorMu.Unlock()
	if m.vendor == "" {
		m.vendor = vendor
	}
}

// addToHistory adds a temperature sample to history
func (m *GPUTempModule) addToHistory(temp float64) {
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
func (m *GPUTempModule) getSparkline() []float32 {
	m.historyMu.Lock()
	defer m.historyMu.Unlock()

	// Return copy to avoid concurrent modification
	spark := make([]float32, len(m.history))
	copy(spark, m.history)
	return spark
}

// getSeverity returns severity based on temperature
func (m *GPUTempModule) getSeverity(temp float64) module.Severity {
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
// The GPU temp module uses the "unit" config to switch between Celsius and Fahrenheit,
// and "graph_type" to change visualization style.
func (m *GPUTempModule) OnConfigChanged(config map[string]interface{}) error {
	// Update unit
	m.unitMu.Lock()
	oldUnit := m.unit
	if unit, ok := config["unit"].(string); ok && unit != "" {
		if unit == "metric" || unit == "imperial" {
			m.unit = unit
			if m.unit != oldUnit {
				fmt.Printf("gpu-temp: unit changed from %q to %q\n", oldUnit, m.unit)
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
				fmt.Printf("gpu-temp: graph_type changed from %q to %q\n", oldGraphType, m.graphType)
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
			"module": &module.ModulePlugin{Impl: NewGPUTempModule()},
		},
	})
}
