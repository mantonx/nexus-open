package display

import (
	"context"
	"fmt"
	"image"
	"image/color"
	"image/draw"
	"log/slog"
	"sync"
	"time"

	"golang.org/x/image/font"
	"golang.org/x/image/font/basicfont"
	"golang.org/x/image/math/fixed"

	"nexus-open/internal/config"
	"nexus-open/internal/device"
	"nexus-open/internal/fonts"
	"nexus-open/internal/instruments"
)

// NexusRenderer implements the display rendering for the Nexus device
type NexusRenderer struct {
	logger         *slog.Logger
	cfg            *config.Manager
	device         device.Device
	fontManager    *fonts.Manager
	iconSet        *IconSet
	iconRenderer   *IconRenderer
	layoutManager  *LayoutManager
	layoutRenderer *LayoutRenderer

	// Display properties
	width  int
	height int

	// Rendering state
	mu              sync.RWMutex
	background      *image.RGBA
	textColor       color.Color
	backgroundColor color.Color
	font            font.Face // Normal text font
	timeFont        font.Face // Larger font for time
	loadedFontName  string    // Name of loaded font

	// Animation state
	blinkState bool
}

// NewNexusRenderer creates a new display renderer
func NewNexusRenderer(logger *slog.Logger, cfg *config.Manager, dev device.Device) *NexusRenderer {
	iconSet := NewIconSet()
	layoutManager := NewLayoutManager()

	return &NexusRenderer{
		logger:          logger,
		cfg:             cfg,
		device:          dev,
		fontManager:     fonts.NewManager(logger),
		iconSet:         iconSet,
		layoutManager:   layoutManager,
		width:           Width,
		height:          Height,
		textColor:       color.White,
		backgroundColor: color.Black,
		font:            basicfont.Face7x13, // Fallback bitmap font
		timeFont:        basicfont.Face7x13, // Will be replaced with TrueType
	}
}

// Initialize prepares the renderer
func (r *NexusRenderer) Initialize() error {
	// Load configuration
	cfg := r.cfg.Get()

	// Parse colors
	if textColor, err := parseColor(cfg.TextColor); err == nil {
		r.textColor = textColor
	} else {
		r.logger.Warn("invalid text color, using white", "color", cfg.TextColor, "error", err)
	}

	if bgColor, err := parseColor(cfg.BackgroundColor); err == nil {
		r.backgroundColor = bgColor
	} else {
		r.logger.Warn("invalid background color, using black", "color", cfg.BackgroundColor, "error", err)
	}

	// Load fonts
	if err := r.loadFonts(cfg.Display); err != nil {
		r.logger.Warn("failed to load TrueType fonts, using fallback", "error", err)
		// Keep fallback bitmap fonts
	}

	// Load Font Awesome for icons at two sizes
	iconFont, err := r.fontManager.GetFace("FontAwesome-Solid", 10.0)
	weatherIconFont, weatherErr := r.fontManager.GetFace("FontAwesome-Solid", 14.0) // Larger for weather

	if err != nil {
		r.logger.Warn("failed to load Font Awesome, using text font for icons", "error", err)
		iconFont = r.font // Fallback to regular font
	} else {
		r.logger.Info("Font Awesome loaded for system icons", "size", 10.0)
	}

	if weatherErr != nil {
		r.logger.Warn("failed to load Font Awesome for weather, using text font", "error", weatherErr)
		weatherIconFont = r.timeFont // Fallback to larger text font
	} else {
		r.logger.Info("Font Awesome loaded for weather icons", "size", 14.0)
	}

	// Initialize icon renderer with Font Awesome at both sizes
	r.iconRenderer = NewIconRenderer(r.iconSet, iconFont, weatherIconFont, r.textColor)

	// Initialize layout renderer
	r.layoutRenderer = NewLayoutRenderer(r)

	// Set layout from config
	if cfg.Display.Layout != "" {
		if !r.layoutManager.SetLayout(LayoutType(cfg.Display.Layout)) {
			r.logger.Warn("invalid layout in config, using default", "layout", cfg.Display.Layout)
		}
	}

	// TODO: Load background images

	r.logger.Info("renderer initialized",
		"font", r.loadedFontName,
		"layout", r.layoutManager.GetLayout().Name())
	return nil
}

// loadFonts loads TrueType fonts based on configuration
func (r *NexusRenderer) loadFonts(displayCfg config.DisplayConfig) error {
	// Use configured font name or try best available
	var normalFont, timeFont font.Face
	var fontName string
	var err error

	if displayCfg.FontFamily != "" {
		// Try to load specific font
		normalFont, err = r.fontManager.GetFace(displayCfg.FontFamily, displayCfg.FontSize)
		if err == nil {
			timeFont, err = r.fontManager.GetFace(displayCfg.FontFamily, displayCfg.TimeFontSize)
			if err == nil {
				fontName = displayCfg.FontFamily
			}
		}
	}

	// Fallback to best available font
	if normalFont == nil || timeFont == nil {
		normalFont, fontName, err = r.fontManager.LoadBestAvailableFont(displayCfg.FontSize)
		if err != nil {
			return fmt.Errorf("no fonts available: %w", err)
		}
		timeFont, _, err = r.fontManager.LoadBestAvailableFont(displayCfg.TimeFontSize)
		if err != nil {
			timeFont = normalFont // Use same font for time if loading fails
		}
	}

	// Update renderer state
	r.mu.Lock()
	r.font = normalFont
	r.timeFont = timeFont
	r.loadedFontName = fontName
	r.mu.Unlock()

	r.logger.Info("fonts loaded",
		"font", fontName,
		"normalSize", displayCfg.FontSize,
		"timeSize", displayCfg.TimeFontSize)

	return nil
}

// Render creates a frame with the given system data
func (r *NexusRenderer) Render(ctx context.Context, data instruments.SystemData) (*image.RGBA, error) {
	r.mu.RLock()
	defer r.mu.RUnlock()

	// Create canvas
	canvas := image.NewRGBA(image.Rect(0, 0, r.width, r.height))

	// Draw background
	draw.Draw(canvas, canvas.Bounds(), &image.Uniform{r.backgroundColor}, image.Point{}, draw.Src)

	// Get config
	cfg := r.cfg.Get()

	// Prepare layout data
	layoutData := LayoutData{
		Time:        r.formatTime(cfg.TimeFormat),
		TimeFormat:  cfg.TimeFormat,
		CPUTemp:     data.Temperature.CPU,
		GPUTemp:     data.Temperature.GPU,
		NetworkDown: data.Network.DownloadSpeed,
		NetworkUp:   data.Network.UploadSpeed,
		Weather:     data.Weather,
		Unit:        cfg.Unit,
	}

	// Render using current layout
	currentLayout := r.layoutManager.GetLayout()
	currentLayout.Render(canvas, layoutData, r.layoutRenderer)

	return canvas, nil
}

// formatTime formats the current time based on config
func (r *NexusRenderer) formatTime(timeFormat string) string {
	now := time.Now()

	if timeFormat == "12h" {
		hour := now.Hour() % 12
		if hour == 0 {
			hour = 12
		}
		ampm := "AM"
		if now.Hour() >= 12 {
			ampm = "PM"
		}

		// Blinking colon effect
		colon := ":"
		if r.blinkState {
			colon = " "
		}
		return fmt.Sprintf("%d%s%02d %s", hour, colon, now.Minute(), ampm)
	}

	// 24h format
	colon := ":"
	if r.blinkState {
		colon = " "
	}
	return fmt.Sprintf("%02d%s%02d", now.Hour(), colon, now.Minute())
}

// drawTime renders the current time
func (r *NexusRenderer) drawTime(canvas *image.RGBA, timeFormat string) {
	now := time.Now()

	var timeStr string
	if timeFormat == "12h" {
		hour := now.Hour() % 12
		if hour == 0 {
			hour = 12
		}
		ampm := "AM"
		if now.Hour() >= 12 {
			ampm = "PM"
		}

		// Blinking colon effect
		colon := ":"
		if r.blinkState {
			colon = " "
		}
		timeStr = fmt.Sprintf("%d%s%02d %s", hour, colon, now.Minute(), ampm)
	} else {
		colon := ":"
		if r.blinkState {
			colon = " "
		}
		timeStr = fmt.Sprintf("%02d%s%02d", now.Hour(), colon, now.Minute())
	}

	// Draw time in center-top area with larger font
	r.drawTextWithFont(canvas, timeStr, 240, 20, r.timeFont)
}

// drawTemperatures renders CPU and GPU temperatures
func (r *NexusRenderer) drawTemperatures(canvas *image.RGBA, temps instruments.TemperatureData) {
	if temps.CPU > 0 {
		// Draw CPU icon
		r.iconRenderer.DrawIcon(canvas, r.iconSet.SystemCPU, 10, 15)
		// Draw temperature text
		cpuText := fmt.Sprintf("%.0f°C", temps.CPU)
		r.drawText(canvas, cpuText, 25, 15)
	}

	if temps.GPU > 0 {
		// Draw GPU icon
		r.iconRenderer.DrawIcon(canvas, r.iconSet.SystemGPU, 10, 35)
		// Draw temperature text
		gpuText := fmt.Sprintf("%.0f°C", temps.GPU)
		r.drawText(canvas, gpuText, 25, 35)
	}
}

// drawNetwork renders network statistics
func (r *NexusRenderer) drawNetwork(canvas *image.RGBA, net instruments.NetworkData) {
	if net.DownloadSpeed > 0 || net.UploadSpeed > 0 {
		// Convert bytes/sec to more readable units
		downMbps := net.DownloadSpeed / (1024 * 1024)
		upMbps := net.UploadSpeed / (1024 * 1024)

		// Download speed with icon
		r.iconRenderer.DrawIcon(canvas, r.iconSet.SystemNetworkDown, 520, 15)
		if downMbps >= 1.0 {
			netText := fmt.Sprintf("%.1f MB/s", downMbps)
			r.drawText(canvas, netText, 535, 15)
		} else {
			downKbps := net.DownloadSpeed / 1024
			netText := fmt.Sprintf("%.0f KB/s", downKbps)
			r.drawText(canvas, netText, 535, 15)
		}

		// Upload speed with icon
		r.iconRenderer.DrawIcon(canvas, r.iconSet.SystemNetworkUp, 520, 35)
		if upMbps >= 1.0 {
			netText := fmt.Sprintf("%.1f MB/s", upMbps)
			r.drawText(canvas, netText, 535, 35)
		} else {
			upKbps := net.UploadSpeed / 1024
			netText := fmt.Sprintf("%.0f KB/s", upKbps)
			r.drawText(canvas, netText, 535, 35)
		}
	}
}

// drawWeather renders weather information
func (r *NexusRenderer) drawWeather(canvas *image.RGBA, weather *instruments.WeatherData, unit string) {
	if weather == nil {
		return
	}

	// Draw weather icon - using larger font for better visibility
	if weather.Icon != "" {
		r.drawTextWithFont(canvas, weather.Icon, 280, 35, r.timeFont)
	}

	// Draw temperature with appropriate symbol
	tempSymbol := "°C"
	if unit == "imperial" {
		tempSymbol = "°F"
	}

	tempText := fmt.Sprintf("%.0f%s", weather.Temperature, tempSymbol)
	r.drawText(canvas, tempText, 310, 35)
}

// drawText draws text at the specified position using the normal font
func (r *NexusRenderer) drawText(canvas *image.RGBA, text string, x, y int) {
	r.drawTextWithFont(canvas, text, x, y, r.font)
}

// drawTextWithFont draws text at the specified position with a specific font face
func (r *NexusRenderer) drawTextWithFont(canvas *image.RGBA, text string, x, y int, face font.Face) {
	point := fixed.Point26_6{
		X: fixed.Int26_6(x * 64),
		Y: fixed.Int26_6(y * 64),
	}

	drawer := &font.Drawer{
		Dst:  canvas,
		Src:  &image.Uniform{r.textColor},
		Face: face,
		Dot:  point,
	}

	drawer.DrawString(text)
}

// UpdateBlinkState toggles the blink state (for blinking colon in time)
func (r *NexusRenderer) UpdateBlinkState() {
	r.mu.Lock()
	r.blinkState = !r.blinkState
	r.mu.Unlock()
}

// SetTextColor updates the text color
func (r *NexusRenderer) SetTextColor(c color.Color) {
	r.mu.Lock()
	r.textColor = c
	r.mu.Unlock()
}

// SetBackgroundColor updates the background color
func (r *NexusRenderer) SetBackgroundColor(c color.Color) {
	r.mu.Lock()
	r.backgroundColor = c
	r.mu.Unlock()
}

// parseColor parses a hex color string like "#RRGGBB"
func parseColor(hexColor string) (color.Color, error) {
	if len(hexColor) != 7 || hexColor[0] != '#' {
		return nil, fmt.Errorf("invalid color format: %s", hexColor)
	}

	var r, g, b uint8
	_, err := fmt.Sscanf(hexColor, "#%02x%02x%02x", &r, &g, &b)
	if err != nil {
		return nil, fmt.Errorf("failed to parse color: %w", err)
	}

	return color.RGBA{R: r, G: g, B: b, A: 255}, nil
}
