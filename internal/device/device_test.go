package device

import (
	"context"
	"testing"
	"time"
)

func TestMockDevice_Connect(t *testing.T) {
	mock := NewMockDevice()

	ctx := context.Background()
	err := mock.Connect(ctx)
	if err != nil {
		t.Fatalf("Connect() failed: %v", err)
	}

	if !mock.IsConnected() {
		t.Error("Expected device to be connected")
	}
}

func TestMockDevice_ConnectFailure(t *testing.T) {
	mock := NewMockDevice()
	mock.FailNext("connect")

	ctx := context.Background()
	err := mock.Connect(ctx)
	if err != ErrConnectionFailed {
		t.Errorf("Expected ErrConnectionFailed, got %v", err)
	}

	if mock.IsConnected() {
		t.Error("Expected device to not be connected after failure")
	}
}

func TestMockDevice_SendFrame(t *testing.T) {
	mock := NewMockDevice()
	ctx := context.Background()

	// Connect first
	if err := mock.Connect(ctx); err != nil {
		t.Fatalf("Connect() failed: %v", err)
	}

	// Create valid frame
	frame := make([]byte, FrameSize)
	for i := range frame {
		frame[i] = byte(i % 256)
	}

	// Send frame
	err := mock.SendFrame(ctx, frame)
	if err != nil {
		t.Fatalf("SendFrame() failed: %v", err)
	}

	// Verify frame was sent
	if mock.GetFramesSent() != 1 {
		t.Errorf("Expected 1 frame sent, got %d", mock.GetFramesSent())
	}

	// Verify frame data
	lastFrame := mock.GetLastFrame()
	if len(lastFrame) != FrameSize {
		t.Errorf("Expected frame size %d, got %d", FrameSize, len(lastFrame))
	}
}

func TestMockDevice_SendFrameInvalidSize(t *testing.T) {
	mock := NewMockDevice()
	ctx := context.Background()

	_ = mock.Connect(ctx)

	// Try to send invalid frame
	invalidFrame := make([]byte, 100)
	err := mock.SendFrame(ctx, invalidFrame)
	if err != ErrInvalidFrame {
		t.Errorf("Expected ErrInvalidFrame, got %v", err)
	}
}

func TestMockDevice_SendFrameWhenDisconnected(t *testing.T) {
	mock := NewMockDevice()
	ctx := context.Background()

	frame := make([]byte, FrameSize)
	err := mock.SendFrame(ctx, frame)
	if err != ErrDeviceDisconnected {
		t.Errorf("Expected ErrDeviceDisconnected, got %v", err)
	}
}

func TestMockDevice_TouchEvents(t *testing.T) {
	mock := NewMockDevice()
	ctx := context.Background()

	_ = mock.Connect(ctx)

	// Simulate touch events
	mock.SimulateTouch(0, true, 100*time.Millisecond)
	mock.SimulateTouch(1, false, 0)

	// Read events
	events, err := mock.ReadTouch(ctx)
	if err != nil {
		t.Fatalf("ReadTouch() failed: %v", err)
	}

	if len(events) != 2 {
		t.Fatalf("Expected 2 events, got %d", len(events))
	}

	// Verify first event
	if events[0].Button != 0 || !events[0].Pressed {
		t.Errorf("First event incorrect: button=%d, pressed=%v", events[0].Button, events[0].Pressed)
	}

	// Verify second event
	if events[1].Button != 1 || events[1].Pressed {
		t.Errorf("Second event incorrect: button=%d, pressed=%v", events[1].Button, events[1].Pressed)
	}

	// Events should be consumed
	events, err = mock.ReadTouch(ctx)
	if err != nil {
		t.Fatalf("ReadTouch() failed: %v", err)
	}
	if len(events) != 0 {
		t.Errorf("Expected 0 events after consumption, got %d", len(events))
	}
}

func TestMockDevice_Health(t *testing.T) {
	mock := NewMockDevice()
	ctx := context.Background()

	// Health check when disconnected
	err := mock.Health()
	if err != ErrDeviceDisconnected {
		t.Errorf("Expected ErrDeviceDisconnected, got %v", err)
	}

	// Health check when connected
	_ = mock.Connect(ctx)
	err = mock.Health()
	if err != nil {
		t.Errorf("Health() failed when connected: %v", err)
	}
}

func TestMockDevice_Disconnect(t *testing.T) {
	mock := NewMockDevice()
	ctx := context.Background()

	_ = mock.Connect(ctx)
	if !mock.IsConnected() {
		t.Fatal("Device should be connected")
	}

	err := mock.Disconnect()
	if err != nil {
		t.Fatalf("Disconnect() failed: %v", err)
	}

	if mock.IsConnected() {
		t.Error("Device should be disconnected")
	}
}

func TestDeviceError_Error(t *testing.T) {
	err := NewDeviceError("test_op", ErrDeviceNotFound)
	expected := "device test_op: device not found"
	if err.Error() != expected {
		t.Errorf("Expected error message %q, got %q", expected, err.Error())
	}
}
