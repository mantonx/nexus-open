package api

import (
	"bytes"
	"encoding/json"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"testing"

	"github.com/mantonx/nexus-next/internal/device"
	"github.com/mantonx/nexus-next/internal/settings"
)

func TestHealthHandler(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, err := config.NewManager("")
	if err != nil {
		t.Fatalf("failed to create config: %v", err)
	}

	mockDev := device.NewMockDevice()
	server := NewServer(":0", cfg, mockDev, logger)

	req := httptest.NewRequest("GET", "/api/health", nil)
	w := httptest.NewRecorder()

	server.handleHealth(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	var response map[string]interface{}
	if err := json.NewDecoder(w.Body).Decode(&response); err != nil {
		t.Fatalf("failed to decode response: %v", err)
	}

	if response["status"] != "ok" {
		t.Errorf("expected status 'ok', got %v", response["status"])
	}
}

func TestGetConfigHandler(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, err := config.NewManager("")
	if err != nil {
		t.Fatalf("failed to create config: %v", err)
	}

	mockDev := device.NewMockDevice()
	server := NewServer(":0", cfg, mockDev, logger)

	req := httptest.NewRequest("GET", "/api/config", nil)
	w := httptest.NewRecorder()

	server.handleGetConfig(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	var response config.Config
	if err := json.NewDecoder(w.Body).Decode(&response); err != nil {
		t.Fatalf("failed to decode response: %v", err)
	}

	// Verify default values are present
	if response.BackgroundColor == "" {
		t.Error("expected non-empty background color")
	}
	if response.TextColor == "" {
		t.Error("expected non-empty text color")
	}
}

func TestUpdateConfigHandler_Valid(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, err := config.NewManager("")
	if err != nil {
		t.Fatalf("failed to create config: %v", err)
	}

	mockDev := device.NewMockDevice()
	server := NewServer(":0", cfg, mockDev, logger)

	update := config.Config{
		BackgroundColor: "#111111",
		TextColor:       "#EEEEEE",
	}

	body, _ := json.Marshal(update)
	req := httptest.NewRequest("POST", "/api/config", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	server.handleUpdateConfig(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	currentCfg := cfg.Get()
	if currentCfg.BackgroundColor != "#111111" {
		t.Errorf("expected background_color '#111111', got %s", currentCfg.BackgroundColor)
	}
}

func TestUpdateConfigHandler_Invalid(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, err := config.NewManager("")
	if err != nil {
		t.Fatalf("failed to create config: %v", err)
	}

	mockDev := device.NewMockDevice()
	server := NewServer(":0", cfg, mockDev, logger)

	update := config.Config{
		BackgroundColor: "not-a-color", // Invalid!
		TextColor:       "#FFFFFF",
	}

	body, _ := json.Marshal(update)
	req := httptest.NewRequest("POST", "/api/config", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	server.handleUpdateConfig(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected status 400, got %d", w.Code)
	}
}

func TestUpdateConfigHandler_InvalidJSON(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, err := config.NewManager("")
	if err != nil {
		t.Fatalf("failed to create config: %v", err)
	}

	mockDev := device.NewMockDevice()
	server := NewServer(":0", cfg, mockDev, logger)

	req := httptest.NewRequest("POST", "/api/config", bytes.NewReader([]byte("invalid json")))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	server.handleUpdateConfig(w, req)

	if w.Code != http.StatusBadRequest {
		t.Errorf("expected status 400, got %d", w.Code)
	}
}

func TestListImagesHandler(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, err := config.NewManager("")
	if err != nil {
		t.Fatalf("failed to create config: %v", err)
	}

	mockDev := device.NewMockDevice()
	server := NewServer(":0", cfg, mockDev, logger)

	req := httptest.NewRequest("GET", "/api/images/list", nil)
	w := httptest.NewRecorder()

	server.handleListImages(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	var response []string
	if err := json.NewDecoder(w.Body).Decode(&response); err != nil {
		t.Fatalf("failed to decode response: %v", err)
	}
}

func TestCORSMiddleware(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")
	mockDev := device.NewMockDevice()
	server := NewServer(":0", cfg, mockDev, logger)

	handler := server.corsMiddleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	req := httptest.NewRequest("OPTIONS", "/api/test", nil)
	req.Header.Set("Origin", "http://localhost:3000")
	w := httptest.NewRecorder()

	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200 for preflight, got %d", w.Code)
	}

	if w.Header().Get("Access-Control-Allow-Origin") != "*" {
		t.Error("missing or incorrect CORS origin header")
	}
}

func TestLoggingMiddleware(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")
	mockDev := device.NewMockDevice()
	server := NewServer(":0", cfg, mockDev, logger)

	handler := server.loggingMiddleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("test"))
	}))

	req := httptest.NewRequest("GET", "/api/test", nil)
	w := httptest.NewRecorder()

	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	if w.Body.String() != "test" {
		t.Errorf("expected body 'test', got %s", w.Body.String())
	}
}
