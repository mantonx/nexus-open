package fonts

import (
	"io"
	"log/slog"
	"sync"
	"testing"

	"github.com/golang/freetype/truetype"
)

func newTestManager() *Manager {
	return &Manager{
		logger: slog.New(slog.NewTextHandler(io.Discard, nil)),
		fonts:  make(map[string]*truetype.Font),
	}
}

func TestGetFaceReturnsFreshFaceEachCall(t *testing.T) {
	m := newTestManager()
	f1, err := m.GetFace("GoRegular", 12)
	if err != nil {
		t.Fatalf("GetFace: %v", err)
	}
	f2, err := m.GetFace("GoRegular", 12)
	if err != nil {
		t.Fatalf("GetFace: %v", err)
	}
	if f1 == f2 {
		t.Fatal("expected distinct face instances to prevent GlyphBuf races")
	}
}

func TestLoadFontCachesUnderlyingFont(t *testing.T) {
	m := newTestManager()
	f1, err := m.LoadFont("GoRegular")
	if err != nil {
		t.Fatalf("LoadFont: %v", err)
	}
	f2, err := m.LoadFont("GoRegular")
	if err != nil {
		t.Fatalf("LoadFont: %v", err)
	}
	if f1 != f2 {
		t.Fatal("expected same *truetype.Font pointer (parse-once semantics)")
	}
}

func TestGetFaceConcurrentlySafe(t *testing.T) {
	m := newTestManager()
	const goroutines = 20
	errs := make(chan error, goroutines)
	var wg sync.WaitGroup
	wg.Add(goroutines)
	for i := 0; i < goroutines; i++ {
		go func() {
			defer wg.Done()
			face, err := m.GetFace("GoRegular", 12)
			if err != nil {
				errs <- err
				return
			}
			// Exercise the face to catch GlyphBuf races under -race.
			face.GlyphAdvance('A')
		}()
	}
	wg.Wait()
	close(errs)
	for err := range errs {
		t.Errorf("concurrent GetFace error: %v", err)
	}
}

func TestLoadBestAvailableFontSucceeds(t *testing.T) {
	m := newTestManager()
	face, name, err := m.LoadBestAvailableFont(12)
	if err != nil {
		t.Fatalf("LoadBestAvailableFont: %v", err)
	}
	if face == nil {
		t.Fatal("expected non-nil face")
	}
	if name == "" {
		t.Fatal("expected non-empty font name")
	}
}
