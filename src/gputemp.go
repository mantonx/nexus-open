package main

import (
	"fmt"
	"os/exec"
	"strconv"
	"strings"
)

// GetGPUTemperature returns the current GPU temperature in Celsius
// Returns temperature as float64 and error if any
func GetGPUTemp() (float64, error) {
	// Linux path
	if data, err := exec.Command("nvidia-smi", "--query-gpu=temperature.gpu", "--format=csv,noheader,nounits").Output(); err == nil {
		temp, err := strconv.ParseFloat(strings.TrimSpace(string(data)), 64)
		if err == nil {
			return temp, nil
		}
	}

	// Windows path using nvidia-smi (requires exec)
	cmd := exec.Command("nvidia-smi", "--query-gpu=temperature.gpu", "--format=csv,noheader,nounits")
	if out, err := cmd.Output(); err == nil {
		temp, err := strconv.ParseFloat(strings.TrimSpace(string(out)), 64)
		if err == nil {
			return temp, nil
		}
	}

	// macOS path using nvidia-smi
	cmd = exec.Command("nvidia-smi", "--query-gpu=temperature.gpu", "--format=csv,noheader,nounits")
	if out, err := cmd.Output(); err == nil {
		temp, err := strconv.ParseFloat(strings.TrimSpace(string(out)), 64)
		if err == nil {
			return temp, nil
		}
	}

	return 0, fmt.Errorf("unable to get GPU temperature")
}
