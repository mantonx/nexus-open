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

	"github.com/mantonx/nexus-next/internal/device"
	config "github.com/mantonx/nexus-next/internal/settings"
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

func TestCORSMiddleware(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg := newTestConfig(t)
	server := NewServer(":0", cfg, device.NewMockDevice(), logger)

	handler := server.corsMiddleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	req := httptest.NewRequest("OPTIONS", "/api/test", nil)
	req.Header.Set("Origin", "http://localhost:3000")
	w := httptest.NewRecorder()
	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected 200, got %d", w.Code)
	}
	if w.Header().Get("Access-Control-Allow-Origin") != "*" {
		t.Error("missing CORS origin header")
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
