package display

import (
	"embed"
	"fmt"
	"log/slog"
	"os"

	"github.com/golang/freetype/truetype"
	"golang.org/x/image/font"
)

//go:embed assets/fonts
var fontFS embed.FS

// FontManager handles font loading and caching
type FontManager struct {
	logger *slog.Logger
	fonts  map[string]*truetype.Font
	faces  map[fontKey]font.Face
}

type fontKey struct {
	name string
	size float64
}

// NewFontManager creates a new font manager
func NewFontManager(logger *slog.Logger) *FontManager {
	return &FontManager{
		logger: logger,
		fonts:  make(map[string]*truetype.Font),
		faces:  make(map[fontKey]font.Face),
	}
}

// LoadFont loads a TrueType font from multiple possible sources
func (fm *FontManager) LoadFont(name string) (*truetype.Font, error) {
	// Check cache
	if font, ok := fm.fonts[name]; ok {
		return font, nil
	}

	// Try to load from different sources
	var fontData []byte
	var err error

	// 1. Try embedded fonts first (both .ttf and .otf)
	for _, ext := range []string{".ttf", ".otf"} {
		embeddedPath := fmt.Sprintf("assets/fonts/%s%s", name, ext)
		fontData, err = fontFS.ReadFile(embeddedPath)
		if err == nil {
			fm.logger.Debug("loaded embedded font", "name", name, "path", embeddedPath)
			return fm.parseAndCache(name, fontData)
		}
	}

	// 2. Try common Linux font paths (both .ttf and .otf)
	systemPaths := []string{
		fmt.Sprintf("/usr/share/fonts/truetype/dejavu/%s.ttf", name),
		fmt.Sprintf("/usr/share/fonts/TTF/%s.ttf", name),
		fmt.Sprintf("/usr/share/fonts/truetype/liberation/%s.ttf", name),
		fmt.Sprintf("/usr/share/fonts/truetype/ubuntu/%s.ttf", name),
		fmt.Sprintf("/usr/share/fonts/%s.ttf", name),
		fmt.Sprintf("/usr/share/fonts/%s.otf", name),
		fmt.Sprintf("/usr/local/share/fonts/%s.ttf", name),
		fmt.Sprintf("/usr/local/share/fonts/%s.otf", name),
	}

	for _, path := range systemPaths {
		fontData, err = os.ReadFile(path)
		if err == nil {
			fm.logger.Debug("loaded system font", "name", name, "path", path)
			return fm.parseAndCache(name, fontData)
		}
	}

	return nil, fmt.Errorf("font %s not found in embedded or system paths", name)
}

// parseAndCache parses font data and caches it
func (fm *FontManager) parseAndCache(name string, data []byte) (*truetype.Font, error) {
	font, err := truetype.Parse(data)
	if err != nil {
		return nil, fmt.Errorf("failed to parse font: %w", err)
	}

	fm.fonts[name] = font
	return font, nil
}

// GetFace returns a font face with the specified size
func (fm *FontManager) GetFace(fontName string, size float64) (font.Face, error) {
	key := fontKey{name: fontName, size: size}

	// Check cache
	if face, ok := fm.faces[key]; ok {
		return face, nil
	}

	// Load font if not loaded
	ttfFont, err := fm.LoadFont(fontName)
	if err != nil {
		return nil, err
	}

	// Create face with specified size
	face := truetype.NewFace(ttfFont, &truetype.Options{
		Size:    size,
		DPI:     72,
		Hinting: font.HintingFull,
	})

	// Cache the face
	fm.faces[key] = face
	return face, nil
}

// GetDefaultFonts returns a list of font names to try in order
func GetDefaultFonts() []string {
	return []string{
		"DejaVuSansMono",
		"DejaVuSans",
		"LiberationMono-Regular",
		"LiberationSans-Regular",
		"Ubuntu-R",
		"UbuntuMono-R",
	}
}

// LoadBestAvailableFont tries to load fonts in order of preference
func (fm *FontManager) LoadBestAvailableFont(size float64) (font.Face, string, error) {
	fonts := GetDefaultFonts()

	for _, fontName := range fonts {
		face, err := fm.GetFace(fontName, size)
		if err == nil {
			fm.logger.Info("loaded font", "name", fontName, "size", size)
			return face, fontName, nil
		}
		fm.logger.Debug("font not available", "name", fontName, "error", err)
	}

	return nil, "", fmt.Errorf("no suitable fonts found")
}
