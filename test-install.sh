#!/usr/bin/env bash
# test-install.sh — smoke-test the install.sh script
#
# Runs install.sh against the real filesystem, then asserts that every
# expected artifact exists and the daemon is healthy. Safe to re-run;
# cleans up and reinstalls on each run.
#
# Usage:
#   ./test-install.sh           # full install + verify
#   ./test-install.sh --keep    # skip cleanup at the end (inspect leftovers)

set -euo pipefail

REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
KEEP=false
[[ "${1:-}" == "--keep" ]] && KEEP=true

# ── colour helpers ─────────────────────────────────────────────────────────────

GREEN='\033[0;32m'; RED='\033[0;31m'; YELLOW='\033[1;33m'; NC='\033[0m'
pass()  { echo -e "${GREEN}  PASS${NC}  $*"; }
fail()  { echo -e "${RED}  FAIL${NC}  $*"; FAILURES=$((FAILURES + 1)); }
skip()  { echo -e "${YELLOW}  SKIP${NC}  $*"; }
title() { echo -e "\n${YELLOW}── $* ──${NC}"; }

FAILURES=0

# ── prerequisites ──────────────────────────────────────────────────────────────

title "Prerequisites"

for cmd in systemctl curl file; do
    if command -v "$cmd" &>/dev/null; then
        pass "command available: $cmd"
    else
        fail "command not found: $cmd (required)"
    fi
done

[[ -f "$REPO_DIR/nexus-open" ]] \
    && pass "daemon binary exists: nexus-open" \
    || fail "daemon binary missing — run: go build -o nexus-open ./cmd/nexus-open"

[[ -f "$REPO_DIR/ui/build/linux/x64/release/bundle/ui" ]] \
    && pass "Flutter UI binary exists" \
    || skip "Flutter UI binary missing (tray tests will be limited) — run: cd ui && flutter build linux --release"

[[ $FAILURES -gt 0 ]] && { echo -e "\n${RED}Prerequisite failures — aborting.${NC}"; exit 1; }

# ── clean slate ────────────────────────────────────────────────────────────────

title "Clean slate"

# Stop any running instance before removing so files aren't busy
systemctl --user stop nexus-open.service 2>/dev/null || true
pkill -x nexus-open 2>/dev/null || true
sleep 1

"$REPO_DIR/install.sh" --remove 2>/dev/null || true
pass "previous install removed"

# ── install ────────────────────────────────────────────────────────────────────

title "Install"

if "$REPO_DIR/install.sh" 2>&1 | sed 's/^/    /'; then
    pass "install.sh exited 0"
else
    fail "install.sh exited non-zero"
fi

# ── file assertions ────────────────────────────────────────────────────────────

title "Installed files"

BIN_DIR="$HOME/.local/bin"
DATA_DIR="${XDG_DATA_HOME:-$HOME/.local/share}/nexus-open"
APP_DIR="$HOME/.local/share/applications"
AUTOSTART_DIR="$HOME/.config/autostart"
SYSTEMD_DIR="$HOME/.config/systemd/user"
ICON_BASE="$HOME/.local/share/icons/hicolor"

assert_file() {
    local path="$1" desc="${2:-$1}"
    [[ -f "$path" ]] && pass "$desc" || fail "missing: $path"
}
assert_exe() {
    local path="$1" desc="${2:-$1}"
    [[ -x "$path" ]] && pass "executable: $desc" || fail "not executable: $path"
}
assert_contains() {
    local path="$1" pattern="$2" desc="${3:-contains '$pattern'}"
    grep -q "$pattern" "$path" 2>/dev/null \
        && pass "$desc" \
        || fail "$path: expected to contain '$pattern'"
}

assert_file "$BIN_DIR/nexus-open"            "daemon binary"
assert_exe  "$BIN_DIR/nexus-open"            "daemon binary"
assert_file "$DATA_DIR/ui"                   "Flutter wrapper script"
assert_exe  "$DATA_DIR/ui"                   "Flutter wrapper script"
assert_file "$DATA_DIR/ui.real"              "Flutter real binary"
assert_exe  "$DATA_DIR/ui.real"              "Flutter real binary"
assert_file "$APP_DIR/nexus-open.desktop"    "app launcher .desktop"
assert_file "$AUTOSTART_DIR/nexus-open-autostart.desktop" "autostart .desktop"
assert_file "$SYSTEMD_DIR/nexus-open.service" "systemd user service"

# Desktop file correctness
assert_contains "$APP_DIR/nexus-open.desktop" "StartupWMClass=io.nexus.open" ".desktop has correct WMClass"
assert_contains "$APP_DIR/nexus-open.desktop" "StartupNotify=true"           ".desktop has StartupNotify"
assert_contains "$APP_DIR/nexus-open.desktop" "nexus-open --show"            ".desktop has Show action"
assert_contains "$APP_DIR/nexus-open.desktop" "$BIN_DIR"                     ".desktop Exec uses absolute path"

# Autostart file correctness
assert_contains "$AUTOSTART_DIR/nexus-open-autostart.desktop" "StartupWMClass=io.nexus.open" "autostart has correct WMClass"
assert_contains "$AUTOSTART_DIR/nexus-open-autostart.desktop" "$BIN_DIR"                     "autostart Exec uses absolute path"

# Service file
assert_contains "$SYSTEMD_DIR/nexus-open.service" "nexus-open --tray" "service file has --tray flag"
assert_contains "$SYSTEMD_DIR/nexus-open.service" "graphical-session.target" "service waits for graphical session"

# Wrapper script forces XWayland
assert_contains "$DATA_DIR/ui" "WAYLAND_DISPLAY=" "wrapper clears WAYLAND_DISPLAY"
assert_contains "$DATA_DIR/ui" "ui.real"          "wrapper delegates to ui.real"

# Icons
for size in 16 22 32 48 64 128 256; do
    icon="$ICON_BASE/${size}x${size}/apps/nexus-open.png"
    [[ -f "$icon" ]] \
        && pass "icon ${size}x${size}" \
        || skip "icon ${size}x${size} missing (icon-${size}.png not built yet)"
done

# ── systemd ────────────────────────────────────────────────────────────────────

title "systemd service"

assert_contains <(systemctl --user cat nexus-open.service 2>/dev/null) \
    "nexus-open --tray" "unit file is registered with systemd"

if systemctl --user is-enabled --quiet nexus-open.service 2>/dev/null; then
    pass "service is enabled (will start on login)"
else
    fail "service is not enabled"
fi

# Wait up to 10s for the service to become active after install
for i in $(seq 1 10); do
    systemctl --user is-active --quiet nexus-open.service 2>/dev/null && break
    sleep 1
done

if systemctl --user is-active --quiet nexus-open.service 2>/dev/null; then
    pass "service is active (running)"
else
    fail "service did not become active within 10s"
    systemctl --user status nexus-open.service --no-pager 2>&1 | sed 's/^/    /' || true
fi

# ── API health ─────────────────────────────────────────────────────────────────

title "API health"

API_UP=false
for i in $(seq 1 15); do
    if curl -sf http://localhost:1985/api/health | grep -q '"status":"ok"'; then
        API_UP=true
        break
    fi
    sleep 1
done

if $API_UP; then
    pass "GET /api/health → ok"
else
    fail "API did not respond within 15s"
fi

# ── --show round-trip ──────────────────────────────────────────────────────────

title "--show round-trip"

if $API_UP; then
    if "$BIN_DIR/nexus-open" --show; then
        pass "nexus-open --show exited 0 (daemon received show command)"
    else
        fail "nexus-open --show exited non-zero"
    fi
else
    skip "--show test skipped (API not up)"
fi

# ── binary sanity ──────────────────────────────────────────────────────────────

title "Binary sanity"

VERSION_OUT=$("$BIN_DIR/nexus-open" --version 2>&1)
if echo "$VERSION_OUT" | grep -q "Nexus Open"; then
    pass "--version output: $VERSION_OUT"
else
    fail "--version did not output expected string (got: $VERSION_OUT)"
fi

# Confirm it's a native ELF binary, not a shell script or stale artifact
FILE_OUT=$(file "$BIN_DIR/nexus-open")
if echo "$FILE_OUT" | grep -q "ELF"; then
    pass "daemon is ELF binary"
else
    fail "daemon is not an ELF binary: $FILE_OUT"
fi

# ── cleanup ────────────────────────────────────────────────────────────────────

if ! $KEEP; then
    title "Cleanup"
    systemctl --user stop nexus-open.service 2>/dev/null || true
    "$REPO_DIR/install.sh" --remove 2>/dev/null | sed 's/^/    /'
    pass "uninstalled cleanly"
else
    echo -e "\n${YELLOW}--keep set: leaving install in place for inspection.${NC}"
fi

# ── summary ────────────────────────────────────────────────────────────────────

echo ""
if [[ $FAILURES -eq 0 ]]; then
    echo -e "${GREEN}All assertions passed.${NC}"
    exit 0
else
    echo -e "${RED}${FAILURES} assertion(s) failed.${NC}"
    exit 1
fi
