// Package main provides configuration management functionality for the nexus-open application.
// It handles reading and writing configuration files, as well as managing application directories.
package configuration

import (
	"os"
	"path/filepath"

	"github.com/spf13/viper"
)

const (
	// defaultConfigPath is the relative path to the configuration file
	defaultConfigPath = "nexus-open/config.yaml"
	// defaultImagesPath is the relative path to the images directory
	defaultImagesPath = "nexus-open/images"

	// Configuration defaults and valid values
	TimeFormat12Hour = "12h"
	TimeFormat24Hour = "24h"
	UnitMetric       = "metric"
	UnitImperial     = "imperial"
)

// NexusConfig holds the application configuration
type NexusConfig struct {
	// Location represents the user's city
	Location string `mapstructure:"location"`

	// TimeFormat can be either "12h" or "24h"
	TimeFormat string `mapstructure:"time_format"`

	// Unit represents the temperature unit (metric/imperial)
	Unit string `mapstructure:"unit"`

	// BackgroundColor is a hex color string (e.g., "#FFFFFF")
	BackgroundColor string `mapstructure:"background_color"`

	// TextColor is a hex color string (e.g., "#000000")
	TextColor string `mapstructure:"text_color"`

	// ImagePaths contains the list of image filenames
	ImagePaths []string `mapstructure:"image_paths"`
}

// GetImagesDir returns the absolute path to the application's images directory.
// It ensures the directory exists, creating it if necessary.
func GetImagesDir() (string, error) {
	configDir, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	imagesPath := filepath.Join(configDir, defaultImagesPath)
	return imagesPath, os.MkdirAll(imagesPath, 0755)
}

// createDefaultConfig creates a new configuration file with default values
func createDefaultConfig(path string) error {
	defaultConfig := &NexusConfig{
		TimeFormat:      TimeFormat24Hour,
		Unit:            UnitMetric,
		BackgroundColor: "#FFFFFF",
		TextColor:       "#000000",
		ImagePaths:      []string{},
	}

	// Ensure the directory exists
	if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
		return err
	}

	return SaveConfig(defaultConfig, path)
}

// LoadConfig reads configuration from a YAML file or environment variables.
// If path is empty, it uses the default configuration location.
// The function also ensures the images directory exists during initial setup.
func LoadConfig(path string) (*NexusConfig, error) {
	if path == "" {
		configDir, err := os.UserConfigDir()
		if err != nil {
			return nil, err
		}
		path = filepath.Join(configDir, defaultConfigPath)
	}

	// Create default config if file doesn't exist
	if _, err := os.Stat(path); os.IsNotExist(err) {
		if err := createDefaultConfig(path); err != nil {
			return nil, err
		}
	}

	// Ensure images directory exists
	if _, err := GetImagesDir(); err != nil {
		return nil, err
	}

	viper.SetConfigFile(path)
	viper.SetConfigType("yaml")
	viper.AutomaticEnv()

	viper.SetDefault("time_format", TimeFormat24Hour)
	viper.SetDefault("unit", UnitMetric)
	viper.SetDefault("background_color", "#FFFFFF")
	viper.SetDefault("text_color", "#000000")
	viper.SetDefault("image_paths", []string{})

	if err := viper.ReadInConfig(); err != nil {
		return nil, err
	}

	var config NexusConfig
	if err := viper.Unmarshal(&config); err != nil {
		return nil, err
	}

	return &config, nil
}

// SaveConfig writes the current configuration to a YAML file.
// If path is empty, it uses the default configuration location
// and ensures the directory structure exists.
func SaveConfig(config *NexusConfig, path string) error {
	if path == "" {
		configDir, err := os.UserConfigDir()
		if err != nil {
			return err
		}
		path = filepath.Join(configDir, defaultConfigPath)

		if err := os.MkdirAll(filepath.Dir(path), 0755); err != nil {
			return err
		}
	}

	viper.SetConfigFile(path)
	viper.SetConfigType("yaml")

	for key, value := range map[string]interface{}{
		"location":         config.Location,
		"time_format":      config.TimeFormat,
		"unit":             config.Unit,
		"background_color": config.BackgroundColor,
		"text_color":       config.TextColor,
		"image_paths":      config.ImagePaths,
	} {
		viper.Set(key, value)
	}

	return viper.WriteConfig()
}
