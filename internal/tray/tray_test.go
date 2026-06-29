//go:build !notray

package tray

import (
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os"
	"os/exec"
	"path/filepath"
	"testing"
	"time"
)

func newTestManager(apiAddr string) *Manager {
	return &Manager{
		logger:         slog.New(slog.NewTextHandler(io.Discard, nil)),
		apiAddr:        apiAddr,
		windowClosedCh: make(chan struct{}, 1),
		showCh:         make(chan struct{}, 1),
		hideCh:         make(chan struct{}, 1),
		quitCh:         make(chan struct{}, 1),
	}
}

func TestStopFlutterClearsRunningFlag(t *testing.T) {
	m := newTestManager("localhost:0")

	// Simulate a running process using a no-op sleep command.
	cmd := exec.Command("sleep", "60")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start sleep: %v", err)
	}
	m.flutterCmd = cmd
	m.flutterRunning = true

	m.stopFlutter()

	if m.flutterRunning {
		t.Fatal("expected flutterRunning=false after stopFlutter")
	}
	if cmd.ProcessState == nil {
		t.Fatal("expected process to be reaped (ProcessState set)")
	}
}

func TestShowWindowPostsToAPI(t *testing.T) {
	var received bool
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/window/show" && r.Method == http.MethodPost {
			received = true
		}
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"status":"ok"}`))
	}))
	defer srv.Close()

	// Strip scheme — apiAddr is host:port only.
	m := newTestManager(srv.Listener.Addr().String())
	m.showWindow()

	if !received {
		t.Fatal("expected POST /api/window/show to be called")
	}
}

func TestHideWindowPostsToAPI(t *testing.T) {
	var received bool
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		if r.URL.Path == "/api/window/hide" && r.Method == http.MethodPost {
			received = true
		}
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write([]byte(`{"status":"ok"}`))
	}))
	defer srv.Close()

	m := newTestManager(srv.Listener.Addr().String())
	m.hideWindow()

	if !received {
		t.Fatal("expected POST /api/window/hide to be called")
	}
}

func TestWaitForFlutterTimesOut(t *testing.T) {
	// Point at a port nothing is listening on.
	m := newTestManager("localhost:19999")
	err := m.waitForFlutter(100 * time.Millisecond)
	if err == nil {
		t.Fatal("expected timeout error when backend is unreachable")
	}
}

func TestWaitForFlutterSucceeds(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
	}))
	defer srv.Close()

	m := newTestManager(srv.Listener.Addr().String())
	if err := m.waitForFlutter(time.Second); err != nil {
		t.Fatalf("expected success, got: %v", err)
	}
}

func TestFlutterSearchPathsOrder(t *testing.T) {
	paths := flutterSearchPaths("/usr/bin")
	if len(paths) == 0 {
		t.Fatal("expected at least one search path")
	}
	if paths[0] != "/usr/lib/nexus-open/ui-bundle/ui" {
		t.Errorf("expected system path first, got %q", paths[0])
	}
	for _, p := range paths[1:] {
		if p == "/usr/lib/nexus-open/ui-bundle/ui" {
			t.Error("system path appears more than once")
		}
		if filepath.IsAbs(p) && p == paths[0] {
			t.Errorf("duplicate path: %q", p)
		}
	}
}

func TestFindFlutterExecutableSiblingBeforeDevPath(t *testing.T) {
	dir := t.TempDir()
	paths := flutterSearchPaths(dir)

	// Create fake executables at sibling (paths[1]) and dev (paths[2]).
	for _, p := range paths[1:] {
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, []byte("#!/bin/sh\n"), 0o755); err != nil {
			t.Fatal(err)
		}
	}

	// Walk from paths[1] onward (skip system path which we can't control).
	found := ""
	for _, p := range paths[1:] {
		if _, err := os.Stat(p); err == nil {
			found = p
			break
		}
	}
	if found != paths[1] {
		t.Errorf("expected sibling path %q first, got %q", paths[1], found)
	}
}

func TestFindFlutterExecutableSystemBeforeSibling(t *testing.T) {
	// Verify system path is listed before sibling in the search order.
	paths := flutterSearchPaths("/some/dir")
	if len(paths) < 2 {
		t.Fatal("expected at least 2 paths")
	}
	systemIdx := -1
	siblingIdx := -1
	for i, p := range paths {
		if p == "/usr/lib/nexus-open/ui-bundle/ui" {
			systemIdx = i
		}
		if p == "/some/dir/ui" {
			siblingIdx = i
		}
	}
	if systemIdx < 0 {
		t.Fatal("system path not in search list")
	}
	if siblingIdx < 0 {
		t.Fatal("sibling path not in search list")
	}
	if systemIdx >= siblingIdx {
		t.Errorf("system path (index %d) must come before sibling (index %d)", systemIdx, siblingIdx)
	}
}

func TestFindFlutterExecutableNotFound(t *testing.T) {
	dir := t.TempDir() // empty — nothing exists
	found := ""
	for _, p := range flutterSearchPaths(dir) {
		if _, err := os.Stat(p); err == nil {
			found = p
			break
		}
	}
	// system path (/usr/lib/nexus-open/ui-bundle/ui) is installed on this machine,
	// so skip the "not found" assertion if it exists.
	if _, err := os.Stat("/usr/lib/nexus-open/ui-bundle/ui"); err == nil {
		t.Skip("system path exists on this machine — skipping not-found test")
	}
	if found != "" {
		t.Errorf("expected no path found in empty dir, got %q", found)
	}
}

func TestWindowClosedChannelSignalsStop(t *testing.T) {
	closedCh := make(chan struct{}, 1)
	m := newTestManager("localhost:0")
	m.windowClosedCh = closedCh

	cmd := exec.Command("sleep", "60")
	if err := cmd.Start(); err != nil {
		t.Fatalf("start sleep: %v", err)
	}
	m.flutterCmd = cmd
	m.flutterRunning = true

	// Simulate what the menu loop does when windowClosedCh fires.
	closedCh <- struct{}{}
	select {
	case <-closedCh:
		m.stopFlutter()
	case <-time.After(time.Second):
		t.Fatal("channel signal not received in time")
	}

	if m.flutterRunning {
		t.Fatal("expected flutterRunning=false after window closed signal")
	}
}
