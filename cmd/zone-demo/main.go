// zone-demo demonstrates the zone rendering system
package main

import (
	"context"
	"flag"
	"fmt"
	"image/png"
	"log/slog"
	"os"
	"time"

	"github.com/mantonx/nexus-next/internal/zone"
	"github.com/mantonx/nexus-next/pkg/module"
)

func main() {
	configPath := flag.String("config", "configs/layouts/default.yaml", "Path to layout config")
	outputPath := flag.String("output", "zone-demo.png", "Output PNG file")
	flag.Parse()

	// Create logger
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{
		Level: slog.LevelDebug,
	}))

	ctx := context.Background()

	// Create zone manager (nil DB — YAML-only mode for this dev binary)
	manager, err := zone.NewManager(ctx, logger, nil, *configPath)
	if err != nil {
		logger.Error("failed to create zone manager", "error", err)
		os.Exit(1)
	}

	logger.Info("zone manager created successfully")

	// Update zones with mock data — using 60-sample spark histories to match
	// what real plugins produce after ~2 minutes of sampling at 2s intervals.
	mockPayloads := map[string]module.Payload{
		"weather": {
			Primary:   "92°F",
			Secondary: "WEATHER",
			Severity:  module.SeverityOK,
			Spark:     genSpark(0.45, 0.05, 60),
			TTL:       5 * time.Minute,
			Timestamp: time.Now(),
		},
		"cpu": {
			Primary:   "28°C",
			Secondary: "CPU TEMP",
			Severity:  module.SeverityOK,
			Spark:     genSpark(0.18, 0.04, 60),
			TTL:       2 * time.Second,
			Timestamp: time.Now(),
		},
		"gpu": {
			Primary:   "52°C",
			Secondary: "GPU TEMP",
			Severity:  module.SeverityOK,
			Spark:     genSpark(0.38, 0.06, 60),
			TTL:       2 * time.Second,
			Timestamp: time.Now(),
		},
		"clock": {
			Primary:   time.Now().Format("3:04 PM"),
			Severity:  module.SeverityOK,
			TTL:       1 * time.Second,
			Timestamp: time.Now(),
		},
		"media": {
			Primary:   "Radiohead",
			Secondary: "Karma Police",
			Progress:  0.65,
			Severity:  module.SeverityOK,
			TTL:       1 * time.Second,
			Timestamp: time.Now(),
		},
		"network": {
			Primary:   "↓ 137K/s",
			Secondary: "NETWORK",
			Severity:  module.SeverityOK,
			Spark:     genSpark(0.55, 0.20, 60),
			TTL:       2 * time.Second,
			Timestamp: time.Now(),
		},
	}

	// Update all zones
	for zoneID, payload := range mockPayloads {
		if err := manager.UpdatePayload(zoneID, payload); err != nil {
			logger.Warn("failed to update zone", "zone_id", zoneID, "error", err)
		}
	}

	// Render frame
	frame, err := manager.RenderFrame()
	if err != nil {
		logger.Error("failed to render frame", "error", err)
		os.Exit(1)
	}

	logger.Info("frame rendered successfully", "width", frame.Bounds().Dx(), "height", frame.Bounds().Dy())

	// Save to PNG
	outFile, err := os.Create(*outputPath)
	if err != nil {
		logger.Error("failed to create output file", "error", err)
		os.Exit(1)
	}
	defer outFile.Close()

	if err := png.Encode(outFile, frame); err != nil {
		logger.Error("failed to encode PNG", "error", err)
		os.Exit(1)
	}

	logger.Info("demo complete", "output", *outputPath)
	fmt.Printf("\n✓ Zone rendering demo complete!\n")
	fmt.Printf("  Output: %s (640x48 PNG)\n", *outputPath)
	fmt.Printf("  Zones rendered: %d\n", len(manager.GetZones()))
	fmt.Printf("  Current page: %s\n", manager.GetConfig().Pages[manager.GetCurrentPage()].Name)
	fmt.Printf("\nTry switching pages and rendering again:\n")
	fmt.Printf("  Page 0: %s\n", manager.GetConfig().Pages[0].Name)
	if len(manager.GetConfig().Pages) > 1 {
		fmt.Printf("  Page 1: %s\n", manager.GetConfig().Pages[1].Name)
	}
}

// genSpark generates n spark values around a centre with the given amplitude of noise.
func genSpark(centre, noise float32, n int) []float32 {
	out := make([]float32, n)
	v := centre
	for i := range out {
		// Random walk bounded around centre.
		v += (float32(i%7)-3) * noise / 10
		if v < centre-noise {
			v = centre - noise
		}
		if v > centre+noise {
			v = centre + noise
		}
		out[i] = v
	}
	return out
}
