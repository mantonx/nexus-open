// gpu-load monitors GPU utilisation.
// NVIDIA: nvidia-smi (utilization.gpu / utilization.memory)
// AMD:    sysfs gpu_busy_percent / mem_busy_percent
// Intel:  sysfs fdinfo (approximate, kernel 5.19+)
// Falls back gracefully when hardware is not detected.
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

const (
	vendorNVIDIA  = "nvidia"
	vendorAMD     = "amd"
	vendorIntel   = "intel"
	vendorGeneric = "generic"
)

// GPULoadPlugin monitors GPU utilisation.
type GPULoadPlugin struct {
	// vendor is detected once on first successful sample
	vendor     string
	vendorOnce sync.Once
	vendorPath string // sysfs card path for AMD/Intel/generic

	// config
	source    string // "gpu" or "memory"
	sourceMu  sync.RWMutex
	showVRAM  bool
	showVRAMMu sync.RWMutex
	graphType plugin.GraphType
	graphMu   sync.RWMutex

	// sparkline
	history    []float32
	historyMu  sync.Mutex
	maxHistory int
}

func NewGPULoadPlugin() *GPULoadPlugin {
	return &GPULoadPlugin{
		source:     "gpu",
		showVRAM:   true,
		graphType:  plugin.GraphTypeSparkline,
		history:    make([]float32, 0, 60),
		maxHistory: 60,
	}
}

func (p *GPULoadPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "GPU Load",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Monitors GPU utilisation (NVIDIA, AMD, Intel)",
		Icon:        "display",
		RefreshMs:   1000,
		HasGraph:    true,
		Schema: plugin.ConfigSchema{
			Fields: []plugin.ConfigField{
				{
					Key: "source", Label: "Source", Type: plugin.FieldTypeEnum, Default: "gpu",
					Options: []plugin.FieldOption{{Value: "gpu", Label: "GPU"}, {Value: "memory", Label: "VRAM"}},
				},
				{Key: "show_vram", Label: "Show VRAM bar", Type: plugin.FieldTypeBool, Default: false},
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

func (p *GPULoadPlugin) Sample() (plugin.Payload, error) {
	p.vendorOnce.Do(p.detectVendor)

	p.sourceMu.RLock()
	src := p.source
	p.sourceMu.RUnlock()

	p.showVRAMMu.RLock()
	showVRAM := p.showVRAM
	p.showVRAMMu.RUnlock()

	gpuPct, memPct, err := p.readLoad()
	if err != nil {
		secondary := "No GPU"
		if p.vendor == vendorIntel {
			secondary = "Intel GPU"
		}
		return plugin.Payload{
			Primary:   "—",
			Secondary: secondary,
			Severity:  plugin.SeverityWarn,
			TTL:       1500 * time.Millisecond,
			Timestamp: time.Now(),
		}, nil
	}

	// Primary value is the configured source
	primary := gpuPct
	if src == "memory" {
		primary = memPct
	}

	p.addToHistory(float32(primary))

	p.graphMu.RLock()
	gt := p.graphType
	p.graphMu.RUnlock()

	primaryStr := fmt.Sprintf("%.0f%%", primary)
	secondary := p.secondaryLabel(src, showVRAM, memPct)

	return plugin.Payload{
		Primary:          primaryStr,
		Secondary:        secondary,
		Severity:         severityFor(primary),
		Spark:            p.getSparkline(),
		GraphType:        gt,
		TTL:              2 * time.Second,
		Timestamp:        time.Now(),
		GraphBgOpacity:   0,
		GraphLineOpacity: 0,
	}, nil
}

// secondaryLabel builds the secondary text.
// When source=gpu and showVRAM=true, shows memory controller % as context.
// When source=memory, just shows "MEM" as the label.
func (p *GPULoadPlugin) secondaryLabel(src string, showVRAM bool, memPct float64) string {
	if src == "memory" {
		return "MEM"
	}
	if showVRAM && memPct >= 0 {
		return fmt.Sprintf("MEM %.0f%%", memPct)
	}
	return "GPU"
}

// readLoad returns (gpuPct, memPct, error).
// memPct is -1 when not available for this vendor/source.
func (p *GPULoadPlugin) readLoad() (gpuPct, memPct float64, err error) {
	switch p.vendor {
	case vendorNVIDIA:
		return p.readNVIDIA()
	case vendorAMD:
		return p.readAMD()
	case vendorIntel:
		return p.readIntel()
	case vendorGeneric:
		return p.readGenericSysfs()
	}
	return 0, -1, fmt.Errorf("no GPU vendor detected")
}

// readNVIDIA queries nvidia-smi for gpu and memory controller utilisation.
func (p *GPULoadPlugin) readNVIDIA() (gpuPct, memPct float64, err error) {
	cmd := exec.Command("nvidia-smi",
		"--query-gpu=utilization.gpu,utilization.memory",
		"--format=csv,noheader,nounits")
	out, err := cmd.Output()
	if err != nil {
		return 0, -1, fmt.Errorf("nvidia-smi failed: %w", err)
	}
	parts := strings.SplitN(strings.TrimSpace(string(out)), ",", 2)
	if len(parts) != 2 {
		return 0, -1, fmt.Errorf("unexpected nvidia-smi output: %q", string(out))
	}
	gpuPct, err = strconv.ParseFloat(strings.TrimSpace(parts[0]), 64)
	if err != nil {
		return 0, -1, fmt.Errorf("parse gpu%%: %w", err)
	}
	memPct, err = strconv.ParseFloat(strings.TrimSpace(parts[1]), 64)
	if err != nil {
		memPct = -1
	}
	return gpuPct, memPct, nil
}

// readAMD reads gpu_busy_percent and mem_busy_percent from sysfs.
func (p *GPULoadPlugin) readAMD() (gpuPct, memPct float64, err error) {
	gpuPct, err = readSysfsFloat(filepath.Join(p.vendorPath, "gpu_busy_percent"))
	if err != nil {
		return 0, -1, fmt.Errorf("amd gpu_busy_percent: %w", err)
	}
	memPct, _ = readSysfsFloat(filepath.Join(p.vendorPath, "mem_busy_percent"))
	if memPct < 0 {
		memPct = -1
	}
	return gpuPct, memPct, nil
}

// readIntel returns GPU load for Intel GPUs.
// i915/xe do not expose a simple sysfs busy_percent — the proper path is fdinfo
// (kernel 5.19+) which requires iterating /proc/*/fdinfo and summing per-engine
// deltas across all DRM clients, similar to what nvtop does. That is deferred.
// For now we fall through to the generic sysfs path, which works on some
// driver/kernel combinations that do expose gpu_busy_percent; otherwise the
// payload shows "— Intel GPU" so the user knows it was detected but unsupported.
func (p *GPULoadPlugin) readIntel() (gpuPct, memPct float64, err error) {
	gpuPct, err = readSysfsFloat(filepath.Join(p.vendorPath, "gpu_busy_percent"))
	if err != nil {
		return 0, -1, fmt.Errorf("intel gpu load requires fdinfo (not yet implemented): %w", err)
	}
	memPct, _ = readSysfsFloat(filepath.Join(p.vendorPath, "mem_busy_percent"))
	if memPct < 0 {
		memPct = -1
	}
	return gpuPct, memPct, nil
}

// readGenericSysfs tries gpu_busy_percent on the detected card path.
func (p *GPULoadPlugin) readGenericSysfs() (gpuPct, memPct float64, err error) {
	gpuPct, err = readSysfsFloat(filepath.Join(p.vendorPath, "gpu_busy_percent"))
	if err != nil {
		return 0, -1, fmt.Errorf("generic gpu_busy_percent: %w", err)
	}
	memPct, _ = readSysfsFloat(filepath.Join(p.vendorPath, "mem_busy_percent"))
	if memPct < 0 {
		memPct = -1
	}
	return gpuPct, memPct, nil
}

// detectVendor probes hardware in priority order: NVIDIA → AMD → Intel → generic.
func (p *GPULoadPlugin) detectVendor() {
	// NVIDIA: try nvidia-smi
	cmd := exec.Command("nvidia-smi", "--query-gpu=utilization.gpu", "--format=csv,noheader,nounits")
	if out, err := cmd.Output(); err == nil && len(strings.TrimSpace(string(out))) > 0 {
		p.vendor = vendorNVIDIA
		return
	}

	// Scan DRM cards for AMD / Intel / generic
	cards, err := filepath.Glob("/sys/class/drm/card[0-9]*")
	if err != nil {
		return
	}
	for _, card := range cards {
		// Skip non-device entries (e.g. card0-DP-1)
		if _, err := os.Stat(filepath.Join(card, "device", "vendor")); err != nil {
			continue
		}
		devPath := filepath.Join(card, "device")
		vendor := strings.TrimSpace(readStringFile(filepath.Join(devPath, "vendor")))

		switch vendor {
		case "0x1002": // AMD
			if _, err := os.Stat(filepath.Join(devPath, "gpu_busy_percent")); err == nil {
				p.vendor = vendorAMD
				p.vendorPath = devPath
				return
			}
		case "0x8086": // Intel
			p.vendor = vendorIntel
			p.vendorPath = devPath
			return
		default:
			// Generic: any card exposing gpu_busy_percent
			if _, err := os.Stat(filepath.Join(devPath, "gpu_busy_percent")); err == nil {
				p.vendor = vendorGeneric
				p.vendorPath = devPath
				return
			}
		}
	}
}

// Configure applies per-zone plugin configuration.
func (p *GPULoadPlugin) Configure(cfg map[string]any) error {
	if src, ok := cfg["source"].(string); ok && (src == "gpu" || src == "memory") {
		p.sourceMu.Lock()
		p.source = src
		p.sourceMu.Unlock()
	}

	if v, ok := cfg["show_vram"].(bool); ok {
		p.showVRAMMu.Lock()
		p.showVRAM = v
		p.showVRAMMu.Unlock()
	}

	p.graphMu.Lock()
	if gt, ok := cfg["graph_type"].(string); ok && gt != "" {
		g := plugin.GraphType(gt)
		if g == plugin.GraphTypeSparkline || g == plugin.GraphTypeBar || g == plugin.GraphTypeArea {
			p.graphType = g
		}
	}
	p.graphMu.Unlock()

	return nil
}

func (p *GPULoadPlugin) addToHistory(v float32) {
	p.historyMu.Lock()
	defer p.historyMu.Unlock()
	p.history = append(p.history, v/100.0)
	if len(p.history) > p.maxHistory {
		p.history = p.history[len(p.history)-p.maxHistory:]
	}
}

func (p *GPULoadPlugin) getSparkline() []float32 {
	p.historyMu.Lock()
	defer p.historyMu.Unlock()
	if len(p.history) == 0 {
		return nil
	}
	out := make([]float32, len(p.history))
	copy(out, p.history)
	return out
}

func severityFor(pct float64) plugin.Severity {
	switch {
	case pct >= 90:
		return plugin.SeverityCrit
	case pct >= 70:
		return plugin.SeverityWarn
	default:
		return plugin.SeverityOK
	}
}

func readSysfsFloat(path string) (float64, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return -1, err
	}
	return strconv.ParseFloat(strings.TrimSpace(string(data)), 64)
}

func readStringFile(path string) string {
	data, err := os.ReadFile(path)
	if err != nil {
		return ""
	}
	return string(data)
}

func main() {
	goplugin.Serve(&goplugin.ServeConfig{
		HandshakeConfig: plugin.Handshake,
		Plugins: goplugin.PluginSet{
			"plugin": &plugin.ExecPlugin{Impl: NewGPULoadPlugin()},
		},
	})
}
