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

	// Create zone manager
	manager, err := zone.NewManager(ctx, logger, *configPath)
	if err != nil {
		logger.Error("failed to create zone manager", "error", err)
		os.Exit(1)
	}

	logger.Info("zone manager created successfully")

	// Update zones with mock data
	mockPayloads := map[string]module.Payload{
		"weather": {
			Primary:   "72°F",
			Secondary: "Albany ☀️",
			Severity:  module.SeverityOK,
			TTL:       5 * time.Minute,
			Timestamp: time.Now(),
		},
		"cpu": {
			Primary:   "42°C",
			Secondary: "Load 31%",
			Severity:  module.SeverityOK,
			Spark:     []float32{0.2, 0.3, 0.35, 0.4, 0.45, 0.3, 0.25, 0.31},
			TTL:       2 * time.Second,
			Timestamp: time.Now(),
		},
		"gpu": {
			Primary:   "68°C",
			Secondary: "Load 87%",
			Severity:  module.SeverityWarn,
			Spark:     []float32{0.7, 0.75, 0.8, 0.85, 0.87, 0.9, 0.85, 0.87},
			TTL:       2 * time.Second,
			Timestamp: time.Now(),
		},
		"clock": {
			Primary:   time.Now().Format("15:04"),
			Secondary: time.Now().Format("Jan 02"),
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
			Primary:   "↓58 MB/s",
			Secondary: "↑12 MB/s",
			Severity:  module.SeverityOK,
			Spark:     []float32{0.4, 0.5, 0.6, 0.7, 0.8, 0.75, 0.6, 0.58},
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
