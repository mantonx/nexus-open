package api

import (
	"bytes"
	"encoding/json"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"testing"

	"nexus-open/internal/settings"
	"nexus-open/internal/device"
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

	// Verify default values
	if response.TimeFormat == "" {
		t.Error("expected non-empty time format")
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

	// Create valid config update
	update := config.Config{
		Location:        "New York, NY",
		TimeFormat:      "24h",
		Unit:            "metric",
		BackgroundColor: "#000000",
		TextColor:       "#FFFFFF",
	}

	body, _ := json.Marshal(update)
	req := httptest.NewRequest("POST", "/api/config", bytes.NewReader(body))
	req.Header.Set("Content-Type", "application/json")
	w := httptest.NewRecorder()

	server.handleUpdateConfig(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200, got %d", w.Code)
	}

	// Verify config was updated
	currentCfg := cfg.Get()
	if currentCfg.Location != "New York, NY" {
		t.Errorf("expected location 'New York, NY', got %s", currentCfg.Location)
	}
	if currentCfg.Unit != "metric" {
		t.Errorf("expected unit 'metric', got %s", currentCfg.Unit)
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

	// Create invalid config (bad time format)
	update := config.Config{
		Location:        "New York, NY",
		TimeFormat:      "invalid",
		Unit:            "metric",
		BackgroundColor: "#000000",
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

	// Should return empty list or list of images (nil is valid for empty array)
	// Just verify it decoded without error
}

func TestCORSMiddleware(t *testing.T) {
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	cfg, _ := config.NewManager("")
	mockDev := device.NewMockDevice()
	server := NewServer(":0", cfg, mockDev, logger)

	handler := server.corsMiddleware(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))

	// Test preflight request
	req := httptest.NewRequest("OPTIONS", "/api/test", nil)
	req.Header.Set("Origin", "http://localhost:3000")
	w := httptest.NewRecorder()

	handler.ServeHTTP(w, req)

	if w.Code != http.StatusOK {
		t.Errorf("expected status 200 for preflight, got %d", w.Code)
	}

	// Check CORS headers
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

	// Logging middleware should not affect the response
	if w.Body.String() != "test" {
		t.Errorf("expected body 'test', got %s", w.Body.String())
	}
}
