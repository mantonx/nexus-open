package instruments

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
	"sync"
	"time"
)

// CPUTemp monitors CPU temperature
type CPUTemp struct {
	logger   *slog.Logger
	interval time.Duration
	dataChan chan float64
	mu       sync.RWMutex
	current  float64
	ctx      context.Context
	cancel   context.CancelFunc
	wg       sync.WaitGroup
}

// NewCPUTemp creates a new CPU temperature instrument
func NewCPUTemp(logger *slog.Logger, interval time.Duration) *CPUTemp {
	if interval == 0 {
		interval = 5 * time.Second
	}
	return &CPUTemp{
		logger:   logger,
		interval: interval,
		dataChan: make(chan float64, 1),
	}
}

func (c *CPUTemp) Name() string {
	return "cpu_temperature"
}

func (c *CPUTemp) UpdateInterval() time.Duration {
	return c.interval
}

func (c *CPUTemp) Start(ctx context.Context) error {
	c.mu.Lock()
	if c.cancel != nil {
		c.mu.Unlock()
		return fmt.Errorf("instrument already started")
	}
	c.ctx, c.cancel = context.WithCancel(ctx)
	c.mu.Unlock()

	c.wg.Add(1)
	go c.run()

	c.logger.Debug("CPU temperature monitor started", "interval", c.interval)
	return nil
}

func (c *CPUTemp) Stop() error {
	c.mu.Lock()
	if c.cancel == nil {
		c.mu.Unlock()
		return nil
	}
	c.cancel()
	c.mu.Unlock()

	c.wg.Wait()
	c.logger.Debug("CPU temperature monitor stopped")
	return nil
}

// GetCurrent returns the most recent temperature reading
func (c *CPUTemp) GetCurrent() float64 {
	c.mu.RLock()
	defer c.mu.RUnlock()
	return c.current
}

// run is the main monitoring loop
func (c *CPUTemp) run() {
	defer c.wg.Done()

	ticker := time.NewTicker(c.interval)
	defer ticker.Stop()

	// Initial read
	c.readAndUpdate()

	for {
		select {
		case <-c.ctx.Done():
			return
		case <-ticker.C:
			c.readAndUpdate()
		}
	}
}

func (c *CPUTemp) readAndUpdate() {
	temp, err := c.readTemperature()
	if err != nil {
		c.logger.Warn("failed to read CPU temperature", "error", err)
		return
	}

	c.mu.Lock()
	c.current = temp
	c.mu.Unlock()

	// Non-blocking send
	select {
	case c.dataChan <- temp:
	default:
	}
}

// readTemperature reads CPU temperature based on OS
func (c *CPUTemp) readTemperature() (float64, error) {
	switch runtime.GOOS {
	case "linux":
		return c.readLinuxTemp()
	case "windows":
		return c.readWindowsTemp()
	case "darwin":
		return c.readMacTemp()
	default:
		return 0, fmt.Errorf("unsupported operating system: %s", runtime.GOOS)
	}
}

func (c *CPUTemp) readLinuxTemp() (float64, error) {
	data, err := os.ReadFile("/sys/class/thermal/thermal_zone0/temp")
	if err != nil {
		return 0, fmt.Errorf("failed to read temperature file: %w", err)
	}

	temp, err := strconv.ParseFloat(strings.TrimSpace(string(data)), 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %w", err)
	}

	return temp / 1000.0, nil
}

func (c *CPUTemp) readWindowsTemp() (float64, error) {
	cmd := exec.CommandContext(c.ctx, "wmic", "/namespace:\\\\root\\wmi", "PATH",
		"MSAcpi_ThermalZoneTemperature", "GET", "CurrentTemperature", "/value")
	out, err := cmd.Output()
	if err != nil {
		return 0, fmt.Errorf("failed to execute wmic: %w", err)
	}

	parts := strings.Split(string(out), "=")
	if len(parts) != 2 {
		return 0, fmt.Errorf("invalid wmic output format")
	}

	temp, err := strconv.ParseFloat(strings.TrimSpace(parts[1]), 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %w", err)
	}

	// Convert from Kelvin to Celsius
	return temp - 273.15, nil
}

func (c *CPUTemp) readMacTemp() (float64, error) {
	cmd := exec.CommandContext(c.ctx, "sysctl", "-n", "machdep.xcpm.cpu_thermal_level")
	out, err := cmd.Output()
	if err != nil {
		return 0, fmt.Errorf("failed to execute sysctl: %w", err)
	}

	temp, err := strconv.ParseFloat(strings.TrimSpace(string(out)), 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %w", err)
	}

	return temp, nil
}
