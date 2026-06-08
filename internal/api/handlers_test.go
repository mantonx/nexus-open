package api

import (
	"bytes"
	"context"
	"encoding/json"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"testing"

	"github.com/mantonx/nexus-open/internal/device"
	config "github.com/mantonx/nexus-open/internal/settings"
)

func newTestConfig(t *testing.T) *config.Manager {
	t.Helper()
	mgr, err := config.NewManagerFromPath(filepath.Join(t.TempDir(), "test.db"), nil)
	if err != nil {
		t.Fatalf("newTestConfig: %v", err)
	}
	return mgr
}

func TestHealthHandler(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg := newTestConfig(t)

	mockDev := device.NewMockDevice()
	if err := mockDev.Connect(context.Background()); err != nil {
		t.Fatalf("failed to connect mock device: %v", err)
	}
	server := NewServer(":0", cfg, mockDev, logger)

	req := httptest.NewRequest("GET", "/api/health", nil)
	w := httptest.NewRecorder()
	server.handleHealth(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
	var resp map[string]interface{}
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if resp["status"] != "ok" {
		t.Errorf("status = %v, want ok", resp["status"])
	}
}

func TestGetConfigHandler(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg := newTestConfig(t)
	server := NewServer(":0", cfg, device.NewMockDevice(), logger)

	req := httptest.NewRequest("GET", "/api/config", nil)
	w := httptest.NewRecorder()
	server.handleGetConfig(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
	var resp config.Config
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("decode: %v", err)
	}
	if resp.BackgroundColor == "" {
		t.Error("expected non-empty background_color")
	}
	if resp.TextColor == "" {
		t.Error("expected non-empty text_color")
	}
}

func TestUpdateConfigHandler_Valid(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg := newTestConfig(t)
	server := NewServer(":0", cfg, device.NewMockDevice(), logger)

	update := config.Config{BackgroundColor: "#111111", TextColor: "#EEEEEE"}
	body, _ := json.Marshal(update)
	req := httptest.NewRequest("POST", "/api/config", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	server.handleUpdateConfig(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
	if got := cfg.Get().BackgroundColor; got != "#111111" {
		t.Errorf("BackgroundColor = %q, want #111111", got)
	}
}

func TestUpdateConfigHandler_Invalid(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg := newTestConfig(t)
	server := NewServer(":0", cfg, device.NewMockDevice(), logger)

	update := config.Config{BackgroundColor: "not-a-color", TextColor: "#FFFFFF"}
	body, _ := json.Marshal(update)
	req := httptest.NewRequest("POST", "/api/config", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	server.handleUpdateConfig(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestUpdateConfigHandler_InvalidJSON(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg := newTestConfig(t)
	server := NewServer(":0", cfg, device.NewMockDevice(), logger)

	req := httptest.NewRequest("POST", "/api/config", bytes.NewReader([]byte("invalid json")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()
	server.handleUpdateConfig(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected 400, got %d", w.Code)
	}
}

func TestListImagesHandler(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg := newTestConfig(t)
	server := NewServer(":0", cfg, device.NewMockDevice(), logger)

	req := httptest.NewRequest("GET", "/api/images/list", nil)
	w := httptest.NewRecorder()
	server.handleListImages(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
	var resp []string
	if err := json.NewDecoder(w.Body).Decode(&resp); err != nil {
		t.Fatalf("decode: %v", err)
	}
}

func TestLocalOnlyMiddleware(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg := newTestConfig(t)
	server := NewServer(":0", cfg, device.NewMockDevice(), logger)
	token := server.Token()

	ok := http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) { w.WriteHeader(http.StatusOK) })
	handler := server.localOnlyMiddleware(ok)

	do := func(host, tok, path string) int {
		req := httptest.NewRequest(http.MethodGet, path, nil)
		req.Host = host
		if tok != "" {
			req.Header.Set("X-Nexus-Token", tok)
		}
		w := httptest.NewRecorder()
		handler.ServeHTTP(w, req)
		return w.Code
	}

	// Correct host + correct token — allowed.
	if code := do("localhost:1985", token, "/api/config"); code != 200 {
		t.Errorf("valid request: want 200, got %d", code)
	}
	// Correct host, no token — rejected.
	if code := do("localhost:1985", "", "/api/config"); code != 401 {
		t.Errorf("missing token: want 401, got %d", code)
	}
	// Wrong host — rejected regardless of token.
	if code := do("evil.example.com", token, "/api/config"); code != 403 {
		t.Errorf("bad host: want 403, got %d", code)
	}
	// Health is token-exempt.
	if code := do("localhost:1985", "", "/api/health"); code != 200 {
		t.Errorf("health without token: want 200, got %d", code)
	}
}

func TestLoggingMiddleware(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg := newTestConfig(t)
	server := NewServer(":0", cfg, device.NewMockDevice(), logger)

	handler := server.loggingMiddleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("test")) //nolint:errcheck
	}))
	req := httptest.NewRequest("GET", "/api/test", nil)
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
	if w.Body.String() != "test" {
		t.Errorf("body = %q, want test", w.Body.String())
	}
}
