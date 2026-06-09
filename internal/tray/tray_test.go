//go:build !notray

package tray

import (
	"io"
	"log/slog"
	"net/http"
	"net/http/httptest"
	"os/exec"
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
