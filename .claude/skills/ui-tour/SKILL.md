---
name: ui-tour
description: Run the Flutter UI screenshot tour for Nexus Open. Navigates all four settings tabs via flutter drive and captures each one via the Dart VM service. Use when you want to see the current state of the UI, review visual changes, or take before/after screenshots before implementing design improvements.
user-invocable: true
allowed-tools:
  - Bash
  - Read
---

# /ui-tour — Nexus Open UI Screenshot Tour

Captures a screenshot of every settings tab in the running Flutter app.

## How It Works

The tour uses two components:

1. **`scripts/ui-tour.sh`** — runs `flutter drive` which launches the app,
   navigates each tab via the integration test harness, and signals when
   each tab is ready to capture.

2. **`scripts/flutter-screenshot.py`** — connects to the app's Dart VM
   service via WebSocket and calls `ext.flutter.inspector.screenshot` to
   capture a PNG of the current widget tree.

The two components coordinate via a done-file handshake: the Python script
writes `/tmp/nexus-shot-done-<tab>` after each capture so the test knows
it's safe to advance to the next tab.

## Prerequisites

The Go backend must be running before the tour starts:

```bash
NEXUS_MOCK_DEVICE=1 ./nexus-open &
```

Or use the dev script which starts it automatically:

```bash
./dev.sh
```

## Running the Tour

```bash
make screenshot-tour
# or directly:
./scripts/ui-tour.sh
```

Screenshots are saved to `ui/screenshots/`:
- `tab_display.png`
- `tab_location.png`
- `tab_modules.png`
- `tab_images.png`

## Viewing Screenshots

Read any screenshot directly — Claude can display them inline:

```
Read ui/screenshots/tab_display.png
```

## Taking a One-Off Screenshot

To screenshot the current tab without running the full tour (requires
`flutter run` to be running in a tmux session):

```bash
python3 scripts/flutter-screenshot.py /tmp/nexus-current.png
```

The script reads the VM service URL from `/tmp/flutter-run.log` automatically.
Pass a URL explicitly as a second argument if needed:

```bash
python3 scripts/flutter-screenshot.py out.png ws://127.0.0.1:PORT/TOKEN=/ws
```

## Skill Instructions

When this skill is invoked:

1. Check that the Go backend is running:
   ```bash
   curl -s http://localhost:1985/api/health
   ```
   If it's not, start it: `NEXUS_MOCK_DEVICE=1 ./nexus-open &`
   Wait for `{"status":"ok"}`.

2. Run the tour:
   ```bash
   DISPLAY=:1 ./scripts/ui-tour.sh 2>&1 | tee /tmp/ui-tour.log
   ```
   Wait for "screenshot(s) saved" in the output.

3. Read and display all four screenshots:
   - `ui/screenshots/tab_display.png`
   - `ui/screenshots/tab_location.png`
   - `ui/screenshots/tab_modules.png`
   - `ui/screenshots/tab_images.png`

4. Summarise what you observe: visual quality, layout issues, anything
   that looks broken or could be improved.

## Troubleshooting

**"no VM URL yet"** — flutter drive launched but the VM service URL line
wasn't found in time. Re-run; it's a race condition on slow builds.

**"screenshot failed"** — the app process exited before the Python script
connected. Usually means the test finished too fast. Re-run.

**Build fails with "Permission denied" on bundle/ui** — the bundle was
previously built as root. Fix with:
```bash
sudo chown -R $USER ~/Projects/nexus-next/ui/build
```

**flutter drive reuses a stale app instance** — if an old `flutter run`
session is still alive, drive will attach to it. Kill it first:
```bash
pkill -f "debug/bundle/ui"
```
