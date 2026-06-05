// Package main provides the Nexus Open application entry point.
// openapi:meta info title Nexus Open API
// openapi:meta info description start
// REST API for Nexus Open - an open-source iCUE Nexus companion app.
// Provides endpoints for device control, configuration management, and zone/module interactions.
// openapi:meta info description end
// openapi:meta info version 2.0.0
// openapi:meta servers http://localhost:1985
// openapi:meta tag Device --- Device control and status
// openapi:meta tag Config --- Configuration management
// openapi:meta tag Zone --- Zone and module management
// openapi:meta tag Animation --- Animation control
// openapi:meta contact https://github.com/mantonx/nexus-next Nexus Team
package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"path/filepath"
	"strconv"
	"syscall"

	"github.com/mantonx/nexus-next/internal/app"
	"github.com/mantonx/nexus-next/internal/tray"
	"github.com/mantonx/nexus-next/internal/udev"
)

var (
	version = "1.0.0"
	commit  = "dev"
)

func main() {
	// Parse command-line flags
	var (
		showVersion = flag.Bool("version", false, "Show version information")
		setupUdev   = flag.Bool("setup-udev", false, "Install udev rules for device access (requires root)")
		debug       = flag.Bool("debug", false, "Enable debug logging")
		configPath  = flag.String("config", "", "Path to configuration file")
		apiPort     = flag.Int("port", 1985, "API server port")
		enableTray  = flag.Bool("tray", false, "Enable system tray mode with Flutter UI")
	)
	flag.Parse()

	// Show version and exit
	if *showVersion {
		fmt.Printf("Nexus Open v%s (commit: %s)\n", version, commit)
		os.Exit(0)
	}

	// Install udev rules and exit
	if *setupUdev {
		fmt.Println("Installing udev rules for Corsair iCUE Nexus...")
		if err := udev.Setup(); err != nil {
			fmt.Fprintf(os.Stderr, "Error: %v\n", err)
			os.Exit(1)
		}
		os.Exit(0)
	}

	// Write PID file so restart scripts can kill the exact previous instance
	// without accumulating stale processes. Cleaned up automatically on exit.
	if pidFile := pidFilePath(); pidFile != "" {
		if err := writePIDFile(pidFile); err != nil {
			fmt.Fprintf(os.Stderr, "warning: could not write PID file: %v\n", err)
		} else {
			defer os.Remove(pidFile)
		}
	}

	// Set up logging — NEXUS_DEBUG=1 enables debug level without editing config files
	logLevel := slog.LevelInfo
	if *debug || os.Getenv("NEXUS_DEBUG") == "1" {
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

	// If tray mode enabled, start system tray
	if *enableTray {
		logger.Info("starting system tray mode")
		apiAddr := fmt.Sprintf("localhost:%d", *apiPort)
		trayManager := tray.New(logger, apiAddr)

		// Run tray in goroutine
		go func() {
			trayManager.Run()
		}()

		// Wait for tray quit, shutdown signal, or error
		select {
		case <-trayManager.QuitChannel():
			logger.Info("quit from system tray")
			cancel()
		case <-ctx.Done():
			logger.Info("received shutdown signal")
		case err := <-errCh:
			logger.Error("application error", "error", err)
		}
	} else {
		// Normal mode: wait for shutdown signal or error
		select {
		case <-ctx.Done():
			logger.Info("received shutdown signal")
		case err := <-errCh:
			logger.Error("application error", "error", err)
		}
	}

	// Graceful shutdown
	logger.Info("shutting down...")
	if err := application.Shutdown(); err != nil {
		logger.Error("shutdown error", "error", err)
		os.Exit(1)
	}

	logger.Info("shutdown complete")
}

// pidFilePath returns the PID file path under $XDG_RUNTIME_DIR (or /tmp as
// fallback). Returns empty string if no suitable directory is available.
func pidFilePath() string {
	dir := os.Getenv("XDG_RUNTIME_DIR")
	if dir == "" {
		dir = filepath.Join(os.TempDir(), fmt.Sprintf("nexus-open-%d", os.Getuid()))
		if err := os.MkdirAll(dir, 0700); err != nil {
			return ""
		}
	}
	return filepath.Join(dir, "nexus-open.pid")
}

// writePIDFile writes the current process PID to path, killing any previous
// instance recorded there so only one nexus-open holds the device at a time.
func writePIDFile(path string) error {
	// If a previous PID file exists, send SIGTERM to that process first.
	if data, err := os.ReadFile(path); err == nil {
		if prev, err := strconv.Atoi(string(data)); err == nil && prev > 0 {
			if proc, err := os.FindProcess(prev); err == nil {
				proc.Signal(syscall.SIGTERM)
			}
		}
	}
	return os.WriteFile(path, []byte(strconv.Itoa(os.Getpid())), 0644)
}
