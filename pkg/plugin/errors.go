package plugin

import (
	"errors"
	"fmt"
)

var (
	// ErrEmptyPrimary indicates the Primary field is required
	ErrEmptyPrimary = errors.New("payload primary field cannot be empty")

	// ErrInvalidSeverity indicates an invalid severity value
	ErrInvalidSeverity = errors.New("severity must be 'ok', 'warn', or 'crit'")

	// ErrSparkTooLong indicates sparkline data exceeds maximum length
	ErrSparkTooLong = errors.New("sparkline data exceeds 60 points")

	// ErrProgressOutOfRange indicates progress value is not in [0.0, 1.0]
	ErrProgressOutOfRange = errors.New("progress must be between 0.0 and 1.0")
)

// ErrSparkOutOfRange indicates a sparkline value is not normalized
type ErrSparkOutOfRange struct {
	Index int
	Value float32
}

func (e *ErrSparkOutOfRange) Error() string {
	return fmt.Sprintf("sparkline value at index %d is %f, must be between 0.0 and 1.0", e.Index, e.Value)
}
