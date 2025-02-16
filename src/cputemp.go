package main

import (
	"fmt"
	"os"
	"os/exec"
	"strconv"
	"strings"
)

func GetCPUTemp() (float64, error) {
	// Linux path
	if data, err := os.ReadFile("/sys/class/thermal/thermal_zone0/temp"); err == nil {
		temp, err := strconv.ParseFloat(strings.TrimSpace(string(data)), 64)
		if err == nil {
			return temp / 1000.0, nil
		}
	}

	// Windows path using wmic (requires exec)
	cmd := exec.Command("wmic", "/namespace:\\\\root\\wmi", "PATH", "MSAcpi_ThermalZoneTemperature", "GET", "CurrentTemperature", "/value")
	if out, err := cmd.Output(); err == nil {
		temp := strings.TrimSpace(strings.Split(string(out), "=")[1])
		if t, err := strconv.ParseFloat(temp, 64); err == nil {
			return (t - 273.15), nil
		}
	}

	// macOS path using sysctl
	cmd = exec.Command("sysctl", "-n", "machdep.xcpm.cpu_thermal_level")
	if out, err := cmd.Output(); err == nil {
		if t, err := strconv.ParseFloat(strings.TrimSpace(string(out)), 64); err == nil {
			return float64(t), nil
		}
	}

	return 0, fmt.Errorf("unable to get CPU temperature")
}
