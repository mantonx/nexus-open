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
	"os"
	"testing"
	"time"


	"github.com/mantonx/nexus-open/internal/api"
	"github.com/mantonx/nexus-open/internal/device"
	config "github.com/mantonx/nexus-open/internal/settings"
	"github.com/mantonx/nexus-open/internal/store"
	"github.com/mantonx/nexus-open/internal/zone"
	"log/slog"
	"github.com/coder/websocket"
	"github.com/coder/websocket/wsjson"
)

// ── Helpers ───────────────────────────────────────────────────────────────────

// testEnv bundles the server, its base URL, and a pre-authorised HTTP client.
type testEnv struct {
	srv    *api.Server
	base   string
	client *http.Client
	token  string
}

// newTestServer creates a Server bound to a random loopback port.
func newTestServer(t *testing.T) (*api.Server, string) {
	env := newTestEnv(t, nil)
	return env.srv, env.base
}

// newTestEnv is the single construction path. If dbOut is non-nil, it receives
// the opened store so callers can seed test data.
func newTestEnv(t *testing.T, dbOut **store.DB) testEnv {
	t.Helper()

	logger := slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: slog.LevelError}))

	tmpDir := t.TempDir()

	s, err := store.Open(tmpDir+"/test.db", logger)
	if err != nil {
		t.Fatalf("store.Open: %v", err)
	}
	t.Cleanup(func() { _ = s.Close() })
	if dbOut != nil {
		*dbOut = s
	}

	cfg, err := config.NewManager(s, logger)
	if err != nil {
		t.Fatalf("config.NewManager: %v", err)
	}

	mockDev := device.NewMockDevice()
	if err := mockDev.Connect(context.Background()); err != nil {
		t.Fatalf("mock device connect: %v", err)
	}
	t.Cleanup(func() { _ = mockDev.Disconnect() })

	zoneCfg := zone.NewConfigManager(s, logger)

	// Bind on :0 so the OS assigns a free port.
	ln, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatalf("net.Listen: %v", err)
	}
	port := ln.Addr().(*net.TCPAddr).Port
	_ = ln.Close()

	srv := api.NewServer(fmt.Sprintf("127.0.0.1:%d", port), cfg, mockDev, logger)
	srv.SetZoneConfigManager(zoneCfg)
	srv.SetLayoutStore(s)

	go func() {
		_ = srv.Start()
	}()

	baseURL := fmt.Sprintf("http://127.0.0.1:%d", port)

	// Wait until the server accepts connections (health is token-exempt).
	deadline := time.Now().Add(3 * time.Second)
	for time.Now().Before(deadline) {
		resp, err := http.Get(baseURL + "/api/health")
		if err == nil {
			_ = resp.Body.Close()
			break
		}
		time.Sleep(20 * time.Millisecond)
	}

	t.Cleanup(func() {
		ctx, cancel := context.WithTimeout(context.Background(), 2*time.Second)
		defer cancel()
		_ = srv.Shutdown(ctx)
	})

	token := srv.Token()
	client := &http.Client{
		Transport: &tokenTransport{token: token, inner: http.DefaultTransport},
	}
	return testEnv{srv: srv, base: baseURL, client: client, token: token}
}

// tokenTransport injects X-Nexus-Token on every request.
type tokenTransport struct {
	token string
	inner http.RoundTripper
}

func (t *tokenTransport) RoundTrip(req *http.Request) (*http.Response, error) {
	clone := req.Clone(req.Context())
	if clone.Header == nil {
		clone.Header = make(http.Header)
	}
	clone.Header.Set("X-Nexus-Token", t.token)
	return t.inner.RoundTrip(clone)
}

func (env testEnv) get(t *testing.T, url string) *http.Response {
	t.Helper()
	resp, err := env.client.Get(url)
	if err != nil {
		t.Fatalf("GET %s: %v", url, err)
	}
	return resp
}

func (env testEnv) postJSON(t *testing.T, url string, body any) *http.Response {
	t.Helper()
	data, err := json.Marshal(body)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	req, err := http.NewRequest(http.MethodPost, url, bytes.NewReader(data))
	if err != nil {
		t.Fatalf("new request: %v", err)
	}
	req.Header.Set("Content-Type", "application/json")
	resp, err := env.client.Do(req)
	if err != nil {
		t.Fatalf("POST %s: %v", url, err)
	}
	return resp
}

func (env testEnv) do(t *testing.T, method, url string) *http.Response {
	t.Helper()
	req, err := http.NewRequest(method, url, nil)
	if err != nil {
		t.Fatalf("new request: %v", err)
	}
	resp, err := env.client.Do(req)
	if err != nil {
		t.Fatalf("%s %s: %v", method, url, err)
	}
	return resp
}

// wsURL converts an http base URL to a ws URL and appends the token.
func (env testEnv) wsURL(base string) string {
	return "ws" + base[4:] + "/api/ws?token=" + env.token
}

// decodeJSON is a standalone helper (no client needed).
func decodeJSON(t *testing.T, resp *http.Response, dst any) {
	t.Helper()
	defer func() { _ = resp.Body.Close() }()
	if err := json.NewDecoder(resp.Body).Decode(dst); err != nil {
		t.Fatalf("decode JSON: %v", err)
	}
}

// Legacy package-level helpers used by a handful of tests that send requests
// the middleware purposefully blocks — they need to bypass token injection.
func get(t *testing.T, url string) *http.Response {
	t.Helper()
	resp, err := http.Get(url)
	if err != nil {
		t.Fatalf("GET %s: %v", url, err)
	}
	return resp
}

// ── Health ────────────────────────────────────────────────────────────────────

func TestHealth_OK(t *testing.T) {
	// /api/health is token-exempt; use the plain package-level get.
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
	// No token needed — health is exempt and 405 fires before handler logic.
	_, base := newTestServer(t)

	resp, err := http.Post(base+"/api/health", "application/json", nil)
	if err != nil {
		t.Fatal(err)
	}
	_ = resp.Body.Close()
	if resp.StatusCode != 405 {
		t.Errorf("expected 405, got %d", resp.StatusCode)
	}
}

// ── Config ────────────────────────────────────────────────────────────────────

func TestConfig_GetReturnsExpectedFields(t *testing.T) {
	env := newTestEnv(t, nil)

	resp := env.get(t, env.base+"/api/config")
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
	env := newTestEnv(t, nil)

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

	resp := env.postJSON(t, env.base+"/api/config", update)
	if resp.StatusCode != 200 {
		_ = resp.Body.Close()
		t.Fatalf("POST /api/config: expected 200, got %d", resp.StatusCode)
	}
	_ = resp.Body.Close()

	var got map[string]any
	decodeJSON(t, env.get(t, env.base+"/api/config"), &got)

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
	env := newTestEnv(t, nil)

	update := map[string]any{
		"background_color": "not-a-color",
		"text_color":       "#FFFFFF",
	}
	resp := env.postJSON(t, env.base+"/api/config", update)
	_ = resp.Body.Close()
	if resp.StatusCode != 400 {
		t.Errorf("expected 400 for invalid color, got %d", resp.StatusCode)
	}
}

func TestConfig_InvalidJSONRejected(t *testing.T) {
	env := newTestEnv(t, nil)

	req, _ := http.NewRequest(http.MethodPost, env.base+"/api/config", bytes.NewBufferString("{bad json"))
	req.Header.Set("Content-Type", "application/json")
	resp, err := env.client.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	_ = resp.Body.Close()
	if resp.StatusCode != 400 {
		t.Errorf("expected 400 for invalid JSON, got %d", resp.StatusCode)
	}
}

// ── Security middleware ───────────────────────────────────────────────────────

func TestSecurity_MissingTokenRejected(t *testing.T) {
	_, base := newTestServer(t)

	// No token header — must get 401 on a protected endpoint.
	resp, err := http.Get(base + "/api/config")
	if err != nil {
		t.Fatal(err)
	}
	_ = resp.Body.Close()
	if resp.StatusCode != 401 {
		t.Errorf("missing token: expected 401, got %d", resp.StatusCode)
	}
}

func TestSecurity_WrongTokenRejected(t *testing.T) {
	_, base := newTestServer(t)

	req, _ := http.NewRequest(http.MethodGet, base+"/api/config", nil)
	req.Header.Set("X-Nexus-Token", "wrong-token-value")
	resp, err := http.DefaultClient.Do(req)
	if err != nil {
		t.Fatal(err)
	}
	_ = resp.Body.Close()
	if resp.StatusCode != 401 {
		t.Errorf("wrong token: expected 401, got %d", resp.StatusCode)
	}
}

func TestSecurity_HealthExemptFromToken(t *testing.T) {
	_, base := newTestServer(t)

	// /api/health must be reachable without a token (used by waitForFlutter).
	resp, err := http.Get(base + "/api/health")
	if err != nil {
		t.Fatal(err)
	}
	_ = resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Errorf("health without token: expected 200, got %d", resp.StatusCode)
	}
}

// ── Device info ───────────────────────────────────────────────────────────────

func TestDeviceInfo_MockConnected(t *testing.T) {
	env := newTestEnv(t, nil)

	resp := env.get(t, env.base+"/api/device/info")
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
	env := newTestEnv(t, nil)

	for _, level := range []int{0, 50, 100} {
		resp := env.postJSON(t, env.base+"/api/device/brightness", map[string]int{"brightness": level})
		_ = resp.Body.Close()
		if resp.StatusCode != 200 {
			t.Errorf("brightness %d: expected 200, got %d", level, resp.StatusCode)
		}
	}
}

func TestBrightness_OutOfRange(t *testing.T) {
	env := newTestEnv(t, nil)

	for _, level := range []int{-1, 101} {
		resp := env.postJSON(t, env.base+"/api/device/brightness", map[string]int{"brightness": level})
		_ = resp.Body.Close()
		if resp.StatusCode != 400 {
			t.Errorf("brightness %d: expected 400, got %d", level, resp.StatusCode)
		}
	}
}

// ── Zone config ───────────────────────────────────────────────────────────────

func TestZoneConfig_SetGetDelete(t *testing.T) {
	var db *store.DB
	env := newTestEnv(t, &db)

	// Seed a real zone row so SetZonePluginConfig (UPDATE) can find it.
	pageID, err := db.CreatePage("Test", 0)
	if err != nil {
		t.Fatalf("CreatePage: %v", err)
	}
	if err := db.CreateZone(store.StoredZone{
		ID: "zone-1", PageID: pageID, Ord: 0, WidthPx: 640,
		Plugin: "builtin:clock", RefreshMs: 1000, Align: "center",
	}); err != nil {
		t.Fatalf("CreateZone: %v", err)
	}

	payload := map[string]any{"color": "#FF0000", "enabled": true}
	resp := env.postJSON(t, env.base+"/api/zones/zone-1/config", payload)
	_ = resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Fatalf("POST zone config: expected 200, got %d", resp.StatusCode)
	}

	var got map[string]any
	decodeJSON(t, env.get(t, env.base+"/api/zones/zone-1/config"), &got)
	cfg, _ := got["config"].(map[string]any)
	if cfg["color"] != "#FF0000" {
		t.Errorf("color: want #FF0000, got %v", cfg["color"])
	}

	delResp := env.do(t, http.MethodDelete, env.base+"/api/zones/zone-1/config")
	_ = delResp.Body.Close()
	if delResp.StatusCode != 200 {
		t.Errorf("DELETE zone config: expected 200, got %d", delResp.StatusCode)
	}
}

// ── Zone cap + redistribution ─────────────────────────────────────────────────

func TestZoneCap_RejectsSeventhZone(t *testing.T) {
	var db *store.DB
	env := newTestEnv(t, &db)

	pageID, err := db.CreatePage("Test", 0)
	if err != nil {
		t.Fatalf("CreatePage: %v", err)
	}

	for i := range 6 {
		if err := db.CreateZone(store.StoredZone{
			ID: fmt.Sprintf("z%d", i), PageID: pageID, Ord: i,
			WidthPx: 640 / 6, Plugin: "builtin:clock", RefreshMs: 1000, Align: "center",
		}); err != nil {
			t.Fatalf("CreateZone %d: %v", i, err)
		}
	}

	resp := env.postJSON(t, env.base+"/api/layout/zones", map[string]any{
		"id": "z6", "page_id": pageID, "width_px": 80,
		"plugin": "builtin:clock", "refresh_ms": 1000, "align": "center",
	})
	_ = resp.Body.Close()
	if resp.StatusCode != 422 {
		t.Errorf("expected 422 for 7th zone, got %d", resp.StatusCode)
	}
}

func TestZoneRedistribute_AfterCreate(t *testing.T) {
	var db *store.DB
	env := newTestEnv(t, &db)

	pageID, err := db.CreatePage("Test", 0)
	if err != nil {
		t.Fatalf("CreatePage: %v", err)
	}

	if err := db.CreateZone(store.StoredZone{
		ID: "z0", PageID: pageID, Ord: 0, WidthPx: 640,
		Plugin: "builtin:clock", RefreshMs: 1000, Align: "center",
	}); err != nil {
		t.Fatalf("seed zone: %v", err)
	}

	resp := env.postJSON(t, env.base+"/api/layout/zones", map[string]any{
		"id": "z1", "page_id": pageID, "width_px": 80,
		"plugin": "builtin:clock", "refresh_ms": 1000, "align": "center",
	})
	_ = resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Fatalf("POST zone: expected 200, got %d", resp.StatusCode)
	}

	zones, _ := db.GetZonesForPage(pageID)
	total := 0
	for _, z := range zones {
		total += z.WidthPx
	}
	if total != 640 {
		t.Errorf("widths after add: want sum=640, got %d", total)
	}
	for _, z := range zones {
		if z.WidthPx != 320 {
			t.Errorf("zone %s: want width=320, got %d", z.ID, z.WidthPx)
		}
	}
}

func TestZoneRedistribute_AfterDelete(t *testing.T) {
	var db *store.DB
	env := newTestEnv(t, &db)

	pageID, err := db.CreatePage("Test", 0)
	if err != nil {
		t.Fatalf("CreatePage: %v", err)
	}

	for i := range 3 {
		if err := db.CreateZone(store.StoredZone{
			ID: fmt.Sprintf("z%d", i), PageID: pageID, Ord: i,
			WidthPx: 640 / 3, Plugin: "builtin:clock", RefreshMs: 1000, Align: "center",
		}); err != nil {
			t.Fatalf("seed zone %d: %v", i, err)
		}
	}

	resp := env.do(t, http.MethodDelete, env.base+"/api/layout/zones/z2")
	_ = resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Fatalf("DELETE zone: expected 200, got %d", resp.StatusCode)
	}

	zones, _ := db.GetZonesForPage(pageID)
	if len(zones) != 2 {
		t.Fatalf("want 2 zones after delete, got %d", len(zones))
	}
	total := 0
	for _, z := range zones {
		total += z.WidthPx
	}
	if total != 640 {
		t.Errorf("widths after delete: want sum=640, got %d", total)
	}
}

// ── Zone status ───────────────────────────────────────────────────────────────

func TestZoneStatus_ReturnsShape(t *testing.T) {
	env := newTestEnv(t, nil)

	resp := env.get(t, env.base+"/api/zones/zone-1/status")
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
	env := newTestEnv(t, nil)

	var state map[string]any
	decodeJSON(t, env.get(t, env.base+"/api/window/state"), &state)
	if state["state"] != "shown" {
		t.Errorf("initial state: want 'shown', got %v", state["state"])
	}

	resp := env.postJSON(t, env.base+"/api/window/hide", nil)
	_ = resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Errorf("POST /hide: expected 200, got %d", resp.StatusCode)
	}

	decodeJSON(t, env.get(t, env.base+"/api/window/state"), &state)
	if state["state"] != "hidden" {
		t.Errorf("after hide: want 'hidden', got %v", state["state"])
	}

	resp = env.postJSON(t, env.base+"/api/window/show", nil)
	_ = resp.Body.Close()
	if resp.StatusCode != 200 {
		t.Errorf("POST /show: expected 200, got %d", resp.StatusCode)
	}

	decodeJSON(t, env.get(t, env.base+"/api/window/state"), &state)
	if state["state"] != "shown" {
		t.Errorf("after show: want 'shown', got %v", state["state"])
	}
}

// ── Images ────────────────────────────────────────────────────────────────────

func TestImages_ListEmpty(t *testing.T) {
	env := newTestEnv(t, nil)

	resp := env.get(t, env.base+"/api/images")
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
	env := newTestEnv(t, nil)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	conn, _, err := websocket.Dial(ctx, env.wsURL(env.base), &websocket.DialOptions{
		// No Origin header — matches Flutter desktop behaviour.
	})
	if err != nil {
		t.Fatalf("WebSocket dial failed: %v", err)
	}
	defer func() { _ = conn.CloseNow() }()
}

func TestWebSocket_ReceivesInitialWindowState(t *testing.T) {
	env := newTestEnv(t, nil)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	conn, _, err := websocket.Dial(ctx, env.wsURL(env.base), nil)
	if err != nil {
		t.Fatalf("WebSocket dial: %v", err)
	}
	defer func() { _ = conn.CloseNow() }()

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
	env := newTestEnv(t, nil)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	conn, _, err := websocket.Dial(ctx, env.wsURL(env.base), nil)
	if err != nil {
		t.Fatalf("dial: %v", err)
	}
	defer func() { _ = conn.CloseNow() }()

	var initial map[string]any
	if err := wsjson.Read(ctx, conn, &initial); err != nil {
		t.Fatalf("read initial: %v", err)
	}

	resp := env.postJSON(t, env.base+"/api/window/hide", nil)
	_ = resp.Body.Close()

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
}

func TestWebSocket_MultipleClients(t *testing.T) {
	env := newTestEnv(t, nil)

	ctx, cancel := context.WithTimeout(context.Background(), 5*time.Second)
	defer cancel()

	conn1, _, err := websocket.Dial(ctx, env.wsURL(env.base), nil)
	if err != nil {
		t.Fatalf("dial conn1: %v", err)
	}
	defer func() { _ = conn1.CloseNow() }()

	conn2, _, err := websocket.Dial(ctx, env.wsURL(env.base), nil)
	if err != nil {
		t.Fatalf("dial conn2: %v", err)
	}
	defer func() { _ = conn2.CloseNow() }()

	var m map[string]any
	_ = wsjson.Read(ctx, conn1, &m)
	_ = wsjson.Read(ctx, conn2, &m)

	_ = env.postJSON(t, env.base+"/api/window/show", nil).Body.Close()

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

func TestErrorResponse_HasExpectedShape(t *testing.T) {
	// POST to /api/health (token-exempt, GET-only) to trigger 405.
	_, base := newTestServer(t)

	resp, err := http.Post(base+"/api/health", "application/json", nil)
	if err != nil {
		t.Fatal(err)
	}
	if resp.StatusCode != 405 {
		_ = resp.Body.Close()
		t.Fatalf("expected 405, got %d", resp.StatusCode)
	}

	var body map[string]any
	decodeJSON(t, resp, &body)

	if _, ok := body["error"]; !ok {
		t.Error("error response missing 'error' field")
	}
}
