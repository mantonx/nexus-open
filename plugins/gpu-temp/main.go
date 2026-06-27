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

	"github.com/mantonx/nexus-open/pkg/plugin"
)

// GPUTempPlugin monitors GPU temperature
type GPUTempPlugin struct {
	history plugin.SparkHistory

	vendorOnce sync.Once
	vendor     string // Detected GPU vendor (nvidia/amd/intel/generic)
	vendorPath string // sysfs card path for AMD/Intel/generic

	unit      string // "metric" (Celsius) or "imperial" (Fahrenheit)
	unitMu    sync.RWMutex
	graphType plugin.GraphType
	graphMu   sync.RWMutex
}

// NewGPUTempPlugin creates a new GPU temperature module
func NewGPUTempPlugin() *GPUTempPlugin {
	return &GPUTempPlugin{
		history:   plugin.NewSparkHistory(60),
		graphType: plugin.GraphTypeSparkline,
		unit:      "metric",
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
		HasGraph:    true,
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
			TTL:       3 * time.Second,
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

	var tempVal, tempUnit string
	if currentUnit == "imperial" {
		fahrenheit := (temp * 9 / 5) + 32
		tempVal = fmt.Sprintf("%.0f", fahrenheit)
		tempUnit = "°F"
	} else {
		tempVal = fmt.Sprintf("%.0f", temp)
		tempUnit = "°C"
	}

	// Get current graph type
	m.graphMu.RLock()
	currentGraphType := m.graphType
	m.graphMu.RUnlock()

	return plugin.Payload{
		Primary:          tempVal + tempUnit,
		Value:            tempVal,
		ValueUnit:        tempUnit,
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
	m.vendorOnce.Do(m.detectVendor)
	switch m.vendor {
	case "nvidia":
		return m.getNVIDIATemp()
	case "amd":
		return m.getAMDTemp()
	case "intel":
		return m.findIntelSysfsTemp()
	case "generic":
		return m.getSysfsTemp()
	}
	return 0, fmt.Errorf("no GPU found or supported")
}

// detectVendor probes hardware once and caches the result.
func (m *GPUTempPlugin) detectVendor() {
	// NVIDIA
	cmd := exec.Command("nvidia-smi", "--query-gpu=temperature.gpu", "--format=csv,noheader,nounits")
	if out, err := cmd.Output(); err == nil && len(strings.TrimSpace(string(out))) > 0 {
		m.vendor = "nvidia"
		return
	}

	cards, _ := filepath.Glob("/sys/class/drm/card[0-9]*")
	for _, card := range cards {
		vendorData, err := os.ReadFile(filepath.Join(card, "device", "vendor"))
		if err != nil {
			continue
		}
		vendor := strings.TrimSpace(string(vendorData))
		devPath := filepath.Join(card, "device")
		switch vendor {
		case "0x1002": // AMD
			m.vendor = "amd"
			m.vendorPath = devPath
			return
		case "0x8086": // Intel
			m.vendor = "intel"
			m.vendorPath = devPath
			return
		default:
			// Generic: any card with a hwmon temp file
			if matches, _ := filepath.Glob(filepath.Join(devPath, "hwmon", "hwmon*", "temp1_input")); len(matches) > 0 {
				m.vendor = "generic"
				m.vendorPath = devPath
				return
			}
		}
	}
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

// getAMDTemp reads AMD GPU temperature via rocm-smi, falling back to sysfs.
func (m *GPUTempPlugin) getAMDTemp() (float64, error) {
	cmd := exec.Command("rocm-smi", "--showtemp", "--csv")
	if output, err := cmd.Output(); err == nil {
		for _, line := range strings.Split(strings.TrimSpace(string(output)), "\n") {
			if strings.Contains(line, "card") && strings.Contains(line, "Temperature") {
				fields := strings.Split(line, ",")
				if len(fields) >= 3 {
					if temp, err := strconv.ParseFloat(strings.TrimSpace(fields[2]), 64); err == nil {
						return temp, nil
					}
				}
			}
		}
	}
	return readHwmonTemp(m.vendorPath)
}

// getSysfsTemp reads temperature for generic or Intel GPUs via hwmon.
func (m *GPUTempPlugin) getSysfsTemp() (float64, error) {
	return readHwmonTemp(m.vendorPath)
}

// findIntelSysfsTemp reads Intel GPU temperature via the cached vendorPath.
func (m *GPUTempPlugin) findIntelSysfsTemp() (float64, error) {
	return readHwmonTemp(m.vendorPath)
}

// readHwmonTemp reads temp1_input from the hwmon directory under devPath
// and converts millidegrees Celsius to degrees.
func readHwmonTemp(devPath string) (float64, error) {
	matches, err := filepath.Glob(filepath.Join(devPath, "hwmon", "hwmon*", "temp1_input"))
	if err != nil || len(matches) == 0 {
		return 0, fmt.Errorf("no hwmon temp file under %s", devPath)
	}
	data, err := os.ReadFile(matches[0])
	if err != nil {
		return 0, err
	}
	milliTemp, err := strconv.ParseInt(strings.TrimSpace(string(data)), 10, 64)
	if err != nil {
		return 0, err
	}
	return float64(milliTemp) / 1000.0, nil
}

func (m *GPUTempPlugin) addToHistory(temp float64) { m.history.Push(float32(temp)) }

func (m *GPUTempPlugin) getSparkline() []float32 { return m.history.Normalized(5, 95, 2.0) }

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
