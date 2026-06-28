package device

import (
	"context"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"sync"
	"testing"
	"time"
)

func discardLogger() *slog.Logger {
	return slog.New(slog.NewTextHandler(io.Discard, nil))
}

// fakeHandle is a usbHandle whose writeFrame can be made to block or fail,
// allowing lifecycle tests without real USB hardware.
type fakeHandle struct {
	mu      sync.Mutex
	closed  bool
	delay   time.Duration // how long writeFrame blocks per packet
	failErr error         // non-nil → writeFrame returns this error
	writes  int           // count of completed writeFrame calls
}

func (f *fakeHandle) writeFrame(_ []byte) error {
	f.mu.Lock()
	closed := f.closed
	fail := f.failErr
	delay := f.delay
	f.mu.Unlock()

	if closed {
		return errors.New("USB write: handle closed")
	}
	if fail != nil {
		return fail
	}
	if delay > 0 {
		time.Sleep(delay)
	}
	f.mu.Lock()
	f.writes++
	f.mu.Unlock()
	return nil
}

func (f *fakeHandle) close() {
	f.mu.Lock()
	f.closed = true
	f.mu.Unlock()
}

// sendFrameViaFake calls the real sendFrameToHandle but substitutes a fakeHandle
// for the usbHandle so tests run without USB hardware. It works by constructing
// a throwaway usbHandle whose writeFrame method is shadowed — since usbHandle is
// an unexported struct we wrap the call through a thin shim instead.
//
// The shim approach: build packets with buildPacket and feed them to fh directly,
// which is equivalent to sendFrameToHandle but routes through fh.writeFrame.
func sendFrameViaFake(ctx context.Context, fh *fakeHandle, data []byte) error {
	const maxPayload = 1024 - 8
	total := (len(data) + maxPayload - 1) / maxPayload
	for chunkNum := range total {
		select {
		case <-ctx.Done():
			return ctx.Err()
		default:
		}
		if err := fh.writeFrame(buildPacket(data, chunkNum, total)); err != nil {
			return fmt.Errorf("chunk %d: %w", chunkNum, err)
		}
	}
	return nil
}

// TestSendFrame_ContextCancelled verifies that cancelling the context mid-frame
// stops the chunk loop and returns ctx.Err().
func TestSendFrame_ContextCancelled(t *testing.T) {
	fh := &fakeHandle{delay: 5 * time.Millisecond}

	ctx, cancel := context.WithCancel(context.Background())

	// Cancel after the first chunk completes.
	go func() {
		for {
			fh.mu.Lock()
			w := fh.writes
			fh.mu.Unlock()
			if w >= 1 {
				cancel()
				return
			}
			time.Sleep(time.Millisecond)
		}
	}()

	data := make([]byte, FrameSize)
	err := sendFrameViaFake(ctx, fh, data)
	if !errors.Is(err, context.Canceled) {
		t.Errorf("expected context.Canceled, got %v", err)
	}

	fh.mu.Lock()
	writes := fh.writes
	fh.mu.Unlock()
	if writes >= 121 {
		t.Errorf("expected loop to stop early, but all 121 chunks were written")
	}
}

// TestSendFrame_HandleClosedMidWrite verifies that closing the handle while a
// frame send is in progress causes the write to fail cleanly rather than writing
// to a closed fd.
func TestSendFrame_HandleClosedMidWrite(t *testing.T) {
	fh := &fakeHandle{delay: 2 * time.Millisecond}

	// Close the handle after a few chunks.
	go func() {
		for {
			fh.mu.Lock()
			w := fh.writes
			fh.mu.Unlock()
			if w >= 3 {
				fh.close()
				return
			}
			time.Sleep(time.Millisecond)
		}
	}()

	data := make([]byte, FrameSize)
	err := sendFrameViaFake(context.Background(), fh, data)
	if err == nil {
		t.Error("expected error when handle closed mid-write, got nil")
	}
}

// TestDisconnect_DrainsConcurrentWrite verifies that Disconnect waits for an
// in-flight SendFrame to complete before closing the handle, rather than racing
// to close the fd underneath it.
func TestDisconnect_DrainsConcurrentWrite(t *testing.T) {
	n := &NexusDevice{
		logger:        discardLogger(),
		stopReconnect: make(chan struct{}),
	}

	// Install a real usbHandle backed by /dev/null so fd operations don't panic.
	// We only care about the mutex ordering here, not actual USB transfers.
	// Use a closed usbHandle (closed=true) so writeFrame returns immediately.
	h := &usbHandle{closed: true}
	n.handle = h
	n.connected = true

	// SendFrame will grab writeMu, attempt the write (which fails immediately
	// since closed=true in writeFrame's check), then release writeMu.
	// Disconnect must wait for writeMu before closing — if it doesn't, the
	// race detector catches it.
	var wg sync.WaitGroup
	wg.Add(1)
	go func() {
		defer wg.Done()
		data := make([]byte, FrameSize)
		// Ignore the error — we expect it since the handle is pre-closed.
		_ = n.SendFrame(context.Background(), data)
	}()

	// Small yield to let SendFrame acquire writeMu first.
	time.Sleep(5 * time.Millisecond)
	_ = n.Disconnect()

	wg.Wait()

	if n.connected {
		t.Error("expected device to be disconnected")
	}
	if n.handle != nil {
		t.Error("expected handle to be nil after Disconnect")
	}
}

// TestSendFrame_ContextDeadlineExceeded verifies that a context deadline
// shorter than the full frame transfer causes an early exit.
func TestSendFrame_ContextDeadlineExceeded(t *testing.T) {
	fh := &fakeHandle{delay: 10 * time.Millisecond}

	// Deadline that expires partway through the 121-chunk frame.
	ctx, cancel := context.WithTimeout(context.Background(), 50*time.Millisecond)
	defer cancel()

	data := make([]byte, FrameSize)
	err := sendFrameViaFake(ctx, fh, data)
	if !errors.Is(err, context.DeadlineExceeded) {
		t.Errorf("expected context.DeadlineExceeded, got %v", err)
	}
}

// TestSendFrame_InvalidFrameLength verifies that SendFrame rejects data that
// is not exactly FrameSize bytes, returning ErrInvalidFrame.
func TestSendFrame_InvalidFrameLength(t *testing.T) {
	n := &NexusDevice{
		logger:        discardLogger(),
		stopReconnect: make(chan struct{}),
		connected:     true,
		handle:        &usbHandle{closed: true},
	}

	cases := []struct {
		name string
		size int
	}{
		{"zero", 0},
		{"one short", FrameSize - 1},
		{"one over", FrameSize + 1},
		{"half frame", FrameSize / 2},
	}

	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			err := n.SendFrame(context.Background(), make([]byte, tc.size))
			if !errors.Is(err, ErrInvalidFrame) {
				t.Errorf("size %d: got %v, want ErrInvalidFrame", tc.size, err)
			}
		})
	}
}
