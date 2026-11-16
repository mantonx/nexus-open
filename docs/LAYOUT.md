# Layout reference

Everything the strip shows is set in the settings window. The same
things can be written as text: `nexusctl edit` opens the layout on the
strip in your editor and applies it when you save and close, `nexusctl
export` prints it, and `nexusctl import FILE` adds a layout from a file.
This page lists every key that text can hold. A mistake is refused with
a message that names the place, and nothing changes.

Comments directly above a table (`#` lines) are kept as notes on it.

## A whole layout

```toml
[settings]
font = "ibm-plex-sans"
temperature = "fahrenheit"
clock = "12h"

[weather]
location = "Lisbon"

[[page]]
name = "System"

[[page.widget]]
type = "clock"
width = 160
tap = { page = "Clock" }

[[page.widget]]
type = "value"
width = 160
label = "CPU"
signal = "cpu.temp"
graph = "area"
warn = 75
crit = 90

[[page.widget]]
type = "rate"
width = 160
label = "Net"
down = "net.down"
up = "net.up"

[[page.widget]]
type = "media"
width = 160
tap = { media = "play-pause" }
hold = { media = "next" }

[[page]]
name = "Clock"

[[page.widget]]
type = "clock"
width = 640
tap = { page = "System" }

[[takeover]]
name = "Volume"
on_change = ["volume.level"]
seconds = 3

[[takeover.widget]]
type = "slider"
width = 480
label = "Volume"
controls = "volume"

[[takeover.widget]]
type = "value"
width = 160
label = "Level"
signal = "volume.level"
```

## `[settings]`

All optional.

| Key           | Values                                                                | Default         |
| ------------- | --------------------------------------------------------------------- | --------------- |
| `font`        | `ibm-plex-sans`, `atkinson-hyperlegible`, `roboto`, `noto-sans`, `lato` | `ibm-plex-sans` |
| `temperature` | `celsius`, `fahrenheit`                                               | `celsius`       |
| `clock`       | `24h`, `12h`                                                          | `24h`           |

## `[weather]`

Optional. Without it there is no `weather.temp`. Weather comes from
Open-Meteo, which is sent this place (or the coordinates) and nothing
else.

| Key                      | Value                                                        |
| ------------------------ | ------------------------------------------------------------ |
| `location`               | `"City"`, or `"City, region, country"` to pick the right one |
| `latitude`, `longitude`  | Degrees, both or neither. When given, nothing is looked up.  |

## `[[page]]`

One or more, shown in order; swipe to change page. Each has a unique
`name` and a row of `[[page.widget]]`s, left to right. The strip is
640 px wide: the widths on a page must add up to 640 or less. A page
must have a widget other than a slider, or it could not be swiped away.

## Widgets

Every widget has `type` and `width` (at least 40 px), and any but a
slider may have `tap` and `hold` actions.

| `type`   | Keys                                                                     | Shows                                                        |
| -------- | ------------------------------------------------------------------------ | ------------------------------------------------------------ |
| `clock`  | none                                                                     | Date and time, in the `clock` setting's format               |
| `value`  | `label`, `signal`; optional `graph` (`area`, `line`), `warn`, `crit`     | One reading, colored past its thresholds                     |
| `rate`   | `label`, `down`, `up` (signal names)                                     | Download and upload rates, with a graph of the download rate |
| `media`  | none                                                                     | Title, artist and progress from the `media.*` signals        |
| `slider` | `label`, `controls`; optional `signal`                                   | A bar to drag                                                |

`warn` and `crit` are in the signal's base unit (°C for temperatures),
whichever unit is shown.

A slider's `controls` is one of:

- `"volume"`: the default audio output, set as the finger moves.
- `"seek"`: the position in the track the media widget shows, set on
  release.
- `{ run = ["program", "arg", "{value}"] }`: runs the program as the
  finger moves, with `{value}` replaced by 0–100. A `signal` (0–100)
  makes the bar show it; without one it shows the last value set.

A touch that lands on a slider belongs to it, so a slider has no `tap`
or `hold`.

## Actions

`tap` and `hold` (a long press) take one action, written as a one-key
table:

| Action                                         | Does                                                         |
| ---------------------------------------------- | ------------------------------------------------------------ |
| `{ page = "Clock" }`                           | Shows a page: its name, `"next"` or `"prev"`                 |
| `{ media = "play-pause" }`                     | `play-pause`, `next`, `previous` or `stop` for the player   |
| `{ run = ["pavucontrol", "-t", "3"] }`         | Starts a program with arguments. No shell: write `["sh", "-c", "…"]` for one |
| `{ open = "https://example.org" }`             | Opens a URL or file in the desktop's default app             |
| `{ publish = { signal = "mode", value = "focus" } }` | Sets a signal: a number if it reads as one, else text  |

## `[[takeover]]`

A row that covers the whole strip for a few seconds when one of its
signals changes: the new track, the new volume. A touch keeps it up; a
swipe or the timer puts the page back.

| Key          | Value                                                                  |
| ------------ | ---------------------------------------------------------------------- |
| `name`       | Unique                                                                 |
| `on_change`  | Signal names. Not when a signal first appears, nor while the page on the strip already shows it. |
| `seconds`    | 0.5 to 60: how long it stays after the change or the last touch       |

Its `[[takeover.widget]]`s follow the same rules as a page's.

## `[[source]]`

A program that publishes signals, one JSON object per line. The
protocol is in [EXTERNAL_SOURCES.md](EXTERNAL_SOURCES.md).

```toml
[[source]]
name = "stars"
run = ["python3", "stars.py"]
```

| Key       | Value                                                                         |
| --------- | ----------------------------------------------------------------------------- |
| `name`    | Letters, digits, `-` and `_`; unique. Names the program in logs.             |
| `run`     | The program and its arguments, with no shell. A relative program is found in the config folder. |
| `enabled` | `false` keeps it without running it. Default `true`.                         |

An exported layout leaves out `[weather]` and the sources: a layout file
is made to be shared, and those say where you live and what is on your
computer.

## Signals

Signals are named readings, in base units. The built-in sources publish:

| Signals                                                  | Unit              |
| -------------------------------------------------------- | ----------------- |
| `cpu.temp`, `gpu.temp`, `weather.temp`                   | degrees Celsius   |
| `cpu.load`, `gpu.load`, `mem.used`                       | percent           |
| `net.down`, `net.up`                                     | bytes per second  |
| `volume.level`                                           | percent           |
| `volume.muted`                                           | 1 if muted, else 0 |
| `media.title`, `media.artist`, `media.status`            | text              |
| `media.position`, `media.length`                         | seconds           |

The settings window's Signals list shows every signal published right
now, with its value.
