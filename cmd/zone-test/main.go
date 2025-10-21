// zone-test demonstrates the complete zone system with live device rendering
// Includes multi-page navigation, touch input, and live module updates
package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"syscall"
	"time"

	"nexus-open/internal/device"
	"nexus-open/internal/touch"
	"nexus-open/internal/zone"
	"nexus-open/pkg/module"
)

func main() {
	configPath := flag.String("config", "configs/layouts/multi-page.yaml", "Path to layout config")
	debug := flag.Bool("debug", false, "Enable debug logging")
	fps := flag.Int("fps", 30, "Target frames per second")
	flag.Parse()

	// Set up logging
	logLevel := slog.LevelInfo
	if *debug {
		logLevel = slog.LevelDebug
	}
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{
		Level: logLevel,
	}))
	slog.SetDefault(logger)

	logger.Info("starting zone system live test",
		"config", *configPath,
		"fps", *fps)

	// Create context with cancellation
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	// Create zone manager
	zoneManager, err := zone.NewManager(ctx, logger, *configPath)
	if err != nil {
		logger.Error("failed to create zone manager", "error", err)
		os.Exit(1)
	}
	logger.Info("zone manager initialized",
		"pages", len(zoneManager.GetConfig().Pages),
		"current_page", zoneManager.GetConfig().Pages[0].Name)

	// Create HID device (full feature support including touch!)
	deviceConfig := device.ConnectionConfig{
		VendorID:         0x1b1c, // Corsair
		ProductID:        0x1b8e, // iCUE Nexus
		ReconnectRetries: 3,
		ReconnectDelay:   2 * time.Second,
	}
	dev := device.NewNexusDevice(logger, deviceConfig)

	// Connect to device
	if err := dev.Connect(ctx); err != nil {
		logger.Error("failed to connect to device", "error", err)
		logger.Info("continuing in offline mode (no device rendering)")
	} else {
		logger.Info("device connected successfully")
	}

	// Create touch handler
	touchHandler := touch.NewHandler(logger, dev, zoneManager)
	if err := touchHandler.Start(ctx); err != nil {
		logger.Error("failed to start touch handler", "error", err)
	} else {
		logger.Info("touch handler started")
	}

	// Start module sampling (mock data for now)
	go mockModuleUpdates(ctx, logger, zoneManager)

	// Main render loop
	frameDuration := time.Second / time.Duration(*fps)
	ticker := time.NewTicker(frameDuration)
	defer ticker.Stop()

	frameCount := 0
	lastPrint := time.Now()

	logger.Info("starting render loop", "target_fps", *fps)

	for {
		select {
		case <-ctx.Done():
			logger.Info("shutting down")
			if err := dev.Disconnect(); err != nil {
				logger.Error("error disconnecting device", "error", err)
			}
			return

		case <-ticker.C:
			// Render frame
			frame, err := zoneManager.RenderFrame()
			if err != nil {
				logger.Error("failed to render frame", "error", err)
				continue
			}

			// Send to device if connected
			if dev.IsConnected() {
				// Device expects RGBA (4 bytes per pixel, 640x48)
				if err := dev.SendFrame(ctx, frame.Pix); err != nil {
					logger.Warn("failed to send frame", "error", err)
				}
			}

			frameCount++

			// Print stats every second
			if time.Since(lastPrint) >= time.Second {
				logger.Info("render stats",
					"fps", frameCount,
					"page", zoneManager.GetCurrentPage(),
					"page_name", zoneManager.GetConfig().Pages[zoneManager.GetCurrentPage()].Name,
					"zones", len(zoneManager.GetZones()))
				frameCount = 0
				lastPrint = time.Now()
			}
		}
	}
}

// mockModuleUpdates simulates module data updates
func mockModuleUpdates(ctx context.Context, logger *slog.Logger, zm *zone.Manager) {
	ticker := time.NewTicker(2 * time.Second)
	defer ticker.Stop()

	logger.Info("starting mock module updates")

	for {
		select {
		case <-ctx.Done():
			return
		case <-ticker.C:
			// Update zones with simulated data
			zones := zm.GetZones()

			for zoneID := range zones {
				var payload module.Payload

				switch zoneID {
				case "weather":
					payload = module.Payload{
						Primary:   fmt.Sprintf("%d°F", 65+time.Now().Second()%20),
						Secondary: "Albany ☀️",
						Severity:  module.SeverityOK,
						TTL:       5 * time.Minute,
						Timestamp: time.Now(),
					}

				case "cpu", "cpu-main":
					temp := 40 + time.Now().Second()%30
					payload = module.Payload{
						Primary:   fmt.Sprintf("%d°C", temp),
						Secondary: "CPU Temp",
						Severity:  getSeverity(float64(temp), 75, 90),
						Spark:     generateSparkline(8),
						TTL:       2 * time.Second,
						Timestamp: time.Now(),
					}

				case "gpu", "gpu-main":
					temp := 50 + time.Now().Second()%40
					payload = module.Payload{
						Primary:   fmt.Sprintf("%d°C", temp),
						Secondary: "GPU Temp",
						Severity:  getSeverity(float64(temp), 75, 90),
						Spark:     generateSparkline(8),
						TTL:       2 * time.Second,
						Timestamp: time.Now(),
					}

				case "network", "network-main":
					payload = module.Payload{
						Primary:   fmt.Sprintf("↓%d MB/s", 10+time.Now().Second()%50),
						Secondary: "Network",
						Severity:  module.SeverityOK,
						Spark:     generateSparkline(8),
						TTL:       2 * time.Second,
						Timestamp: time.Now(),
					}

				case "clock":
					payload = module.Payload{
						Primary:   time.Now().Format("15:04"),
						Secondary: time.Now().Format("Jan 02"),
						Severity:  module.SeverityOK,
						TTL:       1 * time.Second,
						Timestamp: time.Now(),
					}

				default:
					payload = module.Payload{
						Primary:   "Demo",
						Secondary: zoneID,
						Severity:  module.SeverityOK,
						TTL:       2 * time.Second,
						Timestamp: time.Now(),
					}
				}

				if err := zm.UpdatePayload(zoneID, payload); err != nil {
					logger.Debug("failed to update zone", "zone_id", zoneID, "error", err)
				}
			}
		}
	}
}

// getSeverity returns severity based on temperature thresholds
func getSeverity(temp, warnThreshold, critThreshold float64) module.Severity {
	if temp >= critThreshold {
		return module.SeverityCrit
	}
	if temp >= warnThreshold {
		return module.SeverityWarn
	}
	return module.SeverityOK
}

// generateSparkline generates random sparkline data
func generateSparkline(length int) []float32 {
	spark := make([]float32, length)
	base := float32(time.Now().Second()%60) / 100.0
	for i := 0; i < length; i++ {
		spark[i] = base + float32(i)*0.05
		if spark[i] > 1.0 {
			spark[i] = 1.0
		}
	}
	return spark
}
