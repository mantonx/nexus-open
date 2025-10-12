package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	"nexus-open/internal/app"
)

var (
	version = "1.0.0"
	commit  = "dev"
)

func main() {
	// Parse command-line flags
	var (
		showVersion = flag.Bool("version", false, "Show version information")
		debug       = flag.Bool("debug", false, "Enable debug logging")
		configPath  = flag.String("config", "", "Path to configuration file")
		apiPort     = flag.Int("port", 1985, "API server port")
	)
	flag.Parse()

	// Show version and exit
	if *showVersion {
		fmt.Printf("Nexus Open v%s (commit: %s)\n", version, commit)
		os.Exit(0)
	}

	// Set up logging
	logLevel := slog.LevelInfo
	if *debug {
		logLevel = slog.LevelDebug
	}
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{
		Level: logLevel,
	}))
	slog.SetDefault(logger)

	logger.Info("starting Nexus Open",
		"version", version,
		"commit", commit,
		"port", *apiPort,
	)

	// Create application instance
	application, err := app.New(
		app.WithLogger(logger),
		app.WithConfigPath(*configPath),
		app.WithAPIPort(*apiPort),
	)
	if err != nil {
		logger.Error("failed to create application", "error", err)
		os.Exit(1)
	}

	// Set up graceful shutdown
	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	// Run application in goroutine
	errCh := make(chan error, 1)
	go func() {
		if err := application.Run(ctx); err != nil {
			errCh <- err
		}
	}()

	// Wait for shutdown signal or error
	select {
	case <-ctx.Done():
		logger.Info("received shutdown signal")
	case err := <-errCh:
		logger.Error("application error", "error", err)
	}

	// Graceful shutdown
	logger.Info("shutting down...")
	if err := application.Shutdown(); err != nil {
		logger.Error("shutdown error", "error", err)
		os.Exit(1)
	}

	logger.Info("shutdown complete")
}
