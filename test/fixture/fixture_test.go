// Package fixture validates plugin payload fixtures and zone renderer golden output.
//
// Fixtures live in two places:
//   - testdata/payloads/         — shared fixtures with no owning plugin
//   - plugins/<name>/testdata/   — fixtures owned by a specific plugin
//
// Run normally:         go test ./test/fixture/...
// Regenerate goldens:   go test ./test/fixture/... -update
package fixture

import (
	"bytes"
	"encoding/json"
	"image/png"
	"log/slog"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/sebdah/goldie/v2"

	"github.com/mantonx/nexus-open/internal/zone"
	"github.com/mantonx/nexus-open/pkg/plugin"
)

// sharedFixturesDir holds fixtures with no owning plugin (renderer shape tests).
const sharedFixturesDir = "../../testdata/payloads"

// pluginsDir is the root of all plugin subdirectories.
const pluginsDir = "../../plugins"

// goldensDir stores reference PNGs.
const goldensDir = "../../testdata/golden"

// zoneWidth and zoneHeight match the hardware display slot dimensions.
const zoneWidth = 100
const zoneHeight = 48

// loadFixtures returns all *.json fixture files from shared and plugin testdata dirs.
func loadFixtures(t *testing.T) []string {
	t.Helper()
	var paths []string

	// Shared fixtures.
	if entries, err := os.ReadDir(sharedFixturesDir); err == nil {
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".json") {
				paths = append(paths, filepath.Join(sharedFixturesDir, e.Name()))
			}
		}
	}

	// Per-plugin fixtures: plugins/<name>/testdata/*.json
	pluginEntries, err := os.ReadDir(pluginsDir)
	if err != nil {
		t.Fatalf("readdir %s: %v", pluginsDir, err)
	}
	for _, pe := range pluginEntries {
		if !pe.IsDir() {
			continue
		}
		tdDir := filepath.Join(pluginsDir, pe.Name(), "testdata")
		entries, err := os.ReadDir(tdDir)
		if err != nil {
			continue // plugin has no testdata dir — skip
		}
		for _, e := range entries {
			if !e.IsDir() && strings.HasSuffix(e.Name(), ".json") {
				paths = append(paths, filepath.Join(tdDir, e.Name()))
			}
		}
	}

	if len(paths) == 0 {
		t.Fatalf("no fixture files found")
	}
	return paths
}

// scenarioName derives a stable test name from a fixture file path.
// "../../testdata/payloads/cpu_temp_ok.json" → "cpu_temp_ok"
func scenarioName(path string) string {
	base := filepath.Base(path)
	return strings.TrimSuffix(base, filepath.Ext(base))
}

// TestFixtureRoundTrip unmarshals every fixture file to plugin.Payload,
// re-marshals it, and diffs the JSON.  Catches field name drift between
// the Go struct and the committed fixture files.
func TestFixtureRoundTrip(t *testing.T) {
	for _, path := range loadFixtures(t) {
		name := scenarioName(path)
		t.Run(name, func(t *testing.T) {
			raw, err := os.ReadFile(path)
			if err != nil {
				t.Fatalf("read: %v", err)
			}

			var p plugin.Payload
			if err := json.Unmarshal(raw, &p); err != nil {
				t.Fatalf("unmarshal: %v", err)
			}

			if err := p.Validate(); err != nil {
				t.Fatalf("validate: %v", err)
			}

			// Re-marshal and compare as normalised JSON maps so field ordering
			// doesn't matter.
			remarshaled, err := json.Marshal(p)
			if err != nil {
				t.Fatalf("re-marshal: %v", err)
			}

			var original, roundtripped map[string]any
			if err := json.Unmarshal(raw, &original); err != nil {
				t.Fatalf("unmarshal original map: %v", err)
			}
			if err := json.Unmarshal(remarshaled, &roundtripped); err != nil {
				t.Fatalf("unmarshal roundtripped map: %v", err)
			}

			// Check all keys present in original are preserved in the round-trip.
			// (Re-marshaling may add omitempty-zero fields; that's acceptable.)
			for k, origVal := range original {
				rtVal, ok := roundtripped[k]
				if !ok {
					t.Errorf("field %q lost in round-trip", k)
					continue
				}
				// Compare via JSON to normalise number types.
				origJ, _ := json.Marshal(origVal)
				rtJ, _ := json.Marshal(rtVal)
				if !bytes.Equal(origJ, rtJ) {
					t.Errorf("field %q changed: %s → %s", k, origJ, rtJ)
				}
			}
		})
	}
}

// TestFixtureGoldenRender renders every fixture through the zone renderer and
// compares the output PNG to a committed golden in testdata/golden/.
//
// On first run (or with -update flag) golden files are created/updated.
// CI fails when a golden is missing or the rendered output differs.
func TestFixtureGoldenRender(t *testing.T) {
	if err := os.MkdirAll(goldensDir, 0755); err != nil {
		t.Fatalf("mkdir golden: %v", err)
	}

	logger := slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: slog.LevelError}))
	theme := zone.DefaultTheme()

	g := goldie.New(t,
		goldie.WithFixtureDir(goldensDir),
		goldie.WithNameSuffix(".png"),
	)

	for _, path := range loadFixtures(t) {
		name := scenarioName(path)
		t.Run(name, func(t *testing.T) {
			raw, err := os.ReadFile(path)
			if err != nil {
				t.Fatalf("read: %v", err)
			}

			var p plugin.Payload
			if err := json.Unmarshal(raw, &p); err != nil {
				t.Fatalf("unmarshal: %v", err)
			}

			r := zone.NewRenderer(logger, theme, zoneWidth, zoneHeight, zone.AlignLeft)
			img, err := r.Render(p)
			if err != nil {
				t.Fatalf("render: %v", err)
			}

			var buf bytes.Buffer
			if err := png.Encode(&buf, img); err != nil {
				t.Fatalf("png encode: %v", err)
			}

			g.Assert(t, name, buf.Bytes())
		})
	}
}
