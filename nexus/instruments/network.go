package instruments

import (
	"fmt"
	"time"

	"github.com/shirou/gopsutil/net"
)

// GetNetworkUsage retrieves the current network usage statistics for all network interfaces combined.
// It measures network activity over a one-second interval and returns the rate of data transfer.
//
// Returns:
//   - sent: The outbound network traffic in Kbps (kilobits per second)
//   - received: The inbound network traffic in Kbps (kilobits per second)
//   - err: Error if network statistics cannot be retrieved or no interfaces are found
//
// The function uses a sampling method by taking two measurements one second apart
// to calculate the network usage rate. It returns 0 for both sent and received
// if an error occurs during measurement or if no network interfaces are detected.
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

	sent = int(computeKbps(int(final[0].BytesSent-initial[0].BytesSent), time.Second))
	received = int(computeKbps(int(final[0].BytesRecv-initial[0].BytesRecv), time.Second))

	return sent, received, nil
}

// computeKbps calculates the network speed in kilobits per second (Kbps)
// from a given number of bytes transferred over a specific duration.
//
// Parameters:
//   - bytes: The total number of bytes transferred
//   - duration: The time period over which the transfer occurred
//
// Returns:
//
//	float64: The calculated speed in Kbps (kilobits per second)
func computeKbps(bytes int, duration time.Duration) float64 {
	bits := float64(bytes) * 8

	seconds := duration.Seconds()

	kbps := bits / (seconds * 1000)

	return kbps
}
