// media is a plugin that shows the currently playing track via MPRIS/playerctl.
package main

import (
	"fmt"
	"os/exec"
	"strconv"
	"strings"
	"sync"
	"time"

	goplugin "github.com/hashicorp/go-plugin"

	"github.com/mantonx/nexus-open/pkg/plugin"
)

// TrackInfo holds parsed metadata for the current track.
type TrackInfo struct {
	Title    string
	Artist   string
	Album    string
	ArtURL   string // mpris:artUrl — may be https:// or file://
	Status   string // "Playing", "Paused", "Stopped"
	Position int64  // microseconds
	Length   int64  // microseconds
	Player   string
}

// MediaPlugin monitors the currently playing media via playerctl/MPRIS.
type MediaPlugin struct {
	mu         sync.Mutex
	lastTrack  *TrackInfo
	lastSample time.Time
	cacheTTL   time.Duration
}

func NewMediaPlugin() *MediaPlugin {
	return &MediaPlugin{
		cacheTTL: 2 * time.Second,
	}
}

func (m *MediaPlugin) Describe() (plugin.Descriptor, error) {
	return plugin.Descriptor{
		Name:        "Media",
		Version:     "1.0.0",
		Author:      "Nexus Team",
		Description: "Shows currently playing track via MPRIS/playerctl",
		Icon:        "music",
		RefreshMs:   3000,
		Schema:      plugin.ConfigSchema{},
	}, nil
}

// fetchTrack queries all MPRIS players and returns the actively playing one.
// If no player is Playing, falls back to the first Paused player so the zone
// doesn't go blank just because you hit pause.
func fetchTrack() (*TrackInfo, error) {
	format := "{{playerName}}\t{{title}}\t{{artist}}\t{{mpris:length}}\t{{position}}\t{{status}}\t{{mpris:artUrl}}\t{{album}}"
	out, err := exec.Command("playerctl", "metadata", "--format", format, "--all-players").Output()
	if err != nil {
		return nil, fmt.Errorf("playerctl: %w", err)
	}

	var playing, paused *TrackInfo
	for _, line := range strings.Split(strings.TrimSpace(string(out)), "\n") {
		line = strings.TrimSpace(line)
		if line == "" {
			continue
		}
		parts := strings.SplitN(line, "\t", 8)
		if len(parts) < 6 {
			continue
		}
		length, _ := strconv.ParseInt(parts[3], 10, 64)
		position, _ := strconv.ParseInt(parts[4], 10, 64)
		var artURL, album string
		if len(parts) >= 7 {
			artURL = strings.TrimSpace(parts[6])
		}
		if len(parts) == 8 {
			album = strings.TrimSpace(parts[7])
		}
		t := &TrackInfo{
			Player:   parts[0],
			Title:    parts[1],
			Artist:   parts[2],
			Album:    album,
			Length:   length,
			Position: position,
			Status:   parts[5],
			ArtURL:   artURL,
		}
		if t.Status == "Playing" && playing == nil {
			playing = t
		} else if t.Status == "Paused" && paused == nil {
			paused = t
		}
	}

	if playing != nil {
		return playing, nil
	}
	if paused != nil {
		return paused, nil
	}
	return nil, fmt.Errorf("no active player found")
}

// formatPayload converts a TrackInfo into a compact zone payload.
func formatPayload(t *TrackInfo) plugin.Payload {
	icon := "music"
	if t.Status == "Paused" {
		icon = "pause"
	}

	var progress float32
	if t.Length > 0 {
		progress = float32(t.Position) / float32(t.Length)
		if progress > 1 {
			progress = 1
		}
	}

	return plugin.Payload{
		Primary:    t.Title,
		Secondary:  t.Artist,
		Severity:   plugin.SeverityOK,
		Icon:       icon,
		Progress:   progress,
		Expandable: true,
		TTL:        3 * time.Second,
		Timestamp:  time.Now(),
	}
}

func (m *MediaPlugin) Sample() (plugin.Payload, error) {
	m.mu.Lock()
	if m.lastTrack != nil && time.Since(m.lastSample) < m.cacheTTL {
		t := m.lastTrack
		m.mu.Unlock()
		return formatPayload(t), nil
	}
	m.mu.Unlock()

	track, err := fetchTrack()
	if err != nil {
		return plugin.Payload{
			Primary:   "—",
			Secondary: "Nothing playing",
			Severity:  plugin.SeverityOK,
			Icon:      "music",
			TTL:       5 * time.Second,
			Timestamp: time.Now(),
		}, nil
	}

	m.mu.Lock()
	m.lastTrack = track
	m.lastSample = time.Now()
	m.mu.Unlock()

	return formatPayload(track), nil
}

func (m *MediaPlugin) Configure(cfg map[string]any) error {
	return nil
}

func main() {
	goplugin.Serve(&goplugin.ServeConfig{
		HandshakeConfig: plugin.Handshake,
		Plugins: goplugin.PluginSet{
			"plugin": &plugin.ExecPlugin{Impl: NewMediaPlugin()},
		},
	})
}
