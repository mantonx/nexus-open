---
name: dev-start
description: Start the Nexus Open development environment — Go backend + Flutter UI. Use when you want to launch the app for manual testing, verify a change is running, or prepare the environment before running the ui-tour skill.
user-invocable: true
---

# /dev-start — Launch Nexus Open Dev Environment

Starts the Go backend and Flutter UI so the app is running and visible on screen.

**This skill is for launching the app.** If you want screenshots of every settings tab, use `/ui-tour` instead — it requires the app to already be running (or starts it automatically).

## What It Does

1. Kills any existing `nexus-open` / Flutter UI processes
2. Starts the Go backend in mock-device mode (no real hardware required)
3. Launches the pre-built Flutter UI binary
4. Verifies the backend health endpoint responds

It does **not** rebuild Flutter or restart `air` — for a full dev cycle with hot-reload, use `./dev.sh` directly in the terminal.

## Skill Instructions

When this skill is invoked:

### 1. Kill any stale processes

```bash
_PID_FILE="${XDG_RUNTIME_DIR:-/tmp/nexus-open-$(id -u)}/nexus-open.pid"
if [[ -f "$_PID_FILE" ]]; then
    _PREV_PID=$(cat "$_PID_FILE" 2>/dev/null)
    [[ -n "$_PREV_PID" ]] && kill "$_PREV_PID" 2>/dev/null || true
fi
pkill -x nexus-open 2>/dev/null || true
pkill -f "debug/bundle/ui" 2>/dev/null || true
sleep 1
```

### 2. Start the Go backend

```bash
cd /home/fictional/Projects/nexus-open
NEXUS_MOCK_DEVICE=1 ./nexus-open &
BACKEND_PID=$!
echo "Backend PID: $BACKEND_PID"
```

Wait up to 10 seconds for the health endpoint:

```bash
for i in $(seq 1 10); do
    curl -s http://localhost:1985/api/health | grep -q '"status":"ok"' && echo "Backend ready." && break
    sleep 1
done
```

If the binary doesn't exist yet, build it first:

```bash
go build -o nexus-open ./cmd/nexus-open
```

### 3. Launch the Flutter UI

```bash
DISPLAY=:1 /home/fictional/Projects/nexus-open/ui/build/linux/x64/debug/bundle/ui &
UI_PID=$!
echo "UI PID: $UI_PID"
sleep 2
```

If the binary doesn't exist, tell the user to run `cd ui && flutter build linux --debug` first (or run `./dev.sh` for the full build+run cycle).

### 4. Confirm the app is running

```bash
curl -s http://localhost:1985/api/health
```

Report the PIDs and health response. The app window should now be visible on DISPLAY=:1.

## Relationship to Other Skills

| Skill | Purpose | Requires running app? |
|---|---|---|
| `/dev-start` | Launch the app | No — this IS the launch |
| `/ui-tour` | Screenshot every settings tab | Yes (or starts it automatically) |

## Troubleshooting

**Backend binary not found** — run `go build -o nexus-open ./cmd/nexus-open` in the repo root.

**Flutter UI binary not found** — run `cd ui && flutter build linux --debug`.

**Port 1985 already in use** — a previous backend is still alive. The kill step above should catch it; if not, run `fuser -k 1985/tcp`.

**UI window doesn't appear** — check `DISPLAY=:1` is the correct X display. Run `xdpyinfo -display :1` to confirm it exists.
