package main

import (
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"sync/atomic"
	"testing"
	"time"
)

// ── Token XOR ────────────────────────────────────────────────────────────────

func TestBundledTMDbToken_Empty(t *testing.T) {
	orig := bundledTMDbTokenHex
	bundledTMDbTokenHex = ""
	defer func() { bundledTMDbTokenHex = orig }()

	if got := bundledTMDbToken(); got != "" {
		t.Fatalf("expected empty, got %q", got)
	}
}

func TestBundledTMDbToken_RoundTrip(t *testing.T) {
	want := "eyJhbGciOiJIUzI1NiJ9.testtoken12345"

	key := []byte("NXOR")
	encoded := ""
	for i, b := range []byte(want) {
		xb := b ^ key[i%len(key)]
		encoded += fmt.Sprintf("%02x", xb)
	}

	orig := bundledTMDbTokenHex
	bundledTMDbTokenHex = encoded
	defer func() { bundledTMDbTokenHex = orig }()

	if got := bundledTMDbToken(); got != want {
		t.Fatalf("want %q, got %q", want, got)
	}
}

// ── TMDb cache ────────────────────────────────────────────────────────────────

func resetTMDbCache() {
	tmdb.mu.Lock()
	tmdb.entries = make(map[string]tmdbEntry)
	tmdb.backoff = 0
	tmdb.backoffUntil = time.Time{}
	tmdb.mu.Unlock()
}

func TestTMDbCache_PositiveHit(t *testing.T) {
	var calls atomic.Int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		json.NewEncoder(w).Encode(map[string]any{
			"results": []map[string]any{{"poster_path": "/abc.jpg"}},
		})
	}))
	defer srv.Close()

	resetTMDbCache()
	origURL := tmdbSearchURL
	tmdbSearchURL = srv.URL
	defer func() { tmdbSearchURL = origURL }()

	got1 := tmdb.posterURL("Inception")
	got2 := tmdb.posterURL("Inception")

	if got1 == "" {
		t.Fatal("expected poster URL, got empty")
	}
	if got1 != got2 {
		t.Fatalf("second call returned different value: %q vs %q", got1, got2)
	}
	if n := calls.Load(); n != 1 {
		t.Fatalf("expected 1 HTTP call, got %d", n)
	}
}

func TestTMDbCache_NegativeHit(t *testing.T) {
	var calls atomic.Int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		json.NewEncoder(w).Encode(map[string]any{"results": []any{}})
	}))
	defer srv.Close()

	resetTMDbCache()
	origURL := tmdbSearchURL
	tmdbSearchURL = srv.URL
	defer func() { tmdbSearchURL = origURL }()

	got1 := tmdb.posterURL("xyzzy-no-such-title")
	got2 := tmdb.posterURL("xyzzy-no-such-title")

	if got1 != "" {
		t.Fatalf("expected empty for miss, got %q", got1)
	}
	if got2 != "" {
		t.Fatalf("expected cached empty for miss, got %q", got2)
	}
	if n := calls.Load(); n != 1 {
		t.Fatalf("expected 1 HTTP call (negative cached), got %d", n)
	}
}

func TestTMDbCache_BackoffOnError(t *testing.T) {
	var calls atomic.Int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		w.WriteHeader(http.StatusInternalServerError)
	}))
	defer srv.Close()

	resetTMDbCache()
	origURL := tmdbSearchURL
	tmdbSearchURL = srv.URL
	defer func() { tmdbSearchURL = origURL }()

	tmdb.posterURL("Dune")
	tmdb.posterURL("Dune") // should be blocked by backoff, not a new request

	if n := calls.Load(); n != 1 {
		t.Fatalf("expected 1 HTTP call (backoff after error), got %d", n)
	}

	tmdb.mu.Lock()
	bo := tmdb.backoff
	tmdb.mu.Unlock()
	if bo == 0 {
		t.Fatal("expected non-zero backoff after error")
	}
}

func TestTMDbCache_BackoffResetsOnSuccess(t *testing.T) {
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		json.NewEncoder(w).Encode(map[string]any{
			"results": []map[string]any{{"poster_path": "/ok.jpg"}},
		})
	}))
	defer srv.Close()

	resetTMDbCache()
	// Seed a non-zero backoff that has already expired.
	tmdb.mu.Lock()
	tmdb.backoff = 5 * time.Second
	tmdb.backoffUntil = time.Now().Add(-time.Second) // expired
	tmdb.mu.Unlock()

	origURL := tmdbSearchURL
	tmdbSearchURL = srv.URL
	defer func() { tmdbSearchURL = origURL }()

	got := tmdb.posterURL("Interstellar")
	if got == "" {
		t.Fatal("expected poster URL after backoff expired")
	}

	tmdb.mu.Lock()
	bo := tmdb.backoff
	tmdb.mu.Unlock()
	if bo != 0 {
		t.Fatalf("expected backoff reset to 0 after success, got %v", bo)
	}
}

func TestTMDbCache_RateLimitIsTransient(t *testing.T) {
	var calls atomic.Int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		w.WriteHeader(http.StatusTooManyRequests)
	}))
	defer srv.Close()

	resetTMDbCache()
	origURL := tmdbSearchURL
	tmdbSearchURL = srv.URL
	defer func() { tmdbSearchURL = origURL }()

	got := tmdb.posterURL("Avatar")
	if got != "" {
		t.Fatalf("expected empty on 429, got %q", got)
	}

	tmdb.mu.Lock()
	bo := tmdb.backoff
	tmdb.mu.Unlock()
	if bo == 0 {
		t.Fatal("expected backoff set after 429")
	}
	// Second call blocked by backoff — only one real request.
	tmdb.posterURL("Avatar")
	if n := calls.Load(); n != 1 {
		t.Fatalf("expected 1 HTTP call after 429 backoff, got %d", n)
	}
}
