package device

import (
	"errors"
	"fmt"
	"sort"
	"strings"

	"github.com/karalabe/hid"
)

// Additional sentinel errors for actionable UI messages.
var (
	ErrPermissionDenied = errors.New("USB permission denied")
	ErrDeviceBusy       = errors.New("device busy")
)

// DeviceError wraps device-related errors with additional context.
type DeviceError struct {
	Op  string // Operation that failed (e.g., "connect", "send_frame")
	Err error  // Underlying error
}

func (e *DeviceError) Error() string {
	return fmt.Sprintf("device %s: %v", e.Op, e.Err)
}

func (e *DeviceError) Unwrap() error {
	return e.Err
}

// NewDeviceError creates a new DeviceError.
func NewDeviceError(op string, err error) error {
	return &DeviceError{
		Op:  op,
		Err: err,
	}
}

// sortInterfacesByPreference sorts HID interfaces so interface 0 (display)
// is tried before interface 1 (touch/keyboard). Taking the wrong interface
// succeeds at open time but produces hidapi write failures at frame send time.
func sortInterfacesByPreference(devices []hid.DeviceInfo) {
	sort.SliceStable(devices, func(i, j int) bool {
		return devices[i].Interface < devices[j].Interface
	})
}

// classifyOpenError maps a raw hidapi open error to a structured sentinel.
func classifyOpenError(err error) error {
	if err == nil {
		return nil
	}
	msg := strings.ToLower(err.Error())
	switch {
	case strings.Contains(msg, "permission denied") || strings.Contains(msg, "access denied"):
		return fmt.Errorf("%w: %v", ErrPermissionDenied, err)
	case strings.Contains(msg, "busy") || strings.Contains(msg, "resource busy") || strings.Contains(msg, "already open"):
		return fmt.Errorf("%w: %v", ErrDeviceBusy, err)
	default:
		return err
	}
}
