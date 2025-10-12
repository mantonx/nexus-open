package instruments

import (
	"context"
	"fmt"
	"log/slog"
	"sync"
	"time"

	"github.com/shirou/gopsutil/net"
)

// Network monitors network usage statistics
type Network struct {
	logger   *slog.Logger
	interval time.Duration
	dataChan chan NetworkData
	mu       sync.RWMutex
	current  NetworkData
	ctx      context.Context
	cancel   context.CancelFunc
	wg       sync.WaitGroup
}

// NewNetwork creates a new network usage instrument
func NewNetwork(logger *slog.Logger, interval time.Duration) *Network {
	if interval == 0 {
		interval = 1 * time.Second
	}
	return &Network{
		logger:   logger,
		interval: interval,
		dataChan: make(chan NetworkData, 1),
	}
}

func (n *Network) Name() string {
	return "network"
}

func (n *Network) UpdateInterval() time.Duration {
	return n.interval
}

func (n *Network) Start(ctx context.Context) error {
	n.mu.Lock()
	if n.cancel != nil {
		n.mu.Unlock()
		return fmt.Errorf("instrument already started")
	}
	n.ctx, n.cancel = context.WithCancel(ctx)
	n.mu.Unlock()

	n.wg.Add(1)
	go n.run()

	n.logger.Debug("network monitor started", "interval", n.interval)
	return nil
}

func (n *Network) Stop() error {
	n.mu.Lock()
	if n.cancel == nil {
		n.mu.Unlock()
		return nil
	}
	n.cancel()
	n.mu.Unlock()

	n.wg.Wait()
	n.logger.Debug("network monitor stopped")
	return nil
}

// GetCurrent returns the most recent network statistics
func (n *Network) GetCurrent() NetworkData {
	n.mu.RLock()
	defer n.mu.RUnlock()
	return n.current
}

func (n *Network) run() {
	defer n.wg.Done()

	ticker := time.NewTicker(n.interval)
	defer ticker.Stop()

	// Initial read
	n.readAndUpdate()

	for {
		select {
		case <-n.ctx.Done():
			return
		case <-ticker.C:
			n.readAndUpdate()
		}
	}
}

func (n *Network) readAndUpdate() {
	stats, err := n.readNetworkStats()
	if err != nil {
		n.logger.Warn("failed to read network statistics", "error", err)
		return
	}

	n.mu.Lock()
	n.current = stats
	n.mu.Unlock()

	select {
	case n.dataChan <- stats:
	default:
	}
}

func (n *Network) readNetworkStats() (NetworkData, error) {
	// Get initial counters
	initial, err := net.IOCounters(false)
	if err != nil {
		return NetworkData{}, fmt.Errorf("failed to get initial network counters: %w", err)
	}

	if len(initial) == 0 {
		return NetworkData{}, fmt.Errorf("no network interfaces found")
	}

	// Wait for the sample period
	select {
	case <-n.ctx.Done():
		return NetworkData{}, n.ctx.Err()
	case <-time.After(n.interval):
	}

	// Get final counters
	final, err := net.IOCounters(false)
	if err != nil {
		return NetworkData{}, fmt.Errorf("failed to get final network counters: %w", err)
	}

	if len(final) == 0 {
		return NetworkData{}, fmt.Errorf("no network interfaces found")
	}

	// Calculate rates
	bytesSent := final[0].BytesSent - initial[0].BytesSent
	bytesRecv := final[0].BytesRecv - initial[0].BytesRecv

	// Convert to bytes per second
	uploadSpeed := float64(bytesSent) / n.interval.Seconds()
	downloadSpeed := float64(bytesRecv) / n.interval.Seconds()

	return NetworkData{
		DownloadSpeed: downloadSpeed,
		UploadSpeed:   uploadSpeed,
		TotalDown:     final[0].BytesRecv,
		TotalUp:       final[0].BytesSent,
	}, nil
}
