// swipe-sim drives the /api/debug/swipe endpoint in a loop so you can
// watch transition smoothness in real time and tune parameters quickly.
//
// Usage:
//
//	go run ./cmd/swipe-sim [flags]
//
// Flags:
//
//	-addr        Backend base URL (default http://localhost:1985)
//	-dir         Swipe direction: left|right|alternate (default alternate)
//	-duration    Drag phase duration in ms (default 200)
//	-finalize    Snap animation duration in ms (default 120)
//	-steps       Drag steps — higher = smoother simulation (default 20)
//	-velocity    Release velocity in px/s; real swipes ~120–500 (default 150)
//	-count       Number of swipes to run, 0 = loop forever (default 0)
//	-gap         Pause between swipes in ms (default 800)
//	-analyse     Connect to WS, capture frames, and print per-frame displacement
//	             and smoothness analysis after each swipe
//
// Examples:
//
//	# Rapid alternating swipes — watch in Flutter UI
//	go run ./cmd/swipe-sim
//
//	# Analyse one swipe: see frame-by-frame displacement and smoothness report
//	go run ./cmd/swipe-sim -dir left -count 1 -analyse
//
//	# Simulate a fast flick (400 px/s) with analysis
//	go run ./cmd/swipe-sim -dir left -count 1 -velocity 400 -analyse
//
//	# Tune: slow drag (400ms), many steps, with analysis
//	go run ./cmd/swipe-sim -duration 400 -steps 40 -count 2 -analyse
package main

import (
	"bytes"
	"context"
	"encoding/base64"
	"encoding/json"
	"flag"
	"fmt"
	"image"
	"image/png"
	"math"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/coder/websocket"
	"github.com/coder/websocket/wsjson"
)

const displayWidth = 640

func main() {
	addr := flag.String("addr", "http://localhost:1985", "backend base URL")
	dir := flag.String("dir", "alternate", "direction: left|right|alternate")
	durationMs := flag.Int("duration", 200, "drag phase duration (ms)")
	finalizeMs := flag.Int("finalize", 120, "snap animation duration (ms)")
	steps := flag.Int("steps", 20, "drag steps")
	velocity := flag.Float64("velocity", 150, "release velocity in px/s (real swipes: ~120–500)")
	releaseAt := flag.Float64("release", 0.7, "progress (0-1) at which finger lifts, triggering snap")
	count := flag.Int("count", 0, "swipe count (0=forever)")
	gapMs := flag.Int("gap", 800, "pause between swipes (ms)")
	analyse := flag.Bool("analyse", false, "capture WS frames and print smoothness report")
	flag.Parse()

	if _, err := http.Get(*addr + "/api/health"); err != nil {
		fmt.Fprintf(os.Stderr, "error: cannot reach backend at %s: %v\n", *addr, err)
		os.Exit(1)
	}

	stop := make(chan os.Signal, 1)
	signal.Notify(stop, os.Interrupt, syscall.SIGTERM)

	endpoint := *addr + "/api/debug/swipe"
	wsURL := "ws" + (*addr)[4:] + "/api/ws"
	isLeft := true
	n := 0

	fmt.Printf("swipe-sim  addr=%s  dir=%s  duration=%dms  finalize=%dms  steps=%d  velocity=%.1f  analyse=%v\n",
		*addr, *dir, *durationMs, *finalizeMs, *steps, *velocity, *analyse)
	fmt.Println("Press Ctrl+C to stop.")
	fmt.Println()

	for {
		select {
		case <-stop:
			fmt.Println("\nstopped.")
			return
		default:
		}

		direction := "left"
		if !isLeft {
			direction = "right"
		}
		switch *dir {
		case "left":
			direction = "left"
		case "right":
			direction = "right"
		}

		swipeTotalMs := *durationMs + *finalizeMs

		var captureCtx context.Context
		var captureCancel context.CancelFunc
		var frameCh chan frameCapture

		if *analyse {
			// Start capturing WS frames slightly before the swipe fires
			captureCtx, captureCancel = context.WithCancel(context.Background())
			frameCh = make(chan frameCapture, 64)
			go captureFrames(captureCtx, wsURL, frameCh)
			time.Sleep(50 * time.Millisecond) // let WS connect settle
		}

		body, _ := json.Marshal(map[string]any{
			"direction":   direction,
			"duration_ms": *durationMs,
			"finalize_ms": *finalizeMs,
			"steps":       *steps,
			"velocity":    *velocity,
			"release_at":  *releaseAt,
		})

		swipeStart := time.Now()
		resp, err := http.Post(endpoint, "application/json", bytes.NewReader(body))
		if err != nil {
			fmt.Fprintf(os.Stderr, "[%3d] POST error: %v\n", n+1, err)
		} else {
			_ = resp.Body.Close()
			fmt.Printf("[%3d] %-5s  triggered\n", n+1, direction)
		}

		if *analyse {
			// Wait for the full animation to complete, then a little extra
			time.Sleep(time.Duration(swipeTotalMs+200) * time.Millisecond)
			captureCancel()
			time.Sleep(50 * time.Millisecond) // let goroutine drain
			close(frameCh)

			var frames []frameCapture
			for f := range frameCh {
				// Discard frames captured before the swipe triggered
				if !f.t.Before(swipeStart) {
					frames = append(frames, f)
				}
			}
			printAnalysis(frames, swipeStart, *durationMs, *finalizeMs, direction)
		}

		n++
		if *count > 0 && n >= *count {
			fmt.Printf("\ndone (%d swipes).\n", n)
			return
		}

		if *dir == "alternate" {
			isLeft = !isLeft
		}

		waitMs := *gapMs
		if waitMs < swipeTotalMs {
			waitMs = swipeTotalMs + 100
		}

		select {
		case <-stop:
			fmt.Println("\nstopped.")
			return
		case <-time.After(time.Duration(waitMs) * time.Millisecond):
		}
	}
}

// ── Frame capture ─────────────────────────────────────────────────────────────

type frameCapture struct {
	t        time.Time
	seam     int     // X position of the slide seam (-1 = static frame)
	seamGrad float64 // gradient magnitude at the seam column
	img      image.Image
}

func captureFrames(ctx context.Context, wsURL string, ch chan<- frameCapture) {
	conn, _, err := websocket.Dial(ctx, wsURL, nil)
	if err != nil {
		return
	}
	defer func() { _ = conn.CloseNow() }()

	for {
		var msg map[string]any
		if err := wsjson.Read(ctx, conn, &msg); err != nil {
			return
		}
		if msg["type"] != "frame" {
			continue
		}
		dataStr, ok := msg["data"].(string)
		if !ok {
			continue
		}
		raw, err := base64.StdEncoding.DecodeString(dataStr)
		if err != nil {
			continue
		}
		img, err := png.Decode(bytes.NewReader(raw))
		if err != nil {
			continue
		}
		seamX, seamGrad := detectSeamGrad(img)
		ch <- frameCapture{
			t:        time.Now(),
			seam:     seamX,
			seamGrad: seamGrad,
			img:      img,
		}
	}
}

// colAvgGrad computes the column-average colour gradient at position x,
// comparing columns x-gap and x+gap. Returns the Euclidean distance in
// RGB space between the two averaged columns.
func colAvgGrad(avgR, avgG, avgB []float64, x, gap int) float64 {
	dr := avgR[x+gap] - avgR[x-gap]
	dg := avgG[x+gap] - avgG[x-gap]
	db := avgB[x+gap] - avgB[x-gap]
	return math.Sqrt(dr*dr + dg*dg + db*db)
}

// columnAverages computes per-column average RGB over all rows.
func columnAverages(img image.Image) ([]float64, []float64, []float64) {
	bounds := img.Bounds()
	w := bounds.Max.X
	h := bounds.Max.Y
	avgR := make([]float64, w)
	avgG := make([]float64, w)
	avgB := make([]float64, w)
	for x := range w {
		var sr, sg, sb float64
		for y := range h {
			r, g, b, _ := img.At(x, y).RGBA()
			sr += float64(r >> 8)
			sg += float64(g >> 8)
			sb += float64(b >> 8)
		}
		avgR[x] = sr / float64(h)
		avgG[x] = sg / float64(h)
		avgB[x] = sb / float64(h)
	}
	return avgR, avgG, avgB
}

// detectSeamGrad returns the best-candidate seam column and its gradient score.
func detectSeamGrad(img image.Image) (seamX int, maxGrad float64) {
	w := img.Bounds().Max.X
	if w < 10 {
		return -1, 0
	}
	avgR, avgG, avgB := columnAverages(img)
	const gap = 3
	seamX = -1
	for x := gap; x < w-gap; x++ {
		g := colAvgGrad(avgR, avgG, avgB, x, gap)
		if g > maxGrad {
			maxGrad = g
			seamX = x
		}
	}
	return seamX, maxGrad
}

// ── Analysis report ───────────────────────────────────────────────────────────

func printAnalysis(frames []frameCapture, swipeStart time.Time, durationMs, finalizeMs int, direction string) {
	totalMs := durationMs + finalizeMs
	fmt.Printf("\n  ── Analysis (%s swipe) ─────────────────────────\n", direction)
	fmt.Printf("  Total frames received : %d\n", len(frames))

	expectedFrames := (durationMs + finalizeMs) / 100
	fmt.Printf("  Expected frames (~30fps during transition) : ~%d\n", totalMs/33)
	fmt.Printf("  Expected frames (~10fps baseline)          : ~%d\n", expectedFrames)

	if len(frames) == 0 {
		fmt.Println("  No frames captured.")
		fmt.Println()
		return
	}

	// Establish the baseline seam from the first frame (static, pre-swipe content).
	// A frame is a transition frame when its seam has moved from the previous frame
	// by more than a small tolerance — i.e. the boundary is actively moving.
	// Trailing static frames (seam stuck after transition completes) are excluded.
	const motionTolerance = 15 // px; inter-frame seam delta to count as moving
	baselineSeam := frames[0].seam

	// completed is set once the seam reaches the far edge, meaning the transition
	// finished. Frames after that are static and should not be included.
	completed := false
	var transition []frameCapture
	prevSeamForFilter := baselineSeam
	for _, f := range frames {
		if completed {
			break
		}
		moving := f.seam >= 0 && abs(f.seam-prevSeamForFilter) > motionTolerance
		if moving {
			transition = append(transition, f)
			// Seam within 60px of either edge means transition completed.
			if f.seam <= 60 || f.seam >= displayWidth-60 {
				completed = true
			}
		}
		if f.seam >= 0 {
			prevSeamForFilter = f.seam
		}
	}

	fmt.Printf("  Transition frames     : %d  (seam moving >%dpx/frame; baseline %d)\n",
		len(transition), motionTolerance, baselineSeam)

	if len(transition) == 0 {
		fmt.Println("  No transition frames captured — swipe may have been too fast")
		fmt.Println("  Try: -duration 400 -steps 40")
		fmt.Println()
		return
	}

	fmt.Printf("\n  %-6s  %-8s  %-8s  %-8s  %s\n",
		"frame", "t_ms", "seam_x", "Δseam", "smoothness")
	fmt.Println("  " + strings.Repeat("─", 56))

	prevSeam := -1
	var deltas []float64
	var issues []string

	for i, f := range transition {
		tMs := f.t.Sub(swipeStart).Milliseconds()
		delta := 0
		if prevSeam >= 0 {
			delta = f.seam - prevSeam
		}

		// Expected seam position based on linear drag progress.
		// seam = displayWidth * (1 - progress) for a left swipe.
		tNorm := math.Min(1.0, float64(tMs)/float64(totalMs))
		var expectedSeam int
		if direction == "left" {
			expectedSeam = int(float64(displayWidth) * (1 - tNorm))
		} else {
			expectedSeam = int(float64(displayWidth) * tNorm)
		}
		seamErr := math.Abs(float64(f.seam - expectedSeam))

		// Classify frame quality
		quality := "✓"
		atEdge := f.seam >= displayWidth-60 || f.seam <= 60
		if i > 0 && delta == 0 && !atEdge {
			quality = "⚠ duplicate"
			issues = append(issues, fmt.Sprintf("frame %d: duplicate (seam stuck at %d)", i+1, f.seam))
		} else if i > 0 && delta == 0 && atEdge {
			quality = "done"
		} else if i > 0 && math.Abs(float64(delta)) > float64(displayWidth)/3 {
			quality = "⚠ jump"
			issues = append(issues, fmt.Sprintf("frame %d: large jump Δ=%d px", i+1, delta))
		} else if seamErr > 80 {
			quality = "~ off-curve"
		}

		deltaStr := "—"
		if prevSeam >= 0 {
			deltaStr = fmt.Sprintf("%+d", delta)
		}
		fmt.Printf("  %-6d  %-8d  %-8d  %-8s  %s\n", i+1, tMs, f.seam, deltaStr, quality)

		if prevSeam >= 0 {
			deltas = append(deltas, math.Abs(float64(delta)))
		}
		prevSeam = f.seam
	}

	// Summary
	fmt.Println("  " + strings.Repeat("─", 56))

	if len(deltas) > 0 {
		var mean, maxD float64
		for _, d := range deltas {
			mean += d
			if d > maxD {
				maxD = d
			}
		}
		mean /= float64(len(deltas))

		var variance float64
		for _, d := range deltas {
			variance += (d - mean) * (d - mean)
		}
		variance /= float64(len(deltas))
		stddev := math.Sqrt(variance)

		fmt.Printf("\n  Avg Δseam : %.0f px  (stddev %.0f px, max %.0f px)\n",
			mean, stddev, maxD)

		// Smoothness score: low stddev relative to mean = consistent motion
		consistency := 100.0
		if mean > 0 {
			consistency = math.Max(0, 100-stddev/mean*100)
		}
		fmt.Printf("  Consistency : %.0f%%", consistency)
		if consistency >= 80 {
			fmt.Println("  ✓ smooth")
		} else if consistency >= 50 {
			fmt.Println("  ~ acceptable")
		} else {
			fmt.Println("  ✗ choppy — try increasing -steps or -duration")
		}
	}

	if len(issues) > 0 {
		fmt.Println("\n  Issues:")
		for _, iss := range issues {
			fmt.Println("    ⚠ " + iss)
		}
	} else {
		fmt.Println("  No issues detected.")
	}
	fmt.Println()
}

func abs(v int) int {
	if v < 0 {
		return -v
	}
	return v
}
