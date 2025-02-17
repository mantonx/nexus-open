package main

import (
	"fmt"
	"time"

	"github.com/shirou/gopsutil/net"
)

func GetNetworkUsage() (sent, received int, err error) {
	initial, err := net.IOCounters(false)

	if err != nil {
		return 0, 0, err
	}

	time.Sleep(time.Second)

	final, err := net.IOCounters(false)

	if err != nil {
		return 0, 0, err
	}

	if len(initial) == 0 || len(final) == 0 {
		return 0, 0, fmt.Errorf("no network interfaces found")
	}

	sent = int(calculateMbps(int(final[0].BytesSent-initial[0].BytesSent), time.Second))
	received = int(calculateMbps(int(final[0].BytesRecv-initial[0].BytesRecv), time.Second))

	return sent, received, nil
}

func calculateMbps(bytes int, duration time.Duration) float64 {
	bits := float64(bytes) * 8

	seconds := duration.Seconds()

	kbps := bits / (seconds * 1000)

	return kbps
}
