package touch

import (
	"github.com/mantonx/nexus-next/internal/zone"
)

// ZoneTapDetector determines which zone was tapped based on touch coordinates.
type ZoneTapDetector struct {
	zones map[string]*zone.Zone
}

// NewZoneTapDetector creates a new zone tap detector.
func NewZoneTapDetector(zones map[string]*zone.Zone) *ZoneTapDetector {
	return &ZoneTapDetector{
		zones: zones,
	}
}

// DetectZone determines which zone was tapped based on X coordinate.
// Returns the zone ID and true if found, or empty string and false if not found.
func (d *ZoneTapDetector) DetectZone(x int) (string, bool) {
	for zoneID, z := range d.zones {
		// Check if X coordinate is within zone bounds
		if x >= z.Config.X && x < z.Config.X+z.Config.Width {
			return zoneID, true
		}
	}

	return "", false
}

// UpdateZones updates the zone map (called when pages change).
func (d *ZoneTapDetector) UpdateZones(zones map[string]*zone.Zone) {
	d.zones = zones
}
