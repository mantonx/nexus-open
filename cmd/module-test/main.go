// module-test is a utility to test external modules
package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"time"

	"nexus-open/internal/plugin"
)

func main() {
	flag.Parse()

	if flag.NArg() < 1 {
		fmt.Fprintf(os.Stderr, "Usage: module-test <module-path>\n")
		fmt.Fprintf(os.Stderr, "Example: module-test ./modules/gpu-temp/gpu-temp\n")
		os.Exit(1)
	}

	modulePath := flag.Arg(0)

	// Create logger
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{
		Level: slog.LevelInfo,
	}))

	// Create plugin host
	host := plugin.NewHost(logger)
	defer host.KillAll()

	ctx := context.Background()

	// Launch module
	logger.Info("launching module", "path", modulePath)
	mod, err := host.LaunchModule(ctx, "test", modulePath)
	if err != nil {
		logger.Error("failed to launch module", "error", err)
		os.Exit(1)
	}

	// Get description
	desc, err := mod.Describe()
	if err != nil {
		logger.Error("failed to get description", "error", err)
		os.Exit(1)
	}

	fmt.Printf("\n=== Module Description ===\n")
	fmt.Printf("Name:        %s\n", desc.Name)
	fmt.Printf("Version:     %s\n", desc.Version)
	fmt.Printf("Author:      %s\n", desc.Author)
	fmt.Printf("Description: %s\n", desc.Description)
	fmt.Printf("Icon:        %s\n", desc.Icon)
	fmt.Printf("Refresh:     %dms\n", desc.RefreshMs)
	fmt.Printf("\n")

	// Sample 3 times
	for i := 1; i <= 3; i++ {
		fmt.Printf("=== Sample %d ===\n", i)

		payload, err := mod.Sample()
		if err != nil {
			logger.Error("failed to sample", "error", err)
			os.Exit(1)
		}

		fmt.Printf("Primary:   %s\n", payload.Primary)
		fmt.Printf("Secondary: %s\n", payload.Secondary)
		fmt.Printf("Severity:  %s\n", payload.Severity)
		fmt.Printf("Sparkline: %d points\n", len(payload.Spark))
		if len(payload.Spark) > 0 {
			fmt.Printf("  Latest:  %.2f\n", payload.Spark[len(payload.Spark)-1])
		}
		fmt.Printf("Icon:      %s\n", payload.Icon)
		fmt.Printf("Timestamp: %s\n", payload.Timestamp.Format(time.RFC3339))
		fmt.Printf("\n")

		if i < 3 {
			time.Sleep(2 * time.Second)
		}
	}

	fmt.Printf("✓ Module test complete!\n")
}
