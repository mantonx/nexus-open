package instruments

import (
	"fmt"
	"os"
	"os/exec"
	"runtime"
	"strconv"
	"strings"
)

// GetCPUTemp returns the current CPU temperature in Celsius degrees and any error encountered.
// For Linux: Reads from /sys/class/thermal/thermal_zone0/temp (requires root privileges)
// For Windows: Uses WMIC to query MSAcpi_ThermalZoneTemperature
// For macOS: Uses sysctl to query machdep.xcpm.cpu_thermal_level
// Returns an error if the operating system is not supported or if unable to read/parse the temperature.
func GetCPUTemp() (float64, error) {
	switch runtime.GOOS {
	case "linux":
		return getLinuxTemp()
	case "windows":
		return getWindowsTemp()
	case "darwin":
		return getMacTemp()
	default:
		return 0, fmt.Errorf("unsupported operating system: %s", runtime.GOOS)
	}
}

func getLinuxTemp() (float64, error) {
	data, err := os.ReadFile("/sys/class/thermal/thermal_zone0/temp")
	if err != nil {
		return 0, fmt.Errorf("failed to read temperature: %v", err)
	}

	temp, err := strconv.ParseFloat(strings.TrimSpace(string(data)), 64)

	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %v", err)
	}

	return temp / 1000.0, nil
}

func getWindowsTemp() (float64, error) {
	cmd := exec.Command("wmic", "/namespace:\\\\root\\wmi", "PATH",
		"MSAcpi_ThermalZoneTemperature", "GET", "CurrentTemperature", "/value")
	out, err := cmd.Output()
	if err != nil {
		return 0, fmt.Errorf("failed to get temperature: %v", err)
	}

	parts := strings.Split(string(out), "=")
	if len(parts) != 2 {
		return 0, fmt.Errorf("invalid output format")
	}

	temp, err := strconv.ParseFloat(strings.TrimSpace(parts[1]), 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %v", err)
	}
	return (temp - 273.15), nil
}

func getMacTemp() (float64, error) {
	cmd := exec.Command("sysctl", "-n", "machdep.xcpm.cpu_thermal_level")
	out, err := cmd.Output()
	if err != nil {
		return 0, fmt.Errorf("failed to get temperature: %v", err)
	}

	temp, err := strconv.ParseFloat(strings.TrimSpace(string(out)), 64)
	if err != nil {
		return 0, fmt.Errorf("failed to parse temperature: %v", err)
	}
	return temp, nil
}
