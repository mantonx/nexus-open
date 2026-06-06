# Nexus Open — UI Design Plan

**Created:** 2026-06-04
**Status:** Complete (all 6 phases implemented)

This document covers the visual redesign of the Flutter settings application.
The goal is to move from a generic Material app to a polished, distinctive
interface that reflects the hardware it controls — a dark, data-rich dashboard
aesthetic consistent with the 640×48 display renderer.

This work is separate from and follows the [IMPROVEMENT_PLAN.md](IMPROVEMENT_PLAN.md),
which is now complete.

---

## Design Audit Summary

A full audit of the current codebase identified the following state:

| Dimension         | Maturity | Notes |
|-------------------|----------|-------|
| Colors            | 70%      | Brand colors in theme; functional colors (red/orange/green) hardcoded in widgets |
| Typography        | 40%      | Poppins applied globally; no custom size scale; relies on Material defaults |
| Spacing           | 30%      | `gridUnit` constant defined but unused; 18+ distinct hardcoded values |
| Border radius     | 40%      | Three arbitrary values (4/8/16px) with no documented rationale |
| Elevation         | 30%      | Only 0 and 2 used; no depth system |
| Component library | 20%      | One custom component (`StyledDropdown`); all other UI inline |
| Dark mode         | 80%      | Full ColorScheme defined; good contrast |
| Accessibility     | 75%      | WCAG AA colors; Semantics labels present |
| Material 3        | 0%       | `useMaterial3` not set; `ColorScheme.fromSeed` not used |
| Token system      | 0%       | No design tokens; all values scattered across widget files |

**Root cause of generic appearance:** The theme is defined in one place but ignored
everywhere else. Spacing, icon sizes, status colours, and border radii are all
hardcoded inline throughout the widget tree. There is no enforced design language.

---

## Design Direction

**Dark-first, data-dense dashboard.**

The Flutter app controls hardware that renders bold hero numbers on a near-black
screen with cyan accents and atmospheric graphs. The settings window should feel
like it belongs to that hardware — not like a generic white Material app bolted
onto it. Reference aesthetic: Grafana, Raycast, high-end audio app panels.

The dark theme is the primary experience. Light mode is fully supported but
secondary.

### Colour Direction

| Token              | Value       | Role |
|--------------------|-------------|------|
| Background         | `#0D0D0F`   | Page background — near-black, like the device |
| Surface            | `#16161A`   | Card and panel surfaces |
| Surface elevated   | `#1E1E24`   | Raised cards, selected states |
| Border             | `#2A2A35`   | Subtle separation lines |
| Accent orange      | `#DB8720`   | Primary interactive colour (unchanged) |
| Accent dim         | `#DB872033` | Orange at 20% — selection backgrounds, overlays |
| Data cyan          | `#00C8FF`   | Ties the UI to the hardware renderer's accent colour |
| Success            | `#30D158`   | Connection, ok states |
| Warning            | `#FFD60A`   | Disconnected, degraded |
| Error              | `#FF453A`   | Failures, critical states |
| Text primary       | `#E5E5EA`   | Main content |
| Text secondary     | `#B8BDC2`   | Labels, descriptions (WCAG AA compliant) |
| Text muted         | `#636366`   | Placeholders, disabled |

The light theme retains the existing navy + orange palette with the same token
names mapped to appropriate lighter values.

---

## Phase 1 — Design Token Foundation

**Estimated effort:** 1–2 hours
**Blocks:** All subsequent phases
**Files:** `ui/lib/src/theme/app_tokens.dart` (new),
`ui/lib/src/theme/app_theme.dart` (updated)

### 1.1 — Create `app_tokens.dart`

Define a complete token system as static const classes. Every spacing, sizing,
radius, and elevation value used in the app must come from here — no raw numbers
in widget files.

```dart
class AppSpacing {
  static const double xs   = 4.0;
  static const double sm   = 8.0;
  static const double md   = 16.0;
  static const double lg   = 24.0;
  static const double xl   = 32.0;
  static const double xxl  = 48.0;
}

class AppRadius {
  static const double sm   = 4.0;   // Chips, badges
  static const double md   = 8.0;   // Form fields, small containers
  static const double lg   = 16.0;  // Cards, panels
  static const double xl   = 24.0;  // Bottom sheets, dialogs
  static const double pill = 999.0; // Fully rounded (tags, toggles)
}

class AppIconSize {
  static const double xs   = 12.0;  // Status dots, decorative
  static const double sm   = 16.0;  // Inline icons, help
  static const double md   = 20.0;  // Action buttons
  static const double lg   = 24.0;  // Navigation, primary actions
  static const double xl   = 32.0;  // Empty states
  static const double hero = 48.0;  // Onboarding, hero moments
}

class AppElevation {
  static const double flat    = 0.0;
  static const double low     = 1.0;
  static const double mid     = 3.0;
  static const double high    = 6.0;
  static const double overlay = 12.0;
}
```

### 1.2 — Add semantic colour extensions to `ColorScheme`

Replace all `Colors.green` / `Colors.orange` / `Colors.red` hardcoding by adding
a Dart extension on `ColorScheme` that provides semantic colour roles:

```dart
extension AppSemanticColors on ColorScheme {
  Color get success => brightness == Brightness.dark
      ? const Color(0xFF30D158)
      : const Color(0xFF1A7F37);
  Color get warning => brightness == Brightness.dark
      ? const Color(0xFFFFD60A)
      : const Color(0xFFB45309);
  Color get dataAccent => const Color(0xFF00C8FF); // hardware renderer cyan
  Color get accentDim => const Color(0xFFDB8720).withOpacity(0.2);
}
```

### 1.3 — Wire tokens into `app_theme.dart`

- Set `useMaterial3: true` on both themes
- Replace all `EdgeInsets.all(16)` in theme definitions with `AppSpacing.md`
- Replace `BorderRadius.circular(16)` with `AppRadius.lg`
- Set `CardTheme.elevation` to `AppElevation.low`
- Set `FilledButton` shape to `AppRadius.md`

---

## Phase 2 — Dark-First Colour System

**Estimated effort:** 1 hour
**Files:** `ui/lib/src/theme/app_theme.dart`

### 2.1 — Redesign the dark theme surfaces

Replace the current iOS-grey dark theme (`#1C1C1E`, `#2C2C2E`) with the
hardware-adjacent palette defined above. The dark theme should feel like an
extension of the 640×48 display — almost black backgrounds, subtle surface
lift, orange and cyan as the only saturated elements.

Key changes:
- `scaffoldBackgroundColor`: `#0D0D0F`
- `ColorScheme.surface`: `#16161A`
- `ColorScheme.surfaceContainerHighest`: `#1E1E24`
- `ColorScheme.outline`: `#2A2A35`
- `CardTheme.color`: `#16161A` with `#2A2A35` border

### 2.2 — Redesign the light theme surfaces

The light theme retains the navy + orange identity but with the new token
system applied. Key change: cards use `#F5F5F7` scaffold with `#FFFFFF` card
surfaces and `#E5E5EA` borders — cleaner than the current `#EEEEEE` approach.
The `#3984B2` blue is removed; all secondary interactive colour is orange.

### 2.3 — Typography scale

Define a custom `TextTheme` that establishes a real size hierarchy rather than
relying on Material defaults. The scale should work with the 800px window width:

| Style        | Size  | Weight  | Role |
|--------------|-------|---------|------|
| displaySmall | 24sp  | 600     | Section headers, onboarding titles |
| headlineSmall| 18sp  | 600     | Card headers |
| titleMedium  | 15sp  | 600     | List item titles, tab labels |
| bodyLarge    | 14sp  | 400     | Primary body text |
| bodyMedium   | 13sp  | 400     | Secondary body, descriptions |
| bodySmall    | 11sp  | 400     | Labels, captions, help text |
| labelLarge   | 13sp  | 500     | Buttons, form labels |
| labelSmall   | 10sp  | 500     | Badges, chips, metadata |

---

## Phase 3 — Component Library

**Estimated effort:** 2–3 hours
**Files:** `ui/lib/src/widgets/common/` (new components)

Build a small set of reusable components that encode the design language. Every
widget in the app should eventually use these instead of raw Material widgets
with inline styling.

### 3.1 — `NexusCard`

Replaces the current `Card` + `Padding` + `Column` pattern used in every tab.
Enforces consistent surface colour, border, radius, and padding from tokens.
Optional `accentBorder` flag for highlighted/selected state (uses orange).

```dart
NexusCard({
  required Widget child,
  String? title,
  Widget? trailing,
  bool accentBorder = false,
  EdgeInsets? padding,
})
```

### 3.2 — `NexusSection`

A labelled content section — `NexusCard` + title row + optional description.
This replaces the repetitive `Card > Padding > Column > Text(title) > Text(desc)`
pattern that appears 8+ times across the tab files.

### 3.3 — `NexusStatusBadge`

Replaces all `Colors.green` / `Colors.orange` / `Colors.red` status indicator
patterns. Takes a `status` enum (`ok`, `warning`, `error`, `loading`) and
renders a consistent pill with the right semantic colour from the theme extension.

```dart
NexusStatusBadge(status: ZoneStatus.ok, label: 'Connected')
NexusStatusBadge(status: ZoneStatus.error, label: 'Timeout')
```

### 3.4 — `NexusButton`

A thin wrapper around `FilledButton` / `OutlinedButton` that enforces brand
styling and uses token values. Primary variant: orange fill. Secondary: outlined
with orange stroke. Destructive: red fill, only for confirmations.

### 3.5 — `NexusFormField`

Unifies `TextFormField` styling across plugins tab and location tab. Uses a
consistent `InputDecoration` with token-based border radius, padding, focus
colour (orange), and error colour (semantic error). Eliminates duplicate
`InputDecoration` definitions.

---

## Phase 4 — NavigationRail Redesign

**Estimated effort:** 1 hour
**Files:** `ui/lib/src/widgets/settings/settings_page.dart`

### 4.1 — Persistent 640×48 preview strip

Move the live display preview out of the Preview tab and into a persistent
header strip above the NavigationRail — visible on every section. This makes
the display the hero element of the entire app, not a buried sub-tab.

The strip is 640×48 (or scaled to fit the sidebar width) with a subtle border,
showing the real live frame from the WebSocket. On mobile/narrow sizes it hides.

### 4.2 — Icon-only rail destinations

At 800px wide, destination labels in the rail waste horizontal space. Switch to
icon-only with tooltips — this tightens the sidebar from ~88px to ~56px and
gives more room to content. The active indicator becomes an orange pill.

### 4.3 — Rail header / footer tightening

- Header: app name + the 640×48 strip + connection dot (remove redundant firmware text from main view)
- Footer: dark mode toggle + save button — both icon-only with tooltips

---

## Phase 5 — Tab Content Pass

**Estimated effort:** 2–3 hours
**Files:** All tab files

Apply tokens + new components to each tab in a single sweep. For each tab:
replace hardcoded spacing with `AppSpacing.*`, replace `Card + Padding + Column`
with `NexusSection`, replace status colours with `NexusStatusBadge`.

### 5.1 — Preview tab

- Remove the colour picker cards — move to Display tab where they belong
- The 640×48 strip is now persistent (Phase 4.1), so the Preview tab becomes
  a "display settings" surface: background colour, text colour, brightness

### 5.2 — Plugins tab

- Replace expandable list with a 2×2 grid of `NexusCard` — all 4 plugins
  visible at a glance with their live status colour and current value
- Expanding a card shows the config controls inline
- Plugin status (ok/error/timeout) shown as a `NexusStatusBadge` on each card

### 5.3 — Display tab

- Move colour pickers here from Preview tab (5.1)
- Tighter layout: brightness, time format, date format, units all in one
  `NexusSection` each — no unnecessary card nesting

### 5.4 — Images tab

- Larger thumbnails (2-column instead of 3) for better visibility
- Selected state: orange border + checkmark overlay (already done, just apply tokens)
- Empty state: updated to use `AppIconSize.hero` and `AppSpacing` constants

### 5.5 — Location tab

- Map takes full available height (remove fixed 400px cap)
- Search field promoted to full width at the top, auto-focused (already done)
- Selected place shown as a chip below the search field before the map

---

## Phase 6 — Onboarding Polish

**Estimated effort:** 30 minutes
**Files:** `ui/lib/src/widgets/onboarding/onboarding_overlay.dart`

Apply the new token system and components. The onboarding is already
structurally good — it just needs tokens applied and the step icons made
more distinctive (the hardware cyan `#00C8FF` as the step-complete colour).

---

## Sequencing

```
Week 1 (foundations):
  1.1   Create app_tokens.dart
  1.2   Add semantic colour extensions
  1.3   Wire tokens into app_theme.dart
  2.1   Dark theme surface redesign
  2.2   Light theme cleanup
  2.3   Typography scale

Week 2 (components + layout):
  3.1   NexusCard
  3.2   NexusSection
  3.3   NexusStatusBadge
  3.4   NexusButton
  3.5   NexusFormField
  4.1   Persistent 640×48 preview strip
  4.2   Icon-only rail
  4.3   Rail header/footer tightening

Week 3 (tab content + polish):
  5.1   Preview tab
  5.2   Plugins tab
  5.3   Display tab
  5.4   Images tab
  5.5   Location tab
  6.1   Onboarding polish
```

---

## What This Doesn't Cover

- Icon set design (tracked in IMPROVEMENT_PLAN.md as 9.1 — requires design work)
- Screenshots for README/Flatpak (9.2 — follows from this work being complete)
- Animation library or custom painters — staying in the Material widget tree
- Mobile/tablet layout — the app targets 800px+ Linux desktop windows only
- A Figma design file — this is a solo OSS project; code is the source of truth

---

## Success Criteria

The redesign is complete when:

- [ ] Zero hardcoded spacing values in widget files — all use `AppSpacing.*`
- [ ] Zero hardcoded `Colors.red` / `Colors.green` / `Colors.orange` — all use semantic extensions
- [ ] Zero hardcoded `fontSize` values — all use `theme.textTheme.*`
- [ ] `useMaterial3: true` set on both themes
- [ ] All tabs use `NexusCard` / `NexusSection` for their container structure
- [ ] The 640×48 live preview strip is visible on every section
- [ ] The app looks intentionally different from a default Material app
