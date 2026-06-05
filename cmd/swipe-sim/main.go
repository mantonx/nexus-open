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
//	-velocity    Release velocity, 0–3 (default 1.0)
//	-count       Number of swipes to run, 0 = loop forever (default 0)
//	-gap         Pause between swipes in ms (default 800)
//	-watch       Connect to WS and print frame timing alongside swipes
//
// Examples:
//
//	# Rapid alternating swipes with default params — watch in Flutter UI
//	go run ./cmd/swipe-sim
//
//	# Tune: slow drag (400ms), fast snap (60ms), many steps
//	go run ./cmd/swipe-sim -duration 400 -finalize 60 -steps 40
//
//	# Single left swipe
//	go run ./cmd/swipe-sim -dir left -count 1
package main

import (
	"bytes"
	"encoding/json"
	"flag"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"
)

func main() {
	addr := flag.String("addr", "http://localhost:1985", "backend base URL")
	dir := flag.String("dir", "alternate", "direction: left|right|alternate")
	durationMs := flag.Int("duration", 200, "drag phase duration (ms)")
	finalizeMs := flag.Int("finalize", 120, "snap animation duration (ms)")
	steps := flag.Int("steps", 20, "drag steps")
	velocity := flag.Float64("velocity", 1.0, "release velocity (0-3)")
	count := flag.Int("count", 0, "swipe count (0=forever)")
	gapMs := flag.Int("gap", 800, "pause between swipes (ms)")
	flag.Parse()

	// Verify backend is reachable
	if _, err := http.Get(*addr + "/api/health"); err != nil {
		fmt.Fprintf(os.Stderr, "error: cannot reach backend at %s: %v\n", *addr, err)
		os.Exit(1)
	}

	stop := make(chan os.Signal, 1)
	signal.Notify(stop, os.Interrupt, syscall.SIGTERM)

	endpoint := *addr + "/api/debug/swipe"
	isLeft := true
	n := 0

	fmt.Printf("swipe-sim  addr=%s  dir=%s  duration=%dms  finalize=%dms  steps=%d  velocity=%.1f\n",
		*addr, *dir, *durationMs, *finalizeMs, *steps, *velocity)
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
		if *dir == "left" {
			direction = "left"
		} else if *dir == "right" {
			direction = "right"
		}

		body, _ := json.Marshal(map[string]interface{}{
			"direction":   direction,
			"duration_ms": *durationMs,
			"finalize_ms": *finalizeMs,
			"steps":       *steps,
			"velocity":    *velocity,
		})

		start := time.Now()
		resp, err := http.Post(endpoint, "application/json", bytes.NewReader(body))
		if err != nil {
			fmt.Fprintf(os.Stderr, "[%3d] POST error: %v\n", n+1, err)
		} else {
			resp.Body.Close()
			rtt := time.Since(start)
			fmt.Printf("[%3d] %-5s  rtt=%dms\n", n+1, direction, rtt.Milliseconds())
		}

		n++
		if *count > 0 && n >= *count {
			fmt.Printf("\ndone (%d swipes).\n", n)
			return
		}

		// Alternate direction unless locked
		if *dir == "alternate" {
			isLeft = !isLeft
		}

		// Wait for the swipe animation to finish before starting the next
		swipeTotalMs := *durationMs + *finalizeMs
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
