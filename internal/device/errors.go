package device

import "fmt"

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
