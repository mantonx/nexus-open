#!/bin/bash
# A capture rig for the Nexus Open settings window. It runs a THROWAWAY daemon (no device
# access, its own store and runtime folder) and the window inside a headless KWin session,
# so nothing touches the real strip, the user's daemon, layout, window or desktop.
#
# Set NEXUS_RIG_NAME (e.g. u3) and NEXUS_RIG_DIR when more than one rig may run at a time.
#
#   rig.sh up <repo-or-worktree-dir>   build must exist: <dir>/target/release/{nexusd,nexusctl}
#   rig.sh shot <name>                 PNG of the window -> $RIG/shots/<name>.png  (RIG defaults to ~/.cache/nexus-ui-rig)
#   rig.sh click <x> <y>               click at window coordinates (hover first; Slint needs it)
#   rig.sh move <x> <y>                move the pointer there
#   rig.sh drag <x1> <y1> <x2> <y2>    press, move in steps, release
#   rig.sh key <keysym...>             e.g. key Escape, key ctrl+z, key Tab
#   rig.sh type <text>                 type text into the focused field
#   rig.sh size <w> <h>                resize the window (e.g. 1024 640)
#   rig.sh ctl <args...>               nexusctl against the throwaway daemon
#   rig.sh frame <name>                the strip frame the daemon is drawing -> shots/<name>.png (640x48)
#   rig.sh restart-ui [ENV=VAL ...]    close and reopen the window (extra env for it)
#   rig.sh stop-daemon | start-daemon  for the "daemon not running" state
#   rig.sh down                        stop everything and clean up. ALWAYS run this when done.
set -u
HERE="$(cd "$(dirname "$0")" && pwd)"
# Shots and state go here, outside the repo. Override with NEXUS_RIG_DIR.
RIG="${NEXUS_RIG_DIR:-${XDG_CACHE_HOME:-$HOME/.cache}/nexus-ui-rig}"
mkdir -p "$RIG"
# Two rigs can run at once if each has its own name (letters and digits) and NEXUS_RIG_DIR.
NAME="${NEXUS_RIG_NAME:-rig}"
case "$NAME" in *[!a-z0-9]*|"") echo "NEXUS_RIG_NAME must be lowercase letters and digits"; exit 2;; esac
UNIT="nexus-rig-$NAME"
R="$XDG_RUNTIME_DIR/nx$NAME"
ST="$RIG/state"
env_ui() { echo "XDG_RUNTIME_DIR=$R XDG_STATE_HOME=$ST/state XDG_CONFIG_HOME=$ST/config XDG_DATA_HOME=$ST/data"; }
disp() { cat "$ST/display" 2>/dev/null; }
win() { DISPLAY=:$(disp) xdotool search --onlyvisible --name '^Nexus Open' 2>/dev/null | head -1; }
start_daemon() {
  systemd-run --user --quiet --unit "$UNIT" --collect -p DevicePolicy=closed \
    -p DeviceAllow='char-nvidiactl rw' -p DeviceAllow='char-nvidia rw' \
    -E XDG_RUNTIME_DIR="$R" -E XDG_STATE_HOME="$ST/state" -E XDG_CONFIG_HOME="$ST/config" \
    -E DBUS_SESSION_BUS_ADDRESS= \
    dbus-run-session -- "$(cat "$ST/bin")/nexusd" run --no-device --db "$ST/rig.db"
  # A private session bus: on the user's bus the daemon would read their media
  # player and a tap in a test would pause their music. With its own
  # XDG_RUNTIME_DIR it also cannot reach their sound server or start programs.
  for i in $(seq 20); do [ -S "$R/nexus-open.sock" ] && break; sleep 0.3; done
  [ -S "$R/nexus-open.sock" ] || { echo "daemon did not start"; return 1; }
  # Safety: the throwaway daemon must never hold the real strip. (A user unit's
  # DevicePolicy is not enforced, so --no-device is what keeps it off.)
  if XDG_RUNTIME_DIR="$R" "$(cat "$ST/bin")/nexusctl" status | grep -q '^device *connected'; then
    systemctl --user stop "$UNIT"; echo "REFUSED: this build's daemon opened the real strip despite --no-device; rig stopped"; return 1
  fi
}
start_ui() {
  ( env -u WAYLAND_DISPLAY DISPLAY=:$(disp) $(env_ui) "$@" dbus-run-session -- "$(cat "$ST/bin")/nexusd" ui >>"$ST/ui.log" 2>&1 & )
  for i in $(seq 40); do [ -n "$(win)" ] && break; sleep 0.3; done
  [ -n "$(win)" ] || { echo "window did not appear; see $ST/ui.log"; return 1; }
  sleep 1.5; DISPLAY=:$(disp) xdotool windowactivate "$(win)" 2>/dev/null; echo "window $(win) on :$(disp)"
}
# Kill by exact process name and then by arguments, never with `pkill -f`: that also matches
# any shell whose command line merely mentions the pattern.
kill_matching() { for p in $(pgrep -x "$1"); do tr '\0' ' ' < /proc/$p/cmdline 2>/dev/null | grep -qF -- "$2" && kill $p 2>/dev/null; done; }
stop_ui() { kill_matching nexusd "$(cat "$ST/bin")/nexusd ui"; sleep 0.5; }
case "${1:-}" in
  up)
    [ -x "$2/target/release/nexusd" ] || { echo "build first: cargo build --release -p nexusd -p nexusctl (in $2)"; exit 1; }
    "$2/target/release/nexusd" run --help | grep -q -- '--no-device' || { echo "this build has no --no-device flag; merge redesign first"; exit 1; }
    "$0" down >/dev/null 2>&1
    mkdir -p "$R" "$ST/state" "$ST/config" "$ST/data" "$RIG/shots"; chmod 700 "$R"
    echo "$2/target/release" > "$ST/bin"
    # No links to the sound server: a slider drag in a test would change the
    # user's real volume.
    before=$(pgrep -a Xwayland | grep -oE 'Xwayland :[0-9]+' | sort -u)
    ( kwin_wayland --virtual --xwayland --width 1600 --height 1000 --socket "$UNIT" >"$ST/kwin.log" 2>&1 & )
    d=""; for i in $(seq 40); do after=$(pgrep -a Xwayland | grep -oE 'Xwayland :[0-9]+' | sort -u)
      d=$(comm -13 <(echo "$before") <(echo "$after") | grep -oE '[0-9]+$' | head -1); [ -n "$d" ] && break
      # Xwayland starts on demand: poke the candidate displays KWin opened.
      for c in $(ls /tmp/.X11-unix | sed 's/X//'); do DISPLAY=:$c timeout 2 xdotool getdisplaygeometry >/dev/null 2>&1; done; sleep 0.4; done
    [ -n "$d" ] || { echo "no X display from the headless KWin"; exit 1; }
    echo "$d" > "$ST/display"
    start_daemon && start_ui ;;
  shot) DISPLAY=:$(disp) import -window "$(win)" "$RIG/shots/$2.png" && echo "$RIG/shots/$2.png" ;;
  move) DISPLAY=:$(disp) xdotool mousemove --window "$(win)" "$2" "$3" ;;
  click) W=$(win); export DISPLAY=:$(disp); xdotool mousemove --window $W $(($2-5)) $(($3-3)); sleep 0.25
         xdotool mousemove --window $W "$2" "$3"; sleep 0.25; xdotool mousedown 1; sleep 0.08; xdotool mouseup 1; sleep 0.8 ;;
  drag) W=$(win); export DISPLAY=:$(disp); xdotool mousemove --window $W $(($2-5)) $(($3-3)); sleep 0.2
        xdotool mousemove --window $W "$2" "$3"; sleep 0.2; xdotool mousedown 1; sleep 0.1
        for i in 1 2 3 4 5 6 7 8; do xdotool mousemove --window $W $(( $2 + ($4-$2)*i/8 )) $(( $3 + ($5-$3)*i/8 )); sleep 0.06; done
        sleep 0.15; xdotool mouseup 1; sleep 0.8 ;;
  key) shift; DISPLAY=:$(disp) xdotool key --window "$(win)" "$@"; sleep 0.5 ;;
  type) DISPLAY=:$(disp) xdotool type --window "$(win)" --delay 30 "$2"; sleep 0.5 ;;
  size) DISPLAY=:$(disp) xdotool windowsize "$(win)" "$2" "$3"; sleep 1.2; DISPLAY=:$(disp) xdotool getwindowgeometry "$(win)" | tail -1 ;;
  ctl) shift; XDG_RUNTIME_DIR="$R" "$(cat "$ST/bin")/nexusctl" "$@" ;;
  frame) XDG_RUNTIME_DIR="$R" python3 "$HERE/grab.py" "$RIG/shots/$2.png" 0.8 && echo "$RIG/shots/$2.png" ;;
  restart-ui) shift; stop_ui; start_ui "$@" ;;
  stop-daemon) systemctl --user stop "$UNIT"; sleep 1 ;;
  start-daemon) start_daemon ;;
  down)
    [ -f "$ST/bin" ] && stop_ui
    kill_matching kwin_wayland "--socket $UNIT"
    systemctl --user stop "$UNIT" 2>/dev/null
    sleep 0.5
    for p in $(pgrep -x xdg-document-po; pgrep -x xdg-permission-; pgrep -x xdg-desktop-por); do
      tr '\0' '\n' < /proc/$p/environ 2>/dev/null | grep -q "^XDG_RUNTIME_DIR=$R$" && kill $p 2>/dev/null; done
    sleep 0.3
    findmnt -n "$R/doc" >/dev/null 2>&1 && fusermount3 -u "$R/doc" 2>/dev/null
    findmnt -n "$R/doc" >/dev/null 2>&1 && { echo "could not unmount $R/doc; leaving $R in place"; exit 1; }
    rm -rf "$R" "$ST"; echo "rig down" ;;
  *) sed -n 2,20p "$0" ;;
esac
