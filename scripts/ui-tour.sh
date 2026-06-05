#!/usr/bin/env bash
# UI screenshot tour — navigates all tabs via flutter drive,
# captures each tab via the Dart VM service.
#
# Usage:
#   ./scripts/ui-tour.sh                    # no backend (default, fast)
#   NEXUS_WITH_BACKEND=1 ./scripts/ui-tour.sh  # start Go backend with mock device

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
UI_DIR="$REPO_ROOT/ui"
OUT_DIR="$UI_DIR/screenshots"
DRIVE_LOG=$(mktemp /tmp/flutter-drive-XXXXXX.log)
URL_FILE=$(mktemp /tmp/nexus-ws-url-XXXXXX)
PY="$SCRIPT_DIR/flutter-screenshot.py"
BACKEND_PID=""

mkdir -p "$OUT_DIR"
rm -f "$OUT_DIR"/*.png /tmp/nexus-shot-done-*
trap '_cleanup' EXIT

_cleanup() {
  rm -f "$DRIVE_LOG" "$URL_FILE" /tmp/nexus-shot-done-*
  if [[ -n "$BACKEND_PID" ]]; then
    echo "  ▶  Stopping backend (PID $BACKEND_PID)..."
    kill "$BACKEND_PID" 2>/dev/null || true
    wait "$BACKEND_PID" 2>/dev/null || true
  fi
}

# ── Optional: start Go backend with mock device ───────────────────────────────
if [[ "${NEXUS_WITH_BACKEND:-0}" == "1" ]]; then
  # Refuse to start if something is already on port 1985 — avoids a silent
  # bind failure where the health check passes against the old process.
  if curl -sf http://localhost:1985/api/health >/dev/null 2>&1; then
    echo "✗  Port 1985 is already in use."
    echo "   Stop the existing backend first, then re-run with NEXUS_WITH_BACKEND=1."
    echo "   (To run without managing the backend, omit NEXUS_WITH_BACKEND=1 and"
    echo "    start it manually before running this script.)"
    exit 1
  fi

  echo "▶  Building Go backend..."
  cd "$REPO_ROOT"
  go build -o /tmp/nexus-open-tour ./cmd/nexus-open

  echo "▶  Starting backend (NEXUS_MOCK_DEVICE=1)..."
  NEXUS_MOCK_DEVICE=1 /tmp/nexus-open-tour --port 1985 &>/tmp/nexus-backend-tour.log &
  BACKEND_PID=$!

  echo "   Waiting for backend health check..."
  for i in $(seq 1 20); do
    if curl -sf http://localhost:1985/api/health >/dev/null 2>&1; then
      echo "   ✓  Backend ready (attempt $i)"
      break
    fi
    if [[ $i -eq 20 ]]; then
      echo "   ✗  Backend failed to start. Log:"
      cat /tmp/nexus-backend-tour.log
      exit 1
    fi
    sleep 0.5
  done
fi

echo "▶  Starting flutter drive..."
cd "$UI_DIR"

DISPLAY="${DISPLAY:-:1}" flutter drive \
  --driver=test_driver/integration_test.dart \
  --target=integration_test/screenshot_tour_test.dart \
  -d linux 2>&1 | while IFS= read -r line; do
    echo "$line"
    echo "$line" >> "$DRIVE_LOG"

    # Capture VM service URL on first appearance
    if [[ "$line" == *"Connecting to Flutter application at"* ]]; then
      http_url=$(echo "$line" | grep -o 'http://[^ ]*')
      ws="${http_url/http:\/\//ws://}"
      ws="${ws%/}/ws"
      echo "$ws" > "$URL_FILE"
      echo "  🔗  VM service: $ws"
    fi

    # Handle screenshot signal from test
    if [[ "$line" == *"NEXUS_SCREENSHOT:"* ]]; then
      name="${line##*NEXUS_SCREENSHOT:}"
      name="${name//[^a-z_]/}"
      out="$OUT_DIR/${name}.png"

      # Retry reading URL — may not have flushed yet on first signal
      ws=""
      for _ in 1 2 3 4 5; do
        ws=$(cat "$URL_FILE" 2>/dev/null || true)
        [[ -n "$ws" ]] && break
        sleep 0.3
      done

      if [[ -z "$ws" ]]; then
        echo "  ⚠  no VM URL for $name — skipping"
      else
        echo "  📸  $name → $out"
        python3 "$PY" "$out" "$ws" && echo "      ✓" || echo "      ⚠ screenshot failed"
      fi
      # Always touch done-file so the test isn't left waiting
      touch "/tmp/nexus-shot-done-$name"
    fi
done

DRIVE_STATUS=${PIPESTATUS[0]}
echo ""
[[ $DRIVE_STATUS -eq 0 ]] && echo "✓  All tests passed." || echo "✗  Drive failed (status $DRIVE_STATUS)"

SHOTS=$(ls "$OUT_DIR/"*.png 2>/dev/null | wc -l || echo 0)
echo "   $SHOTS screenshot(s) saved to $OUT_DIR/"
ls -1 "$OUT_DIR/"*.png 2>/dev/null | xargs -I{} basename {} || true

# flutter drive overwrites the bundle binary with a test-harness build.
# Rebuild the plain app binary so subsequent launches don't run the tour.
echo ""
echo "▶  Rebuilding plain app binary (flutter drive leaves a test binary)..."
flutter build linux --debug --target=lib/main.dart --suppress-analytics 2>&1 | grep -E "Built|Error|error" || true
echo "   ✓  Plain binary restored."
