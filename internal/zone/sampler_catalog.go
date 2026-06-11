package zone

import (
	"os"
	"path/filepath"
	"strings"

	"github.com/mantonx/nexus-open/internal/plugins/builtin"
	pluginhost "github.com/mantonx/nexus-open/internal/plugins/host"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

// CatalogEntry is one entry in the plugin catalog returned by GetCatalog.
type CatalogEntry struct {
	ID         string            `json:"id"`   // e.g. "builtin:clock" or "exec:cpu-temp"
	Kind       string            `json:"kind"` // "builtin" or "exec"
	Descriptor plugin.Descriptor `json:"descriptor"`
}

// GetCatalog returns metadata for all available plugins: running builtins (from
// the loaded modules map) plus every executable found in pluginsDir.
// Exec plugins not currently loaded are launched briefly to retrieve their Descriptor.
func (s *Sampler) GetCatalog() []CatalogEntry {
	seen := make(map[string]bool)
	var entries []CatalogEntry

	// Collect builtins from loaded modules.
	s.mu.RLock()
	for zoneID, mod := range s.modules {
		spec := s.pluginSpec[zoneID]
		if !strings.HasPrefix(spec, "builtin:") || seen[spec] {
			continue
		}
		seen[spec] = true
		if desc, err := mod.Describe(); err == nil {
			entries = append(entries, CatalogEntry{ID: spec, Kind: "builtin", Descriptor: desc})
		}
	}
	s.mu.RUnlock()

	// Always include static builtins even if no zone is using them right now.
	for _, id := range []string{"builtin:clock", "builtin:placeholder", "builtin:debug"} {
		if seen[id] {
			continue
		}
		seen[id] = true
		var mod plugin.Plugin
		switch id {
		case "builtin:clock":
			mod = builtin.NewClock()
		case "builtin:placeholder":
			mod = builtin.NewPlaceholder("")
		case "builtin:debug":
			mod = builtin.NewDebug("catalog", 640)
		}
		if desc, err := mod.Describe(); err == nil {
			entries = append(entries, CatalogEntry{ID: id, Kind: "builtin", Descriptor: desc})
		}
	}

	// Scan pluginsDir for exec plugin binaries.
	if s.pluginsDir != "" {
		dirEntries, err := os.ReadDir(s.pluginsDir)
		if err == nil {
			for _, de := range dirEntries {
				if !de.IsDir() {
					continue
				}
				name := de.Name()
				binPath := filepath.Join(s.pluginsDir, name, name)
				id := "exec:" + name
				if seen[id] {
					continue
				}

				desc, err := pluginhost.DescribePlugin(binPath)
				if err != nil {
					s.logger.Debug("catalog: describe failed", "plugin", name, "error", err)
					continue
				}
				seen[id] = true
				entries = append(entries, CatalogEntry{ID: id, Kind: "exec", Descriptor: desc})
			}
		}
	}

	return entries
}
