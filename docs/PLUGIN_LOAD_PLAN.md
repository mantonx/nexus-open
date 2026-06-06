# Plugin Plan: cpu-load & gpu-load

**Status: Implemented** — both plugins are live in `plugins/cpu-load/` and
`plugins/gpu-load/`. This document records design decisions and known issues.

Planning document for two new external plugins that measure CPU and GPU utilisation
(load %) rather than temperature.

---

## 1. Why These Plugins Matter

Temperature tells you *how hot* a component is; load tells you *how busy* it is.
A CPU running a single-threaded task at 100 % on one core may be at moderate temp,
while a lightly-threaded compile job may look warm but idle.  Both metrics together
give a much fuller picture on the Nexus bar.

---

## 2. Architecture Fit

Both plugins follow the **external (exec) plugin** pattern used by `cpu-temp`,
`gpu-temp`, and `network`:

```
plugins/
  cpu-load/
    main.go
    go.mod
    go.sum
  gpu-load/
    main.go
    go.mod
    go.sum
```

Each binary:
- Implements `plugin.Plugin` (`Describe()` + `Sample()`)
- Optionally implements `plugin.PluginConfigNotifier` (`OnConfigChanged()`)
- Is registered in a zone config as `exec:./plugins/cpu-load/cpu-load`
- Has its own `go.mod` with a `replace` directive pointing at the repo root

---

## 3. cpu-load Plugin

### 3.1 What We Measure

A single load percentage (0–100 %) averaged over the CPUs that belong to the
configured `source` group.  The source is read from `/proc/stat` (per logical CPU
lines) with topology grouping built at startup from
`/sys/devices/system/cpu/cpuN/topology/`.

### 3.2 Source Groups

The `source` config key selects which CPUs contribute to the measurement.

| `source` value | What it averages                                                    | Hardware requirement                                                               |
| -------------- | ------------------------------------------------------------------- | ---------------------------------------------------------------------------------- |
| `all`          | All logical CPUs — the top `cpu` line in `/proc/stat`               | Universal                                                                          |
| `package:N`    | All logical CPUs in physical package N                              | Multi-socket servers (EPYC, Xeon); single-socket systems only have `package:0`     |
| `numa:N`       | All logical CPUs in NUMA node N                                     | AMD EPYC/Threadripper (multiple nodes per socket); also multi-socket Xeon          |
| `die:N`        | All logical CPUs on die/CCD N                                       | AMD Ryzen (chiplet architecture); requires kernel 6.10+                            |
| `cluster:N`    | All logical CPUs in cluster/CCX N                                   | AMD Ryzen CCX grouping; requires kernel 6.10+                                      |
| `p-cores`      | All logical CPUs with high `cpu_capacity` value                     | Intel 12th gen+ (Alder Lake, Raptor Lake, Meteor Lake)                             |
| `e-cores`      | All logical CPUs with low `cpu_capacity` value                      | Intel 12th gen+                                                                    |
| `core:N`       | Logical CPUs sharing physical core N (both hyperthreads if enabled) | Universal                                                                          |
| `logical:N`    | A single logical CPU thread                                         | Universal                                                                          |

**Detection at startup:**

The plugin reads the topology files once when it initialises and builds a map of
`logicalIndex → {package, die, cluster, core, capacity}`.  If a requested grouping
doesn't exist on the current hardware (e.g. `die:1` on a single-die CPU, or
`p-cores` on a non-hybrid Intel), the plugin falls back to `all` and logs a warning
in the secondary text.

`die` and `cluster` groupings check for file existence at
`/sys/devices/system/cpu/cpu0/topology/die_id` — if absent (pre-6.10 kernel or
non-AMD), those source values are treated as unavailable.

`p-cores` / `e-cores` check for `/sys/devices/system/cpu/cpu0/cpu_capacity` —
if absent (non-hybrid CPU), those source values fall back to `all`.

**NUMA node detection** reads `/sys/devices/system/node/nodeN/cpulist`.

### 3.3 Sensor Sources (by OS)

#### Linux

All groupings read `/proc/stat` per-logical-CPU lines, then average the load
across the logical CPUs belonging to the configured group.

Load for each logical CPU:

```
total_delta = (user+nice+system+idle+iowait+irq+softirq+steal) difference between snapshots
busy_delta  = total_delta − idle_delta
load %      = busy_delta / total_delta × 100
```

Two snapshots required; first sample returns 0 % while baseline is established.

#### Windows / macOS

`gopsutil/cpu.Percent(0, true)` provides per-logical-CPU percentages on all
platforms.  Only `all`, `core:N`, and `logical:N` groupings are supported on
Windows/macOS — topology sysfs files don't exist, so package/NUMA/die/cluster/
p-cores/e-cores fall back to `all`.

### 3.4 Configurable Options

| Key          | Type            | Default        | Description                                          |
| ------------ | --------------- | -------------- | ---------------------------------------------------- |
| `source`     | string or array | `"all"`        | One source string, or an array of up to 3            |
| `graph_type` | string          | `"sparkline"`  | `"sparkline"`, `"bar"`, `"area"`                     |

`source` accepts either a bare string (`"all"`) or a YAML array of up to 3 source
values (`["p-cores", "e-cores"]`).  Beyond 3 entries the plugin logs a warning and
uses only the first 3.

### 3.5 Display Format

**Single source:**

```
Primary:   "42%"
Secondary: "CPU"       source=all
           "PKG 0"     source=package:0
           "NUMA 1"    source=numa:1
           "CCD 0"     source=die:0
           "CCX 2"     source=cluster:2
           "P-CORES"   source=p-cores
           "E-CORES"   source=e-cores
           "CORE 3"    source=core:3
           "CPU 7"     source=logical:7
           "CPU ?"     source unavailable, fell back to all
Spark:     history of that source, fixed 0–100 % scale
Severity:  ok <70 % / warn ≥70 % / crit ≥90 %
```

**Multiple sources (2–3):**

Each source gets one line, combining its value and label.  The `Payload.Primary`
field carries all lines joined by `\n`; the `Payload.Secondary` field is left empty
since the labels are embedded in primary.

```text
# source: ["p-cores", "e-cores"]
Primary:   "72%  P-CORES\n45%  E-CORES"

# source: ["package:0", "package:1", "numa:0"]
Primary:   "61%  PKG 0\n58%  PKG 1\n44%  NUMA 0"
```

Severity is the worst (highest) value across all sources.

Sparkline tracks the first source only.  Rendering multiple sparklines is a future
renderer feature.

### 3.6 Sparkline Normalisation

Fixed 0–100 % scale (not percentile-relative like cpu-temp).  A machine idling at
5 % should still show spikes clearly at their true scale.

---

## 4. gpu-load Plugin

### 4.1 What We Measure

**Target metric:** GPU engine utilisation as a percentage (0–100 %).  On discrete
GPUs this is the 3D/compute engine; on integrated GPUs it is whatever the driver
exposes.

Optionally: VRAM usage % (secondary metric), exposed as `Secondary` text.

### 4.2 Sensor Sources (priority order)

#### NVIDIA — `nvidia-smi` (most reliable)

```bash
nvidia-smi --query-gpu=utilization.gpu,utilization.memory \
           --format=csv,noheader,nounits
# → "35, 5"
```

Both GPU core and VRAM utilisation in one call.  The network/gpu-temp approach of
running a subprocess per sample is acceptable here; `nvidia-smi` typically adds
~30 ms latency but at a 2 s refresh this is negligible.

Additional optional metrics from nvidia-smi:
- `power.draw` (W)
- `clocks.current.graphics` (MHz)
- `clocks.current.memory` (MHz)
- `memory.used` / `memory.total` (MiB)

#### AMD — sysfs `gpu_busy_percent` (primary for amdgpu driver)

```
/sys/class/drm/cardX/device/gpu_busy_percent   → integer 0–100
/sys/class/drm/cardX/device/mem_busy_percent   → integer 0–100 (VRAM)
```

Detection: glob `/sys/class/drm/card[0-9]`, check `device/vendor` == `0x1002`.

Fallback: `rocm-smi --showuse --csv` (ROCm stack only, less common).

#### Intel — sysfs `i915` perf

```
/sys/class/drm/cardX/gt/gt0/rc6_residency_ms   → inverted (100 - rc6 % ≈ active %)
```

Intel's i915 driver does not expose a direct `busy_percent` sysfs file.
RC6 residency measures idle time; active ≈ 100 % − RC6 %.
This is a rough approximation; label it as such in the secondary text.

Detection: vendor `0x8086`.

#### Generic sysfs fallback

```
/sys/class/drm/cardX/device/gpu_busy_percent
```

Try any card that exposes this file, regardless of vendor.

#### Windows — `gopsutil` or WMI

`gopsutil` does not reliably expose GPU utilisation on Windows without additional
libraries.  For now: run `nvidia-smi` if available, else show "—".  A future
revision can add DXGI/WMI-based reads.

#### macOS — `powermetrics` (root required) or show "—"

`powermetrics --samplers gpu_power` provides GPU active residency.  Requires root.
For a non-root default: show "— GPU" and note in secondary that elevated
privileges are needed.

### 4.3 Configurable Options

| Key          | Type   | Default       | Description                                     |
| ------------ | ------ | ------------- | ----------------------------------------------- |
| `source`     | string | `"gpu"`       | Which metric to display — see source table      |
| `graph_type` | string | `"sparkline"` | `"sparkline"`, `"bar"`, `"area"`                |
| `show_vram`  | bool   | `true`        | Include VRAM % in secondary line                |

**Source values for gpu-load:**

| `source` value | What it measures              | Vendor support                                          |
| -------------- | ----------------------------- | ------------------------------------------------------- |
| `gpu`          | 3D/compute engine utilisation | NVIDIA, AMD, Intel (fdinfo)                             |
| `memory`       | Memory controller busy %      | NVIDIA (`utilization.memory`), AMD (`mem_busy_percent`) |

### 4.4 Display Format

```
Primary:   "35%"              (GPU core load)
Secondary: "VRAM 5%"          (when show_vram=true and VRAM data available)
           "GPU"              (fallback if VRAM not available)
Spark:     60-point history of GPU core load, fixed 0–100 range
Severity:  ok / warn / crit
```

### 4.5 Sparkline Normalisation

Same rationale as cpu-load: fixed 0–100 range.  GPU load swings more dramatically
than temperature; relative normalisation would make a spike at 80 % look identical
to a sustained 80 % baseline.

### 4.6 Vendor Detection Caching

Detecting the GPU vendor (running nvidia-smi, globbing sysfs) on every `Sample()`
call is wasteful.  Detect once at startup (or on first successful sample) and cache
in a `vendor` field protected by a `sync.Once` or `sync.Mutex`, mirroring the
`vendorMu` pattern in `gpu-temp`.

---

## 5. Shared Patterns Between Both Plugins

### History / sparkline

```go
type loadPlugin struct {
    history    []float32   // last 60 samples, 0.0–1.0 (normalised from %)
    historyMu  sync.Mutex
    maxHistory int         // 60
    // ...
}
```

Normalise for sparkline storage as `value / 100.0` so the slice is already in the
expected `[0, 1]` domain.

### Config change handler

Both plugins support `OnConfigChanged(config map[string]interface{})` for:
- `graph_type` — live switch between sparkline/bar/area
- `warn_above` / `crit_above` — live threshold update
- `show_vram` (gpu-load only)

### Error payload

When the sensor cannot be read (no GPU, no permissions, binary not found):

```go
plugin.Payload{
    Primary:   "—",
    Secondary: "No data",
    Severity:  plugin.SeverityWarn,
    TTL:       2 * time.Second,
    Timestamp: time.Now(),
}
```

Return `nil` for the error so the sampler continues polling rather than marking the
zone permanently failed.

---

## 6. Compatibility Matrix

| Platform | cpu-load | gpu-load (NVIDIA) | gpu-load (AMD) | gpu-load (Intel) |
|----------|----------|-------------------|----------------|------------------|
| Linux    | `/proc/stat` or gopsutil | `nvidia-smi` | sysfs `gpu_busy_percent` | RC6 approximation |
| Windows  | gopsutil (WMI) | `nvidia-smi` | — (future) | — |
| macOS    | gopsutil | — | — | — |

**Linux is the primary target** (Corsair iCUE Nexus is typically connected to a Linux
desktop/gaming rig).  Windows and macOS are best-effort.

---

## 7. Dependencies

### cpu-load

Add `gopsutil/cpu` to the plugin's `go.mod`:

```
github.com/shirou/gopsutil v3.21.11+incompatible
```

Alternatively implement the `/proc/stat` delta directly (no extra dep, ~30 lines).
The direct approach avoids the WMI indirect dep on Linux, but gopsutil gives
cross-platform for free.

**Recommendation:** Use `gopsutil/cpu` — already a precedent in `network` plugin;
keeps the code short and cross-platform.

### gpu-load

No extra Go deps needed on Linux (sysfs reads + `nvidia-smi` subprocess).
NVIDIA path already demonstrated in `gpu-temp`.

---

## 8. File Layout

```
plugins/
  cpu-load/
    main.go        ← plugin implementation
    go.mod
    go.sum
  gpu-load/
    main.go        ← plugin implementation
    go.mod
    go.sum
```

Binary names follow the project convention: `nexus-cpu-load`, `nexus-gpu-load`
(built via `go build -o nexus-cpu-load .` inside each directory).

---

## 9. Example Zone Config

```yaml
pages:
  - name: "Load"
    zones:
      # Single-source: total CPU load
      - id: load.cpu
        width: 160
        plugin: exec:./plugins/cpu-load/cpu-load
        refresh_ms: 1000
        align: left
        module_config:
          source: "all"
          graph_type: sparkline

      # Multi-source: P-cores vs E-cores side by side (Intel 12th gen+)
      - id: load.cpu-hybrid
        width: 160
        plugin: exec:./plugins/cpu-load/cpu-load
        refresh_ms: 1000
        align: left
        module_config:
          source: ["p-cores", "e-cores"]
          graph_type: sparkline

      # GPU core load with memory controller % in secondary
      - id: load.gpu
        width: 160
        plugin: exec:./plugins/gpu-load/gpu-load
        refresh_ms: 1000
        align: left
        module_config:
          source: gpu
          graph_type: sparkline
          show_vram: true

      # GPU memory controller as primary (useful for VRAM-bound workloads)
      - id: load.gpu-mem
        width: 160
        plugin: exec:./plugins/gpu-load/gpu-load
        refresh_ms: 1000
        align: left
        module_config:
          source: memory
          graph_type: sparkline
```

---

## 10. Implementation Notes

Both plugins were built and are live. Significant issues found during implementation:

### gob `[]interface{}` registration

The RPC layer uses `encoding/gob`. A `source` config value that is a YAML array
arrives as `[]interface{}` inside the `map[string]interface{}` config map. gob
requires explicit registration of all concrete types stored in interfaces.
`[]interface{}` was not registered, causing the plugin subprocess to crash when
receiving an array source config. Fixed by adding `gob.Register([]interface{}{})`
to `pkg/plugin/plugin.go`.

### Renderer multi-line path

The renderer's `drawContent` only entered the split/multi-line layout when
`hasGraph && secondary != ""`. Multi-source payloads have an empty `Secondary`
(labels are embedded in `Primary` lines). Fixed by also entering the multi-line
path when `isMulti` is true, regardless of secondary or graph presence.

### P/E-core detection fallback

`cpu_capacity` is uniform (all 1024) on the i9-12900K under this kernel even
though P-cores and E-cores are physically different. Added a fallback that uses
`thread_siblings_list` — P-cores have a range (`"0-1"`), E-cores have a single
number (`"16"`). Works without any kernel version requirement.

### config_json vs zone_module_config split

`POST /api/layout/zones` wrote `config` to `zones.config_json` but the sampler
reads from `zone_module_config` — a separate table. Fixed by syncing `config_json`
to `zone_module_config` in `CreateZone`, `UpdateZone`, and `ImportLayout`.

### Layout reload not restarting sampler

`triggerLayoutReload` → `ReloadFromConfig` rebuilt the in-memory layout but did
not fire the `onPageChange` callback that restarts the sampler. New zones were
visible to the renderer but not sampled until a manual page switch. Fixed by
calling `onPageChange` for the current page at the end of `ReloadFromConfig`.

---

## 11. Future Work

- **Intel GPU load (fdinfo)**: i915 and xe drivers expose per-engine busyness via
  `/proc/*/fdinfo` (kernel 5.19+). Requires iterating all DRM-holding processes
  and summing nanosecond-delta counters — similar to nvtop. Currently shows
  `"— Intel GPU"`. Implement as a follow-up.
- **Per-core sparklines**: display individual core bars. Needs wider zone and a
  multi-bar rendering mode. Deferred.
- **GPU power draw**: nvidia-smi provides `power.draw`; expose as a future
  `show_power: true` config option.
- **AMD Windows / macOS**: Not in scope for v1.
