package display

import (
	"image"

	"nexus-open/internal/instruments"
)

// LayoutType defines different display layout styles
type LayoutType string

const (
	LayoutDashboard  LayoutType = "dashboard"  // Modern dashboard with sections
	LayoutMinimalist LayoutType = "minimalist" // Centered, minimal info
	LayoutCompact    LayoutType = "compact"    // Dense, maximum info
	LayoutCards      LayoutType = "cards"      // Card-based sections
	LayoutBalanced   LayoutType = "balanced"   // Default balanced layout
)

// Layout defines how elements are positioned on the display
type Layout interface {
	// Name returns the layout identifier
	Name() LayoutType

	// Description returns a human-readable description
	Description() string

	// Render renders the layout with given data
	Render(canvas *image.RGBA, data LayoutData, renderer *LayoutRenderer)
}

// LayoutData holds all the data needed for rendering
type LayoutData struct {
	Time        string
	TimeFormat  string
	CPUTemp     float64
	GPUTemp     float64
	NetworkDown float64
	NetworkUp   float64
	Weather     *instruments.WeatherData
	Unit        string
}

// Position represents an X,Y coordinate
type Position struct {
	X int
	Y int
}

// Region represents a rectangular area
type Region struct {
	X      int
	Y      int
	Width  int
	Height int
}

// LayoutRenderer provides helper methods for rendering layout elements
type LayoutRenderer struct {
	renderer     *NexusRenderer
	iconRenderer *IconRenderer
	iconSet      *IconSet
}

// NewLayoutRenderer creates a new layout renderer
func NewLayoutRenderer(renderer *NexusRenderer) *LayoutRenderer {
	return &LayoutRenderer{
		renderer:     renderer,
		iconRenderer: renderer.iconRenderer,
		iconSet:      renderer.iconSet,
	}
}

// DrawSeparator draws a horizontal line separator
func (lr *LayoutRenderer) DrawSeparator(canvas *image.RGBA, y int, width int) {
	for x := 10; x < width-10; x++ {
		canvas.Set(x, y, lr.renderer.textColor)
	}
}

// DrawVerticalSeparator draws a vertical line separator
func (lr *LayoutRenderer) DrawVerticalSeparator(canvas *image.RGBA, x int, yStart, yEnd int) {
	for y := yStart; y < yEnd; y++ {
		canvas.Set(x, y, lr.renderer.textColor)
	}
}

// CenterText calculates X position to center text
func (lr *LayoutRenderer) CenterText(text string, canvasWidth int) int {
	// Approximate: 7 pixels per character for 11pt font
	textWidth := len(text) * 7
	return (canvasWidth - textWidth) / 2
}

// ========================
// Dashboard Layout
// ========================

type DashboardLayout struct{}

func (l *DashboardLayout) Name() LayoutType {
	return LayoutDashboard
}

func (l *DashboardLayout) Description() string {
	return "Modern dashboard with clear sections"
}

func (l *DashboardLayout) Render(canvas *image.RGBA, data LayoutData, lr *LayoutRenderer) {
	// Top section: Time (large, centered) + Weather (right)
	// ┌────────────────────────────────────────┐
	// │      2:30 PM              ☀ 59°F      │
	// ├────────────────────────────────────────┤
	// │  ■ 45°  ▣ 62°        ↓ 5.2  ↑ 1.1    │
	// └────────────────────────────────────────┘

	// Top section height: 0-22px
	// Separator at 23px
	// Bottom section: 24-48px

	// Draw time (large, left-aligned with system stats below)
	lr.renderer.drawTextWithFont(canvas, data.Time, 15, 18, lr.renderer.timeFont)

	// Draw weather on right side
	if data.Weather != nil {
		// Weather icon (using Font Awesome at 14pt for better size)
		if data.Weather.WeatherCode > 0 {
			weatherIcon := lr.iconSet.GetWeatherIcon(data.Weather.WeatherCode)
			// Position weather icon - baseline at y=18
			weatherIconY := 18 - 2
			lr.iconRenderer.DrawWeatherIcon(canvas, weatherIcon, 492, weatherIconY)
		}
		// Temperature with more spacing from icon
		tempSymbol := "°C"
		if data.Unit == "imperial" {
			tempSymbol = "°F"
		}
		weatherTemp := formatTemp(data.Weather.Temperature, tempSymbol)
		lr.renderer.drawText(canvas, weatherTemp, 520, 18) // More spacing from icon
	}

	// Draw horizontal separator
	lr.DrawSeparator(canvas, 23, Width)

	// Bottom section: System stats
	// Text baseline is at y=38
	// Temperature unit symbol
	tempUnit := "°C"
	if data.Unit == "imperial" {
		tempUnit = "°F"
	}

	textY := 38
	iconY := textY - 1 // Raise icons by 1px to align with text baseline

	// CPU temp (left)
	if data.CPUTemp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemCPU, 15, iconY)
		lr.renderer.drawText(canvas, formatTemp(data.CPUTemp, tempUnit), 30, textY)
	}

	// GPU temp (left-center)
	if data.GPUTemp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemGPU, 90, iconY)
		lr.renderer.drawText(canvas, formatTemp(data.GPUTemp, tempUnit), 105, textY)
	}

	// Network stats (right side)
	if data.NetworkDown > 0 || data.NetworkUp > 0 {
		// Download
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemNetworkDown, 480, iconY)
		lr.renderer.drawText(canvas, formatSpeed(data.NetworkDown), 495, textY)

		// Upload
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemNetworkUp, 560, iconY)
		lr.renderer.drawText(canvas, formatSpeed(data.NetworkUp), 575, textY)
	}
}

// ========================
// Minimalist Layout
// ========================

type MinimalistLayout struct{}

func (l *MinimalistLayout) Name() LayoutType {
	return LayoutMinimalist
}

func (l *MinimalistLayout) Description() string {
	return "Centered, minimal information"
}

func (l *MinimalistLayout) Render(canvas *image.RGBA, data LayoutData, lr *LayoutRenderer) {
	//              2:30 PM
	//
	//        ■ 45°  ▣ 62°  ☀ 59°
	//
	//             ↓ 5.2  ↑ 1.1

	// Time centered at top
	timeX := lr.CenterText(data.Time, Width)
	lr.renderer.drawTextWithFont(canvas, data.Time, timeX, 15, lr.renderer.timeFont)

	// Middle row: temps and weather (centered)
	centerY := 30

	// CPU
	if data.CPUTemp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemCPU, 220, centerY)
		lr.renderer.drawText(canvas, formatTemp(data.CPUTemp, "°"), 235, centerY)
	}

	// GPU
	if data.GPUTemp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemGPU, 290, centerY)
		lr.renderer.drawText(canvas, formatTemp(data.GPUTemp, "°"), 305, centerY)
	}

	// Weather
	if data.Weather != nil && data.Weather.Icon != "" {
		lr.renderer.drawText(canvas, data.Weather.Icon, 360, centerY)
		tempSymbol := "°"
		if data.Unit == "imperial" {
			tempSymbol = "°F"
		} else {
			tempSymbol = "°C"
		}
		lr.renderer.drawText(canvas, formatTemp(data.Weather.Temperature, tempSymbol), 380, centerY)
	}

	// Bottom row: network (centered)
	if data.NetworkDown > 0 || data.NetworkUp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemNetworkDown, 280, 45)
		lr.renderer.drawText(canvas, formatSpeed(data.NetworkDown), 295, 45)

		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemNetworkUp, 350, 45)
		lr.renderer.drawText(canvas, formatSpeed(data.NetworkUp), 365, 45)
	}
}

// ========================
// Compact Layout
// ========================

type CompactLayout struct{}

func (l *CompactLayout) Name() LayoutType {
	return LayoutCompact
}

func (l *CompactLayout) Description() string {
	return "Maximum information density"
}

func (l *CompactLayout) Render(canvas *image.RGBA, data LayoutData, lr *LayoutRenderer) {
	// Dense 3-column layout
	// │ ■45° ▣62° │ 2:30 PM │ ↓5.2 ↑1.1 │
	// │           │  ☀ 59°  │           │

	// Left column: temps
	if data.CPUTemp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemCPU, 10, 15)
		lr.renderer.drawText(canvas, formatTemp(data.CPUTemp, "°"), 25, 15)
	}
	if data.GPUTemp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemGPU, 80, 15)
		lr.renderer.drawText(canvas, formatTemp(data.GPUTemp, "°"), 95, 15)
	}

	// Center column: time and weather
	lr.renderer.drawTextWithFont(canvas, data.Time, 240, 15, lr.renderer.timeFont)
	if data.Weather != nil {
		if data.Weather.Icon != "" {
			lr.renderer.drawText(canvas, data.Weather.Icon, 260, 35)
		}
		tempSymbol := "°"
		lr.renderer.drawText(canvas, formatTemp(data.Weather.Temperature, tempSymbol), 280, 35)
	}

	// Right column: network
	if data.NetworkDown > 0 || data.NetworkUp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemNetworkDown, 520, 15)
		lr.renderer.drawText(canvas, formatSpeed(data.NetworkDown), 535, 15)

		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemNetworkUp, 520, 35)
		lr.renderer.drawText(canvas, formatSpeed(data.NetworkUp), 535, 35)
	}
}

// ========================
// Balanced Layout (Current)
// ========================

type BalancedLayout struct{}

func (l *BalancedLayout) Name() LayoutType {
	return LayoutBalanced
}

func (l *BalancedLayout) Description() string {
	return "Balanced information layout (current)"
}

func (l *BalancedLayout) Render(canvas *image.RGBA, data LayoutData, lr *LayoutRenderer) {
	// Current layout - keep as fallback
	// Left: temps, Center: time, Right: network, Bottom-center: weather

	// CPU/GPU temps (left)
	if data.CPUTemp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemCPU, 10, 15)
		lr.renderer.drawText(canvas, formatTemp(data.CPUTemp, "°C"), 25, 15)
	}
	if data.GPUTemp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemGPU, 10, 35)
		lr.renderer.drawText(canvas, formatTemp(data.GPUTemp, "°C"), 25, 35)
	}

	// Time (center)
	lr.renderer.drawTextWithFont(canvas, data.Time, 240, 20, lr.renderer.timeFont)

	// Network (right)
	if data.NetworkDown > 0 || data.NetworkUp > 0 {
		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemNetworkDown, 520, 15)
		lr.renderer.drawText(canvas, formatSpeed(data.NetworkDown), 535, 15)

		lr.iconRenderer.DrawIcon(canvas, lr.iconSet.SystemNetworkUp, 520, 35)
		lr.renderer.drawText(canvas, formatSpeed(data.NetworkUp), 535, 35)
	}

	// Weather (bottom center)
	if data.Weather != nil {
		if data.Weather.Icon != "" {
			lr.renderer.drawTextWithFont(canvas, data.Weather.Icon, 280, 35, lr.renderer.timeFont)
		}
		tempSymbol := "°C"
		if data.Unit == "imperial" {
			tempSymbol = "°F"
		}
		lr.renderer.drawText(canvas, formatTemp(data.Weather.Temperature, tempSymbol), 310, 35)
	}
}

// ========================
// Helper Functions
// ========================

func formatTemp(temp float64, symbol string) string {
	return formatFloat(temp, 0) + symbol
}

func formatSpeed(bytesPerSec float64) string {
	// Convert bytes/sec to bits/sec (multiply by 8)
	bitsPerSec := bytesPerSec * 8

	// Convert to Gbps if >= 1 Gb/s
	gbps := bitsPerSec / (1000 * 1000 * 1000)
	if gbps >= 1.0 {
		return formatFloat(gbps, 1) + "Gbps"
	}

	// Convert to Mbps if >= 1 Mb/s
	mbps := bitsPerSec / (1000 * 1000)
	if mbps >= 1.0 {
		return formatFloat(mbps, 1) + "Mbps"
	}

	// Otherwise show Kbps
	kbps := bitsPerSec / 1000
	return formatFloat(kbps, 0) + "Kbps"
}

func formatFloat(val float64, decimals int) string {
	if decimals == 0 {
		return formatInt(int(val))
	}
	// Simple float formatting
	intPart := int(val)
	fracPart := int((val - float64(intPart)) * 10)
	return formatInt(intPart) + "." + formatInt(fracPart)
}

func formatInt(val int) string {
	if val == 0 {
		return "0"
	}

	negative := val < 0
	if negative {
		val = -val
	}

	digits := make([]byte, 0, 10)
	for val > 0 {
		digits = append(digits, byte('0'+val%10))
		val /= 10
	}

	// Reverse
	for i, j := 0, len(digits)-1; i < j; i, j = i+1, j-1 {
		digits[i], digits[j] = digits[j], digits[i]
	}

	if negative {
		return "-" + string(digits)
	}
	return string(digits)
}

// LayoutManager manages available layouts
type LayoutManager struct {
	layouts       map[LayoutType]Layout
	currentLayout Layout
}

// NewLayoutManager creates a new layout manager
func NewLayoutManager() *LayoutManager {
	layouts := map[LayoutType]Layout{
		LayoutDashboard:  &DashboardLayout{},
		LayoutMinimalist: &MinimalistLayout{},
		LayoutCompact:    &CompactLayout{},
		LayoutBalanced:   &BalancedLayout{},
	}

	return &LayoutManager{
		layouts:       layouts,
		currentLayout: layouts[LayoutDashboard], // Default to dashboard
	}
}

// SetLayout changes the current layout
func (lm *LayoutManager) SetLayout(layoutType LayoutType) bool {
	if layout, ok := lm.layouts[layoutType]; ok {
		lm.currentLayout = layout
		return true
	}
	return false
}

// GetLayout returns the current layout
func (lm *LayoutManager) GetLayout() Layout {
	return lm.currentLayout
}

// GetAvailableLayouts returns all available layout types
func (lm *LayoutManager) GetAvailableLayouts() []LayoutType {
	types := make([]LayoutType, 0, len(lm.layouts))
	for t := range lm.layouts {
		types = append(types, t)
	}
	return types
}
