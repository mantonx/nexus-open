package instruments

import (
	"context"
	"encoding/json"
	"fmt"
	"log/slog"
	"os/exec"
	"strconv"
	"strings"
	"sync"
	"time"
)

// GPUTemp monitors GPU temperature
type GPUTemp struct {
	logger   *slog.Logger
	interval time.Duration
	dataChan chan float64
	mu       sync.RWMutex
	current  float64
	ctx      context.Context
	cancel   context.CancelFunc
	wg       sync.WaitGroup
}

// NewGPUTemp creates a new GPU temperature instrument
func NewGPUTemp(logger *slog.Logger, interval time.Duration) *GPUTemp {
	if interval == 0 {
		interval = 5 * time.Second
	}
	return &GPUTemp{
		logger:   logger,
		interval: interval,
		dataChan: make(chan float64, 1),
	}
}

func (g *GPUTemp) Name() string {
	return "gpu_temperature"
}

func (g *GPUTemp) UpdateInterval() time.Duration {
	return g.interval
}

func (g *GPUTemp) Start(ctx context.Context) error {
	g.mu.Lock()
	if g.cancel != nil {
		g.mu.Unlock()
		return fmt.Errorf("instrument already started")
	}
	g.ctx, g.cancel = context.WithCancel(ctx)
	g.mu.Unlock()

	g.wg.Add(1)
	go g.run()

	g.logger.Debug("GPU temperature monitor started", "interval", g.interval)
	return nil
}

func (g *GPUTemp) Stop() error {
	g.mu.Lock()
	if g.cancel == nil {
		g.mu.Unlock()
		return nil
	}
	g.cancel()
	g.mu.Unlock()

	g.wg.Wait()
	g.logger.Debug("GPU temperature monitor stopped")
	return nil
}

// GetCurrent returns the most recent temperature reading
func (g *GPUTemp) GetCurrent() float64 {
	g.mu.RLock()
	defer g.mu.RUnlock()
	return g.current
}

func (g *GPUTemp) run() {
	defer g.wg.Done()

	ticker := time.NewTicker(g.interval)
	defer ticker.Stop()

	// Initial read
	g.readAndUpdate()

	for {
		select {
		case <-g.ctx.Done():
			return
		case <-ticker.C:
			g.readAndUpdate()
		}
	}
}

func (g *GPUTemp) readAndUpdate() {
	temp, err := g.readTemperature()
	if err != nil {
		g.logger.Warn("failed to read GPU temperature", "error", err)
		return
	}

	g.mu.Lock()
	g.current = temp
	g.mu.Unlock()

	select {
	case g.dataChan <- temp:
	default:
	}
}

func (g *GPUTemp) readTemperature() (float64, error) {
	// Try different GPU vendors in order
	for _, tryFunc := range []func() (float64, error){
		g.tryNVIDIA,
		g.tryAMD,
		g.tryIntel,
	} {
		if temp, err := tryFunc(); err == nil {
			return temp, nil
		}
	}
	return 0, fmt.Errorf("no supported GPU found")
}

func (g *GPUTemp) tryNVIDIA() (float64, error) {
	cmd := exec.CommandContext(g.ctx, "nvidia-smi",
		"--query-gpu=temperature.gpu",
		"--format=csv,noheader,nounits")
	out, err := cmd.Output()
	if err != nil {
		return 0, fmt.Errorf("nvidia-smi failed: %w", err)
	}

	temp, err := strconv.ParseFloat(strings.TrimSpace(string(out)), 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse nvidia-smi output: %w", err)
	}

	return temp, nil
}

func (g *GPUTemp) tryAMD() (float64, error) {
	return g.getTemperatureFromSensors("amdgpu")
}

func (g *GPUTemp) tryIntel() (float64, error) {
	return g.getTemperatureFromSensors("i915")
}

func (g *GPUTemp) getTemperatureFromSensors(chipName string) (float64, error) {
	cmd := exec.CommandContext(g.ctx, "sensors", "-j")
	data, err := cmd.Output()
	if err != nil {
		return 0, fmt.Errorf("sensors command failed: %w", err)
	}

	var sensors map[string]interface{}
	if err := json.Unmarshal(data, &sensors); err != nil {
		return 0, fmt.Errorf("failed to parse sensors output: %w", err)
	}

	adapters, ok := sensors["adapters"].([]interface{})
	if !ok {
		return 0, fmt.Errorf("invalid sensors data format")
	}

	for _, adapter := range adapters {
		adapterMap, ok := adapter.(map[string]interface{})
		if !ok {
			continue
		}

		if adapterStr, ok := adapterMap["adapter"].(string); ok && strings.Contains(adapterStr, chipName) {
			if temp, ok := adapterMap["temp1_input"].(float64); ok {
				return temp, nil
			}
		}
	}

	return 0, fmt.Errorf("no %s GPU temperature found in sensors output", chipName)
}
