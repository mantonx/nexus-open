package config

// loadYAMLConfig reads a legacy config.yaml using viper and returns a Config.
// This is the only place viper is used — purely for the one-time import path.

import (
	"fmt"

	"github.com/spf13/viper"
)

func loadYAMLConfig(path string) (*Config, error) {
	v := viper.New()
	v.SetConfigFile(path)
	v.SetConfigType("yaml")

	if err := v.ReadInConfig(); err != nil {
		return nil, err
	}

	cfg := &Config{
		BackgroundColor: v.GetString("background_color"),
		BackgroundImage: v.GetString("background_image"),
		TextColor:       v.GetString("text_color"),
		Display: DisplayConfig{
			FontFamily:   v.GetString("display.font_family"),
			FontSize:     v.GetFloat64("display.font_size"),
			TimeFontSize: v.GetFloat64("display.time_font_size"),
			Layout:       v.GetString("display.layout"),
			DateFormat:   v.GetString("display.date_format"),
		},
	}

	// Apply defaults for any missing values.
	if cfg.BackgroundColor == "" {
		cfg.BackgroundColor = DefaultBackgroundColor
	}
	if cfg.TextColor == "" {
		cfg.TextColor = DefaultTextColor
	}
	if cfg.BackgroundImage == "" {
		cfg.BackgroundImage = DefaultBackgroundImage
	}
	if cfg.Display.FontFamily == "" {
		cfg.Display.FontFamily = DefaultFontFamily
	}
	if cfg.Display.FontSize == 0 {
		cfg.Display.FontSize = DefaultFontSize
	}
	if cfg.Display.TimeFontSize == 0 {
		cfg.Display.TimeFontSize = DefaultTimeFontSize
	}
	if cfg.Display.Layout == "" {
		cfg.Display.Layout = DefaultLayout
	}
	if cfg.Display.DateFormat == "" {
		cfg.Display.DateFormat = DefaultDateFormat
	}

	if !isValidHexColor(cfg.BackgroundColor) {
		return nil, fmt.Errorf("legacy config has invalid background_color: %s", cfg.BackgroundColor)
	}
	if !isValidHexColor(cfg.TextColor) {
		return nil, fmt.Errorf("legacy config has invalid text_color: %s", cfg.TextColor)
	}

	return cfg, nil
}
