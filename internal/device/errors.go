package device

import (
	"errors"
	"fmt"
	"strings"
)

// Additional sentinel errors for actionable UI messages.
var (
	ErrPermissionDenied = errors.New("USB permission denied")
	ErrDeviceBusy       = errors.New("device busy")
)

// DeviceError wraps device-related errors with additional context.
type DeviceError struct {
	Op  string
	Err error
}

func (e *DeviceError) Error() string {
	return fmt.Sprintf("device %s: %v", e.Op, e.Err)
}

func (e *DeviceError) Unwrap() error {
	return e.Err
}

// NewDeviceError creates a new DeviceError.
func NewDeviceError(op string, err error) error {
	return &DeviceError{Op: op, Err: err}
}

// classifyOpenError maps a raw libusb open error to a structured sentinel.
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
