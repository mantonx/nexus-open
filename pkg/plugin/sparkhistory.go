package plugin

import "sync"

// SparkHistory is a thread-safe ring buffer of float32 samples used to build
// sparkline graphs. Embed it in a plugin struct and call Push on each sample;
// call Normalized or Raw to read the current window.
type SparkHistory struct {
	mu         sync.Mutex
	samples    []float32
	maxSamples int
}

// NewSparkHistory returns a SparkHistory that retains up to maxSamples values.
func NewSparkHistory(maxSamples int) SparkHistory {
	return SparkHistory{
		samples:    make([]float32, 0, maxSamples),
		maxSamples: maxSamples,
	}
}

// Push appends v and trims the buffer to maxSamples.
func (h *SparkHistory) Push(v float32) {
	h.mu.Lock()
	defer h.mu.Unlock()
	h.samples = append(h.samples, v)
	if len(h.samples) > h.maxSamples {
		h.samples = h.samples[len(h.samples)-h.maxSamples:]
	}
}

// Raw returns a copy of the current sample window, safe for the caller to read
// without holding any lock.
func (h *SparkHistory) Raw() []float32 {
	h.mu.Lock()
	defer h.mu.Unlock()
	if len(h.samples) == 0 {
		return nil
	}
	out := make([]float32, len(h.samples))
	copy(out, h.samples)
	return out
}

// Normalized returns the sample window scaled to [0, 1] using the loPct and
// hiPct percentiles as the range bounds. Using percentiles instead of min/max
// prevents a single spike from compressing all other values to a flatline.
// If the data range is too narrow (< minRange), it is expanded symmetrically
// around the midpoint.
func (h *SparkHistory) Normalized(loPct, hiPct int, minRange float32) []float32 {
	h.mu.Lock()
	defer h.mu.Unlock()
	if len(h.samples) == 0 {
		return nil
	}

	mn, mx := PercentileRange(h.samples, loPct, hiPct)
	rng := mx - mn
	if rng < minRange {
		mid := (mn + mx) / 2
		mn = mid - minRange/2
		rng = minRange
	}

	out := make([]float32, len(h.samples))
	for i, v := range h.samples {
		s := (v - mn) / rng
		if s < 0 {
			s = 0
		}
		if s > 1 {
			s = 1
		}
		out[i] = s
	}
	return out
}

// PercentileRange returns the loPct-th and hiPct-th percentile of data using
// insertion sort (suitable for the small windows used by sparklines).
func PercentileRange(data []float32, loPct, hiPct int) (float32, float32) {
	sorted := make([]float32, len(data))
	copy(sorted, data)
	for i := 1; i < len(sorted); i++ {
		for j := i; j > 0 && sorted[j] < sorted[j-1]; j-- {
			sorted[j], sorted[j-1] = sorted[j-1], sorted[j]
		}
	}
	loIdx := loPct * (len(sorted) - 1) / 100
	hiIdx := hiPct * (len(sorted) - 1) / 100
	return sorted[loIdx], sorted[hiIdx]
}
