# External sources

An external source is any program that prints signals for the strip to show:
a shell script, a Python script, a compiled tool. The daemon starts it, reads
what it prints, and starts it again if it stops. Nothing needs to be linked
in or installed as a plugin, and the program can be written in any language
that can print a line.

For a one-off value from a terminal or another tool, `nexusctl publish` does
the same without a running program:

```sh
nexusctl publish backup.state "ok"
nexusctl publish disk.used 71 percent
```

## Adding one

Put the program next to `nexus.toml` (on Linux, `~/.config/nexus-open/`) and
add a `[[source]]` section:

```toml
[[source]]
name = "disk"
run = ["sh", "disk.sh"]
```

- `name`: letters, digits, `-` and `_`. It names the program in the log and,
  on Linux, its systemd unit (`nexus-source-disk`).
- `run`: the program and its arguments. There is no shell, so nothing is
  expanded: write `["sh", "-c", "…"]` if you want one. Programs run in the
  config folder, so a bare script name next to `nexus.toml` is found. A
  relative program path (`./disk.sh`) is taken from that folder too.

Then show its signals with a widget, as for any built-in reading:

```toml
[[page.widget]]
type = "value"
width = 107
label = "Disk"
signal = "disk.used"
graph = "line"
warn = 85
crit = 95
```

Saving the file applies it: new sources start, removed ones stop, and a source
whose `run` changed is restarted. The others keep running.

## The protocol

The program writes to standard output, one JSON object per line. Each key is
a signal name; each value is the reading:

```json
{"disk.used": {"value": 71.5, "unit": "percent"}}
{"backup.state": "ok", "backup.age": 3600}
```

A reading is one of:

| Written as | Means |
|---|---|
| `42`, `3.5` | a plain number |
| `"text"` | text, shown as written |
| `{"value": 52, "unit": "celsius"}` | a number in a unit |

The units are the base units every built-in source uses. Widgets choose how
to show them, so a temperature is always published in Celsius and appears in
°F when the settings ask for it.

| Unit | Use for | Shown as |
|---|---|---|
| `none` | counts, anything else | the number |
| `celsius` | temperatures | °C or °F, per `temperature` in `[settings]` |
| `percent` | 0–100 | `71%` |
| `bytes-per-second` | transfer rates | `9.5M/s` |

Rules:

- **One object per line.** A line can carry any number of signals. A blank
  line is ignored.
- **Signal names** are 1 to 64 characters with no spaces. Use dots to group
  them: `disk.used`, `disk.free`. A name a built-in source also uses (such as
  `cpu.temp`) works but the two will overwrite each other, so pick your own.
- **A mistake drops the whole line**, never part of it. The first three bad
  lines of each run are logged with the reason; after that they are only
  dropped.
- **Lines longer than 64 KB are dropped.**
- **Only what changed needs printing.** A signal keeps its last value until
  the next one. Printing the same value again is harmless.
- **Numbers build history.** Every number is kept for the widget's graph
  (the last 60 readings), so print at the rate the graph should move: every
  second for a live load graph, every few minutes for a disk.

### Flush your output

When a program's output goes to a pipe rather than a terminal, most languages
hold it back until several kilobytes have collected. A script that works in a
terminal then shows nothing on the strip. Flush after each line:

| Language | How |
|---|---|
| Shell (`echo`, `printf`) | nothing to do |
| Python | `print(..., flush=True)`, or run with `python3 -u` |
| Node.js | `console.log` already writes at once |
| C | `fflush(stdout)` after each line, or `setvbuf(stdout, NULL, _IOLBF, 0)` |
| Rust (`println!`) | nothing to do: stdout is line-buffered even into a pipe |

### Standard error

Anything the program writes to standard error goes to the daemon's log,
prefixed with the source's name. On Linux:

```sh
journalctl --user -u nexusd -f
```

## What the daemon does with it

- **Starts it** when the daemon starts and when the source is added.
- **Starts it again** if it exits, for whatever reason: after 2 seconds, then
  4, 8 and so on up to a minute while it keeps failing. A run that lasted a
  minute counts as healthy, and the wait goes back to 2 seconds. Each exit is
  logged with its status.
- **Stops it** when the source is removed or changed, and when the daemon
  stops.
- **Keeps the last values** if it exits or is removed: the strip shows the
  last reading rather than going blank.

On Linux, when the daemon runs as its systemd service, each source runs as
its own user unit, `nexus-source-NAME`, outside the daemon's sandbox. It can
do what you can do in a terminal: read your files, use the network, talk to
Docker. The unit is bound to the daemon's, so systemd stops it if the daemon
stops or crashes. To see one:

```sh
systemctl --user status nexus-source-disk
```

Standard input is empty. Under systemd the program gets your user service
manager's environment (`systemctl --user show-environment`); started by
hand, the daemon's.

## Trying one out

Run the program in a terminal first: each line it prints is what the daemon
will read. Then add it to the config and check:

```sh
nexusctl signals | grep disk
```

## Examples

Both are in [`examples/sources/`](../examples/sources/).

### Shell: disk use

```sh
#!/bin/sh
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
```

```toml
[[source]]
name = "disk"
run = ["sh", "disk.sh"]
```

### Python: GitHub stars

Standard library only. Note `flush=True`.

```python
#!/usr/bin/env python3
import json, sys, time, urllib.error, urllib.request

REPO = sys.argv[1] if len(sys.argv) > 1 else "mantonx/nexus-open"

def publish(signals):
    print(json.dumps(signals), flush=True)

while True:
    request = urllib.request.Request(
        f"https://api.github.com/repos/{REPO}",
        headers={"Accept": "application/vnd.github+json", "User-Agent": "nexus-open-source"},
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            repo = json.load(response)
        publish({"github.stars": repo["stargazers_count"], "github.issues": repo["open_issues_count"]})
    except (urllib.error.URLError, KeyError, ValueError) as e:
        print(f"{REPO}: {e}", file=sys.stderr, flush=True)
    time.sleep(15 * 60)
```

```toml
[[source]]
name = "github"
run = ["python3", "github-stars.py", "owner/repo"]
```
