# Nexus Open — UI Depth & Visual Hierarchy Improvements

**Created:** 2026-06-04  
**Based on:** Screenshot analysis + existing design token audit  
**Goal:** Move from "dark Material app" → "premium hardware control panel"

---

## Root Cause: Tonal Compression

The existing palette has three surface levels that are only ~9 luminance steps apart:

| Token            | Hex       | Luminance |
|------------------|-----------|-----------|
| `darkBg`         | `#0D0D0F` | ~2%       |
| `darkSurface`    | `#16161A` | ~4%       |
| `darkElevated`   | `#1E1E24` | ~6%       |

On most monitors these three values read as nearly identical. Cards appear
to "float on nothing" — there is no perceived depth. The fix is not to add
new colors but to use **contrast amplifiers** that punch above tonal weight:
directional shadows, inner-top highlights, micro-borders, and strategic use
of the accent color as a depth cue.

---

## 1. Card Treatment

### Current
Cards use a 1px `#2A2A35` border on a `#16161A` fill. The border and fill
are so close in tone that the card edge is barely visible. No shadow, no
inner highlight.

### Improvements

**A — Top-edge highlight (most impactful, cheapest)**  
Add a 1px `rgba(255,255,255,0.06)` top border to every card. This simulates
a light source from above and immediately separates the card from the
background, exactly how Raycast and Linear achieve depth without relying
on shadows.

```dart
// In NexusCard, replace the single Border.all with:
border: Border(
  top:    BorderSide(color: Colors.white.withOpacity(0.06), width: 1),
  left:   BorderSide(color: AppColors.darkBorder, width: 1),
  right:  BorderSide(color: AppColors.darkBorder, width: 1),
  bottom: BorderSide(color: AppColors.darkBorder, width: 1),
),
```

**B — Subtle drop shadow**  
A tight, dark shadow (not Material elevation) separates the card plane:

```dart
boxShadow: [
  BoxShadow(
    color: Colors.black.withOpacity(0.35),
    blurRadius: 8,
    offset: Offset(0, 2),
  ),
  // Optional: soft ambient
  BoxShadow(
    color: Colors.black.withOpacity(0.15),
    blurRadius: 24,
    offset: Offset(0, 8),
  ),
],
```

**C — Section title divider line**  
In `NexusSection`, replace the whitespace below the title with a 1px
`#2A2A35` hairline. This visually "locks" the title to its card and
separates it from the content — a detail used in Grafana panels.

```dart
// After the title Row in NexusSection:
Divider(height: 1, thickness: 1, color: AppColors.darkBorder),
SizedBox(height: AppSpacing.md),
```

---

## 2. Navigation Rail

### Current
The rail is `#1A2236` (darkNavy) against `#0D0D0F` bg. On the right edge,
there's no visual separation — the rail bleeds into content. The selected
item uses an orange icon + `accentSubtle` background but feels muted.

### Improvements

**A — Right-edge border**  
A single 1px right border creates immediate structural separation:

```dart
// In _NexusRail Container decoration:
border: Border(
  right: BorderSide(color: AppColors.darkBorder, width: 1),
),
```

**B — Selected item: accent left-bar**  
Replace the `accentSubtle` background with a 2px left orange bar +
dimmer background. This is the Raycast/VS Code pattern — more deliberate
than a filled pill:

```dart
decoration: BoxDecoration(
  color: isSelected
      ? AppColors.accent.withOpacity(0.08)
      : Colors.transparent,
  borderRadius: AppRadius.smBr,
  border: Border(
    left: BorderSide(
      color: isSelected ? AppColors.accent : Colors.transparent,
      width: 2,
    ),
  ),
),
```

**C — NEXUS wordmark with letter-spacing glow**  
The wordmark currently uses a plain orange label. Add a subtle text shadow
that matches the accent to give it a "lit" quality:

```dart
Text(
  'NEXUS',
  style: theme.textTheme.labelSmall?.copyWith(
    color: AppColors.accent,
    fontWeight: FontWeight.w700,
    letterSpacing: 3,
    shadows: [
      Shadow(
        color: AppColors.accent.withOpacity(0.6),
        blurRadius: 8,
      ),
    ],
  ),
),
```

---

## 3. Content Area Background

### Current
The content area is a flat `#0D0D0F`. With dark cards on a dark bg, the
whole right panel reads as one undifferentiated mass.

### Improvements

**A — Subtle grid texture**  
A very faint dot-grid or line-grid in the background (1px dots at ~4%
opacity on an 24px grid) gives the panel a "control surface" quality
without adding visual noise. This is a signature Grafana technique.

Implement as a Flutter `CustomPainter` on the scaffold background:

```dart
class _GridPainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final paint = Paint()
      ..color = Colors.white.withOpacity(0.035)
      ..strokeWidth = 1;
    const step = 24.0;
    for (double x = 0; x < size.width; x += step) {
      for (double y = 0; y < size.height; y += step) {
        canvas.drawCircle(Offset(x, y), 1, paint);
      }
    }
  }
  @override
  bool shouldRepaint(_GridPainter _) => false;
}
```

Wrap the `Expanded` content area:

```dart
Expanded(
  child: Stack(
    children: [
      Positioned.fill(child: CustomPaint(painter: _GridPainter())),
      AnimatedSwitcher(...),
    ],
  ),
),
```

**B — Orange ambient glow at top of rail (optional, high impact)**  
A very soft radial gradient from `accent.withOpacity(0.04)` centered
at the top of the rail header gives the whole chrome a "warm light source"
feeling — as if the hardware device itself is casting ambient light upward.

---

## 4. Typography Hierarchy

### Current
`titleMedium` (15px w600) for section titles, `bodySmall` (11px) for
descriptions. The gap in visual weight between label text and body text
is small.

### Improvements

**A — Uppercase micro-labels for section titles**  
Short section labels like "Display Colours" rendered as:
`11px, w600, letter-spacing: 1.2, color: textSecondary (70% opacity)`
with the orange accent on the left as a 2px dot or short rule.

This is the Grafana panel-header treatment — it reads as "instrument label"
rather than "app title":

```dart
Row(
  children: [
    Container(
      width: 2, height: 10,
      color: AppColors.accent,
      margin: EdgeInsets.only(right: AppSpacing.xs),
    ),
    Text(
      title.toUpperCase(),
      style: theme.textTheme.labelSmall?.copyWith(
        color: theme.colorScheme.onSurfaceVariant,
        letterSpacing: 1.2,
        fontWeight: FontWeight.w600,
      ),
    ),
  ],
),
```

**B — Hero value display (Display tab)**  
The colour preview strip (`14:30  25°C  New York`) is the one place a
larger, more expressive type treatment is warranted. Render it in
`GoRegular` (the hardware font) at a larger size with the cyan data accent
on the numbers — directly mirroring how it looks on the physical device.

---

## 5. Status & Feedback Surfaces

### Current
The "Backend disconnected" banner uses `darkElevated` fill — nearly
invisible against the scaffold.

### Improvements

**A — Warning banner with left-accent bar**  
Match the card accent-border pattern: a 3px amber left bar on the banner
makes it immediately scannable as a status message without being aggressive:

```dart
// MaterialBanner replacement or wrapper:
Container(
  decoration: BoxDecoration(
    color: AppColors.darkElevated,
    border: Border(
      left: BorderSide(color: AppColors.warning, width: 3),
      bottom: BorderSide(color: AppColors.darkBorder, width: 1),
    ),
  ),
  ...
)
```

**B — Connection status dot with pulse animation**  
The dot in the rail header is static. A subtle CSS-style scale pulse on
the `NexusStatus.ok` dot (scale 1.0→1.3→1.0 over 2s, repeat) communicates
"live" at a glance — matching the live-data aesthetic of the hardware.

---

## 6. The 640×48 Display Strip

### Current
8px tall, 56px wide, always-on in the rail header. Good idea, but at that
size it reads as a decorative line rather than a meaningful preview.

### Improvements

**A — Increase to 10px tall, add cyan border when live**  
When a frame is received, the border transitions from `outline` to
`dataAccent.withOpacity(0.7)`. Currently this is implemented but the
size is too small to see the effect.

**B — Hover-to-expand (if the rail supports hover)**  
On desktop hover, the strip animates to 24px tall with an easing curve.
This is a delightful "peek" interaction — you get a real read of what's
on the display without leaving the nav rail.

---

## Implementation Priority

| Priority | Item                              | Effort | Impact |
|----------|-----------------------------------|--------|--------|
| 1        | Card top-edge highlight           | 5 min  | High   |
| 2        | Card drop shadow                  | 5 min  | High   |
| 3        | Rail right-edge border            | 2 min  | Medium |
| 4        | Selected nav item: left-bar style | 10 min | High   |
| 5        | Section title divider line        | 5 min  | Medium |
| 6        | NEXUS wordmark glow               | 5 min  | Medium |
| 7        | Section title uppercase treatment | 15 min | High   |
| 8        | Dot-grid background painter       | 20 min | Medium |
| 9        | Warning banner left-bar           | 10 min | Medium |
| 10       | Display strip hover-expand        | 20 min | Low    |

**Recommended first pass:** Items 1–6. Under 30 minutes total, transforms
the perceived quality from flat → layered without touching the layout or
color palette.
