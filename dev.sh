#!/bin/bash

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Enable mock device mode by default for development (set to 0 to use real hardware)
export NEXUS_MOCK_DEVICE="${NEXUS_MOCK_DEVICE:-1}"

if [ "$NEXUS_MOCK_DEVICE" = "1" ]; then
    echo "🎭 Mock device mode enabled (no physical hardware required)"
    echo "   To use real hardware: NEXUS_MOCK_DEVICE=0 ./dev.sh"
else
    echo "🔌 Using real hardware device"
fi
echo ""

# Stop any existing instance via PID file (clean shutdown, releases HID device).
# Fall back to killall for processes that didn't write a PID file.
_PID_FILE="${XDG_RUNTIME_DIR:-/tmp/nexus-open-$(id -u)}/nexus-open.pid"
if [[ -f "$_PID_FILE" ]]; then
    _PREV_PID=$(cat "$_PID_FILE" 2>/dev/null)
    if [[ -n "$_PREV_PID" ]] && kill -0 "$_PREV_PID" 2>/dev/null; then
        echo "Stopping previous nexus-open (PID $_PREV_PID)..."
        kill "$_PREV_PID" 2>/dev/null
        # Wait up to 5s for clean exit (wg.Wait drains goroutines)
        for _ in 1 2 3 4 5; do
            kill -0 "$_PREV_PID" 2>/dev/null || break
            sleep 1
        done
    fi
fi
# Catch anything that bypassed the PID file (old binaries, debug runs)
pkill -x nexus-open 2>/dev/null || true
pkill -f "debug/bundle/ui" 2>/dev/null || true

# Generate OpenAPI 3.0 spec from annotations
echo "Generating OpenAPI 3.0 spec from code annotations..."
if [ -f "$HOME/go/bin/go-openapi" ]; then
    cd "$SCRIPT_DIR"
    "$HOME/go/bin/go-openapi" -dir cmd/nexus-open,internal/api,internal/config -output api/openapi.yaml
    # Fix server URL (go-openapi defaults to 8080)
    sed -i 's|http://localhost:8080|http://localhost:1985|g' api/openapi.yaml
    echo "✓ OpenAPI spec generated at api/openapi.yaml"
else
    echo "⚠ go-openapi not found, skipping spec generation"
fi

# Generate Flutter API client from OpenAPI 3.0 spec (optional, requires openapi-generator)
if [ -x "$SCRIPT_DIR/scripts/generate-flutter-api.sh" ]; then
    "$SCRIPT_DIR/scripts/generate-flutter-api.sh"
fi

# Build Flutter UI in debug mode
echo "Building Flutter UI..."
cd "$SCRIPT_DIR/ui" && flutter build linux --debug

# Start the Go backend with air (which will watch for changes)
echo "Starting Go backend with air..."
cd "$SCRIPT_DIR"

# Use air from Go bin (search common locations)
AIR_BIN=""
if command -v air &> /dev/null; then
    AIR_BIN="air"
elif [ -f "$HOME/go/bin/air" ]; then
    AIR_BIN="$HOME/go/bin/air"
elif [ -f "$(go env GOPATH)/bin/air" ]; then
    AIR_BIN="$(go env GOPATH)/bin/air"
else
    echo "Error: air not found. Install with: go install github.com/air-verse/air@latest"
    exit 1
fi

$AIR_BIN &
AIR_PID=$!

# Give the backend a moment to start
sleep 3

# Launch the Flutter UI
echo "Starting Flutter UI..."
"$SCRIPT_DIR/ui/build/linux/x64/debug/bundle/ui" &
UI_PID=$!

# Ensure both processes are killed on exit, Ctrl+C, or SIGTERM
trap 'kill $UI_PID 2>/dev/null; kill $AIR_PID 2>/dev/null; wait' EXIT SIGINT SIGTERM

echo ""
echo "Development environment started!"
echo "  - Go backend (air): PID $AIR_PID"
echo "  - Flutter UI: PID $UI_PID"
echo ""
echo "Press Ctrl+C to stop both..."

# Wait for air (it will stay running and watch for changes)
wait $AIR_PID
