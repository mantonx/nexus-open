package device

import (
	"errors"
	"fmt"
	"syscall"
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

// classifyOpenError maps a usbfs open error to a structured sentinel.
func classifyOpenError(err error) error {
	if err == nil {
		return nil
	}
	switch {
	case errors.Is(err, syscall.EACCES) || errors.Is(err, syscall.EPERM):
		return fmt.Errorf("%w: %v", ErrPermissionDenied, err)
	case errors.Is(err, syscall.EBUSY):
		return fmt.Errorf("%w: %v", ErrDeviceBusy, err)
	default:
		return err
	}
}
