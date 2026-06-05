# Display Rendering Plan
_Created: 2026-06-05 · Based on deep research + codebase audit_
_Last updated: 2026-06-05 · Steps 1–4 complete_

This document covers the hardware display rendering pipeline only — the 640×48
pixel LCD strip sent over USB HID at up to 30fps. It supersedes the ad-hoc
renderer work done during the UI session (the half-finished `drawFullHeightGraph`
and `renderContent` rewrites in `renderer.go`).

---

## Research conclusions

### What the research confirmed (high-confidence)

**Stack: `fogleman/gg` + `golang/freetype`**

- `fogleman/gg` is the proven production choice for Go hardware LCD rendering.
  Used by kubesail/pibox-framebuffer (240×240 SPI LCD, CPU/memory/disk stats).
  Pure Go, no CGo, cross-compiles cleanly. Already used in our dep tree
  transitively via `golang/freetype`.
- `golang/freetype/raster` (standalone, no font files needed) is the
  battle-tested approach for anti-aliased sparkline strokes at 32–48px height.
  Used by tv42/quobar with 5–8px strokes at 32px height — exactly our scale.
  Same area/coverage algorithm as FreeType smooth and Anti-Grain Geometry.
- Both `golang/freetype` and `golang.org/x/image` are already in `go.mod`.
  Only `fogleman/gg` needs to be added.

**Font rendering: hinting matters**

- `HintingFull` is the right choice for 48px-height displays. It aligns glyph
  nodes to the pixel grid. `HintingNone` (the default in
  `golang.org/x/image/font/opentype`) is wrong for this use case.
- The current renderer uses Go bitmap fonts loaded through a custom `fonts.Manager`
  that bypasses the hinting pipeline entirely. This is why text looks slightly
  off-baseline and blurry.
- `golang.org/x/image/font/opentype` is the actively maintained replacement for
  the archived `golang/freetype` Context — same hinting API, same output.

**Graph fills: path polygons, not pixel loops**

- Area fill = draw path along data points, close polygon back to baseline, call
  `Fill()`. Five lines of `gg` code. The current nested-loop approach is fragile
  and leaves gaps between data points.
- Linear gradient fill is available in `gg` at no extra cost and gives the
  bottom-fade effect that makes graphs look more polished.

**Wire format: JPEG not PNG**

- Stream Deck-class hardware display projects (rafaelmartins/streamdeck) encode
  output as `jpeg.Encode(Quality:100)` or `bmp.Encode`, not PNG. PNG compression
  adds unnecessary latency at 30fps.
- The current pipeline encodes every frame as PNG and base64s it over WebSocket.
  Switching the HID path to JPEG-100 would reduce frame encode time.
  The WebSocket path can stay PNG (Flutter doesn't need 30fps fidelity).

**What to rule out (confirmed)**

- **Ebitengine**: GPU-first, wrong compositing model. etxt (its text library)
  explicitly warns CPU mode "performance will die" for real-time apps. No
  subpixel antialiasing. Not appropriate.
- **Cairo bindings**: No production Go hardware LCD project found using them.
  CGo dependency breaks cross-compilation. No advantage over gg for this use case.
- **go-chart**: Capable but defaults are 1024×400px / 8pt minimum font — 8×
  oversized. Fighting the library for every setting.
- **pixfont**: Bitmap fonts, no anti-aliasing. Explicitly redirects users who
  need anti-aliasing to golang/freetype.

---

## Current state

### What's broken in the current renderer

The `renderer.go` file (1,270 lines) uses hand-rolled pixel loops for everything:
- Graph fills use `blendPixel` in a nested loop — leaves gaps, slow, fragile
- Graph lines use Bresenham's algorithm — 1px, nearly invisible at this scale
- Font baselines are guessed rather than measured from actual font metrics
- The accent colour was being stomped by `TextColor` in `app.go` (fixed, but
  reveals the fragility of the current design)
- `drawZoneBackground`, `drawBackgroundGraph`, and the new `drawFullHeightGraph`
  all exist simultaneously in the file in various states of repair
- The file has grown to 1,270 lines through accretion and is no longer coherent

### What's working

- **Compositor** (`compositor.go`): rewritten, clean. Background image + GIF
  pipeline is correct. Zone compositing is correct. Keep as-is.
- **Plugin system**: working, modules sampling correctly.
- **Wire format** (WS broadcast): works, stay PNG for WebSocket path.
- **App wiring** (`app.go`): `SetBackground` wired correctly.

---

## Planned work

### Step 1 — Add `fogleman/gg` dependency

```
go get github.com/fogleman/gg
```

No other dependency changes. `golang/freetype` and `golang.org/x/image` are
already present.

---

### Step 2 — Rewrite `renderer.go` using `gg`

Replace the 1,270-line hand-rolled renderer with a clean `gg`-based
implementation. Target: ~300 lines.

**Rendering pipeline per zone:**

```
1. Fill background (gg.DrawRectangle + SetHexColor + Fill)
2. Zone tint (accent colour at low alpha, luminance-aware)
3. Graph: gg path polygon → close to baseline → Fill (area) + Stroke (line)
   - Optional: linear gradient fill (top of graph = accent at 30%, bottom = 0%)
4. Text:
   - Primary value: top-anchored, HintingFull, 24pt GoRegular or Inter
   - Secondary label: bottom-anchored, HintingFull, 9pt
   - Icon: gg.DrawImageAnchored with pre-rasterised icon at zone size
5. Progress bar (optional, 2px, bottom edge)
```

**Font loading:**

Replace the custom `fonts.Manager` with direct `opentype.Parse` +
`opentype.NewFace` using `HintingFull`. Load the embedded Go fonts or Inter
(already in pubspec for Flutter; can embed the TTF in Go via `//go:embed`).

**Graph fill (the key change):**

```go
// Current (broken): nested pixel loop
// New (gg):
dc.MoveTo(x0, float64(height))       // baseline start
for i, v := range data {
    dc.LineTo(float64(i)*xStep, yOf(v))
}
dc.LineTo(float64(len(data)-1)*xStep, float64(height)) // baseline end
dc.ClosePath()
dc.SetRGBA(r, g, b, fillAlpha)
dc.Fill()
// Then stroke the top line separately at higher opacity
dc.SetRGBA(r, g, b, lineAlpha)
dc.SetLineWidth(1.5)
dc.Stroke()
```

**Layout constants (from measured font metrics):**

```
24pt GoRegular: ascent=23, descent=6  → primary baseline = 4 + 23 = 27
12pt GoRegular: ascent=12, descent=3  → multi-line baselines = 14, 28
 9pt GoRegular: ascent=9,  descent=2  → label baseline = H - 3 = 45
```

**Zone identity colours:**

Each zone's graph/tint colour comes from `zoneConfig.ThemeOverride.Accent` if
set, falling back to the plugin's descriptor accent, falling back to the global
theme accent. This is how zones get distinct colours (weather = amber, CPU =
green, GPU = red, network = cyan) regardless of severity.

Text colour reflects severity (white = OK, yellow = warn, red = crit). Graph
colour is always the zone's identity colour.

---

### Step 3 — Wire format: JPEG for HID path

In `app.go` renderLoop, encode frames as JPEG quality=95 (not PNG) before
sending to the USB device. The WebSocket broadcast stays PNG/base64 — Flutter
needs lossless for the preview.

```go
// HID path (fast, lossy OK — hardware display has its own JPEG decoder)
var buf bytes.Buffer
jpeg.Encode(&buf, frame, &jpeg.Options{Quality: 95})
device.SendFrame(ctx, buf.Bytes())

// WS path (stays as-is — lossless for Flutter preview)
png.Encode(&wsBuf, frame)
```

Note: verify the Nexus USB protocol accepts JPEG before enabling. The current
`SendFrame` signature takes raw pixel bytes — may need to stay raw RGBA and let
the device handle it. Investigate before changing.

---

### Step 4 — Plugin payload conventions

Document and enforce the following payload conventions so plugins produce
consistent results with the new renderer:

| Field | Convention |
|---|---|
| `GraphBgOpacity` | 0 = use renderer default (18%). Plugins should not set this. |
| `GraphLineOpacity` | 0 = use renderer default (75%). Plugins should not set this. |
| `Severity` | Used for **text colour only**. Does not affect graph colour. |
| `Primary` with `\n` | Renders as two lines using 12pt font. Max 2 lines. |
| `Secondary` | Bottom-anchored label. Omit for multi-line Primary zones. |
| `Icon` | Shown left of Primary for single-line zones. Omitted when graph is present unless meaningful (weather icon). |

Update all four external plugins (`weather`, `cpu-temp`, `gpu-temp`, `network`)
to follow these conventions.

---

### Step 5 — Design validation

After implementing, capture live frames from all three pages and compare against
the target mockups (`/tmp/mock_final_all.png`). The target is:

- Zone tints visible and distinct (amber/green/red/cyan)
- Graph fill clearly visible at 18% opacity
- Graph line crisp at 1.5px width
- Primary value top-anchored with correct baseline
- Label bottom-anchored, readable at 9pt with HintingFull
- No text truncation or collision on any page/zone combination

---

### Step 6 — Flutter UI: background image selection

Wire the Images tab into the display settings so the user can select an uploaded
image or GIF as the hardware display background. The backend pipeline
(`compositor.go` + `manager.SetBackground()`) is already implemented.

What's missing:
- Flutter `SettingsState` needs a `backgroundImage` field synced to the backend
- Display tab needs a "Background image" selector showing uploaded images
- Selecting an image saves to backend config and triggers immediate preview update
- A "None" option clears the background

---

## Sequence

```
1. Add fogleman/gg dependency                        ✅ done
2. Rewrite renderer.go with gg                       ✅ done
3. Update plugin payload conventions                 ✅ done (opacities zeroed)
4. Design validation — capture + compare frames      ✅ done
5. JPEG wire format investigation + change           (pending — needs protocol check)
6. Flutter UI: background image selection            (pending)
```

Steps 1–4 are complete. Steps 5–6 are improvements that can ship separately.

### What was delivered (Steps 1–4)

- `fogleman/gg` added as a direct dependency (`go.mod`)
- `renderer.go` rewritten from 1,270 lines of hand-rolled pixel loops to ~360
  lines using gg path operations. Key improvements:
  - Graph fill: proper closed polygon path → `Fill()` — no more gap artifacts
  - Graph line: 1.5px anti-aliased stroke with round caps/joins
  - Zone tint: luminance-aware accent colour wash (4–14% alpha)
  - Font faces: loaded via `fonts.Manager` which already applies `HintingFull`
    via `truetype.NewFace` — correct pixel-grid alignment at small sizes
  - Layout: value top-anchored (baseline=27), label bottom-anchored (baseline=45)
  - Alignment-aware: `AlignCenter` centres text horizontally; display-only zones
    (no graph, no secondary) get full vertical centring (clock zone)
  - Text colour: accent colour for display-only zones; severity colour for data zones
  - Truncation with ellipsis via `dc.MeasureString`
- Plugin opacities zeroed (cpu-temp, gpu-temp, network) — renderer defaults
  (28% fill, 90% line) apply
- Compositor background image/GIF pipeline implemented and wired into `app.go`

---

## Out of scope for this plan

- Subpixel rendering (requires specific LCD subpixel layout knowledge)
- Stem darkening (requires gamma-correct blending pipeline, medium-confidence
  research finding, skip for now)
- GPU-accelerated rendering (overkill for 640×48 at 30fps)
- Changing the USB wire format (needs protocol investigation first)
