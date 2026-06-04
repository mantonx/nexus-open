# Nexus Open - Modern Dashboard UI Redesign

**Status:** Planning
**Started:** 2025-10-22
**Target:** v2.0 Visual Overhaul

---

## Executive Summary

The current UI for Nexus Open feels like a prototype. This document outlines a comprehensive redesign to create a modern dashboard aesthetic with professional polish, leveraging the unique constraints of the 640x48px Corsair iCUE Nexus display.

**Design Philosophy:** Aggressive modernization - large, scannable data with atmospheric context, not cluttered information displays.

---

## Hardware Context

- **Display:** Corsair iCUE Nexus
- **Resolution:** 640x48 pixels (ultra-wide, minimal height)
- **Layout:** 4 zones @ 160px width each (2px gutters between)
- **Interaction:** Touch-enabled with swipe gestures
- **Constraint:** Extreme vertical limitation demands perfect information hierarchy

---

## Current State Analysis

### What's Working
- ✅ Solid architecture with zone-based layout system
- ✅ Multiple graph types (sparkline, bar, area, line)
- ✅ Theme system with per-zone overrides
- ✅ Smooth transitions with easing functions
- ✅ Touch interaction with velocity-aware filtering
- ✅ Module plugin system for extensibility

### Critical Issues
- ❌ **Typography too small** (14pt primary, 11pt secondary) - not scannable
- ❌ **Weak visual hierarchy** - all elements compete equally
- ❌ **Cramped spacing** (6px H, 4px V) - feels claustrophobic
- ❌ **Disconnected graphs** - tacked on at bottom, not integrated
- ❌ **Muted colors too dim** (#9AA0A6) - poor readability
- ❌ **Arbitrary positioning** - manual pixel tweaks (icon moved up 1px)
- ❌ **Prototype feel** - lacks cohesive design system

---

## Design Direction: Modern Dashboard

### Selected Approach
**Aggressive Complete Redesign** with:
1. **Background graphs** - Subtle atmospheric context, doesn't take space
2. **Smart icons** - Only shown when meaningful, not decorative
3. **Zone backgrounds** - Subtle visual separation (#141414 vs #101010)
4. **Bold typography** - Hero numbers that demand attention

### Visual Design Principles

#### 1. Information Hierarchy
```
PRIMARY DATA (24pt bold, accent color)
  ↓ The hero - instant recognition

Graph Background (15% opacity fill, 40% line)
  ↓ Atmospheric context

Secondary Label (9pt, improved gray)
  ↓ Contextual information

Icon (18pt, conditional)
  ↓ Visual anchor when meaningful
```

#### 2. Typography Scale
```
Current:                    New:
- Primary: 14pt            - Primary: 24pt bold
- Secondary: 11pt          - Secondary: 9pt regular
- Icon: 14pt              - Icon: 18pt (conditional)
```

**Rationale:** With 48px height, maximum contrast between hero data and labels = faster recognition. Bigger primary, smaller secondary = clear hierarchy.

#### 3. Spacing System (8px Grid)
```
Current:                    New:
- H Padding: 6px           - H Padding: 16px
- V Padding: 4px           - V Padding: 8px
- Element spacing: ad-hoc  - Element spacing: 8px units
- Text positions: arbitrary - Text: vertically centered
```

**Rationale:** Consistent 8px grid creates rhythm. More horizontal padding uses 160px width better.

#### 4. Color Palette
```
Background base:      #101010 (main background)
Zone background:      #141414 (subtle lift for separation)
Primary text:         Accent color or severity color
Label text:           #B8BDC2 (was #9AA0A6 - improved contrast)
Graph bg fill:        Accent @ 15% opacity (subtle atmosphere)
Graph line:           Accent @ 40% opacity (visible but not dominant)

Severity colors:
- OK:   Accent (#00C8FF - cyan)
- WARN: #FFB020 (orange/yellow)
- CRIT: #FF4444 (red)
```

#### 5. Zone Visual Treatment
```
┌────────────────────────────────────────────────┐
│ Zone Background #141414 (160x48px)            │
│                                                │
│  [graph area fill 15% opacity ............... ]│
│  [graph line 40% opacity ──────────────────── ]│
│                                                │
│        75°F  ← 24pt bold accent               │
│        Jersey City  ← 9pt #B8BDC2             │
│                                                │
└────────────────────────────────────────────────┘
     ↑                           ↑
  Subtle lift               Atmospheric graph
  from #101010              behind text
```

---

## Implementation Plan

### Phase 1: Core Rendering System Redesign

**Objective:** Transform renderer from "elements stacked" to "integrated composition"

#### 1.1 Theme System Updates
**File:** `internal/zone/config.go`

```go
// Update DefaultTheme() (Lines 49-59)
func DefaultTheme() Theme {
    return Theme{
        Bg:                "#101010",  // Main background
        Fg:                "#EAEAEA",  // Keep
        Muted:             "#B8BDC2",  // NEW: Improved from #9AA0A6
        Accent:            "#00C8FF",  // Keep
        GutterPx:          2,          // Keep
        FontSizePrimary:   24,         // NEW: Was 14
        FontSizeSecondary: 9,          // NEW: Was 10
    }
}

// Add new theme properties
type Theme struct {
    // ... existing fields ...
    ZoneBg              string `yaml:"zone_bg,omitempty" json:"zone_bg,omitempty"`
    GraphBackgroundOpacity int `yaml:"graph_bg_opacity,omitempty" json:"graph_bg_opacity,omitempty"`
    GraphLineOpacity    int    `yaml:"graph_line_opacity,omitempty" json:"graph_line_opacity,omitempty"`
}
```

**Default Values:**
- `ZoneBg`: "#141414" (subtle lift)
- `GraphBackgroundOpacity`: 15 (15% for area fill)
- `GraphLineOpacity`: 40 (40% for line)

#### 1.2 Renderer Complete Rewrite
**File:** `internal/zone/renderer.go`

**New rendering order:**
```go
func (r *Renderer) Render(payload module.Payload) (*image.RGBA, error) {
    img := image.NewRGBA(image.Rect(0, 0, r.width, r.height))

    // 1. Fill main background
    draw.Draw(img, img.Bounds(), &image.Uniform{r.theme.GetBgColor()}, image.Point{}, draw.Src)

    // 2. Draw subtle zone background
    r.drawZoneBackground(img)

    // 3. Draw graph as full-height atmospheric background
    if len(payload.Spark) > 0 {
        r.drawBackgroundGraph(img, payload.Spark, payload.GraphType, r.getSeverityColor(payload.Severity))
    }

    // 4. Draw large primary text (vertically centered)
    if payload.Primary != "" {
        primaryColor := r.getSeverityColor(payload.Severity)
        r.drawCenteredPrimaryText(img, payload.Primary, primaryColor)
    }

    // 5. Draw small secondary label (below primary)
    if payload.Secondary != "" {
        labelColor := r.theme.GetMutedColor()
        r.drawSecondaryLabel(img, payload.Secondary, labelColor)
    }

    // 6. Draw icon only if meaningful (no graph OR special case)
    if r.shouldShowIcon(payload) {
        iconColor := r.getSeverityColor(payload.Severity)
        r.drawIcon(img, payload.Icon, iconColor)
    }

    return img, nil
}
```

**New Methods to Implement:**

```go
// drawZoneBackground fills zone with subtle background color
func (r *Renderer) drawZoneBackground(img *image.RGBA) {
    zoneBg := r.theme.GetZoneBgColor() // #141414
    draw.Draw(img, img.Bounds(), &image.Uniform{zoneBg}, image.Point{}, draw.Src)
}

// drawBackgroundGraph renders graph as full-height atmospheric background
func (r *Renderer) drawBackgroundGraph(img *image.RGBA, data []float32, graphType module.GraphType, col color.RGBA) {
    // 1. Draw area fill at 15% opacity (atmospheric)
    // 2. Draw line at 40% opacity (visible but subtle)
    // 3. Full height (0 to 48px), respects padding
}

// drawCenteredPrimaryText renders large hero text vertically centered
func (r *Renderer) drawCenteredPrimaryText(img *image.RGBA, text string, col color.RGBA) {
    // 24pt bold, vertically centered in 48px height
    // Horizontal: respect alignment (left/center/right)
}

// drawSecondaryLabel renders small contextual label
func (r *Renderer) drawSecondaryLabel(img *image.RGBA, text string, col color.RGBA) {
    // 9pt regular, positioned below primary
    // Higher contrast color (#B8BDC2)
}

// shouldShowIcon determines if icon should be displayed
func (r *Renderer) shouldShowIcon(payload module.Payload) bool {
    // Show icon if:
    // - No graph data (len(Spark) == 0), OR
    // - Icon is semantically meaningful (weather condition, warning)
    hasGraph := len(payload.Spark) > 0
    isMeaningful := payload.Icon == "cloud" ||
                    payload.Icon == "cloud-rain" ||
                    payload.Severity != module.SeverityOK

    return !hasGraph || isMeaningful
}
```

#### 1.3 Font Loading Updates
**File:** `internal/zone/renderer.go` (Lines 44-56)

```go
// Load 24pt primary font (was 14pt)
if face, _, err := fontManager.LoadBestAvailableFont(24); err == nil {
    r.primaryFace = face
} else {
    logger.Warn("failed to load primary font", "error", err)
    r.primaryFace = basicfont.Face7x13
}

// Load 9pt secondary font (was 11pt)
if face, _, err := fontManager.LoadBestAvailableFont(9); err == nil {
    r.secondaryFace = face
} else {
    logger.Warn("failed to load secondary font", "error", err)
    r.secondaryFace = basicfont.Face7x13
}

// Load 18pt icon font (was 14pt)
if iconFace, err := fontManager.GetFace("FontAwesome-Solid", 18); err == nil {
    r.iconFace = iconFace
} else {
    logger.Warn("failed to load icon font", "error", err)
}
```

### Phase 2: Background Graph Implementation

**Objective:** Create atmospheric graph that doesn't compete with text

#### 2.1 Background Graph Rendering
```go
func (r *Renderer) drawBackgroundGraph(img *image.RGBA, data []float32, graphType module.GraphType, col color.RGBA) {
    if len(data) == 0 {
        return
    }

    const paddingH = 16
    const paddingV = 8

    // Full height minus vertical padding
    graphHeight := r.height - (2 * paddingV)
    availableWidth := r.width - (2 * paddingH)

    // Create very transparent fill color (15% opacity)
    fillColor := color.RGBA{
        R: col.R,
        G: col.G,
        B: col.B,
        A: 38, // 15% of 255
    }

    // Create semi-transparent line color (40% opacity)
    lineColor := color.RGBA{
        R: col.R,
        G: col.G,
        B: col.B,
        A: 102, // 40% of 255
    }

    // Render based on graph type
    switch graphType {
    case module.GraphTypeArea:
        r.drawBackgroundAreaGraph(img, data, fillColor, lineColor, paddingH, paddingV, graphHeight, availableWidth)
    case module.GraphTypeBar:
        r.drawBackgroundBarGraph(img, data, fillColor, lineColor, paddingH, paddingV, graphHeight, availableWidth)
    case module.GraphTypeLine:
        r.drawBackgroundLineGraph(img, data, lineColor, paddingH, paddingV, graphHeight, availableWidth)
    default: // Sparkline
        r.drawBackgroundSparkline(img, data, fillColor, lineColor, paddingH, paddingV, graphHeight, availableWidth)
    }
}
```

#### 2.2 Area Graph (Most Common)
```go
func (r *Renderer) drawBackgroundAreaGraph(img *image.RGBA, data []float32,
    fillColor, lineColor color.RGBA, paddingH, paddingV, height, width int) {

    yBase := r.height - paddingV

    // First pass: Fill area
    for i := 0; i < len(data); i++ {
        val := clampFloat(data[i])
        x := paddingH + (i * width / len(data))
        y := yBase - int(float32(height) * val)

        // Fill column from bottom to value
        for py := y; py < yBase; py++ {
            if x >= 0 && x < r.width && py >= 0 && py < r.height {
                // Blend with existing pixel for transparency
                existing := img.At(x, py)
                blended := r.blendColors(existing.(color.RGBA), fillColor)
                img.Set(x, py, blended)
            }
        }
    }

    // Second pass: Draw top line
    for i := 0; i < len(data)-1; i++ {
        val1 := clampFloat(data[i])
        val2 := clampFloat(data[i+1])

        x1 := paddingH + (i * width / len(data))
        x2 := paddingH + ((i + 1) * width / len(data))
        y1 := yBase - int(float32(height) * val1)
        y2 := yBase - int(float32(height) * val2)

        r.drawBlendedLine(img, x1, y1, x2, y2, lineColor)
    }
}
```

### Phase 3: Module Updates

**Objective:** Update each module to leverage new design system

#### 3.1 Weather Module
**File:** `modules/weather/main.go`

```go
// Leverage background graph for temperature history
return module.Payload{
    Primary:   fmt.Sprintf("%d°%s", temp, unit),  // Large temp
    Secondary: location,                           // Small location
    Severity:  module.SeverityOK,
    Spark:     tempHistory,                        // Background graph
    GraphType: module.GraphTypeArea,               // Atmospheric area
    Icon:      weatherIcon,                        // Cloud icon (meaningful)
    TTL:       5 * time.Minute,
    Timestamp: time.Now(),
}, nil
```

#### 3.2 CPU/GPU Temperature Modules
**Files:** `modules/cpu-temp/main.go`, `modules/gpu-temp/main.go`

```go
// Background graph shows thermal history
return module.Payload{
    Primary:   fmt.Sprintf("%d°%s", temp, unit),  // Large temp
    Secondary: "CPU Load 45%",                     // Context
    Severity:  severity,                           // Color-coded
    Spark:     tempHistory,                        // Background graph
    GraphType: module.GraphTypeLine,               // Crisp line graph
    Icon:      "",                                 // No icon (graph tells story)
    TTL:       2 * time.Second,
    Timestamp: time.Now(),
}, nil
```

#### 3.3 Network Module
**File:** `modules/network/main.go`

```go
// Already updated to side-by-side format, add background
return module.Payload{
    Primary:   fmt.Sprintf("↓%s ↑%s", downStr, upStr),  // Both speeds
    Secondary: interfaceName,                            // eth0, wlan0, etc
    Severity:  module.SeverityOK,
    Spark:     downloadHistory,                          // Background sparkline
    GraphType: module.GraphTypeSparkline,                // Fast line
    Icon:      "",                                       // No icon needed
    TTL:       2 * time.Second,
    Timestamp: time.Now(),
}, nil
```

---

## Future Polish Opportunities

### High Priority

#### 1. Animated Transitions (internal/zone/transition.go)
- Add spring easing for bounce-back effects
- Implement blur transitions during swipe
- Add scale/zoom for page changes
- Make transition duration configurable

**Impact:** Feels premium and polished
**Effort:** Medium (1-2 days)

#### 2. Touch Feedback (internal/touch/handler.go)
- Implement haptic feedback on swipe
- Add visual tap highlights
- Create long-press detection
- Add resistance during invalid swipes

**Impact:** Much better interaction feel
**Effort:** Medium (2-3 days)

#### 3. Loading/Error States (internal/zone/compositor.go)
- Animated loading spinners
- Pulsing error indicators
- Skeleton screens for initial load
- Retry indicators with countdown

**Impact:** Professional error handling
**Effort:** Low (1 day)

### Medium Priority

#### 4. Advanced Graph Rendering (internal/zone/renderer.go)
- Gradient fills for area graphs
- Smooth curve interpolation (Catmull-Rom)
- Data point markers on lines
- Animated graph drawing

**Impact:** More polished visualizations
**Effort:** High (3-4 days)

#### 5. Typography Improvements (internal/fonts/manager.go)
- Font antialiasing/subpixel rendering
- Text shadow/outline rendering
- Better baseline alignment
- Kerning optimizations

**Impact:** Crisper, more readable text
**Effort:** High (4-5 days)

#### 6. Adaptive Brightness (internal/device/device.go)
- Smooth brightness transitions
- Auto-adjust based on ambient light
- Eye-comfort mode with color temperature
- Gamma correction for contrast

**Impact:** Better visibility in all conditions
**Effort:** Medium (2-3 days)

### Low Priority

#### 7. Performance Optimization
- Double buffering for flicker-free rendering
- Dirty-region tracking to skip unchanged zones
- Frame rate limiting to save power
- Memory pooling for image buffers

**Impact:** Smoother performance, lower power
**Effort:** High (5-6 days)

#### 8. Color System Enhancements
- Gradient support for backgrounds
- Light/dark mode variations
- Theme transition animations
- Color accessibility checker

**Impact:** More visual options
**Effort:** Medium (2-3 days)

---

## Testing Strategy

### Visual Regression Testing
1. Capture screenshots before/after each phase
2. Compare rendering with different data scenarios
3. Test all graph types with new background treatment
4. Verify text readability at all sizes

### Module Testing
1. Test each module with new renderer
2. Verify graph background doesn't obscure text
3. Test icon visibility logic
4. Validate severity color changes

### Performance Testing
1. Profile rendering time before/after
2. Measure memory usage with new fonts
3. Test with rapid data updates
4. Validate smooth transitions

---

## Risk Analysis

### High Risk
**Text readability over background graphs**
- **Mitigation:** Very low graph opacity (15%), high text contrast
- **Fallback:** Increase background blur, add text outline/shadow

**Larger fonts might not fit**
- **Mitigation:** Test with actual data, adjust if needed
- **Fallback:** Dynamic font sizing based on content length

### Medium Risk
**Breaking existing modules**
- **Mitigation:** Update modules incrementally, test each one
- **Fallback:** Module compatibility layer for old rendering style

**Performance regression**
- **Mitigation:** Profile after each phase, optimize hot paths
- **Fallback:** Cache rendered elements, use dirty-region tracking

### Low Risk
**Over-designed appearance**
- **Mitigation:** Keep graph subtle (15% opacity), use restraint
- **Fallback:** Make opacity configurable in theme

---

## Success Metrics

### Qualitative
- [ ] UI no longer feels like a prototype
- [ ] Data is scannable at a glance
- [ ] Visual hierarchy is immediately clear
- [ ] Graphs enhance rather than distract
- [ ] Professional, modern aesthetic

### Quantitative
- [ ] Primary text size increased from 14pt to 24pt (+71%)
- [ ] Label contrast improved from #9AA0A6 to #B8BDC2 (+20% lighter)
- [ ] Horizontal padding increased from 6px to 16px (+167%)
- [ ] Graph integration doesn't impact rendering performance (<5% regression)

---

## Timeline

### Week 1: Core Redesign
- **Day 1-2:** Theme updates, renderer rewrite
- **Day 3-4:** Background graph implementation
- **Day 5:** Module updates and testing

### Week 2: Polish & Refinement
- **Day 1-2:** Animated transitions
- **Day 3-4:** Touch feedback
- **Day 5:** Error states and final testing

### Week 3+: Future Enhancements
- Advanced graph rendering
- Typography improvements
- Adaptive brightness
- Performance optimization

---

## Files Modified

### Core Rendering
- `internal/zone/config.go` - Theme system updates
- `internal/zone/renderer.go` - Complete rewrite (548 lines)
- `internal/zone/compositor.go` - Zone background support

### Modules
- `modules/weather/main.go` - Graph integration
- `modules/cpu-temp/main.go` - Graph integration
- `modules/gpu-temp/main.go` - Graph integration
- `modules/network/main.go` - Already updated

### Documentation
- `docs/UI_REDESIGN.md` - This document
- `README.md` - Update screenshots

---

## References

### Design Inspiration
- Modern automotive HUDs (BMW iDrive, Tesla displays)
- Smart home dashboards (Nest, Home Assistant)
- Gaming peripheral displays (Elgato Stream Deck)
- Terminal UI libraries (Textual, Rich)

### Technical References
- Go image/draw package documentation
- TrueType font rendering with golang.org/x/image/font
- Color blending algorithms
- Typography best practices for small displays

---

## Appendix: Before/After Comparison

### Before (Current)
```
┌────────────────────┐
│ ☁ 72°F            │  <- 14pt text, small icon
│    0.0b/s          │  <- 11pt secondary
│ ▁▂▃▄▅▆▇█ (graph)  │  <- Disconnected sparkline
└────────────────────┘
   Cramped, prototype feel
```

### After (Redesigned)
```
┌────────────────────┐
│ [subtle atmospheric│  <- Background graph (15% opacity)
│  graph fills zone] │
│                    │
│    75°F            │  <- 24pt hero number
│    Jersey City     │  <- 9pt label, better contrast
│                    │
└────────────────────┘
   Spacious, modern dashboard
```

---

**Document Version:** 1.0
**Last Updated:** 2025-10-22
**Author:** Nexus Open Development Team
**Status:** Ready for Implementation
