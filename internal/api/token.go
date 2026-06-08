package api

import (
	"crypto/rand"
	"encoding/hex"
	"fmt"
	"os"
	"path/filepath"
)

const tokenFileName = "token"

// tokenDir returns the XDG config directory for nexus-open.
func tokenDir() string {
	base := os.Getenv("XDG_CONFIG_HOME")
	if base == "" {
		base = filepath.Join(os.Getenv("HOME"), ".config")
	}
	return filepath.Join(base, "nexus-open")
}

// LoadOrCreateToken reads the capability token from disk, creating a new one
// if none exists. The file is written with mode 0600 so only the owning user
// can read it.
func LoadOrCreateToken() (string, error) {
	dir := tokenDir()
	if err := os.MkdirAll(dir, 0700); err != nil {
		return "", fmt.Errorf("create config dir: %w", err)
	}

	path := filepath.Join(dir, tokenFileName)
	if data, err := os.ReadFile(path); err == nil {
		tok := string(data)
		if len(tok) == 64 {
			return tok, nil
		}
	}

	// Generate a fresh 32-byte (256-bit) token.
	raw := make([]byte, 32)
	if _, err := rand.Read(raw); err != nil {
		return "", fmt.Errorf("generate token: %w", err)
	}
	tok := hex.EncodeToString(raw)

	if err := os.WriteFile(path, []byte(tok), 0600); err != nil {
		return "", fmt.Errorf("write token: %w", err)
	}
	return tok, nil
}
