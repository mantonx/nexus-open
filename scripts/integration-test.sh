#!/usr/bin/env bash
# Run all integration tests — Go API tests + Flutter backend integration tests.
#
# Usage:
#   ./scripts/integration-test.sh           # Go tests only (no Flutter/X11 needed)
#   ./scripts/integration-test.sh --flutter # Go + Flutter (requires DISPLAY)

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(dirname "$SCRIPT_DIR")"
UI_DIR="$REPO_ROOT/ui"
BACKEND_PID=""
RUN_FLUTTER=0

for arg in "$@"; do
  [[ "$arg" == "--flutter" ]] && RUN_FLUTTER=1
done

_cleanup() {
  if [[ -n "$BACKEND_PID" ]]; then
    echo "  ▶  Stopping backend (PID $BACKEND_PID)..."
    kill "$BACKEND_PID" 2>/dev/null || true
    wait "$BACKEND_PID" 2>/dev/null || true
  fi
}
trap '_cleanup' EXIT

# ── Start Go backend ──────────────────────────────────────────────────────────
echo "▶  Building Go backend..."
cd "$REPO_ROOT"
go build -o /tmp/nexus-open-integration ./cmd/nexus-open

echo "▶  Starting backend (NEXUS_MOCK_DEVICE=1)..."
NEXUS_MOCK_DEVICE=1 /tmp/nexus-open-integration --port 1985 &>/tmp/nexus-integration.log &
BACKEND_PID=$!

echo "   Waiting for health check..."
for i in $(seq 1 20); do
  if curl -sf http://localhost:1985/api/health >/dev/null 2>&1; then
    echo "   ✓  Backend ready"
    break
  fi
  if [[ $i -eq 20 ]]; then
    echo "   ✗  Backend failed to start:"
    cat /tmp/nexus-integration.log
    exit 1
  fi
  sleep 0.5
done

# ── Go integration tests ──────────────────────────────────────────────────────
echo ""
echo "▶  Running Go integration tests..."
cd "$REPO_ROOT"
go test ./test/integration/... -v -timeout 60s
GO_STATUS=$?
echo ""
[[ $GO_STATUS -eq 0 ]] && echo "✓  Go integration tests passed." || echo "✗  Go integration tests failed."

# ── Flutter integration tests ─────────────────────────────────────────────────
FLUTTER_STATUS=0
if [[ $RUN_FLUTTER -eq 1 ]]; then
  echo ""
  echo "▶  Running Flutter backend integration tests..."
  cd "$UI_DIR"
  NEXUS_WITH_BACKEND=1 DISPLAY="${DISPLAY:-:1}" flutter drive \
    --driver=test_driver/integration_test.dart \
    --target=integration_test/backend_integration_test.dart \
    -d linux 2>&1
  FLUTTER_STATUS=$?
  echo ""
  [[ $FLUTTER_STATUS -eq 0 ]] && echo "✓  Flutter integration tests passed." || echo "✗  Flutter integration tests failed."
fi

# ── Summary ───────────────────────────────────────────────────────────────────
echo ""
echo "══════════════════════════════════════"
[[ $GO_STATUS -eq 0 ]]      && echo "  Go API tests:      ✓ passed" || echo "  Go API tests:      ✗ FAILED"
[[ $RUN_FLUTTER -eq 1 ]] && {
  [[ $FLUTTER_STATUS -eq 0 ]] && echo "  Flutter UI tests:  ✓ passed" || echo "  Flutter UI tests:  ✗ FAILED"
}
echo "══════════════════════════════════════"

[[ $GO_STATUS -eq 0 && $FLUTTER_STATUS -eq 0 ]] && exit 0 || exit 1
