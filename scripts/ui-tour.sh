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
    echo "  ▶  Stopping dev backend (PID $BACKEND_PID)..."
    kill "$BACKEND_PID" 2>/dev/null || true
    wait "$BACKEND_PID" 2>/dev/null || true
  fi
  # Remove tour binary — it lives next to the installed plugins and shouldn't persist.
  rm -f "${XDG_DATA_HOME:-$HOME/.local/share}/nexus-open/nexus-open-tour"
  if [[ "$STOPPED_SYSTEMD" == "1" ]]; then
    echo "  ▶  Restarting installed nexus-open service..."
    systemctl --user start "$SYSTEMD_UNIT" 2>/dev/null && echo "  ✓  Service restarted" || echo "  ⚠  Failed to restart service — run: systemctl --user start nexus-open"
  fi
}

# ── Optional: start Go backend with mock device ───────────────────────────────
SYSTEMD_UNIT="app-nexus\\x2dopen\\x2dautostart@autostart.service"
STOPPED_SYSTEMD=0

if [[ "${NEXUS_WITH_BACKEND:-0}" == "1" ]]; then
  # If the installed daemon owns port 1985, stop it temporarily so our dev
  # binary can bind. We restart it after the tour via the EXIT trap.
  if curl -sf http://localhost:1985/api/health >/dev/null 2>&1; then
    echo "▶  Stopping installed nexus-open service for tour..."
    if systemctl --user stop "$SYSTEMD_UNIT" 2>/dev/null; then
      STOPPED_SYSTEMD=1
      echo "   ✓  Service stopped"
    else
      echo "✗  Port 1985 is in use and could not stop the service."
      echo "   Stop it manually first: systemctl --user stop nexus-open"
      exit 1
    fi
    # Kill any stale nexus-open* processes that may still hold the port
    # (e.g. dev binaries started manually and not cleanly shut down).
    pkill -TERM -f 'nexus-open' 2>/dev/null || true
    # Wait for port to fully clear — systemd stop returns before the socket is released.
    echo "   Waiting for port 1985 to clear..."
    for i in $(seq 1 20); do
      curl -sf http://localhost:1985/api/health >/dev/null 2>&1 || break
      [[ $i -eq 20 ]] && { echo "✗  Port 1985 still in use after 10s — aborting."; exit 1; }
      sleep 0.5
    done
    sleep 0.5  # extra margin after the port disappears
  fi

  echo "▶  Building Go backend..."
  cd "$REPO_ROOT"
  # Build into the installed data dir so the sibling plugins/ directory is found
  # automatically — the binary resolves exec: plugins relative to its own location.
  TOUR_BIN="${XDG_DATA_HOME:-$HOME/.local/share}/nexus-open/nexus-open-tour"
  go build -o "$TOUR_BIN" ./cmd/nexus-open

  echo "▶  Starting backend (NEXUS_MOCK_DEVICE=1)..."
  NEXUS_MOCK_DEVICE=1 "$TOUR_BIN" &>/tmp/nexus-backend-tour.log &
  BACKEND_PID=$!

  echo "   Waiting for dev backend health check (PID $BACKEND_PID)..."
  for i in $(seq 1 30); do
    # Only accept a response if our dev backend process is still alive.
    if ! kill -0 "$BACKEND_PID" 2>/dev/null; then
      echo "   ✗  Backend process exited early. Log:"
      cat /tmp/nexus-backend-tour.log
      exit 1
    fi
    if curl -sf http://localhost:1985/api/health >/dev/null 2>&1; then
      echo "   ✓  Backend ready (attempt $i)"
      break
    fi
    if [[ $i -eq 30 ]]; then
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
