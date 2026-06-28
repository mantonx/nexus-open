package main

import (
	"image"
	"net/http"
	"net/http/httptest"
	"sync/atomic"
	"testing"
)

func resetArtCache() {
	artCache.Lock()
	artCache.entries = make(map[string]image.Image)
	artCache.Unlock()
}

func TestArtCache_HitAvoidsFetch(t *testing.T) {
	var calls atomic.Int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		// Return a valid 1×1 PNG.
		w.Header().Set("Content-Type", "image/png")
		w.Write(minimalPNG())
	}))
	defer srv.Close()

	resetArtCache()

	fetchArt(srv.URL, 48) //nolint:errcheck
	fetchArt(srv.URL, 48) //nolint:errcheck

	if n := calls.Load(); n != 1 {
		t.Fatalf("expected 1 HTTP fetch, got %d", n)
	}
}

func TestArtCache_MissCached(t *testing.T) {
	var calls atomic.Int32
	srv := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		calls.Add(1)
		w.WriteHeader(http.StatusNotFound)
	}))
	defer srv.Close()

	resetArtCache()

	img1, _ := fetchArt(srv.URL, 48)
	img2, _ := fetchArt(srv.URL, 48)

	if img1 != nil || img2 != nil {
		t.Fatal("expected nil image for 404")
	}
	if n := calls.Load(); n != 1 {
		t.Fatalf("expected 1 HTTP fetch (miss cached), got %d", n)
	}
}

func TestArtCache_FileURL(t *testing.T) {
	resetArtCache()

	// A file:// URL that doesn't exist should return nil without panicking.
	img, err := fetchArt("file:///nonexistent/art.png", 48)
	if err != nil {
		t.Fatalf("unexpected error: %v", err)
	}
	if img != nil {
		t.Fatal("expected nil for missing file")
	}
	// Second call should use cache, not attempt to open the file again.
	artCache.Lock()
	_, cached := artCache.entries["file:///nonexistent/art.png"]
	artCache.Unlock()
	if !cached {
		t.Fatal("expected miss to be cached")
	}
}

// minimalPNG returns a valid 1×1 white PNG.
func minimalPNG() []byte {
	return []byte{
		0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
		0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR length + type
		0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // 1×1
		0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // 8-bit RGB, CRC
		0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, // IDAT length + type
		0x54, 0x08, 0xd7, 0x63, 0xf8, 0xcf, 0xc0, 0x00, // compressed pixel
		0x00, 0x00, 0x02, 0x00, 0x01, 0xe2, 0x21, 0xbc, // CRC
		0x33, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, // IEND length + type
		0x44, 0xae, 0x42, 0x60, 0x82, // IEND CRC
	}
}
