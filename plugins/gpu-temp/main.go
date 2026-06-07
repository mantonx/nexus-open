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

	goplugin "github.com/hashicorp/go-plugin"

	"github.com/mantonx/nexus-next/pkg/plugin"
)

// GPUTempPlugin monitors GPU temperature
type GPUTempPlugin struct {
	history    []float32  // Sparkline data (last 60 samples)
	historyMu  sync.Mutex // Protect history
	maxHistory int        // Maximum history length
	vendor     string     // Detected GPU vendor (nvidia/amd/intel)
	vendorMu   sync.Mutex // Protect vendor detection
	unit       string     // "metric" (Celsius) or "imperial" (Fahrenheit)
	unitMu     sync.RWMutex
	graphType  plugin.GraphType // Graph visualization type
	graphMu    sync.RWMutex
}

// NewGPUTempPlugin creates a new GPU temperature module
func NewGPUTempPlugin() *GPUTempPlugin {
	return &GPUTempPlugin{
		history:    make([]float32, 0, 60),
		maxHistory: 60,
		graphType:  plugin.GraphTypeSparkline, // default to sparkline
		unit:       "metric", // default to Celsius
	}
}

// Describe returns module metadata
func (m *GPUTempPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "GPU Temperature",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Monitors GPU temperature (NVIDIA, AMD, Intel)",
		Icon:        "microchip",
		RefreshMs:   2000,
		Schema: plugin.ConfigSchema{
			Fields: []plugin.ConfigField{
				{
					Key: "unit", Label: "Units", Type: plugin.FieldTypeEnum, Default: "metric",
					Options: []plugin.FieldOption{{Value: "metric", Label: "°C"}, {Value: "imperial", Label: "°F"}},
				},
				{
					Key: "graph_type", Label: "Graph", Type: plugin.FieldTypeEnum, Default: "sparkline",
					Options: []plugin.FieldOption{
						{Value: "sparkline", Label: "Sparkline"},
						{Value: "bar", Label: "Bar"},
						{Value: "area", Label: "Area"},
					},
				},
			},
		},
	}, nil
}

// Sample returns current GPU temperature
func (m *GPUTempPlugin) Sample() (plugin.Payload, error) {
	// Get GPU temperature
	temp, err := m.getGPUTemp()
	if err != nil {
		return plugin.Payload{
			Primary:   "—",
			Secondary: "No GPU",
			Severity:  plugin.SeverityWarn,
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

	return plugin.Payload{
		Primary:          tempStr,
		Secondary:        "GPU",
		Severity:         severity,
		Spark:            spark,
		GraphType:        currentGraphType,
		TTL:              2 * time.Second,
		Icon:             "desktop",
		GraphBgOpacity:   0,
		GraphLineOpacity: 0,
		Timestamp:        time.Now(),
	}, nil
}

// getGPUTemp queries for GPU temperature using multiple detection methods
func (m *GPUTempPlugin) getGPUTemp() (float64, error) {
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
func (m *GPUTempPlugin) getNVIDIATemp() (float64, error) {
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
func (m *GPUTempPlugin) getAMDTemp() (float64, error) {
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
func (m *GPUTempPlugin) getIntelTemp() (float64, error) {
	// Intel GPUs typically expose temperature via sysfs
	// /sys/class/drm/card*/device/hwmon/hwmon*/temp1_input
	return m.findIntelSysfsTemp()
}

// getSysfsTemp is a generic fallback that searches sysfs for any GPU temp
func (m *GPUTempPlugin) getSysfsTemp() (float64, error) {
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
func (m *GPUTempPlugin) findAMDSysfsTemp() (float64, error) {
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
func (m *GPUTempPlugin) findIntelSysfsTemp() (float64, error) {
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
func (m *GPUTempPlugin) setVendor(vendor string) {
	m.vendorMu.Lock()
	defer m.vendorMu.Unlock()
	if m.vendor == "" {
		m.vendor = vendor
	}
}

// addToHistory adds a temperature sample to history
func (m *GPUTempPlugin) addToHistory(temp float64) {
	m.historyMu.Lock()
	defer m.historyMu.Unlock()

	m.history = append(m.history, float32(temp))

	// Keep only last N samples
	if len(m.history) > m.maxHistory {
		m.history = m.history[len(m.history)-m.maxHistory:]
	}
}

// getSparkline returns sparkline data normalised using the 5th/95th percentile
// so single spikes don't compress everything else to a flatline.
func (m *GPUTempPlugin) getSparkline() []float32 {
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

func percentileRange(data []float32, loPct, hiPct int) (float32, float32) {
	sorted := make([]float32, len(data))
	copy(sorted, data)
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
func (m *GPUTempPlugin) getSeverity(temp float64) plugin.Severity {
	switch {
	case temp >= 90:
		return plugin.SeverityCrit // Critical: ≥90°C
	case temp >= 75:
		return plugin.SeverityWarn // Warning: ≥75°C
	default:
		return plugin.SeverityOK // OK: <75°C
	}
}

// Configure applies per-zone plugin configuration.
func (m *GPUTempPlugin) Configure(cfg map[string]any) error {
	m.unitMu.Lock()
	if unit, ok := cfg["unit"].(string); ok && (unit == "metric" || unit == "imperial") {
		m.unit = unit
	}
	m.unitMu.Unlock()

	m.graphMu.Lock()
	if gt, ok := cfg["graph_type"].(string); ok && gt != "" {
		g := plugin.GraphType(gt)
		if g == plugin.GraphTypeSparkline || g == plugin.GraphTypeBar || g == plugin.GraphTypeArea {
			m.graphType = g
		}
	}
	m.graphMu.Unlock()

	return nil
}

func main() {
	goplugin.Serve(&goplugin.ServeConfig{
		HandshakeConfig: plugin.Handshake,
		Plugins: goplugin.PluginSet{
			"plugin": &plugin.ExecPlugin{Impl: NewGPUTempPlugin()},
		},
	})
}
