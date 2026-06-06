// cpu-load monitors CPU utilisation across configurable topology groups.
// Supports Linux (full topology), Windows and macOS (aggregate/core/logical only).
package main

import (
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"time"

	goplugin "github.com/hashicorp/go-plugin"
	"github.com/shirou/gopsutil/cpu"

	"github.com/mantonx/nexus-next/pkg/plugin"
)

// cpuInfo holds the topology attributes of one logical CPU, read once at startup.
type cpuInfo struct {
	idx      int // logical CPU index (position in p.topo)
	pkg      int // physical_package_id
	die      int // die_id (-1 if unavailable)
	cluster  int // cluster_id (-1 if unavailable)
	core     int // core_id
	capacity int // cpu_capacity (-1 if unavailable)
}

// sourceSpec identifies one resolved source group.
type sourceSpec struct {
	raw    string // original config string, e.g. "p-cores"
	label  string // display label, e.g. "P-CORES"
	cpus   []int  // logical CPU indices belonging to this group
	valid  bool   // false if the group couldn't be resolved on this hardware
}

// CPULoadPlugin monitors CPU utilisation.
type CPULoadPlugin struct {
	// topology is built once at init; read-only thereafter
	topo     []cpuInfo // index = logical CPU number
	topoOnce sync.Once

	// resolved sources, rebuilt on config change
	sources   []sourceSpec
	sourcesMu sync.RWMutex

	// sparkline history for first source
	history    []float32
	historyMu  sync.Mutex
	maxHistory int

	graphType plugin.GraphType
	graphMu   sync.RWMutex
}

func NewCPULoadPlugin() *CPULoadPlugin {
	p := &CPULoadPlugin{
		history:    make([]float32, 0, 60),
		maxHistory: 60,
		graphType:  plugin.GraphTypeSparkline,
	}
	// Default: single source "all"
	p.sources = []sourceSpec{{raw: "all", label: "CPU", valid: true}}
	return p
}

func (p *CPULoadPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "CPU Load",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Monitors CPU utilisation by topology group (Linux, Windows, macOS)",
		Icon:        "microchip",
		RefreshMs:   1000,
	}, nil
}

func (p *CPULoadPlugin) Sample() (plugin.Payload, error) {
	p.topoOnce.Do(p.buildTopology)

	// Per-logical-CPU percentages since last call (interval=0 = non-blocking delta)
	perCPU, err := cpu.Percent(0, true)
	if err != nil || len(perCPU) == 0 {
		return plugin.Payload{
			Primary:   "—",
			Secondary: "No CPU",
			Severity:  plugin.SeverityWarn,
			TTL:       2 * time.Second,
			Timestamp: time.Now(),
		}, nil
	}

	p.sourcesMu.RLock()
	srcs := p.sources
	p.sourcesMu.RUnlock()

	// Compute load % for each source
	loads := make([]float64, len(srcs))
	for i, s := range srcs {
		loads[i] = p.groupLoad(s, perCPU)
	}

	// Sparkline tracks first source
	p.addToHistory(float32(loads[0]))

	p.graphMu.RLock()
	gt := p.graphType
	p.graphMu.RUnlock()

	primary, secondary := p.formatDisplay(srcs, loads)
	severity := p.worstSeverity(loads)

	return plugin.Payload{
		Primary:          primary,
		Secondary:        secondary,
		Severity:         severity,
		Spark:            p.getSparkline(),
		GraphType:        gt,
		TTL:              2 * time.Second,
		Timestamp:        time.Now(),
		GraphBgOpacity:   0,
		GraphLineOpacity: 0,
	}, nil
}

// groupLoad averages perCPU percentages across the logical CPUs in s.
// Falls back to averaging all CPUs when s.valid is false.
func (p *CPULoadPlugin) groupLoad(s sourceSpec, perCPU []float64) float64 {
	indices := s.cpus
	if !s.valid || len(indices) == 0 {
		// fallback: all
		indices = make([]int, len(perCPU))
		for i := range indices {
			indices[i] = i
		}
	}
	var sum float64
	var n int
	for _, idx := range indices {
		if idx < len(perCPU) {
			sum += perCPU[idx]
			n++
		}
	}
	if n == 0 {
		return 0
	}
	return sum / float64(n)
}

// formatDisplay builds Primary and Secondary strings for single or multi-source.
func (p *CPULoadPlugin) formatDisplay(srcs []sourceSpec, loads []float64) (primary, secondary string) {
	if len(srcs) == 1 {
		lbl := srcs[0].label
		if !srcs[0].valid {
			lbl = "CPU ?"
		}
		return fmt.Sprintf("%.0f%%", loads[0]), lbl
	}
	lines := make([]string, len(srcs))
	for i, s := range srcs {
		lbl := sourceLabel(s.raw) // short label for inline display
		if !s.valid {
			lbl = "?"
		}
		lines[i] = fmt.Sprintf("%.0f%%  %s", loads[i], lbl)
	}
	return strings.Join(lines, "\n"), ""
}

func (p *CPULoadPlugin) worstSeverity(loads []float64) plugin.Severity {
	worst := plugin.SeverityOK
	for _, v := range loads {
		s := severityFor(v)
		if s == plugin.SeverityCrit {
			return plugin.SeverityCrit
		}
		if s == plugin.SeverityWarn {
			worst = plugin.SeverityWarn
		}
	}
	return worst
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

// OnConfigChanged handles live config updates.
func (p *CPULoadPlugin) OnConfigChanged(config map[string]interface{}) error {
	p.topoOnce.Do(p.buildTopology)

	// Parse source — accepts string or []interface{}
	if raw, ok := config["source"]; ok {
		specs := p.parseSourceConfig(raw)
		p.sourcesMu.Lock()
		p.sources = specs
		p.sourcesMu.Unlock()
	}

	p.graphMu.Lock()
	if gt, ok := config["graph_type"].(string); ok && gt != "" {
		g := plugin.GraphType(gt)
		if g == plugin.GraphTypeSparkline || g == plugin.GraphTypeBar || g == plugin.GraphTypeArea {
			p.graphType = g
		}
	}
	p.graphMu.Unlock()

	return nil
}

// parseSourceConfig converts the raw config value (string or []interface{}) into
// resolved sourceSpecs, capped at 3.
func (p *CPULoadPlugin) parseSourceConfig(raw interface{}) []sourceSpec {
	var names []string
	switch v := raw.(type) {
	case string:
		names = []string{v}
	case []interface{}:
		for _, item := range v {
			if s, ok := item.(string); ok {
				names = append(names, s)
			}
		}
	}
	if len(names) == 0 {
		names = []string{"all"}
	}
	if len(names) > 3 {
		fmt.Printf("cpu-load: source capped at 3 (got %d)\n", len(names))
		names = names[:3]
	}
	specs := make([]sourceSpec, len(names))
	for i, name := range names {
		specs[i] = p.resolveSource(name)
	}
	return specs
}

// resolveSource maps a source name to a sourceSpec with the matching logical CPU indices.
func (p *CPULoadPlugin) resolveSource(name string) sourceSpec {
	label := sourceLabelLong(name)

	// "all" — use empty cpus slice; groupLoad handles the fallback
	if name == "all" {
		return sourceSpec{raw: name, label: label, cpus: nil, valid: true}
	}

	// Linux-only topology groupings
	if runtime.GOOS != "linux" {
		fmt.Printf("cpu-load: source %q not supported on %s, falling back to all\n", name, runtime.GOOS)
		return sourceSpec{raw: name, label: label, valid: false}
	}

	switch {
	case strings.HasPrefix(name, "package:"):
		n := parseIndex(name, "package:")
		return p.topoGroup(name, label, func(c cpuInfo) bool { return c.pkg == n })

	case strings.HasPrefix(name, "numa:"):
		n := parseIndex(name, "numa:")
		cpus := p.numaNodeCPUs(n)
		if len(cpus) == 0 {
			fmt.Printf("cpu-load: NUMA node %d not found\n", n)
			return sourceSpec{raw: name, label: label, valid: false}
		}
		return sourceSpec{raw: name, label: label, cpus: cpus, valid: true}

	case strings.HasPrefix(name, "die:"):
		n := parseIndex(name, "die:")
		s := p.topoGroup(name, label, func(c cpuInfo) bool { return c.die >= 0 && c.die == n })
		if !s.valid {
			fmt.Printf("cpu-load: die grouping unavailable (kernel 6.10+ required)\n")
		}
		return s

	case strings.HasPrefix(name, "cluster:"):
		n := parseIndex(name, "cluster:")
		s := p.topoGroup(name, label, func(c cpuInfo) bool { return c.cluster >= 0 && c.cluster == n })
		if !s.valid {
			fmt.Printf("cpu-load: cluster grouping unavailable (kernel 6.10+ required)\n")
		}
		return s

	case name == "p-cores":
		// Prefer cpu_capacity differentiation; fall back to thread-sibling count.
		// P-cores have SMT siblings (thread_siblings_list is a range like "0-1").
		// E-cores are single-threaded (thread_siblings_list is a single number).
		minCap, maxCap := p.minMaxCapacity()
		if maxCap >= 0 && minCap != maxCap {
			return p.topoGroup(name, label, func(c cpuInfo) bool { return c.capacity == maxCap })
		}
		// Fallback: detect by SMT sibling count
		s := p.topoGroup(name, label, func(c cpuInfo) bool { return p.hasSMTSibling(c) })
		if !s.valid {
			fmt.Printf("cpu-load: p-cores not detectable on this CPU\n")
		}
		return s

	case name == "e-cores":
		minCap, maxCap := p.minMaxCapacity()
		if maxCap >= 0 && minCap != maxCap {
			return p.topoGroup(name, label, func(c cpuInfo) bool { return c.capacity == minCap })
		}
		// Fallback: no SMT sibling = E-core (single-threaded)
		s := p.topoGroup(name, label, func(c cpuInfo) bool { return !p.hasSMTSibling(c) })
		if !s.valid {
			fmt.Printf("cpu-load: e-cores not detectable on this CPU\n")
		}
		return s
	case strings.HasPrefix(name, "core:"):
		n := parseIndex(name, "core:")
		return p.topoGroup(name, label, func(c cpuInfo) bool { return c.core == n })

	case strings.HasPrefix(name, "logical:"):
		n := parseIndex(name, "logical:")
		if n < 0 || n >= len(p.topo) {
			return sourceSpec{raw: name, label: label, valid: false}
		}
		return sourceSpec{raw: name, label: label, cpus: []int{n}, valid: true}
	}

	fmt.Printf("cpu-load: unknown source %q, falling back to all\n", name)
	return sourceSpec{raw: name, label: "CPU ?", valid: false}
}

// topoGroup filters p.topo by predicate and returns a sourceSpec.
func (p *CPULoadPlugin) topoGroup(raw, label string, match func(cpuInfo) bool) sourceSpec {
	var cpus []int
	for i, c := range p.topo {
		if match(c) {
			cpus = append(cpus, i)
		}
	}
	if len(cpus) == 0 {
		return sourceSpec{raw: raw, label: label, valid: false}
	}
	return sourceSpec{raw: raw, label: label, cpus: cpus, valid: true}
}

// buildTopology reads sysfs topology files once and populates p.topo.
func (p *CPULoadPlugin) buildTopology() {
	if runtime.GOOS != "linux" {
		return
	}
	entries, err := filepath.Glob("/sys/devices/system/cpu/cpu[0-9]*")
	if err != nil {
		return
	}
	// Find max logical CPU index
	maxIdx := -1
	for _, e := range entries {
		n := parseCPUIndex(e)
		if n > maxIdx {
			maxIdx = n
		}
	}
	if maxIdx < 0 {
		return
	}
	p.topo = make([]cpuInfo, maxIdx+1)
	for i := range p.topo {
		p.topo[i] = cpuInfo{idx: i, pkg: 0, die: -1, cluster: -1, core: 0, capacity: -1}
	}
	for _, e := range entries {
		n := parseCPUIndex(e)
		if n < 0 {
			continue
		}
		base := filepath.Join(e, "topology")
		p.topo[n].pkg = readIntFile(filepath.Join(base, "physical_package_id"), 0)
		p.topo[n].die = readIntFile(filepath.Join(base, "die_id"), -1)
		p.topo[n].cluster = readIntFile(filepath.Join(base, "cluster_id"), -1)
		p.topo[n].core = readIntFile(filepath.Join(base, "core_id"), 0)
		p.topo[n].capacity = readIntFile(filepath.Join(e, "cpu_capacity"), -1)
	}
}

// hasSMTSibling returns true when the logical CPU has a hyperthread sibling —
// its thread_siblings_list is a range like "0-1" rather than a single number.
// P-cores (Intel hybrid) have SMT siblings; E-cores do not.
func (p *CPULoadPlugin) hasSMTSibling(c cpuInfo) bool {
	path := fmt.Sprintf("/sys/devices/system/cpu/cpu%d/topology/thread_siblings_list", c.idx)
	data, err := os.ReadFile(path)
	if err != nil {
		return false
	}
	return strings.Contains(strings.TrimSpace(string(data)), "-")
}

// numaNodeCPUs reads the CPU list for a NUMA node from sysfs.
func (p *CPULoadPlugin) numaNodeCPUs(node int) []int {
	path := fmt.Sprintf("/sys/devices/system/node/node%d/cpulist", node)
	data, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	return parseCPUList(strings.TrimSpace(string(data)))
}

// maxCapacity returns the highest cpu_capacity value seen, or -1 if none.
func (p *CPULoadPlugin) maxCapacity() int {
	max := -1
	for _, c := range p.topo {
		if c.capacity > max {
			max = c.capacity
		}
	}
	return max
}

// minMaxCapacity returns the lowest and highest cpu_capacity values.
func (p *CPULoadPlugin) minMaxCapacity() (min, max int) {
	min, max = -1, -1
	for _, c := range p.topo {
		if c.capacity < 0 {
			continue
		}
		if min < 0 || c.capacity < min {
			min = c.capacity
		}
		if c.capacity > max {
			max = c.capacity
		}
	}
	return
}

// sourceLabel returns a display label for a source name.
// Single-source labels can be longer (shown in Secondary at small font).
// Multi-source labels are embedded in Primary lines alongside the value,
// so they must be short enough to fit in a 160px zone at 12pt.
func sourceLabel(name string) string {
	switch {
	case name == "all":
		return "CPU"
	case name == "p-cores":
		return "P"
	case name == "e-cores":
		return "E"
	case strings.HasPrefix(name, "package:"):
		return "PKG" + strings.TrimPrefix(name, "package:")
	case strings.HasPrefix(name, "numa:"):
		return "N" + strings.TrimPrefix(name, "numa:")
	case strings.HasPrefix(name, "die:"):
		return "CCD" + strings.TrimPrefix(name, "die:")
	case strings.HasPrefix(name, "cluster:"):
		return "CCX" + strings.TrimPrefix(name, "cluster:")
	case strings.HasPrefix(name, "core:"):
		return "C" + strings.TrimPrefix(name, "core:")
	case strings.HasPrefix(name, "logical:"):
		return "CPU" + strings.TrimPrefix(name, "logical:")
	}
	return strings.ToUpper(name)
}

// sourceLabelLong returns the full display label used in Secondary for single-source display.
func sourceLabelLong(name string) string {
	switch {
	case name == "all":
		return "CPU"
	case name == "p-cores":
		return "P-CORES"
	case name == "e-cores":
		return "E-CORES"
	case strings.HasPrefix(name, "package:"):
		return "PKG " + strings.TrimPrefix(name, "package:")
	case strings.HasPrefix(name, "numa:"):
		return "NUMA " + strings.TrimPrefix(name, "numa:")
	case strings.HasPrefix(name, "die:"):
		return "CCD " + strings.TrimPrefix(name, "die:")
	case strings.HasPrefix(name, "cluster:"):
		return "CCX " + strings.TrimPrefix(name, "cluster:")
	case strings.HasPrefix(name, "core:"):
		return "CORE " + strings.TrimPrefix(name, "core:")
	case strings.HasPrefix(name, "logical:"):
		return "CPU " + strings.TrimPrefix(name, "logical:")
	}
	return strings.ToUpper(name)
}

func (p *CPULoadPlugin) addToHistory(v float32) {
	p.historyMu.Lock()
	defer p.historyMu.Unlock()
	p.history = append(p.history, v/100.0)
	if len(p.history) > p.maxHistory {
		p.history = p.history[len(p.history)-p.maxHistory:]
	}
}

func (p *CPULoadPlugin) getSparkline() []float32 {
	p.historyMu.Lock()
	defer p.historyMu.Unlock()
	if len(p.history) == 0 {
		return nil
	}
	out := make([]float32, len(p.history))
	copy(out, p.history)
	return out
}

// --- helpers ---

func parseIndex(s, prefix string) int {
	n, err := strconv.Atoi(strings.TrimPrefix(s, prefix))
	if err != nil {
		return -1
	}
	return n
}

func parseCPUIndex(path string) int {
	base := filepath.Base(path)
	n, err := strconv.Atoi(strings.TrimPrefix(base, "cpu"))
	if err != nil {
		return -1
	}
	return n
}

func readIntFile(path string, def int) int {
	data, err := os.ReadFile(path)
	if err != nil {
		return def
	}
	n, err := strconv.Atoi(strings.TrimSpace(string(data)))
	if err != nil {
		return def
	}
	return n
}

// parseCPUList parses a Linux cpulist string like "0-3,8,10-11" into a slice of ints.
func parseCPUList(s string) []int {
	var out []int
	for _, part := range strings.Split(s, ",") {
		part = strings.TrimSpace(part)
		if dash := strings.Index(part, "-"); dash >= 0 {
			lo, err1 := strconv.Atoi(part[:dash])
			hi, err2 := strconv.Atoi(part[dash+1:])
			if err1 != nil || err2 != nil {
				continue
			}
			for i := lo; i <= hi; i++ {
				out = append(out, i)
			}
		} else {
			n, err := strconv.Atoi(part)
			if err == nil {
				out = append(out, n)
			}
		}
	}
	return out
}

func main() {
	goplugin.Serve(&goplugin.ServeConfig{
		HandshakeConfig: plugin.Handshake,
		Plugins: goplugin.PluginSet{
			"plugin": &plugin.ExecPlugin{Impl: NewCPULoadPlugin()},
		},
	})
}
