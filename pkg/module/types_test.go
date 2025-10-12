package module

import (
	"testing"
	"time"
)

func TestPayloadValidation(t *testing.T) {
	tests := []struct {
		name    string
		payload Payload
		wantErr bool
	}{
		{
			name: "valid payload",
			payload: Payload{
				Primary:   "42°C",
				Secondary: "CPU Temp",
				Severity:  SeverityOK,
			},
			wantErr: false,
		},
		{
			name: "empty primary",
			payload: Payload{
				Primary: "",
			},
			wantErr: true,
		},
		{
			name: "invalid severity",
			payload: Payload{
				Primary:  "42°C",
				Severity: "invalid",
			},
			wantErr: true,
		},
		{
			name: "spark too long",
			payload: Payload{
				Primary: "42°C",
				Spark:   make([]float32, 61),
			},
			wantErr: true,
		},
		{
			name: "spark value out of range (negative)",
			payload: Payload{
				Primary: "42°C",
				Spark:   []float32{0.5, -0.1, 0.8},
			},
			wantErr: true,
		},
		{
			name: "spark value out of range (too high)",
			payload: Payload{
				Primary: "42°C",
				Spark:   []float32{0.5, 1.5, 0.8},
			},
			wantErr: true,
		},
		{
			name: "progress out of range (negative)",
			payload: Payload{
				Primary:  "Playing",
				Progress: -0.5,
			},
			wantErr: true,
		},
		{
			name: "progress out of range (too high)",
			payload: Payload{
				Primary:  "Playing",
				Progress: 1.5,
			},
			wantErr: true,
		},
		{
			name: "valid sparkline",
			payload: Payload{
				Primary: "CPU",
				Spark:   []float32{0.1, 0.2, 0.5, 0.8, 1.0, 0.7},
			},
			wantErr: false,
		},
		{
			name: "valid progress",
			payload: Payload{
				Primary:  "Playing",
				Progress: 0.65,
			},
			wantErr: false,
		},
	}

	for _, tt := range tests {
		t.Run(tt.name, func(t *testing.T) {
			err := tt.payload.Validate()
			if (err != nil) != tt.wantErr {
				t.Errorf("Payload.Validate() error = %v, wantErr %v", err, tt.wantErr)
			}
		})
	}
}

func TestPayloadExpiry(t *testing.T) {
	// Not expired (no TTL)
	p1 := Payload{
		Primary:   "Test",
		Timestamp: time.Now().Add(-5 * time.Second),
		TTL:       0,
	}
	if p1.IsExpired() {
		t.Error("Payload with no TTL should not expire")
	}

	// Not expired (within TTL)
	p2 := Payload{
		Primary:   "Test",
		Timestamp: time.Now().Add(-1 * time.Second),
		TTL:       5 * time.Second,
	}
	if p2.IsExpired() {
		t.Error("Payload within TTL should not be expired")
	}

	// Expired (beyond TTL)
	p3 := Payload{
		Primary:   "Test",
		Timestamp: time.Now().Add(-10 * time.Second),
		TTL:       5 * time.Second,
	}
	if !p3.IsExpired() {
		t.Error("Payload beyond TTL should be expired")
	}
}

func TestSeverityLevels(t *testing.T) {
	severities := []Severity{SeverityOK, SeverityWarn, SeverityCrit}

	for _, sev := range severities {
		p := Payload{
			Primary:  "Test",
			Severity: sev,
		}
		if err := p.Validate(); err != nil {
			t.Errorf("Valid severity %s should not error: %v", sev, err)
		}
	}
}
