package instruments

import (
	"time"

	"github.com/shirou/gopsutil/cpu"
)

// GetCPULoad returns the current CPU load percentage across all cores
// averaged over a 1 second interval
func GetCPULoad() (float64, error) {
	// Get CPU percentage with 1 second interval
	percentage, err := cpu.Percent(time.Second, false)
	if err != nil {
		return 0, err
	}

	// Return the total CPU usage (first element is overall percentage)
	if len(percentage) > 0 {
		return percentage[0], nil
	}

	return 0, nil
}
