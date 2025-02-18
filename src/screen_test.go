package main

import (
	"testing"

	"github.com/google/gousb"
)

func TestSetNexusImage(t *testing.T) {
	tests := []struct {
		name        string
		connected   bool
		imageData   []byte
		deviceError bool
		wantErr     bool
	}{
		{
			name:      "device not connected",
			connected: false,
			imageData: make([]byte, width*height*4),
			wantErr:   false,
		},
		{
			name:      "invalid image data length",
			connected: true,
			imageData: make([]byte, 100),
			wantErr:   true,
		},
		{
			name:        "device error",
			connected:   true,
			imageData:   make([]byte, width*height*4),
			deviceError: true,
			wantErr:     true,
		},
		{
			name:      "successful update",
			connected: true,
			imageData: make([]byte, width*height*4),
			wantErr:   false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			// Setup test state
			connected = tt.connected

			// Create mock device
			mockDevice := &gousb.Device{}
			if tt.deviceError {
				mockDevice = nil
			}

			err := setNexusImage(mockDevice, tt.imageData)

			if (err != nil) != tt.wantErr {
				t.Errorf("setNexusImage() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}
