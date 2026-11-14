#!/bin/sh
# An external source for Nexus Open: how full a disk is.
#
# Copy this file next to nexus.toml and add:
#
#   [[source]]
#   name = "disk"
#   run = ["sh", "disk.sh"]
#
# It publishes disk.used, in percent, every 30 seconds. Give a mount point
# as the first argument to watch that instead of /. Works on Linux and macOS.

mount=${1:-/}
while :; do
    used=$(df -P "$mount" | awk 'NR == 2 { sub("%", "", $5); print $5 }')
    if [ -n "$used" ]; then
        printf '{"disk.used": {"value": %s, "unit": "percent"}}\n' "$used"
    else
        echo "cannot read $mount" >&2
    fi
    sleep 30
done
