// Package main provides configuration management functionality for the nexus-open application.
// It handles reading and writing configuration files, as well as managing application directories.
package configuration

import (
	"fmt"
	"os"
	"path/filepath"
	"sync"

	"github.com/spf13/viper"
)

const (
	// defaultConfigPath is the relative path to the configuration file
	defaultConfigPath = "nexus-open/config.yaml"
	// defaultImagesPath is the relative path to the images directory
	defaultImagesPath = "nexus-open/images"

	// Configuration defaults and valid values
	Location         = "Jersey City, NJ"
	TimeFormat12Hour = "12h"
	TimeFormat24Hour = "24h"
	UnitMetric       = "metric"
	UnitImperial     = "imperial"
	TextColor        = "#FFFFFF"
	BackgroundColor  = "#000000"
	BackgroundImage  = "background.png"
)

// NexusConfig holds the application configuration
type NexusConfig struct {
	// Location represents the user's city
	Location string `mapstructure:"location"`

	// TimeFormat can be either "12h" or "24h"
	TimeFormat string `mapstructure:"time_format"`

	// Unit represents the temperature unit (metric/imperial)
	Unit string `mapstructure:"unit"`

	// BackgroundColor is a hex color string (e.g., "#000000")
	BackgroundColor string `mapstructure:"background_color"`

	// BackgroundImage is the filename of the background image
	BackgroundImage string `mapstructure:"background_image"`

	// TextColor is a hex color string (e.g., "#FFFFFF")
	TextColor string `mapstructure:"text_color"`

	// ImagePaths contains the list of image filenames
	ImagePaths []string `mapstructure:"image_paths"`
}

// Configuration state
var (
	config   *NexusConfig
	configMu sync.RWMutex
	unit     string // Current unit setting
)

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
		Location:        Location,
		TimeFormat:      TimeFormat12Hour,
		Unit:            UnitImperial,
		BackgroundColor: BackgroundColor,
		BackgroundImage: BackgroundImage,
		TextColor:       TextColor,
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

	viper.SetDefault("location", Location)
	viper.SetDefault("time_format", TimeFormat24Hour)
	viper.SetDefault("unit", UnitMetric)
	viper.SetDefault("background_color", BackgroundColor)
	viper.SetDefault("background_image", BackgroundImage)
	viper.SetDefault("text_color", TextColor)
	viper.SetDefault("image_paths", []string{})

	if err := viper.ReadInConfig(); err != nil {
		return nil, err
	}

	var config NexusConfig

	if err := viper.Unmarshal(&config); err != nil {
		return nil, err
	}

	fmt.Printf("Loaded configuration from %s\n", path)

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
		"background_image": config.BackgroundImage,
		"text_color":       config.TextColor,
		"image_paths":      config.ImagePaths,
	} {
		viper.Set(key, value)
	}

	return viper.WriteConfig()
}
