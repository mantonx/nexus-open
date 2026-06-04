# Zone Layout System Design

## Overview

A flexible, visually appealing layout system for rendering content in the 160x48px zones of the Corsair iCUE Nexus display.

---

## Design Goals

1. **Flexibility**: Support diverse content types (single values, multi-line text, icons, graphs)
2. **Visual Appeal**: Clean hierarchy, proper spacing, professional appearance
3. **No Overlaps**: Intelligent positioning that prevents element collision
4. **Maintainability**: Clear separation of measurement → layout → rendering phases
5. **Performance**: Calculate once, render once

---

## Current Problems

- **Layered rendering without layout planning**: Elements drawn independently
- **Icon positioning conflicts**: No awareness of text positioning
- **Center alignment issues**: No safe corner for icons when text is centered
- **No multi-line support**: Cannot stack network speeds or show weather descriptions

---

## Proposed Layout Engine

### Phase 1: Content Analysis & Measurement

Measure all elements before positioning:

```go
type LayoutMeasurements struct {
    // Icon
    HasIcon    bool
    IconWidth  int
    IconHeight int

    // Primary text
    PrimaryLines    []string  // Split on \n
    PrimaryWidths   []int     // Width of each line
    PrimaryHeight   int       // Total height with line spacing

    // Secondary text
    HasSecondary    bool
    SecondaryWidth  int
    SecondaryHeight int

    // Graph
    HasGraph bool
}
```

**Key Functions:**
- `measureText(text, face) → (width, height)`
- `splitMultiline(text) → []string`
- `measureAllElements(payload) → LayoutMeasurements`

---

### Phase 2: Layout Strategy Selection

Choose rendering strategy based on content type:

```go
type LayoutStrategy int

const (
    // Hero centered value with optional graph background
    // Use when: Graph present, no icon needed
    // Visual: Large centered text, graph tells the story
    LayoutHeroValue LayoutStrategy = iota

    // Icon + text on same baseline (horizontal)
    // Use when: Icon present, no graph, no multi-line
    // Visual: ☁ 57°F
    LayoutIconValue

    // Multi-line text with optional icon
    // Use when: Primary text contains \n
    // Visual: ↓ 241Kb
    //         ↑ 80Kb
    LayoutMultiLine

    // Icon + value with label below
    // Use when: Icon + secondary text
    // Visual: ☁ 57°F
    //        Partly Cloudy
    LayoutIconValueLabel
)
```

**Selection Logic:**
```go
func SelectLayoutStrategy(measurements LayoutMeasurements) LayoutStrategy {
    // Multi-line primary text takes precedence
    if len(measurements.PrimaryLines) > 1 {
        return LayoutMultiLine
    }

    // With graph, use hero layout (graph provides visual context)
    if measurements.HasGraph {
        return LayoutHeroValue
    }

    // Icon + value combinations
    if measurements.HasIcon {
        if measurements.HasSecondary {
            return LayoutIconValueLabel
        }
        return LayoutIconValue
    }

    // Default to hero for single values
    return LayoutHeroValue
}
```

---

### Phase 3: Position Calculation

Calculate exact pixel positions for all elements:

```go
type ElementPositions struct {
    // Icon position (if present)
    IconX, IconY int

    // Primary text position(s) - may be multi-line
    PrimaryLines []LinePosition

    // Secondary text position (if present)
    SecondaryX, SecondaryY int
}

type LinePosition struct {
    Text string
    X, Y int
}
```

**Position calculation per strategy:**

#### LayoutHeroValue
- Primary: Center-aligned, vertically centered
- Secondary: Center-aligned, below primary
- No icon

#### LayoutIconValue
- Calculate: `totalWidth = iconWidth + 8px spacing + textWidth`
- Starting X based on alignment:
  - Left: `paddingH`
  - Center: `(zoneWidth - totalWidth) / 2`
  - Right: `zoneWidth - totalWidth - paddingH`
- Icon: `(startX, verticalCenter)`
- Text: `(startX + iconWidth + 8, verticalCenter)`
- Secondary: Below, aligned with text

#### LayoutMultiLine
- Each line measured independently
- Lines left-aligned at `paddingH`
- Vertical spacing: 12px between line baselines
- First line baseline: `height/2`
- Icon: If present, left of first line
- Secondary: Below last line

#### LayoutIconValueLabel
- Icon + primary on first line (same as LayoutIconValue)
- Secondary below, indented to align with primary text (not icon)

---

### Phase 4: Rendering

Execute rendering with calculated positions:

```go
func (r *Renderer) Render(payload module.Payload) (*image.RGBA, error) {
    // 1. Background
    drawBackground(img)
    drawGraph(img, payload.Spark)

    // 2. Measure all elements
    measurements := r.measureAllElements(payload)

    // 3. Select strategy
    strategy := r.selectLayoutStrategy(measurements)

    // 4. Calculate positions
    positions := r.calculatePositions(strategy, measurements)

    // 5. Render elements at calculated positions
    r.renderElements(img, positions, measurements, colors)

    return img
}
```

---

## Visual Examples

### Network Module (Multi-line)
```
┌─────────────────────────────────────┐ 160x48px
│                                     │
│  ↓ 241Kb          ░▓▓░░▓▓░░▓░░░    │ Line 1: y=20
│  ↑ 80Kb                             │ Line 2: y=32
│                                     │
└─────────────────────────────────────┘
```
- Strategy: `LayoutMultiLine`
- Primary: 2 lines, left-aligned, 12px spacing
- Secondary: "Network" below
- Graph: Sparkline background

### Weather Module (Icon + Value + Label)
```
┌─────────────────────────────────────┐ 160x48px
│                                     │
│  ☁  57°F                            │ y=22 (centered)
│     Partly Cloudy                   │ y=38
│                                     │
└─────────────────────────────────────┘
```
- Strategy: `LayoutIconValueLabel`
- Icon: Left-aligned at paddingH (16px)
- Primary: 8px spacing from icon
- Secondary: Below, aligned with primary text (not icon)
- No graph for weather

### CPU/GPU Module (Hero with graph)
```
┌─────────────────────────────────────┐ 160x48px
│                                     │
│            53°C        ░░▓▓▓▓▓░░░   │ y=24 (centered)
│          GPU Temp                   │ y=38
│                                     │
└─────────────────────────────────────┘
```
- Strategy: `LayoutHeroValue`
- Primary: Center-aligned, large (14pt)
- Secondary: Center-aligned below
- Graph: Atmospheric background
- No icon (graph provides context)

---

## Spacing & Typography

### Grid System
- **Base unit**: 8px
- **Horizontal padding**: 16px (2 units)
- **Vertical padding**: 8px (1 unit)
- **Icon-to-text spacing**: 8px
- **Line spacing** (multi-line): 12px between baselines
- **Primary-to-secondary spacing**: 10px

### Font Sizes
- **Primary**: 14pt (currently, will be 24pt in future)
- **Secondary**: 10pt (currently, will be 9pt in future)
- **Icon**: 18pt

### Color Hierarchy
- **Primary text**: Full brightness, severity-aware color
- **Secondary text**: Muted color (60% opacity)
- **Icon**: Matches primary color
- **Graph**: Atmospheric (30% opacity) background

---

## Implementation Plan

### Step 1: Measurement Infrastructure
- [ ] Add `measureText()` helper
- [ ] Add `splitMultiline()` to split on `\n`
- [ ] Add `measureAllElements()` to create `LayoutMeasurements`

### Step 2: Multi-line Text Rendering
- [ ] Add `drawMultilineText()` function
- [ ] Handle line spacing and alignment
- [ ] Support optional icon on first line

### Step 3: Layout Strategy System
- [ ] Define `LayoutStrategy` enum
- [ ] Implement `selectLayoutStrategy()`
- [ ] Implement position calculation for each strategy

### Step 4: Refactor Render Pipeline
- [ ] Update `Render()` to use new phased approach
- [ ] Replace individual draw calls with unified rendering
- [ ] Remove old icon positioning code

### Step 5: Testing
- [ ] Test network module with stacked speeds
- [ ] Test weather module with icon + description
- [ ] Test CPU/GPU with centered hero layout
- [ ] Verify no overlaps in any scenario

---

## Module Updates

### Network Module ✓
Changed from:
```go
Primary: "↓241Kb ↑80Kb"
```
To:
```go
Primary: "↓ 241Kb\n↑ 80Kb"
```

### Weather Module ✓
Changed from:
```go
Secondary: data.Location  // "Jersey City, NJ"
```
To:
```go
Secondary: data.Description  // "Partly Cloudy"
```

---

## Benefits

1. **No More Overlaps**: All positions calculated before rendering
2. **Flexible**: Easy to add new layout strategies
3. **Consistent**: All layouts follow same spacing rules
4. **Maintainable**: Clear separation of concerns
5. **Visually Appealing**: Each content type gets optimal presentation
6. **Multi-line Support**: Network speeds stack, weather descriptions show

---

## Future Enhancements

- **Dynamic font scaling**: Reduce font size if content doesn't fit
- **Text truncation**: Ellipsis for overflowing text
- **Icon scaling**: Adjust icon size based on available space
- **Grid layouts**: Support 2x2 grid for multiple values
- **Animation**: Smooth transitions between layouts
