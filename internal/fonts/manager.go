package fonts

import (
	"embed"
	"fmt"
	"log/slog"
	"os"

	"github.com/golang/freetype/truetype"
	"golang.org/x/image/font"
	"golang.org/x/image/font/gofont/gobold"
	"golang.org/x/image/font/gofont/gomedium"
	"golang.org/x/image/font/gofont/gomono"
	"golang.org/x/image/font/gofont/goregular"
)

//go:embed assets/fonts/*
var fontFS embed.FS

// Manager handles font loading and caching.
type Manager struct {
	logger *slog.Logger
	fonts  map[string]*truetype.Font
	faces  map[fontKey]font.Face
}

type fontKey struct {
	name string
	size float64
}

// NewManager creates a new font manager.
func NewManager(logger *slog.Logger) *Manager {
	return &Manager{
		logger: logger,
		fonts:  make(map[string]*truetype.Font),
		faces:  make(map[fontKey]font.Face),
	}
}

var bundledGoFonts = map[string][]byte{
	"GoRegular":              goregular.TTF,
	"GoMedium":               gomedium.TTF,
	"GoBold":                 gobold.TTF,
	"GoMono":                 gomono.TTF,
	"GoMono-Regular":         gomono.TTF,
	"DejaVuSans":             goregular.TTF, // Aliases for legacy configs
	"DejaVuSansMono":         gomono.TTF,
	"LiberationSans-Regular": goregular.TTF,
	"LiberationMono-Regular": gomono.TTF,
	"Ubuntu-R":               goregular.TTF,
	"UbuntuMono-R":           gomono.TTF,
}

// LoadFont loads a TrueType font from embedded, system, or bundled sources.
func (m *Manager) LoadFont(name string) (*truetype.Font, error) {
	if font, ok := m.fonts[name]; ok {
		return font, nil
	}

	var (
		fontData []byte
		err      error
	)

	for _, ext := range []string{".ttf", ".otf"} {
		embeddedPath := fmt.Sprintf("assets/fonts/%s%s", name, ext)
		fontData, err = fontFS.ReadFile(embeddedPath)
		if err == nil {
			m.logger.Debug("loaded embedded font", "name", name, "path", embeddedPath)
			return m.parseAndCache(name, fontData)
		}
	}

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
			m.logger.Debug("loaded system font", "name", name, "path", path)
			return m.parseAndCache(name, fontData)
		}
	}

	if data, ok := bundledGoFonts[name]; ok {
		m.logger.Debug("loaded bundled gofont", "name", name)
		return m.parseAndCache(name, data)
	}

	return nil, fmt.Errorf("font %s not found in embedded or system paths", name)
}

func (m *Manager) parseAndCache(name string, data []byte) (*truetype.Font, error) {
	font, err := truetype.Parse(data)
	if err != nil {
		return nil, fmt.Errorf("failed to parse font: %w", err)
	}
	m.fonts[name] = font
	return font, nil
}

// GetFace returns a font face with the specified size.
func (m *Manager) GetFace(fontName string, size float64) (font.Face, error) {
	key := fontKey{name: fontName, size: size}
	if face, ok := m.faces[key]; ok {
		return face, nil
	}

	ttfFont, err := m.LoadFont(fontName)
	if err != nil {
		return nil, err
	}

	face := truetype.NewFace(ttfFont, &truetype.Options{
		Size:    size,
		DPI:     72,
		Hinting: font.HintingFull,
	})

	m.faces[key] = face
	return face, nil
}

// GetDefaultFonts returns an ordered list of preferred fonts.
func GetDefaultFonts() []string {
	return []string{
		"GoRegular",
		"GoMedium",
		"GoMono",
		"DejaVuSansMono",
		"DejaVuSans",
		"LiberationMono-Regular",
		"LiberationSans-Regular",
		"Ubuntu-R",
		"UbuntuMono-R",
	}
}

// LoadBestAvailableFont tries to load fonts in order of preference.
func (m *Manager) LoadBestAvailableFont(size float64) (font.Face, string, error) {
	for _, fontName := range GetDefaultFonts() {
		face, err := m.GetFace(fontName, size)
		if err == nil {
			m.logger.Info("loaded font", "name", fontName, "size", size)
			return face, fontName, nil
		}
		m.logger.Debug("font not available", "name", fontName, "error", err)
	}
	return nil, "", fmt.Errorf("no suitable fonts found")
}
