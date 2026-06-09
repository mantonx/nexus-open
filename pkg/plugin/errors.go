package plugin

import (
	"errors"
	"fmt"
)

var (
	// ErrNotTapper is returned over RPC when a plugin does not implement Tapper.
	ErrNotTapper = errors.New("plugin does not implement Tapper")

	// ErrEmptyPrimary indicates the Primary field is required
	ErrEmptyPrimary = errors.New("payload primary field cannot be empty")

	// ErrInvalidSeverity indicates an invalid severity value
	ErrInvalidSeverity = errors.New("severity must be 'ok', 'warn', or 'crit'")

	// ErrSparkTooLong indicates sparkline data exceeds maximum length
	ErrSparkTooLong = errors.New("sparkline data exceeds 60 points")

	// ErrLoadSparkTooLong indicates load_spark data exceeds maximum length
	ErrLoadSparkTooLong = errors.New("load_spark data exceeds 60 points")

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

// ErrLoadSparkOutOfRange indicates a load_spark value is not normalized
type ErrLoadSparkOutOfRange struct {
	Index int
	Value float32
}

func (e *ErrLoadSparkOutOfRange) Error() string {
	return fmt.Sprintf("load_spark value at index %d is %f, must be between 0.0 and 1.0", e.Index, e.Value)
}
