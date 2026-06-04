# Flexible Zone Layout System - Redesign

## Core Philosophy

**Stop thinking in strategies. Start thinking in elements and relationships.**

Instead of predefined layouts, let's describe what we have and let the renderer figure it out.

---

## The Problem with Our Current Approach

```
Current: payload → pick strategy → calculate positions → render
Problem: Rigid, not extensible, lots of if/else logic
```

**Better approach:**
```
payload → parse into elements → apply constraints → flow layout → render
```

---

## Key Insight: Content as a Flow

Think of the zone as a **container** with **content that flows naturally**.

### Elements We Have:

1. **Icon** (optional) - 18pt FontAwesome glyph
2. **Primary Text** (required) - Can be single or multi-line
3. **Secondary Text** (optional) - Label or description
4. **Graph** (optional) - Background visualization

### Natural Flow Rules:

```
IF icon exists:
  - Icon flows first (left side)
  - Text flows after icon (with spacing)

IF primary has newlines:
  - Split into lines
  - Stack vertically
  - Each line left-aligned

IF secondary exists:
  - Flows below primary
  - Same horizontal alignment as primary

IF graph exists:
  - Renders as atmospheric background
  - Doesn't affect text flow
```

---

## Visual Design: Content Blocks

Think of content as **blocks that stack and flow**:

### Example 1: Weather (Icon + Text Block)

```
┌─────────────────────────────────────┐
│ padding                             │
│ ┌──┐  ┌─────────────┐               │
│ │☁ │  │ 57°F        │ ← Primary     │
│ │  │  │ Partly      │ ← Secondary   │
│ └──┘  │ Cloudy      │               │
│       └─────────────┘               │
│ padding                             │
└─────────────────────────────────────┘
     ↑        ↑
   icon    text block
   flows   flows with
   first   8px spacing
```

**Layout logic:**
1. Measure icon: 18px wide
2. Measure text block: Primary + Secondary heights
3. Vertically center the GROUP (icon + text)
4. Position icon at paddingH
5. Position text at paddingH + iconWidth + 8px
6. Stack secondary below primary

### Example 2: Network (Multi-line Text Block)

```
┌─────────────────────────────────────┐
│ padding                             │
│ ┌─────────────┐  ░░▓▓░░▓▓░         │
│ │ ↓ 241Kb     │ ← Line 1           │
│ │ ↑ 80Kb      │ ← Line 2           │
│ │ Network     │ ← Secondary        │
│ └─────────────┘                     │
│ padding                             │
└─────────────────────────────────────┘

  text block    atmospheric
  left-aligned  graph background
  (no icon)
```

**Layout logic:**
1. Split primary on `\n`: ["↓ 241Kb", "↑ 80Kb"]
2. Measure text block height: 2 lines + secondary
3. Vertically center the text block
4. Left-align at paddingH
5. Stack lines with 12px spacing
6. Stack secondary below with 6px spacing

### Example 3: CPU/GPU (Centered Text Block)

```
┌─────────────────────────────────────┐
│ padding                             │
│         ┌───────────┐  ░░▓▓▓▓▓░░    │
│         │   53°C    │ ← Primary     │
│         │  GPU Temp │ ← Secondary   │
│         └───────────┘               │
│ padding                             │
└─────────────────────────────────────┘

  text block        atmospheric
  center-aligned    graph background
  (no icon)
```

**Layout logic:**
1. Measure text block height
2. Vertically center the text block
3. Center-align primary horizontally
4. Center-align secondary horizontally below
5. No icon

---

## Implementation: The Layout Engine

### Step 1: Parse Payload into Content Model

```go
type ContentModel struct {
    Icon         *IconContent      // nil if no icon
    PrimaryLines []string          // Split on \n
    Secondary    string            // Empty if none
    GraphData    []float32         // nil if no graph
    GraphType    string
}

func parsePayload(payload module.Payload) ContentModel {
    model := ContentModel{
        PrimaryLines: strings.Split(payload.Primary, "\n"),
        Secondary:    payload.Secondary,
        GraphData:    payload.Spark,
        GraphType:    payload.GraphType,
    }

    // Add icon if meaningful
    if shouldShowIcon(payload) {
        model.Icon = &IconContent{
            Glyph: resolveIcon(payload.Icon),
        }
    }

    return model
}
```

### Step 2: Measure Everything

```go
type Measurements struct {
    IconWidth       int  // 0 if no icon
    IconHeight      int

    PrimaryWidths   []int  // One per line
    PrimaryHeight   int    // Total height with line spacing

    SecondaryWidth  int    // 0 if no secondary
    SecondaryHeight int

    TotalContentHeight int  // Everything stacked
}

func measureContent(model ContentModel, r *Renderer) Measurements {
    m := Measurements{}

    // Measure icon
    if model.Icon != nil {
        m.IconWidth = font.MeasureString(r.iconFace, model.Icon.Glyph).Ceil()
        m.IconHeight = 18  // Font size
    }

    // Measure primary lines
    lineHeight := 14  // Base font size
    lineSpacing := 12 // Spacing between lines

    for _, line := range model.PrimaryLines {
        width := font.MeasureString(r.primaryFace, line).Ceil()
        m.PrimaryWidths = append(m.PrimaryWidths, width)
    }
    m.PrimaryHeight = len(model.PrimaryLines) * lineSpacing
    if len(model.PrimaryLines) > 0 {
        m.PrimaryHeight -= (lineSpacing - lineHeight)  // Remove last spacing
    }

    // Measure secondary
    if model.Secondary != "" {
        m.SecondaryWidth = font.MeasureString(r.secondaryFace, model.Secondary).Ceil()
        m.SecondaryHeight = 10  // Secondary font size
    }

    // Calculate total height
    m.TotalContentHeight = m.PrimaryHeight
    if m.SecondaryHeight > 0 {
        m.TotalContentHeight += 6 + m.SecondaryHeight  // 6px spacing
    }

    return m
}
```

### Step 3: Calculate Layout (The Magic)

```go
type LayoutBox struct {
    X, Y          int
    Width, Height int
}

type CalculatedLayout struct {
    IconBox       *LayoutBox
    PrimaryBoxes  []LayoutBox  // One per line
    SecondaryBox  *LayoutBox
}

func calculateLayout(model ContentModel, measurements Measurements, zoneWidth, zoneHeight int) CalculatedLayout {
    const paddingH = 16
    const paddingV = 8

    layout := CalculatedLayout{}

    // Determine if we have an icon
    hasIcon := model.Icon != nil

    // Calculate content starting X
    contentStartX := paddingH
    if hasIcon {
        // Icon flows first
        iconCenterY := zoneHeight / 2
        layout.IconBox = &LayoutBox{
            X: paddingH,
            Y: iconCenterY - measurements.IconHeight/2,
            Width: measurements.IconWidth,
            Height: measurements.IconHeight,
        }

        // Text flows after icon
        contentStartX = paddingH + measurements.IconWidth + 8  // 8px spacing
    }

    // Calculate vertical centering for text block
    contentCenterY := zoneHeight / 2
    textBlockStartY := contentCenterY - measurements.TotalContentHeight/2

    // Position primary lines
    currentY := textBlockStartY
    for i, line := range model.PrimaryLines {
        lineWidth := measurements.PrimaryWidths[i]

        // Determine X position based on alignment
        var lineX int
        if hasIcon || r.align == AlignLeft {
            lineX = contentStartX  // Left-aligned (or after icon)
        } else if r.align == AlignCenter {
            lineX = (zoneWidth - lineWidth) / 2  // Center-aligned
        } else {
            lineX = zoneWidth - lineWidth - paddingH  // Right-aligned
        }

        layout.PrimaryBoxes = append(layout.PrimaryBoxes, LayoutBox{
            X: lineX,
            Y: currentY,
            Width: lineWidth,
            Height: 14,  // Line height
        })

        currentY += 12  // Line spacing
    }

    // Position secondary below primary
    if model.Secondary != "" {
        currentY += 6  // Spacing between primary and secondary

        // Match alignment with primary
        var secX int
        if hasIcon || r.align == AlignLeft {
            secX = contentStartX
        } else if r.align == AlignCenter {
            secX = (zoneWidth - measurements.SecondaryWidth) / 2
        } else {
            secX = zoneWidth - measurements.SecondaryWidth - paddingH
        }

        layout.SecondaryBox = &LayoutBox{
            X: secX,
            Y: currentY,
            Width: measurements.SecondaryWidth,
            Height: 10,
        }
    }

    return layout
}
```

### Step 4: Render (Simple!)

```go
func (r *Renderer) Render(payload module.Payload) (*image.RGBA, error) {
    // Create canvas
    img := image.NewRGBA(image.Rect(0, 0, r.width, r.height))

    // Background
    draw.Draw(img, img.Bounds(), &image.Uniform{r.theme.GetBgColor()}, image.Point{}, draw.Src)
    r.drawZoneBackground(img)

    // Parse content
    model := r.parsePayload(payload)

    // Draw graph as background
    if model.GraphData != nil {
        primaryColor := r.getSeverityColor(payload.Severity)
        r.drawBackgroundGraph(img, model.GraphData, model.GraphType, primaryColor)
    }

    // Measure and layout
    measurements := r.measureContent(model)
    layout := r.calculateLayout(model, measurements, r.width, r.height)

    // Get colors
    primaryColor := r.getSeverityColor(payload.Severity)
    secondaryColor := r.theme.GetMutedColor()

    // Render icon
    if layout.IconBox != nil {
        r.drawIconGlyph(img, model.Icon.Glyph, primaryColor,
            layout.IconBox.X, layout.IconBox.Y+14, r.iconFace)  // +14 for baseline
    }

    // Render primary lines
    for i, box := range layout.PrimaryBoxes {
        r.drawRawText(img, model.PrimaryLines[i], primaryColor,
            box.X, box.Y+14, r.primaryFace)  // +14 for baseline
    }

    // Render secondary
    if layout.SecondaryBox != nil {
        r.drawRawText(img, model.Secondary, secondaryColor,
            layout.SecondaryBox.X, layout.SecondaryBox.Y+10, r.secondaryFace)  // +10 for baseline
    }

    return img, nil
}
```

---

## Why This is Better

### 1. **Truly Flexible**
- Add new content types? Just add to ContentModel
- New layout constraints? Adjust calculateLayout logic
- No strategy enum, no decision trees

### 2. **Visually Appealing**
- Everything naturally centers and flows
- Consistent spacing automatically applied
- Icon + text always grouped properly

### 3. **Simple to Understand**
```
Parse → Measure → Layout → Render
```
Each step is clean and focused.

### 4. **Easy to Extend**
Want right-aligned icons? Change one line in calculateLayout.
Want bigger spacing? Change the constants.
Want responsive font sizes? Add logic in measureContent.

---

## Visual Examples with New System

### Weather Zone
```
Input:
  Icon: "cloud"
  Primary: "57°F"
  Secondary: "Partly Cloudy"

Flow:
  1. Parse: Icon + 1 primary line + secondary
  2. Measure: Icon 18px, Primary 45px, Secondary 80px, Total 30px high
  3. Layout: Vertically center 30px block
     - Icon at (16, 15)
     - Primary at (42, 15)
     - Secondary at (42, 27)
  4. Render at calculated positions
```

### Network Zone
```
Input:
  Primary: "↓ 241Kb\n↑ 80Kb"
  Secondary: "Network"

Flow:
  1. Parse: 2 primary lines + secondary
  2. Measure: Lines [55px, 55px], Secondary 50px, Total 34px high
  3. Layout: Vertically center 34px block
     - Line 1 at (16, 13)
     - Line 2 at (16, 25)
     - Secondary at (16, 37)
  4. Render at calculated positions
```

### CPU/GPU Zone
```
Input:
  Primary: "53°C"
  Secondary: "GPU Temp"
  Align: Center

Flow:
  1. Parse: 1 primary line + secondary
  2. Measure: Primary 40px, Secondary 60px, Total 30px high
  3. Layout: Vertically center 30px block, center-align
     - Primary at (60, 15)  // Centered in 160px
     - Secondary at (50, 27) // Centered in 160px
  4. Render at calculated positions
```

---

## Implementation Plan

### Phase 1: Content Model
- [ ] Create `ContentModel` struct
- [ ] Implement `parsePayload()` function
- [ ] Handle icon resolution and visibility

### Phase 2: Measurement
- [ ] Create `Measurements` struct
- [ ] Implement `measureContent()` function
- [ ] Handle multi-line text measurement
- [ ] Calculate baseline positions

### Phase 3: Layout Calculation
- [ ] Create `CalculatedLayout` struct
- [ ] Implement `calculateLayout()` function
- [ ] Handle icon + text flow
- [ ] Respect alignment settings
- [ ] Vertical centering logic

### Phase 4: Rendering
- [ ] Refactor `Render()` to use new pipeline
- [ ] Remove old draw functions (drawCenteredPrimaryText, etc.)
- [ ] Use calculated positions directly
- [ ] Test with all current modules

### Phase 5: Testing
- [ ] Weather: Icon + temp + description
- [ ] Network: Stacked speeds + label
- [ ] CPU/GPU: Centered temp + label
- [ ] Verify no overlaps
- [ ] Check visual balance

---

## Benefits Summary

✅ **Flexible**: No hardcoded strategies, just natural flow
✅ **Simple**: Clean pipeline, easy to understand
✅ **Extensible**: Add features by extending model
✅ **Maintainable**: Single layout calculation function
✅ **Visually Appealing**: Proper spacing, centering, grouping
✅ **No Overlaps**: Positions calculated before rendering

This is the way.
