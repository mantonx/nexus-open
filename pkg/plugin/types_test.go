package plugin

import (
	"encoding/json"
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
		// New fields added in Phase 2.
		{
			name: "value accepted without primary",
			payload: Payload{
				Value:     "41",
				ValueUnit: "°C",
				Severity:  SeverityOK,
			},
			wantErr: false,
		},
		{
			name: "empty primary and empty value",
			payload: Payload{
				ValueUnit: "°C",
			},
			wantErr: true,
		},
		{
			name: "load_spark too long",
			payload: Payload{
				Value:     "72",
				LoadSpark: make([]float32, 61),
			},
			wantErr: true,
		},
		{
			name: "load_spark value out of range (negative)",
			payload: Payload{
				Value:     "72",
				LoadSpark: []float32{0.3, -0.1, 0.9},
			},
			wantErr: true,
		},
		{
			name: "load_spark value out of range (too high)",
			payload: Payload{
				Value:     "72",
				LoadSpark: []float32{0.3, 1.2, 0.9},
			},
			wantErr: true,
		},
		{
			name: "valid load_spark",
			payload: Payload{
				Value:     "72",
				Spark:     []float32{0.5, 0.6, 0.7},
				LoadSpark: []float32{0.3, 0.4, 0.5},
				GraphType: GraphTypeCombo,
			},
			wantErr: false,
		},
		{
			name: "span and expandable are optional",
			payload: Payload{
				Value:      "Now Playing",
				Span:       3,
				Expandable: true,
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

func TestConfigSchema_JSONRoundTrip(t *testing.T) {
	minVal, maxVal := 0, 100
	schema := ConfigSchema{
		Fields: []ConfigField{
			{Key: "unit", Label: "Units", Type: FieldTypeEnum, Default: "metric",
				Options: []FieldOption{
					{Value: "metric", Label: "°C"},
					{Value: "imperial", Label: "°F"},
				}},
			{Key: "threshold", Label: "Warn above", Type: FieldTypeInt, Default: 80, Min: &minVal, Max: &maxVal},
			{Key: "enabled", Label: "Enabled", Type: FieldTypeBool, Default: true},
		},
	}

	data, err := json.Marshal(schema)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	var got ConfigSchema
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if len(got.Fields) != 3 {
		t.Fatalf("fields: want 3, got %d", len(got.Fields))
	}
	if got.Fields[0].Key != "unit" {
		t.Errorf("field[0].key: want unit, got %s", got.Fields[0].Key)
	}
	if len(got.Fields[0].Options) != 2 {
		t.Errorf("field[0].options: want 2, got %d", len(got.Fields[0].Options))
	}
	if got.Fields[0].Options[0].Value != "metric" {
		t.Errorf("option[0].value: want metric, got %s", got.Fields[0].Options[0].Value)
	}
	if got.Fields[1].Min == nil || *got.Fields[1].Min != 0 {
		t.Errorf("field[1].min: want 0, got %v", got.Fields[1].Min)
	}
	if got.Fields[1].Max == nil || *got.Fields[1].Max != 100 {
		t.Errorf("field[1].max: want 100, got %v", got.Fields[1].Max)
	}
}

func TestDescriptor_SchemaInJSON(t *testing.T) {
	desc := Descriptor{
		Name:      "CPU Temp",
		Version:   "1.0.0",
		RefreshMs: 2000,
		Schema: ConfigSchema{
			Fields: []ConfigField{
				{Key: "unit", Label: "Units", Type: FieldTypeEnum, Default: "metric"},
			},
		},
	}

	data, err := json.Marshal(desc)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}

	var m map[string]any
	if err := json.Unmarshal(data, &m); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}

	if _, ok := m["config_schema"]; !ok {
		t.Error("Descriptor JSON missing config_schema field")
	}
}

func TestGraphTypeConstants(t *testing.T) {
	// All graph types must round-trip through JSON without loss.
	types := []GraphType{
		GraphTypeSparkline,
		GraphTypeBar,
		GraphTypeArea,
		GraphTypeLine,
		GraphTypeSegmented,
		GraphTypeBarThresh,
		GraphTypeCombo,
		GraphTypeNumberDelta,
	}
	for _, gt := range types {
		p := Payload{Value: "42", GraphType: gt}
		data, err := json.Marshal(p)
		if err != nil {
			t.Fatalf("%s: marshal: %v", gt, err)
		}
		var got Payload
		if err := json.Unmarshal(data, &got); err != nil {
			t.Fatalf("%s: unmarshal: %v", gt, err)
		}
		if got.GraphType != gt {
			t.Errorf("%s: round-trip got %s", gt, got.GraphType)
		}
	}
}

func TestNewFieldsJSONRoundTrip(t *testing.T) {
	orig := Payload{
		Value:      "41",
		ValueUnit:  "°C",
		Span:       2,
		Expandable: true,
		LoadSpark:  []float32{0.1, 0.5, 0.9},
		GraphType:  GraphTypeCombo,
	}
	data, err := json.Marshal(orig)
	if err != nil {
		t.Fatalf("marshal: %v", err)
	}
	var got Payload
	if err := json.Unmarshal(data, &got); err != nil {
		t.Fatalf("unmarshal: %v", err)
	}
	if got.Value != orig.Value {
		t.Errorf("Value: want %q got %q", orig.Value, got.Value)
	}
	if got.ValueUnit != orig.ValueUnit {
		t.Errorf("ValueUnit: want %q got %q", orig.ValueUnit, got.ValueUnit)
	}
	if got.Span != orig.Span {
		t.Errorf("Span: want %d got %d", orig.Span, got.Span)
	}
	if !got.Expandable {
		t.Error("Expandable: want true got false")
	}
	if len(got.LoadSpark) != 3 || got.LoadSpark[1] != 0.5 {
		t.Errorf("LoadSpark: got %v", got.LoadSpark)
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
