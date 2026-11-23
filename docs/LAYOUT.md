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
accent = "sky"
night = { from = "22:30", to = "07:00" }
idle_dim = true

[weather]
location = "Lisbon"

[[page]]
name = "System"

[[page.widget]]
type = "clock"
width = 128
tap = { page = "Desk" }

[[page.widget]]
type = "value"
width = 200
label = "CPU"
signal = "cpu.temp"
warn = 75
crit = 90
second = "cpu.load"
second_label = "load"
second_warn = 80

[[page.widget]]
type = "rate"
width = 140
label = "Net"
down = "net.down"
up = "net.up"

[[page.widget]]
type = "media"
width = 172
hold = { media = "next" }

[[page]]
name = "Desk"

[[page.widget]]
type = "clock"
width = 320
tap = { page = "System" }

[[page.widget]]
type = "value"
width = 160
label = "Water"
signal = "loop.temp"
icon = "drop"
scale = [20, 45]
crit = 40

[[page.widget]]
type = "key"
width = 160
icon = "terminal-window"
label = "Shell"
sub = "Console"
tap = { run = ["kgx"] }

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
| `font`        | `rubik`, `ibm-plex-sans`, `atkinson-hyperlegible`, `roboto`, `noto-sans`, `lato` | `rubik` |
| `temperature` | `celsius`, `fahrenheit`                                               | `celsius`       |
| `clock`       | `24h`, `12h`                                                          | `24h`           |
| `accent`      | A colour written `"#rrggbb"` (either case), or a preset's name: `lilac`, `sky`, `teal`, `mint`, `rose`, `lemon` | `lilac` |
| `night`       | `{ from = "22:30", to = "07:00" }`: hours, `"HH:MM"`, when the strip is dimmed; across midnight when `to` is earlier | none |
| `idle_dim`    | `true`, `false`: dim after five minutes with no touch               | `true`          |

`font` chooses the face for words: labels, titles, keys. Numbers are
always drawn in Rubik, so a reading keeps its shape whatever is chosen.

The accent goes on what a finger can act on: a slider's fill, a key's
press, the play key. Warning (amber) and critical (red) are separate
from it and do not change with it. So that no accent can be mistaken for
them, a colour in the orange-to-yellow range is moved to the nearest hue
outside it: `"#ffc53a"` is drawn as an olive yellow, `"#ff9a62"` as a
pink. An exported layout writes a preset by its name.

Dimming draws everything darker, never smaller, and night dims more
than idle. Idle dimming waits while a reading on any page is at warning
or critical. Nothing is dimmed while the alarm or a takeover is up, and
a warning or critical card stays bright even at night.

## `[weather]`

Optional. Without it there is no weather. Weather comes from
Open-Meteo, which is sent this place (or the coordinates) and nothing
else, every 15 minutes. With no new forecast for 45 minutes, the weather
widget shows the last one dimmed, with the time it is from.

| Key                      | Value                                                        |
| ------------------------ | ------------------------------------------------------------ |
| `location`               | `"City"`, or `"City, region, country"` to pick the right one |
| `latitude`, `longitude`  | Degrees, both or neither. When given, nothing is looked up.  |

## `[news]`

Optional. Without it, or with no feeds, there is no news and nothing is
fetched. Each feed is asked for its headlines every 15 minutes, and
nothing else is sent. A feed that fails is tried again after a minute,
then two, four, and so on up to half an hour; its last headlines stay
meanwhile.

| Key       | Value                                                                    | Default |
| --------- | ------------------------------------------------------------------------ | ------- |
| `feeds`   | Feed addresses (RSS, Atom or JSON Feed), `https://` or `http://`. On a tie the earlier feed's copy of a story is shown | none |
| `rotate`  | How long each headline shows before the next; at least 2 seconds         | `"12s"` |
| `max_age` | Headlines published longer ago are not shown                             | `"48h"` |

A duration is a number of seconds (`rotate = 12`) or a number with a
unit: `"30s"`, `"15m"`, `"48h"`, `"2d"`. Up to 50 headlines are kept
across all feeds, newest first, and shown one at a time on every news
widget.

A page for the news, with a clock beside it:

```toml
[news]
feeds = ["https://news.example/rss.xml"]

[[page]]
name = "News"

[[page.widget]]
type = "clock"
width = 128

[[page.widget]]
type = "news"
width = 512
```

## `[media]`

Optional.

| Key         | Values                       | Default |
| ----------- | ---------------------------- | ------- |
| `album_art` | `"all"`, `"local"`, `"off"`  | `"all"` |

The media widget shows the track's artwork where the player gives an
address for it (MPRIS `mpris:artUrl`), and a generated picture
otherwise: while the artwork is on its way, and whenever it cannot be
read. `album_art` says where artwork may come from:

- `"all"` also **fetches cover images from the player's image host**:
  an `https://` or `http://` address is asked for once per track with a
  plain GET, with Nexus Open's user agent and nothing else, at most
  5 MB, giving up after 5 seconds. It is the address the player itself
  fetched the picture from (a music service's image server, or the site
  a browser is playing from), so that host learns your computer's
  address and that this cover was asked for, as it did from the player.
- `"local"` reads only images on this computer (`file://`: a regular
  file under 10 MB, not a link) or inside the address itself (`data:`,
  base64, up to 2 MB). Nothing is fetched.
- `"off"`: always the generated picture.

Any other kind of address is ignored. Images are PNG, JPEG or WebP, up
to 4096 px a side. One that cannot be read leaves the generated
picture, and a line in the log once per track that says why, with no
query string or image data from the address. The last few covers are
kept in memory, so going back to a track shows its cover at once.

## `[[page]]`

One or more, shown in order; swipe to change page. Each has a unique
`name` and a row of `[[page.widget]]`s, left to right. The strip is
640 px wide: the widths on a page must add up to 640 or less. A page
must have a widget other than a slider, or it could not be swiped away.

## Widgets

Every widget has `type` and `width` (at least 40 px), and any but a
slider may have `tap` and `hold` actions. Any widget may have `icon`,
the name of an icon (below) to draw instead of the one it would choose.

| `type`   | Keys                                                                     | Shows                                                        |
| -------- | ------------------------------------------------------------------------ | ------------------------------------------------------------ |
| `clock`  | none                                                                     | Time, and the date when there is room, in the `clock` setting's format |
| `value`  | `label`, `signal`; optional `warn`, `crit`, `scale`, `second`, `second_label`, `second_warn`, `second_crit` | One reading over a fixed-scale bar, the card amber or red past its thresholds; with `second`, a second reading beside it. `signal = "weather.temp"` shows the weather |
| `rate`   | `label`, `down`, `up` (signal names)                                     | Download and upload rates, each over its bar                 |
| `media`  | none                                                                     | Title, artist and progress from the `media.*` signals, with transport keys |
| `slider` | `label`, `controls`; optional `signal`                                   | A bar to drag                                                |
| `key`    | optional `icon`, `label`, `sub`                                          | A control to press; nothing live. `sub` is a second line under the label on a key 300 px or wider |
| `news`   | none                                                                     | The headline showing, over two lines; on a hero card where it is from and how old it is (`Harbour Gazette · 12 min`) after it, when there is room |

How much a widget shows depends on its width: up to 149 px it is
compact (the reading alone), up to 299 px medium (with its label, or a
second reading), and from 300 px hero (everything, large). A `second`
reading shows from medium up.

`warn` and `crit` are in the signal's base unit (°C for temperatures),
whichever unit is shown. A hero card says the limit (`limit 167°`) from
`warn`, or `crit` without one. `second` names another signal, shown
beside the first; `second_warn` and `second_crit` are its thresholds, in
its own base unit, and need a `second`. `second_label` names it where it
is named: in [the critical alarm](#the-critical-alarm) it is "CPU load"
from `label = "CPU"`, `second_label = "load"`; without one, the
signal's last word. A `crit` on any page, first or second reading, can
raise the alarm.

Bars have a fixed scale, so a glance compares them: temperatures from
30 to 100 °C, percentages from 0 to 100, rates from 1 kB/s to 1 GB/s on
a logarithmic scale. `scale = [min, max]`, in the signal's base unit,
sets another (for a coolant loop, say). A signal with no unit has no bar
without one. `graph` from older layouts is accepted and drawn as the bar.

A key is what to press for something: give it a `tap`, and a `hold` if
you like. Pressing lights it at once; while a `hold` is pending it fills
from the left, and full, it runs when the finger lifts.

A media widget showing a track has transport keys where it draws them:
a compact one has a play/pause key; a medium one is one play/pause key
all over, and its `hold` still works; a hero one has a play/pause key
at the right end, or previous, play/pause and next from 400 px. A long
press on a key is a slow tap. Elsewhere on the widget its own `tap` and
`hold` apply. With nothing playing there are no keys.

A news widget shows one headline at a time, over two lines, the second
cut short with an ellipsis when it is too long; it never scrolls. A compact one has room for the
newspaper icon and the headline's age only. A tap opens the headline in
the default browser (the one showing when the finger landed, even if
the next arrives before it lifts), and a hold shows the next one; its
own `tap` and `hold` replace them. Without a `[news]` feed it says how
to add one. While the first headlines are fetched it says "No news
yet"; with every feed failing and nothing new enough left, the last
headline stays, dimmed, marked with a broken link, its age still
growing. Neither of those can be tapped. News never covers the strip: a takeover cannot be
raised by a `news.` signal.

Icons: `activity`, `arrow-down`, `arrow-fat-down`, `arrow-fat-up`,
`arrow-up`, `arrows-left-right`, `bell`, `browser`, `calculator`,
`calendar-blank`, `camera`, `chat-circle`, `check`, `check-circle`,
`clock`, `cloud`, `cloud-moon`, `cloud-rain`, `cloud-slash`, `cloud-sun`,
`code`, `cpu`, `drop`, `envelope-simple`, `fan`, `folder-simple`,
`game-controller`, `gauge`, `gear`, `globe-simple`, `graphics-card`,
`hand-pointing`, `hand-tap`, `headphones`, `house`, `image`,
`lightning`, `link-break`, `map-pin`, `map-pin-plus`, `memory`, `moon`,
`moon-stars`, `music-note`, `music-notes`, `newspaper`, `pause`, `play`, `plugs`,
`plus`, `power`, `rocket-launch`, `skip-back`, `skip-forward`,
`speaker-high`, `speaker-low`, `speaker-none`, `speaker-x`,
`squares-four`, `sun`, `terminal-window`, `thermometer`,
`thermometer-simple`, `warning`, `warning-circle`, `wifi-high`, `x`
(from [Phosphor](https://phosphoricons.com)). Another name is kept but
draws no icon.

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
table. A long press is ready once the finger has been held still half a
second (the card fills to show it) and acts when the finger lifts.
Sliding the finger away instead cancels it (see Gestures). Lifting
sooner is a tap.

| Action                                         | Does                                                         |
| ---------------------------------------------- | ------------------------------------------------------------ |
| `{ page = "Clock" }`                           | Shows a page: its name, `"next"` or `"prev"`                 |
| `{ media = "play-pause" }`                     | `play-pause`, `next`, `previous` or `stop` for the player   |
| `{ run = ["pavucontrol", "-t", "3"] }`         | Starts a program with arguments. No shell: write `["sh", "-c", "…"]` for one |
| `{ open = "https://example.org" }`             | Opens a URL or file in the desktop's default app; a leading `~` is your home folder |
| `{ publish = { signal = "mode", value = "focus" } }` | Sets a signal: a number if it reads as one, else text  |
| `{ news = "open" }`                            | Opens the headline showing in the default browser            |
| `{ news = "next" }`                            | Shows the next headline now                                  |

Only `https://` and `http://` links are opened, passed to the desktop's
opener as they are, never through a shell. A link with spaces, control
characters or invisible characters in it is not opened.

## Gestures

The strip senses where a finger is along its length, and nothing else.

| Gesture         | How                                                        | Does |
| --------------- | ---------------------------------------------------------- | ---- |
| Tap             | Touch and lift within half a second                        | The widget's `tap` |
| Hold            | Keep the finger still half a second, then lift             | The widget's `hold`, on the lift |
| Swipe           | Move along the strip straight away                         | The next or previous page |
| Drag            | Move along a slider                                        | Sets the slider; never changes page |
| Hold, then slide | Keep the finger still half a second, then slide it either way | Opens the layout picker: slide to a layout's name and lift to switch to it, or lift back on the current one to leave things as they are |

Hold, then slide works anywhere except on a slider, and only with two
or more layouts. It takes the place of a widget's `hold`: the hold
action does not run once the finger slides away. Layouts appear in the
order the settings window lists them. After a switch the strip shows the
layout's name for a moment, then its first page.

Up and down swipes are not possible on this hardware: the strip reports
no vertical position, and a vertical swipe arrives as a tap
([USB_PROTOCOL.md](USB_PROTOCOL.md)).

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

A takeover fades in over the page and, when its time is up, fades out.
A volume takeover shows the new level, the change, and a faint mark on
the bar at the level it came from.

Its `[[takeover.widget]]`s follow the same rules as a page's, with the
takeover's width in place of the strip's. Beside a `left` or `right`
takeover the page stays on the strip and works as usual: its readings
stay live, and touches beside the takeover go to the page (a swipe there
changes page, and the takeover goes). Touches on the takeover go to it.

## The critical alarm

When a reading with a `crit` on any page of the layout rises from under
it to it or above, the whole strip says so, red and slowly breathing:
the reading's name, value and limit, and an OK key at the right end
(the last 94 px take the touch). It covers every takeover, and none
shows while it is up. It stays until one of:

- OK is tapped (or long-pressed), which acknowledges every reading
  critical at the time, and shows the page with the reading;
- the strip is swiped (or held, then slid), which does the same and does
  not change page or layout;
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

An exported layout leaves out `[weather]`, `[news]` and the sources: a
layout file is made to be shared, and those say where you live, what you
read (a feed address can hold a private key) and what is on your
computer.

## Signals

Signals are named readings, in base units. The built-in sources publish:

| Signals                                                  | Unit              |
| -------------------------------------------------------- | ----------------- |
| `cpu.temp`, `gpu.temp`, `weather.temp`, `weather.high`, `weather.low` | degrees Celsius |
| `cpu.load`, `gpu.load`, `mem.used`                       | percent           |
| `weather.code`                                           | WMO weather code  |
| `weather.condition`                                      | text, as "Cloudy" |
| `weather.day`                                            | 1 by day, else 0  |
| `net.down`, `net.up`                                     | bytes per second  |
| `volume.level`                                           | percent           |
| `volume.muted`                                           | 1 if muted, else 0 |
| `media.title`, `media.artist`, `media.status`            | text              |
| `media.position`, `media.length`                         | seconds           |
| `media.art`                                              | text: which artwork the media widget draws (`art-` and a hash of its address), empty while it shows the generated picture |
| `news.title`, `news.source`, `news.link`                 | text              |
| `news.age`                                               | seconds since the headline was published |
| `news.count`                                             | how many headlines are kept |
| `news.lost`                                              | 1 while every feed fails and the headline shown is too old, else 0 |

The settings window's Signals list shows every signal published right
now, with its value. When a source stops publishing a signal, a widget
keeps its last reading, dimmed, with the time it was lost; a lost
reading raises nothing.
