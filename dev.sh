#!/bin/bash

# Get the directory where this script is located
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# Kill any existing instances
killall nexus-open ui 2>/dev/null

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

echo ""
echo "Development environment started!"
echo "  - Go backend (air): PID $AIR_PID"
echo "  - Flutter UI: PID $UI_PID"
echo ""
echo "Press Ctrl+C to stop both..."

# Wait for air (it will stay running and watch for changes)
wait $AIR_PID
