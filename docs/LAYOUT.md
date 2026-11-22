# Layout reference

Everything the strip shows is set in the settings window. The same
things can be written as text: `nexusctl edit` opens the layout on the
strip in your editor and applies it when you save and close, `nexusctl
export` prints it, and `nexusctl import FILE` adds a layout from a file.
This page lists every key that text can hold. A mistake is refused with
a message that names the place, and nothing changes.

Comments directly above a table (`#` lines) are kept as notes on it.

## Coming from the old app

The Go and Flutter Nexus Open kept its layout in
`~/.config/nexus-open/nexus.db`. To bring it over:

```
nexusctl import-legacy --print     # see the result; changes nothing
nexusctl import-legacy             # add it as a layout
```

Give a path to read another database, or a `.yaml` file the old app's
Export wrote. The old database is only read, never changed. Each zone
becomes the widget that shows the same reading, with the thresholds the
old plugin colored at; what has no equivalent (a plugin of your own, a
tap that cycled plugins, zone colors, VRAM use) is left out and listed.
The old app's °F and weather place are listed too: set them in Settings
if this app does not already have them.

## A whole layout

```toml
[settings]
font = "ibm-plex-sans"
temperature = "fahrenheit"
clock = "12h"
accent = "#e08a3c"

[weather]
location = "Lisbon"

[[page]]
name = "System"

[[page.widget]]
type = "clock"
width = 140
tap = { page = "Clock" }

[[page.widget]]
type = "value"
width = 160
label = "CPU"
signal = "cpu.temp"
graph = "area"
warn = 75
crit = 90
second = "cpu.load"
second_warn = 80

[[page.widget]]
type = "rate"
width = 140
label = "Net"
down = "net.down"
up = "net.up"

[[page.widget]]
type = "media"
width = 140
tap = { media = "play-pause" }
hold = { media = "next" }

[[page.widget]]
type = "key"
width = 60
icon = "forward"
label = "Next"
tap = { media = "next" }

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

[[takeover]]
name = "Mode"
on_change = ["mode"]
seconds = 4
align = "right"
width = 160

[[takeover.widget]]
type = "key"
width = 160
icon = "moon"
label = "Focus"
tap = { publish = { signal = "mode", value = "focus" } }
```

## `[settings]`

All optional.

| Key           | Values                                                                | Default         |
| ------------- | --------------------------------------------------------------------- | --------------- |
| `font`        | `ibm-plex-sans`, `atkinson-hyperlegible`, `roboto`, `noto-sans`, `lato` | `ibm-plex-sans` |
| `temperature` | `celsius`, `fahrenheit`                                               | `celsius`       |
| `clock`       | `24h`, `12h`                                                          | `24h`           |
| `accent`      | A colour written `"#rrggbb"`, as in `"#e08a3c"`; either case          | the default accent |

Warning and critical colours are separate from the accent and do not
change with it.

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
| `value`  | `label`, `signal`; optional `graph` (`area`, `line`), `warn`, `crit`, `second`, `second_warn`, `second_crit` | One reading, colored past its thresholds; with `second`, a second reading beside it |
| `rate`   | `label`, `down`, `up` (signal names)                                     | Download and upload rates, with a graph of the download rate |
| `media`  | none                                                                     | Title, artist and progress from the `media.*` signals        |
| `slider` | `label`, `controls`; optional `signal`                                   | A bar to drag                                                |
| `key`    | optional `icon` (an icon's name), `label`                                | A control to press; nothing live                             |

`warn` and `crit` are in the signal's base unit (°C for temperatures),
whichever unit is shown. `second` names another signal, shown beside the
first; `second_warn` and `second_crit` are its thresholds, in its own
base unit, and need a `second`. A `crit` on any page, first or second
reading, can raise [the critical alarm](#the-critical-alarm).

A key is what to press for something: give it a `tap`, and a `hold` if
you like. The icon set is not chosen yet, so `icon` is any name for now.

A slider's `controls` is one of:

- `"volume"`: the default audio output, set as the finger moves.
- `"seek"`: the position in the track the media widget shows, set on
  release.
- `{ run = ["program", "arg", "{value}"] }`: runs the program as the
  finger moves, with `{value}` replaced by 0–100. A `signal` (0–100)
  makes the bar show it; without one it shows the last value set.

A touch that lands on a slider belongs to it, so a slider has no `tap`
or `hold`. A slider is moved, not pointed at: a finger landing on it, or
a tap, changes nothing. Dragging moves the value by the distance the
finger travels, from the value it had when the touch began: across the
whole bar is the whole range, and the value stops at its ends.

## Actions

`tap` and `hold` (a long press) take one action, written as a one-key
table. A long press acts as soon as the finger has been held half a
second, while it is still down; lifting afterwards does nothing more.
Lifting sooner is a tap.

| Action                                         | Does                                                         |
| ---------------------------------------------- | ------------------------------------------------------------ |
| `{ page = "Clock" }`                           | Shows a page: its name, `"next"` or `"prev"`                 |
| `{ media = "play-pause" }`                     | `play-pause`, `next`, `previous` or `stop` for the player   |
| `{ run = ["pavucontrol", "-t", "3"] }`         | Starts a program with arguments. No shell: write `["sh", "-c", "…"]` for one |
| `{ open = "https://example.org" }`             | Opens a URL or file in the desktop's default app             |
| `{ publish = { signal = "mode", value = "focus" } }` | Sets a signal: a number if it reads as one, else text  |

## `[[takeover]]`

A row that covers the strip, or one end of it, for a few seconds when
one of its signals changes: the new track, the new volume. A touch on it
keeps it up; a swipe on it or the timer puts the page back.

| Key          | Value                                                                  |
| ------------ | ---------------------------------------------------------------------- |
| `name`       | Unique                                                                 |
| `on_change`  | Signal names. Not when a signal first appears, nor while the page on the strip already shows it. |
| `seconds`    | 0.5 to 60: how long it stays after the change or the last touch on it |
| `align`      | Optional: `"full"` (the default), `"left"` or `"right"`               |
| `width`      | Pixels, 40 to 640: required for `left` and `right`, not allowed for `full` |

Its `[[takeover.widget]]`s follow the same rules as a page's, with the
takeover's width in place of the strip's. Beside a `left` or `right`
takeover the page stays on the strip and works as usual: its readings
stay live, and touches beside the takeover go to the page (a swipe there
changes page, and the takeover goes). Touches on the takeover go to it.

## The critical alarm

When a reading with a `crit` on any page of the layout rises from under
it to it or above, the whole strip says so: the reading's label and
value, and an OK key at the right end. It covers every takeover, and
none shows while it is up. It stays until one of:

- OK is tapped (or long-pressed), which acknowledges every reading
  critical at the time;
- the strip is swiped, which does the same and does not change page;
- the reading has been back under its `crit` for 10 seconds.

Then the page it covered is back. If several readings go critical, it
names the latest. Once its alarm has ended, a reading raises it again
only after staying under its `crit` for 30 seconds in a row.

The first reading of a signal counts: if it is already at or above its
`crit` when it arrives, for example when Nexus Open starts on a hot
machine, the alarm is raised. A signal with no reading, or with text
instead of a number, never raises it, and one whose source stops takes
its alarm down. A signal on several widgets alarms at the lowest `crit`
and is named by that widget's `label`.

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
