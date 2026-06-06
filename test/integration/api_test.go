// Package integration tests the full HTTP API against a real running Server.
// Unlike the unit tests in internal/api/handlers_test.go (which call handlers
// directly), these tests exercise the complete request path: routing,
// middleware, serialisation, and WebSocket upgrade.
//
// Run: go test ./test/integration/... -v
package integration

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net"
	"net/http"
	"net/http/httptest"
	"os"
	"testing"
	"time"

	"github.com/mantonx/nexus-next/internal/api"
	"github.com/mantonx/nexus-next/internal/device"
	config "github.com/mantonx/nexus-next/internal/settings"
	"github.com/mantonx/nexus-next/internal/store"
	"github.com/mantonx/nexus-next/internal/zone"
	"log/slog"
	"nhooyr.io/websocket"
	"nhooyr.io/websocket/wsjson"
)

// ── Helpers ───────────────────────────────────────────────────────────────────

// newTestServer creates a Server bound to a random localhost port and returns
// both the server and its base URL. The caller must call ts.Close() to stop it.
func newTestServer(t *testing.T) (*api.Server, string) {
	t.Helper()

	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	tmpDir := t.TempDir()

	s, err := store.Open(tmpDir+"/test.db", logger)
	if err != nil {
		t.Fatalf("store.Open: %v", err)
	}
	t.Cleanup(func() { s.Close() })

	cfg, err := config.NewManager(s, logger)
	if err != nil {
		t.Fatalf("config.NewManager: %v", err)
	}

	mockDev := device.NewMockDevice()
	if err := mockDev.Connect(context.Background()); err != nil {
		t.Fatalf("mock device connect: %v", err)
	}
	t.Cleanup(func() { mockDev.Disconnect() })

	zoneCfg := zone.NewConfigManager(s, logger)

	// Bind on :0 so the OS assigns a free port.
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("net.Listen: %v", err)
	}
	port := ln.Addr().(*net.TCPAddr).Port
	ln.Close()

	srv := api.NewServer(fmt.Sprintf(":%d", port), cfg, mockDev, logger)
	srv.SetZoneConfigManager(zoneCfg)

	go func() {
		if err := srv.Start(); err != nil {
			// Server closed normally — not an error.
		}
	}()

	// Wait until the server accepts connections.
	baseURL := fmt.Sprintf("http://127.0.0.1:%d", port)
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		resp, err := http.Get(baseURL + "/api/health")
		if err == nil {
			resp.Body.Close()
			break
		}
		time.Sleep(20 * time.Millisecond)
	}

	t.Cleanup(func() {
		ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		defer cancel()
		srv.Shutdown(ctx)
	})

	return srv, baseURL
}

func get(t *testing.T, url string) *http.Response {
	t.Helper()
	resp, err := http.Get(url)
	if err != nil {
		t.Fatalf("GET %s: %v", url, err)
	}
	return resp
}

func postJSON(t *testing.T, url string, body any) *http.Response {
	t.Helper()
	data, err := json.Marshal(body)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	resp, err := http.Post(url, "application/json", bytes.NewReader(data))
	if err != nil {
		t.Fatalf("POST %s: %v", url, err)
	}
	return resp
}

func decodeJSON(t *testing.T, resp *http.Response, dst any) {
	t.Helper()
	defer resp.Body.Close()
	if err := json.NewDecoder(resp.Body).Decode(dst); err != nil {
		t.Fatalf("decode JSON: %v", err)
	}
}

// ── Health ────────────────────────────────────────────────────────────────────

func TestHealth_OK(t *testing.T) {
	_, base := newTestServer(t)

	resp := get(t, base+"/api/health")
	if resp.StatusCode != 200 {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}

	var body map[string]any
	decodeJSON(t, resp, &body)

	if body["status"] != "ok" {
		t.Errorf("status: want 'ok', got %v", body["status"])
	}
	if body["version"] == nil {
		t.Error("missing 'version' field")
	}
	// first_run must be present (bool) — Flutter depends on this field.
	if _, ok := body["first_run"]; !ok {
		t.Error("missing 'first_run' field — Flutter health contract broken")
	}
}

func TestHealth_MethodNotAllowed(t *testing.T) {
	_, base := newTestServer(t)

	resp, err := http.Post(base+"/api/health", "application/json", nil)
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	if resp.StatusCode != 405 {
		t.Errorf("expected 405, got %d", resp.StatusCode)
	}
}

// ── Config ────────────────────────────────────────────────────────────────────

func TestConfig_GetReturnsExpectedFields(t *testing.T) {
	_, base := newTestServer(t)

	resp := get(t, base+"/api/config")
	if resp.StatusCode != 200 {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}

	var body map[string]any
	decodeJSON(t, resp, &body)

	// These are the fields Flutter's NexusConfig.fromJson reads.
	// Any missing field causes a silent default — which is how we got the
	// "backend disconnected" banner despite a healthy server.
	for _, field := range []string{"background_color", "background_image", "text_color", "image_paths", "display"} {
		if _, ok := body[field]; !ok {
			t.Errorf("GET /api/config missing field %q — Flutter contract broken", field)
		}
	}

	display, _ := body["display"].(map[string]any)
	for _, field := range []string{"font_family", "font_size", "time_font_size", "layout", "date_format"} {
		if _, ok := display[field]; !ok {
			t.Errorf("GET /api/config: display missing field %q", field)
		}
	}
}

func TestConfig_RoundTrip(t *testing.T) {
	_, base := newTestServer(t)

	update := map[string]any{
		"background_color": "#112233",
		"background_image": "bg.png",
		"text_color":       "#AABBCC",
		"image_paths":      []string{},
		"display": map[string]any{
			"font_family":    "GoRegular",
			"font_size":      12.0,
			"time_font_size": 16.0,
			"layout":         "dashboard",
			"date_format":    "DD/MM/YYYY",
		},
	}

	resp := postJSON(t, base+"/api/config", update)
	if resp.StatusCode != 200 {
		resp.Body.Close()
		t.Fatalf("POST /api/config: expected 200, got %d", resp.StatusCode)
	}
	resp.Body.Close()

	// Read back and verify the saved values.
	var got map[string]any
	decodeJSON(t, get(t, base+"/api/config"), &got)

	if got["background_color"] != "#112233" {
		t.Errorf("background_color: want #112233, got %v", got["background_color"])
	}
	if got["text_color"] != "#AABBCC" {
		t.Errorf("text_color: want #AABBCC, got %v", got["text_color"])
	}
	if display, ok := got["display"].(map[string]any); ok {
		if display["date_format"] != "DD/MM/YYYY" {
			t.Errorf("date_format: want DD/MM/YYYY, got %v", display["date_format"])
		}
	}
}

func TestConfig_InvalidColorRejected(t *testing.T) {
	_, base := newTestServer(t)

	update := map[string]any{
		"background_color": "not-a-color",
		"text_color":       "#FFFFFF",
	}
	resp := postJSON(t, base+"/api/config", update)
	resp.Body.Close()
	if resp.StatusCode != 400 {
		t.Errorf("expected 400 for invalid color, got %d", resp.StatusCode)
	}
}

func TestConfig_InvalidJSONRejected(t *testing.T) {
	_, base := newTestServer(t)

	resp, err := http.Post(base+"/api/config", "application/json", bytes.NewBufferString("{bad json"))
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	if resp.StatusCode != 400 {
		t.Errorf("expected 400 for invalid JSON, got %d", resp.StatusCode)
	}
}

// ── CORS ──────────────────────────────────────────────────────────────────────

func TestCORS_HeadersPresent(t *testing.T) {
	_, base := newTestServer(t)

	resp := get(t, base+"/api/health")
	resp.Body.Close()

	if resp.Header.Get("Access-Control-Allow-Origin") != "*" {
		t.Error("missing Access-Control-Allow-Origin: *")
	}
}

func TestCORS_PreflightReturns200(t *testing.T) {
	_, base := newTestServer(t)

	req, _ := http.NewRequest("OPTIONS", base+"/api/config", nil)
	req.Header.Set("Origin", "http://localhost:3000")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Errorf("preflight: expected 200, got %d", resp.StatusCode)
	}
}

// ── Device info ───────────────────────────────────────────────────────────────

func TestDeviceInfo_MockConnected(t *testing.T) {
	_, base := newTestServer(t)

	resp := get(t, base+"/api/device/info")
	if resp.StatusCode != 200 {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}

	var body map[string]any
	decodeJSON(t, resp, &body)

	data, _ := body["data"].(map[string]any)
	if data["firmware"] == nil {
		t.Error("missing firmware field in device info")
	}
}

// ── Brightness ────────────────────────────────────────────────────────────────

func TestBrightness_ValidRange(t *testing.T) {
	_, base := newTestServer(t)

	for _, level := range []int{0, 50, 100} {
		resp := postJSON(t, base+"/api/device/brightness", map[string]int{"brightness": level})
		resp.Body.Close()
		if resp.StatusCode != 200 {
			t.Errorf("brightness %d: expected 200, got %d", level, resp.StatusCode)
		}
	}
}

func TestBrightness_OutOfRange(t *testing.T) {
	_, base := newTestServer(t)

	for _, level := range []int{-1, 101} {
		resp := postJSON(t, base+"/api/device/brightness", map[string]int{"brightness": level})
		resp.Body.Close()
		if resp.StatusCode != 400 {
			t.Errorf("brightness %d: expected 400, got %d", level, resp.StatusCode)
		}
	}
}

// ── Module config ─────────────────────────────────────────────────────────────

func TestPluginConfig_GetEmpty(t *testing.T) {
	_, base := newTestServer(t)

	resp := get(t, base+"/api/plugins/cpu-temp/config")
	if resp.StatusCode != 200 {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}

	var body map[string]any
	decodeJSON(t, resp, &body)

	if _, ok := body["plugin"]; !ok {
		t.Error("missing 'plugin' field in response")
	}
	if _, ok := body["config"]; !ok {
		t.Error("missing 'config' field in response")
	}
}

func TestPluginConfig_SetAndGet(t *testing.T) {
	_, base := newTestServer(t)

	payload := map[string]any{"unit": "metric", "graph": "sparkline"}
	resp := postJSON(t, base+"/api/plugins/cpu-temp/config", payload)
	resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Fatalf("POST plugin config: expected 200, got %d", resp.StatusCode)
	}

	var got map[string]any
	decodeJSON(t, get(t, base+"/api/plugins/cpu-temp/config"), &got)

	cfg, _ := got["config"].(map[string]any)
	if cfg["unit"] != "metric" {
		t.Errorf("unit: want 'metric', got %v", cfg["unit"])
	}
	if cfg["graph"] != "sparkline" {
		t.Errorf("graph: want 'sparkline', got %v", cfg["graph"])
	}
}

// ── Zone config ───────────────────────────────────────────────────────────────

func TestZoneConfig_SetGetDelete(t *testing.T) {
	_, base := newTestServer(t)

	// Set override
	payload := map[string]any{"color": "#FF0000", "enabled": true}
	resp := postJSON(t, base+"/api/zones/zone-1/config", payload)
	resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Fatalf("POST zone config: expected 200, got %d", resp.StatusCode)
	}

	// Get it back
	var got map[string]any
	decodeJSON(t, get(t, base+"/api/zones/zone-1/config"), &got)
	cfg, _ := got["config"].(map[string]any)
	if cfg["color"] != "#FF0000" {
		t.Errorf("color: want #FF0000, got %v", cfg["color"])
	}

	// Delete override
	req, _ := http.NewRequest("DELETE", base+"/api/zones/zone-1/config", nil)
	delResp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	delResp.Body.Close()
	if delResp.StatusCode != 200 {
		t.Errorf("DELETE zone config: expected 200, got %d", delResp.StatusCode)
	}
}

// ── Zone status ───────────────────────────────────────────────────────────────

func TestZoneStatus_ReturnsShape(t *testing.T) {
	_, base := newTestServer(t)

	resp := get(t, base+"/api/zones/zone-1/status")
	if resp.StatusCode != 200 {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}

	var body map[string]any
	decodeJSON(t, resp, &body)

	if _, ok := body["status"]; !ok {
		t.Error("missing 'status' field")
	}
}

// ── Window state ──────────────────────────────────────────────────────────────

func TestWindowState_ShowHide(t *testing.T) {
	_, base := newTestServer(t)

	// Initial state
	var state map[string]any
	decodeJSON(t, get(t, base+"/api/window/state"), &state)
	if state["state"] != "shown" {
		t.Errorf("initial state: want 'shown', got %v", state["state"])
	}

	// Hide
	resp := postJSON(t, base+"/api/window/hide", nil)
	resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Errorf("POST /hide: expected 200, got %d", resp.StatusCode)
	}

	decodeJSON(t, get(t, base+"/api/window/state"), &state)
	if state["state"] != "hidden" {
		t.Errorf("after hide: want 'hidden', got %v", state["state"])
	}

	// Show
	resp = postJSON(t, base+"/api/window/show", nil)
	resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Errorf("POST /show: expected 200, got %d", resp.StatusCode)
	}

	decodeJSON(t, get(t, base+"/api/window/state"), &state)
	if state["state"] != "shown" {
		t.Errorf("after show: want 'shown', got %v", state["state"])
	}
}

// ── Images ────────────────────────────────────────────────────────────────────

func TestImages_ListEmpty(t *testing.T) {
	_, base := newTestServer(t)

	resp := get(t, base+"/api/images")
	if resp.StatusCode != 200 {
		t.Fatalf("expected 200, got %d", resp.StatusCode)
	}

	var images []any
	decodeJSON(t, resp, &images)
	if images == nil {
		t.Error("expected [] not null — Flutter's (List<dynamic>?) cast breaks on null")
	}
}

// ── WebSocket ─────────────────────────────────────────────────────────────────

func TestWebSocket_UpgradeSucceeds(t *testing.T) {
	_, base := newTestServer(t)
	wsURL := "ws" + base[4:] + "/api/ws"

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	conn, _, err := websocket.Dial(ctx, wsURL, &websocket.DialOptions{
		// No Origin header — matches Flutter desktop behaviour.
	})
	if err != nil {
		t.Fatalf("WebSocket dial failed: %v", err)
	}
	defer conn.CloseNow()
}

func TestWebSocket_ReceivesInitialWindowState(t *testing.T) {
	_, base := newTestServer(t)
	wsURL := "ws" + base[4:] + "/api/ws"

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	conn, _, err := websocket.Dial(ctx, wsURL, nil)
	if err != nil {
		t.Fatalf("WebSocket dial: %v", err)
	}
	defer conn.CloseNow()

	// The server sends a window_state message immediately on connect.
	var msg map[string]any
	if err := wsjson.Read(ctx, conn, &msg); err != nil {
		t.Fatalf("read first message: %v", err)
	}

	if msg["type"] != "window_state" {
		t.Errorf("type: want 'window_state', got %v", msg["type"])
	}
	if msg["data"] != "shown" {
		t.Errorf("data: want 'shown', got %v", msg["data"])
	}
}

func TestWebSocket_BroadcastsWindowStateChange(t *testing.T) {
	srv, base := newTestServer(t)
	wsURL := "ws" + base[4:] + "/api/ws"

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	conn, _, err := websocket.Dial(ctx, wsURL, nil)
	if err != nil {
		t.Fatalf("dial: %v", err)
	}
	defer conn.CloseNow()

	// Drain the initial window_state message.
	var initial map[string]any
	if err := wsjson.Read(ctx, conn, &initial); err != nil {
		t.Fatalf("read initial: %v", err)
	}

	// Trigger a state change via the REST API.
	resp := postJSON(t, base+"/api/window/hide", nil)
	resp.Body.Close()

	// Expect a window_state broadcast on the WebSocket.
	var msg map[string]any
	if err := wsjson.Read(ctx, conn, &msg); err != nil {
		t.Fatalf("read broadcast: %v", err)
	}

	if msg["type"] != "window_state" {
		t.Errorf("type: want 'window_state', got %v", msg["type"])
	}
	if msg["data"] != "hidden" {
		t.Errorf("data: want 'hidden', got %v", msg["data"])
	}

	// Suppress unused variable warning.
	_ = srv
}

func TestWebSocket_MultipleClients(t *testing.T) {
	_, base := newTestServer(t)
	wsURL := "ws" + base[4:] + "/api/ws"

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	// Connect two clients simultaneously.
	conn1, _, err := websocket.Dial(ctx, wsURL, nil)
	if err != nil {
		t.Fatalf("dial conn1: %v", err)
	}
	defer conn1.CloseNow()

	conn2, _, err := websocket.Dial(ctx, wsURL, nil)
	if err != nil {
		t.Fatalf("dial conn2: %v", err)
	}
	defer conn2.CloseNow()

	// Drain initial messages from both.
	var m map[string]any
	wsjson.Read(ctx, conn1, &m)
	wsjson.Read(ctx, conn2, &m)

	// Trigger a broadcast.
	postJSON(t, base+"/api/window/show", nil).Body.Close()

	// Both clients should receive the broadcast.
	var msg1, msg2 map[string]any
	if err := wsjson.Read(ctx, conn1, &msg1); err != nil {
		t.Errorf("conn1: read broadcast: %v", err)
	}
	if err := wsjson.Read(ctx, conn2, &msg2); err != nil {
		t.Errorf("conn2: read broadcast: %v", err)
	}

	if msg1["type"] != "window_state" {
		t.Errorf("conn1 type: want 'window_state', got %v", msg1["type"])
	}
	if msg2["type"] != "window_state" {
		t.Errorf("conn2 type: want 'window_state', got %v", msg2["type"])
	}
}

// ── httptest.Server variant ───────────────────────────────────────────────────
// These tests use httptest.NewServer for handler-level checks that don't need
// the full server lifecycle (e.g. checking response body structure for error cases).

func newHandlerServer(t *testing.T) (*api.Server, *httptest.Server) {
	t.Helper()
	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))
	s, err := store.Open(t.TempDir()+"/test.db", logger)
	if err != nil {
		t.Fatalf("store.Open: %v", err)
	}
	t.Cleanup(func() { s.Close() })
	cfg, err := config.NewManager(s, logger)
	if err != nil {
		t.Fatalf("config.NewManager: %v", err)
	}
	mockDev := device.NewMockDevice()
	mockDev.Connect(context.Background()) //nolint:errcheck
	t.Cleanup(func() { mockDev.Disconnect() })
	srv := api.NewServer(":0", cfg, mockDev, logger)
	return srv, nil
}

func TestErrorResponse_HasExpectedShape(t *testing.T) {
	_, base := newTestServer(t)

	// POST to a GET-only endpoint to trigger a 405.
	resp, err := http.Post(base+"/api/health", "application/json", nil)
	if err != nil {
		t.Fatal(err)
	}
	if resp.StatusCode != 405 {
		resp.Body.Close()
		t.Fatalf("expected 405, got %d", resp.StatusCode)
	}

	var body map[string]any
	decodeJSON(t, resp, &body)

	// Error responses must have "error" and optionally "message".
	if _, ok := body["error"]; !ok {
		t.Error("error response missing 'error' field")
	}
}
