package main

import (
	"bytes"
	"context"
	"encoding/base64"
	"fmt"
	"time"

	"nexus-open/nexus/configuration"
)

type Config struct {
	Location        string   `json:"location"`
	TimeFormat      string   `json:"time_format"`
	Unit            string   `json:"unit"`
	BackgroundColor string   `json:"background_color"`
	TextColor       string   `json:"text_color"`
	ImagePaths      []string `json:"image_paths"`
}

type ImageInfo struct {
	OriginalName string `json:"originalName"`
	StoredName   string `json:"storedName"`
}

// App struct
type App struct {
	ctx    context.Context
	config *configuration.NexusConfig
}

// NewApp creates a new App application struct
func NewApp() *App {
	return &App{}
}

// startup is called when the app starts. The context is saved
// so we can call the runtime methods
func (a *App) startup(ctx context.Context) {
	a.ctx = ctx
	// // Load initial configuration
	config, err := configuration.LoadConfig("")
	if err != nil {
		fmt.Println("Error loading config:", err)
		return
	}
	fmt.Println("Config loaded:", config)
	a.config = config
}

// GetConfig returns the current configuration
func (a *App) GetConfig() Config {
	return Config{
		Location:        a.config.Location,
		TimeFormat:      a.config.TimeFormat,
		Unit:            a.config.Unit,
		BackgroundColor: a.config.BackgroundColor,
		TextColor:       a.config.TextColor,
		ImagePaths:      a.config.ImagePaths,
	}
}

// UpdateConfig updates the application configuration and saves it to disk
func (a *App) UpdateConfig(newConfig Config) error {
	a.config.Location = newConfig.Location
	a.config.TimeFormat = newConfig.TimeFormat
	a.config.Unit = newConfig.Unit
	a.config.BackgroundColor = newConfig.BackgroundColor
	a.config.TextColor = newConfig.TextColor
	a.config.ImagePaths = newConfig.ImagePaths

	return configuration.SaveConfig(a.config, "")
}

// UploadImage handles file uploads from the frontend
func (a *App) UploadImage(originalName string, data []byte) (*ImageInfo, error) {
	// Force unique filename by adding timestamp
	timestamp := time.Now().UnixNano()
	storedName := configuration.GenerateUniqueFileName(fmt.Sprintf("%d-%s", timestamp, originalName))
	r := bytes.NewReader(data)

	// Save and resize the image
	err := configuration.SaveImage(storedName, r)
	if err != nil {
		return nil, fmt.Errorf("failed to save image: %w", err)
	}

	// Update config with new image path
	a.config.ImagePaths = append(a.config.ImagePaths, storedName)
	if err := configuration.SaveConfig(a.config, ""); err != nil {
		return nil, fmt.Errorf("failed to update config: %w", err)
	}

	return &ImageInfo{
		OriginalName: originalName,
		StoredName:   storedName,
	}, nil
}

// DeleteImage removes an image and updates the config
func (a *App) DeleteImage(filename string) error {
	// Delete the file
	if err := configuration.DeleteImage(filename); err != nil {
		return err
	}

	// Update config to remove the path
	a.config.ImagePaths = removeFromSlice(a.config.ImagePaths, filename)
	return configuration.SaveConfig(a.config, "")
}

func removeFromSlice(slice []string, item string) []string {
	result := make([]string, 0, len(slice))
	for _, s := range slice {
		if s != item {
			result = append(result, s)
		}
	}
	return result
}

// Helper function to check if a string is in a slice
func contains(slice []string, item string) bool {
	for _, s := range slice {
		if s == item {
			return true
		}
	}
	return false
}

// GetImagePreview returns a base64 encoded image for preview
func (a *App) GetImagePreview(filename string) (string, error) {
	data, err := configuration.ReadImage(filename)
	if err != nil {
		return "", err
	}

	return base64.StdEncoding.EncodeToString(data), nil
}
