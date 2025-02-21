package instruments

import (
	"encoding/json"
	"fmt"
	"os/exec"
	"strconv"
	"strings"
)

// GetGPUTemperature returns the current GPU temperature in Celsius
// Returns temperature as float64 and error if any
func GetGPUTemp() (float64, error) {
	// Try different GPU vendors in order
	for _, tryFunc := range []func() (float64, error){tryNVIDIA, tryAMD, tryIntel} {
		if temp, err := tryFunc(); err == nil {
			return temp, nil
		}
	}
	return 0, fmt.Errorf("no GPU found")
}

func tryNVIDIA() (float64, error) {
	out, err := exec.Command("nvidia-smi", "--query-gpu=temperature.gpu", "--format=csv,noheader,nounits").Output()
	if err == nil {
		if temp, err := strconv.ParseFloat(strings.TrimSpace(string(out)), 64); err == nil {
			return temp, nil
		}
	}
	return 0, fmt.Errorf("unable to get NVIDIA GPU temperature")
}

func tryAMD() (float64, error) {
	return getTemperatureFromSensors("amdgpu")
}

func tryIntel() (float64, error) {
	return getTemperatureFromSensors("i915")
}

func getTemperatureFromSensors(chipName string) (float64, error) {
	data, err := exec.Command("sensors", "-j").Output()
	if err != nil {
		return 0, fmt.Errorf("unable to get %s GPU temperature", chipName)
	}

	var sensors map[string]interface{}
	if err := json.Unmarshal(data, &sensors); err != nil {
		return 0, fmt.Errorf("failed to parse sensors output")
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

	return 0, fmt.Errorf("no %s GPU temperature found", chipName)
}
