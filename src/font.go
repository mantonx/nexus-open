package main

import (
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"

	"github.com/golang/freetype/truetype"
	"golang.org/x/image/font"
	"golang.org/x/image/font/basicfont"
)

var (
	systemFont     font.Face
	systemFontOnce sync.Once

	fontDirs = map[string][]string{
		"windows": {"C:\\Windows\\Fonts"},
		"darwin":  {"/Library/Fonts", "/System/Library/Fonts", "/System/Library/Fonts/Supplemental"},
		"linux": {
			"/usr/share/fonts",
			"/usr/local/share/fonts",
			os.Getenv("HOME") + "/.fonts",
			"/usr/share/fonts/truetype",
			"/usr/share/fonts/TTF",
		},
	}

	popularFonts = map[string][]string{
		"windows": {
			"arial.ttf",
			"segoeui.ttf",
			"verdana.ttf",
			"tahoma.ttf",
			"consolas.ttf",
			"calibri.ttf",
			"times.ttf",
			"cour.ttf",
			"candara.ttf",
			"cambria.ttf",
		},
		"darwin": {
			"Arial.ttf",
			"Helvetica.ttf",
			"Monaco.ttf",
			"Menlo.ttc",
			"TimesNewRomanPSMT.ttf",
			"SFPro.ttf",
			"Courier.ttf",
			"GeezaPro.ttc",
		},
		"linux": {
			"DejaVuSans.ttf",
			"Ubuntu.ttf",
			"NotoSans-Regular.ttf",
			"FreeSans.ttf",
			"LiberationSans-Regular.ttf",
			"DroidSans.ttf",
			"OpenSans-Regular.ttf",
			"Roboto-Regular.ttf",
		},
	}
)

// LoadSystemFont loads and caches a system font specified by the preferredFont parameter.
// It uses sync.Once to ensure the font is loaded only once, making it safe for concurrent use.
// The function returns a font.Face that can be used for text rendering.
//
// Parameters:
//   - preferredFont: The name or path of the preferred system font to load
//
// Returns:
//   - font.Face: The loaded font face instance that can be used for text rendering
func LoadSystemFont(preferredFont string) font.Face {
	systemFontOnce.Do(func() {
		systemFont = loadFont(preferredFont)
	})
	return systemFont
}

// loadFont attempts to load a font face based on the provided preferred font name.
// It follows this order:
// 1. Tries to load the preferred font if specified
// 2. Attempts to load system fonts based on the operating system
// 3. Falls back to basic font (7x13) if no other fonts are available
//
// Parameters:
//   - preferredFont: The name of the preferred font to try first. If empty, skips to system fonts.
//
// Returns:
//   - font.Face: The loaded font face. Will never return nil as it falls back to basicfont.Face7x13.
func loadFont(preferredFont string) font.Face {
	osType := runtime.GOOS

	// Try preferred font first
	if preferredFont != "" {
		if f := tryLoadFont(preferredFont, osType); f != nil {
			return f
		}
	}

	// Try system fonts
	if f := tryLoadSystemFonts(osType); f != nil {
		return f
	}

	// Fallback to basic font
	return basicfont.Face7x13
}

// tryLoadFont attempts to load a font from the specified path based on the operating system.
// It iterates through system font directories to find and create a font face.
// For Windows systems, the font path is converted to lowercase.
//
// Parameters:
//   - fontPath: The name or relative path of the font file to load
//   - osType: The operating system type ("windows", "darwin", "linux", etc.)
//
// Returns:
//   - font.Face: A valid font face if found, nil otherwise
func tryLoadFont(fontPath, osType string) font.Face {
	if osType == "windows" {
		fontPath = strings.ToLower(fontPath)
	}

	for _, dir := range fontDirs[osType] {
		path := filepath.Join(dir, fontPath)
		if face := createFontFace(path); face != nil {
			println("Using font:", path)
			return face
		}
	}
	return nil
}

// tryLoadSystemFonts attempts to load system fonts based on the operating system type.
// It first tries to load from a predefined list of popular fonts for the given OS.
// If no popular fonts are found, it scans system font directories for TTF or OTF files.
//
// Parameters:
//   - osType: String identifying the operating system (e.g., "windows", "darwin", "linux")
//
// Returns:
//   - font.Face: A valid font face if found, nil otherwise
//
// The function searches in system-specific font directories defined in fontDirs[osType]
// and tries to load fonts in the following order:
//  1. Popular fonts defined in popularFonts[osType]
//  2. Any .ttf or .otf files found in the system font directories
func tryLoadSystemFonts(osType string) font.Face {
	// Try popular fonts first
	for _, fontName := range popularFonts[osType] {
		for _, dir := range fontDirs[osType] {
			path := filepath.Join(dir, fontName)
			if face := createFontFace(path); face != nil {
				return face
			}
		}
	}

	// Scan directories for any available fonts
	extensions := []string{".ttf", ".otf"}
	for _, dir := range fontDirs[osType] {
		filepath.Walk(dir, func(path string, info os.FileInfo, err error) error {
			if err != nil || info.IsDir() {
				return nil
			}
			ext := strings.ToLower(filepath.Ext(path))
			for _, validExt := range extensions {
				if ext == validExt {
					if face := createFontFace(path); face != nil {
						return filepath.SkipAll
					}
				}
			}
			return nil
		})
	}
	return nil
}

// createFontFace creates and returns a new font.Face from a TrueType font file.
// It takes a file path as input and returns the created font face.
// The font is rendered with a size of 13pt at 72 DPI.
// If there are any errors reading the file or parsing the font, it returns nil.
func createFontFace(path string) font.Face {
	fontBytes, err := os.ReadFile(path)
	if err != nil {
		return nil
	}

	f, err := truetype.Parse(fontBytes)
	if err != nil {
		return nil
	}

	return truetype.NewFace(f, &truetype.Options{
		Size: 13,
		DPI:  72,
	})
}
