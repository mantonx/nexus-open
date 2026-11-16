#!/bin/sh
# Records the running nexusd service once a minute, for the Phase 5 soak:
# its PID (a change means it restarted), CPU used since the last sample,
# resident memory, threads, open files and lines logged. Read-only: it
# looks at /proc and the journal, nothing else.
#
#   systemd-run --user --unit nexusd-soak scripts/soak-record.sh [HOURS]
#
# Writes ~/.local/state/nexusd/soak/<start>.csv. Stop early with
# systemctl --user stop nexusd-soak.
set -eu
hours=${1:-72}
dir="${XDG_STATE_HOME:-$HOME/.local/state}/nexusd/soak"
mkdir -p "$dir"
out="$dir/$(date +%Y%m%d-%H%M%S).csv"
tick=$(getconf CLK_TCK)
echo "time,pid,cpu_percent,rss_kb,threads,fds,log_lines" > "$out"
end=$(( $(date +%s) + hours * 3600 ))
last_pid="" last_ticks=0 last_time=0
since=$(date '+%Y-%m-%d %H:%M:%S')
while [ "$(date +%s)" -lt "$end" ]; do
    now=$(date +%s)
    pid=$(systemctl --user show -p MainPID --value nexusd)
    if [ "$pid" != 0 ] && [ -r "/proc/$pid/stat" ]; then
        # utime + stime: fields 14 and 15, after the command in brackets.
        ticks=$(sed 's/.*) //' "/proc/$pid/stat" | awk '{print $12 + $13}')
        cpu=""
        if [ "$pid" = "$last_pid" ] && [ "$now" -gt "$last_time" ]; then
            cpu=$(awk -v d=$((ticks - last_ticks)) -v t="$tick" -v s=$((now - last_time)) \
                'BEGIN { printf "%.2f", 100 * d / t / s }')
        fi
        rss=$(awk '/^VmRSS:/ {print $2}' "/proc/$pid/status")
        threads=$(awk '/^Threads:/ {print $2}' "/proc/$pid/status")
        fds=$(ls "/proc/$pid/fd" 2>/dev/null | wc -l)
        lines=$(journalctl --user -u nexusd --since "$since" -q -o cat 2>/dev/null | wc -l)
        echo "$(date -Is),$pid,$cpu,$rss,$threads,$fds,$lines" >> "$out"
        last_pid=$pid last_ticks=$ticks last_time=$now
    else
        echo "$(date -Is),0,,,,," >> "$out"
    fi
    sleep 60
done
