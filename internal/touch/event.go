// Package touch provides touch input handling and gesture recognition.
package touch

import "time"

// Event represents a touch/button input event from the device.
type Event struct {
	Button        int           // Button identifier (0-4 for 5 buttons)
	Pressed       bool          // true if pressed, false if released
	Duration      time.Duration // How long the button was held
	SwipeProgress float32       // Live swipe progress (0.0 to 1.0)
	SwipeActive   bool          // true if swipe is in progress (live tracking)
	Velocity      float32       // Swipe velocity in pixels/second (for completed swipes)
	SwipePixels   int           // Signed pixel delta from gesture start (left is negative)
	Timestamp     time.Time     // When this event was captured
	TapX          int           // Display pixel X position of tap (0–639); only valid for Button==0 taps
	SlideX        int           // Net horizontal movement during the press (px, signed); non-zero means finger slid on lift
}
