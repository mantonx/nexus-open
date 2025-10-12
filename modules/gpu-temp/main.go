// gpu-temp is a module that monitors GPU temperature using nvidia-smi
package main

import (
	"fmt"
	"os/exec"
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
}

// NewGPUTempModule creates a new GPU temperature module
func NewGPUTempModule() *GPUTempModule {
	return &GPUTempModule{
		history:    make([]float32, 0, 60),
		maxHistory: 60,
	}
}

// Describe returns module metadata
func (m *GPUTempModule) Describe() (module.Descriptor, error) {
	return module.Descriptor{
		Name:        "GPU Temperature",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Monitors NVIDIA GPU temperature via nvidia-smi",
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

	// Determine severity
	severity := m.getSeverity(temp)

	return module.Payload{
		Primary:   fmt.Sprintf("%.0f°C", temp),
		Secondary: "GPU Temp",
		Severity:  severity,
		Spark:     spark,
		TTL:       2 * time.Second,
		Icon:      "microchip",
		Timestamp: time.Now(),
	}, nil
}

// getGPUTemp queries nvidia-smi for GPU temperature
func (m *GPUTempModule) getGPUTemp() (float64, error) {
	// Run nvidia-smi to get temperature
	// nvidia-smi --query-gpu=temperature.gpu --format=csv,noheader,nounits
	cmd := exec.Command("nvidia-smi", "--query-gpu=temperature.gpu", "--format=csv,noheader,nounits")

	output, err := cmd.Output()
	if err != nil {
		return 0, fmt.Errorf("nvidia-smi failed: %w", err)
	}

	// Parse temperature (first GPU if multiple)
	tempStr := strings.TrimSpace(string(output))
	lines := strings.Split(tempStr, "\n")
	if len(lines) == 0 {
		return 0, fmt.Errorf("no GPU found")
	}

	temp, err := strconv.ParseFloat(lines[0], 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %w", err)
	}

	return temp, nil
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

func main() {
	plugin.Serve(&plugin.ServeConfig{
		HandshakeConfig: module.Handshake,
		Plugins: map[string]plugin.Plugin{
			"module": &module.ModulePlugin{Impl: NewGPUTempModule()},
		},
	})
}
