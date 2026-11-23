---
name: Nexus Open strip
description: The design system of the 640×48 touch strip, warm and friendly, built in crates/nexus-core
colors:
  ground: "#14110f"
  card: "#241e1a"
  key: "#4a3e35"
  key-top: "#65564a"
  track: "#3d342d"
  tick: "#7a6b5e"
  text: "#fbf1e3"
  text-2: "#d2c2ad"
  text-3: "#9c8c7c"
  fill: "#e6d6c0"
  warn: "#ffb23e"
  warn-surface: "#4f3612"
  warn-zone: "#6b4a1d"
  crit: "#d93425"
  crit-ink: "#ffffff"
  crit-deep: "#8a1d14"
  crit-zone: "#6e2a22"
  accent-default: "#b6a2ff"
typography:
  num-hero: { fontFamily: "Rubik", fontSize: "50px", fontWeight: 600, letterSpacing: "-0.5px", fontFeature: "tnum" }
  num-medium: { fontFamily: "Rubik", fontSize: "40px", fontWeight: 600, letterSpacing: "-0.4px", fontFeature: "tnum" }
  num-hero-banded: { fontFamily: "Rubik", fontSize: "40px", fontWeight: 600, letterSpacing: "-0.4px", fontFeature: "tnum" }
  num-banded: { fontFamily: "Rubik", fontSize: "36px", fontWeight: 600, letterSpacing: "-0.4px", fontFeature: "tnum" }
  num-compact: { fontFamily: "Rubik", fontSize: "32px", fontWeight: 600, letterSpacing: "-0.3px", fontFeature: "tnum" }
  num-second: { fontFamily: "Rubik", fontSize: "26px", fontWeight: 600, letterSpacing: "-0.2px", fontFeature: "tnum" }
  title-big: { fontFamily: "Rubik", fontSize: "26px", fontWeight: 560, letterSpacing: "-0.2px" }
  unit-hero: { fontFamily: "Rubik", fontSize: "22px", fontWeight: 560 }
  title: { fontFamily: "Rubik", fontSize: "20px", fontWeight: 560 }
  key-label: { fontFamily: "Rubik", fontSize: "18px", fontWeight: 560 }
  unit: { fontFamily: "Rubik", fontSize: "18px", fontWeight: 560 }
  body: { fontFamily: "Rubik", fontSize: "18px", fontWeight: 480, letterSpacing: "0.1px" }
  label: { fontFamily: "Rubik", fontSize: "18px", fontWeight: 560, letterSpacing: "0.2px" }
rounded:
  card: "10px"
  key: "9px"
  bar: "999px"
spacing:
  edge: "2px"
  gutter: "4px"
  pad: "9px"
  icon-gap: "6px"
components:
  card: { backgroundColor: "{colors.card}", rounded: "{rounded.card}", height: "41px" }
  card-warn: { backgroundColor: "{colors.warn-surface}", textColor: "{colors.warn}", rounded: "{rounded.card}" }
  card-crit: { backgroundColor: "{colors.crit}", textColor: "{colors.crit-ink}", rounded: "{rounded.card}" }
  key: { backgroundColor: "{colors.key}", rounded: "{rounded.key}", height: "33px", width: "44px" }
  bar: { backgroundColor: "{colors.track}", rounded: "{rounded.bar}", height: "5px" }
---

# Design System: Nexus Open strip

This is the strip's visual system, as `crates/nexus-core` draws it since
phase U3. Its frames are rendered by the shipped renderer from real
layouts and readings (`nexus_core::scenes`):
`cargo run -p nexus-core --example design_system -- <dir>`, and
`tests/frames.rs` holds them to it. Every number below is a named constant
in **one module**, `crates/nexus-core/src/theme.rs` (the media and alarm
geometry in `widget.rs`), and the names here are those names. **All sizes
are provisional**: the panel has not been measured (U0's legibility card
has not been read). When it has, change `theme.rs` and this file
together.

The structure is the one Matt chose at the U1 gate: pages of widgets the
user arranges and swipes between ("better cards"), with the by-context
direction's critical alarm, media scene and hero scale, and the instrument
direction's fixed-scale bar. The character is new.

## Overview

**Creative North Star: "Lamplight on walnut."**

A small companion that lives at the foot of the screen, like a clock on a
desk under a warm lamp: dark walnut ground, cream lettering the colour of
paper in lamplight, rounded pebble-soft cards, and one small enamel colour
(the accent, the user's choice) on the things a finger can move. It is
calm at rest and plain-spoken; when something is wrong it does not
whisper, it lights a whole card amber or the whole strip red.

It is not an instrument panel (no tapes of tiny segments, no monospace, no
uppercase labels), not gaming hardware (no neon, no glow-on-black, no
angular shapes), and not a toy (one sturdy face at real weights, no
mascots, no bouncy motion, no pastel rainbow). The three U1 mockups were
cool blue-black with an aqua accent; that look is gone. From C we keep the
structure and its good ideas: accent means touchable, page markers, zoned
bars, dashed "set up" cards, a distinct "no data".

**Key characteristics**

- Warm neutrals only: every grey leans brown (OKLCH hue about 60). Nothing
  blue-black.
- One face, Rubik, whose slightly softened corners carry the friendliness;
  numerals at weight 600, words at 480–560, sentence case everywhere.
- Rounded everything: cards 10 px, keys 9 px, bars and slider tracks are
  pills, thumbs are discs. No sharp corner on the strip.
- Icons from Phosphor: filled for things (CPU, cloud, speaker), bold
  outline for actions on keys (globe, folder, terminal) and for empty
  states.
- Cream is data, the accent is touch, amber and red are trouble. Nothing
  else has colour, except artwork.
- Plain words: "Nothing playing", "Add your town", "GPU is too hot".

## Colors

Warm, low-chroma browns and creams, with one user accent and two severity
colours that never come from the accent. Contrast ratios are WCAG, printed
by the example (`tokens.txt` beside the frames).

### Neutrals (fixed)

| Token | Hex | On CARD | On GROUND | Use |
|---|---|---|---|---|
| `GROUND` | #14110f | 1.1 | – | The strip between and around cards. Walnut, almost black. |
| `CARD` | #241e1a | – | 1.1 | Every widget's surface. |
| `KEY` | #4a3e35 | 1.6 | 1.8 | Raised key faces: two steps lighter than the card, so a key never looks like a reading. |
| `KEY_TOP` | #65564a | 2.3 | 2.7 | The 1 px lit top edge of a key. |
| `TRACK` | #3d342d | 1.4 | 1.5 | Empty part of bars, sliders and progress lines. |
| `TICK` | #7a6b5e | 3.2 | 3.7 | Scale ticks under a hero slider. |
| `TEXT` | #fbf1e3 | 14.7 | 16.8 | Numerals, titles. Paper cream. |
| `TEXT2` | #d2c2ad | 9.5 | 10.8 | Second lines, labels, data icons. |
| `TEXT3` | #9c8c7c | 5.1 | 5.8 | Quiet words, "no data" dashes, lost readings. |
| `FILL` | #e6d6c0 | 11.6 | 13.2 | A bar's fill when nothing is wrong. |

KEY and TRACK sit only 1.4–1.6:1 from CARD on purpose: they are surfaces,
told apart by shape (raised edge, pill) more than by tone. On the real
panel they may need to be lighter; that is a U0 measurement.

### Severity (fixed, never from the accent)

| Token | Hex | Contrast | Use |
|---|---|---|---|
| `WARN` | #ffb23e | 6.3 on WARN_SURFACE, 9.2 on CARD | Numeral, icon and bar fill of a warning card; warning page marker. OKLCH L 0.82, h 73. |
| `WARN_SURFACE` | #4f3612 | – | The whole card's surface at warning: brighter than CARD (L 0.35 vs 0.24). |
| `WARN_ZONE` | #6b4a1d | – | The warning zone tinted on every bar's track. |
| `CRIT` | #d93425 | 4.7 with CRIT_INK | The whole card's surface at critical; the whole strip in the alarm. L 0.58, h 30. |
| `CRIT_INK` | #ffffff | 4.7 on CRIT | Everything drawn on CRIT: numerals, icons, the bar fill, the OK key face. |
| `CRIT_DEEP` | #8a1d14 | – | The empty track on a critical card. |
| `CRIT_ZONE` | #6e2a22 | – | The critical zone tinted on every bar's track. |

### The accent (the user's colour)

The accent marks what a finger can act on: the primary key's face (play
and pause, OK is the exception), slider thumbs and fills, glyphs on keys,
the dashed "set up" outline and its words, the medium media card's
play/pause key (the whole card is the key it labels), the rim
of a partial takeover that holds a control, and the rim of anything
pressed. **It never fills a card, never colours a reading, never colours
a page marker, and never colours information**: the volume change
("+15"), the artist, the times, the progress line and a slider's number
(even while dragged; the held thumb shows the finger) are all in the
text ramp. A partial takeover that holds no control is rimmed in
`KEY_TOP`, not the accent.

The user picks any colour (a setting, `accent`). Only its **hue** is used,
and its chroma is capped; every role's lightness is fixed, in OKLCH:

| Role | Field | OKLCH L | Chroma | Use |
|---|---|---|---|---|
| base | `Accent::base` | `ACCENT_L` 0.80 | min(user, `ACCENT_C_MAX` 0.15) | Key faces, thumbs, dashed outline |
| ink | `Accent::ink` | = GROUND | – | Glyphs on `base` (always dark: 9.7–10.2:1 on the four accents) |
| text | `Accent::text` | `ACCENT_TEXT_L` 0.84 | min(user, 0.12) | Accent words and icons on a card (9.8–10.2:1 on CARD) |
| fill | `Accent::fill` | `ACCENT_FILL_L` 0.66 | min(user, 0.12) | Slider fill: a step darker so a long fill stays calm |
| soft | `Accent::soft` | `ACCENT_SOFT_L` 0.38 | min(user, 0.07) | Pressed key faces, the halo of a held thumb |

Below `ACCENT_GREY_C` (0.02 chroma) the user's colour counts as grey and
every role is a warm grey at the same lightness. Out-of-gamut results lose
chroma (hue and lightness kept) until they fit sRGB. Code:
`Accent::from` and `from_oklch` in `theme.rs`.

**No accent in amber or red's hues.** A hue from `ACCENT_AVOID` 15° to
105° (OKLCH) is moved to the nearer end of that band before the roles are
made: from a little under CRIT (h 30) to 32° past WARN (h 73). So an
apricot pick is drawn as a pink and a marigold pick as an olive yellow,
and nothing the accent touches can be read as a warning. (The U2 frames
drew apricot and marigold as asked; the review read a marigold slider
beside a warning card as a second warning, open question 9.) The band
first ended at 98°, but a marigold snapped there still read as amber's
mustard cousin beside a warning card.

Presets, which the `accent` setting also takes by name, all outside the
band: `lilac` (the default), `sky`, `teal`, `mint`, `rose`, `lemon`.

| Accent | User picks | base | text | fill | soft |
|---|---|---|---|---|---|
| **lilac (default)** | #b6a2ff | #c0b0fe | #ccc1fe | #9583d5 | #443b64 |
| sky | #5fb4ff | #86c4fe | #a0d0fe | #5098d8 | #204566 |
| teal | #4fd1c5 | #54d5c9 | #63e3d6 | #09a89d | #014d48 |
| mint | #7ad9b0 | #75d4ab | #82e1b8 | #46a780 | #144e38 |
| rose | #ff8fb1 | #fe9cb8 | #ffb1c6 | #cd718d | #603140 |
| lemon | #d8e04a | #c0c745 | #ccd373 | #949938 | #434614 |
| apricot pick, moved | #ff9a62 | #ff9fa6 | #ffb3b8 | #d1717a | #623236 |
| marigold pick, moved | #ffc53a | #ccc23b | #d6cf6e | #9d9631 | #484411 |

**Why lilac by default.** The world is warm brown and cream; a warm accent
(peach, coral, gold) would sit between the two severity hues and blur them,
and green reads as "healthy". Lilac is the complement of the walnut: it
stands out on every card without meaning anything about the machine, it is
gentle rather than gamer-neon, and no part of the system uses its hue.

### The severity rule (holds for any accent)

Severity never relies on hue alone, so an accent near amber or red cannot
be mistaken for it:

1. **Area.** Warning turns the reading's **whole card** WARN_SURFACE and
   critical turns it **solid** CRIT. The alarm turns the **whole strip**
   CRIT. The accent is never allowed on more than a key or a thumb, so no
   accent shape is ever card-sized.
2. **Brightness.** A critical card is the largest bright area on the strip
   with white figures; a warning card's surface is lighter than every
   normal card. The accent's lightness is pinned (L 0.80), so it can never
   outshine CRIT's area or reach CRIT_INK white.
3. **What changes colour.** Readings (numerals, bars, their icons) change
   colour only for severity. The accent is never applied to a reading, so
   an amber numeral can only mean "warning".
4. **Page markers** for a page in trouble get longer and taller
   (`MARKER_W_ALERT`, `MARKER_H_ALERT`), not only coloured, and an edge
   flag stands at the strip's end on that page's side (Page markers).
   The current page's marker stays cream with its trouble as a separate
   segment after it, so "you are here" is never told by length alone.

Proof: `accent-marigold-severity.png` puts a marigold pick's play key and
slider beside a warning card and a critical card. The key is a 44 px key
with a glyph; the warning is a whole amber card with amber figures. With
the hue moved, the slider fill is olive, not amber: they now differ by
colour as well as by place.

## Typography

One face: **Rubik** (variable, wght 300–900, OFL; google/fonts
`ofl/rubik` v2.300, `assets/fonts/Rubik.ttf`). Its softened corners and
round bowls are the friendliness; its figures are sturdy and open (the
3, 5, 6, 9 do not close up at 32 px). Tabular figures (`tnum`) on for
every numeral, so a ticking value does not shift. Sentence case only; no
uppercase labels, no letter-spaced caps.

Every size is a constant in `theme.rs`. Cap height is 0.70 em; at about
128 px per inch one pixel is 0.198 mm.

| Constant | Size | Weight | Tracking | Cap height | Role |
|---|---|---|---|---|---|
| `NUM_HERO` | 50 | 600 | −0.5 | 35 px, 6.9 mm | Hero numerals with no bar below (clock, weather, slider), the alarm's reading. B's scale. |
| `NUM_MEDIUM` | 40 | 600 | −0.4 | 28 px, 5.5 mm | Medium numerals with no bar below (clock, weather), weather at compact. |
| `NUM_HERO_BANDED` | 40 | 600 | −0.4 | 28 px, 5.5 mm | Hero numerals with a bar band below (value, load, memory): a step under `NUM_HERO` so the bar can run the whole card. |
| `NUM_BANDED` | 36 | 600 | −0.4 | 25 px, 5.0 mm | Medium numerals with a bar band below (values, network at hero). |
| `NUM_COMPACT` | 32 | 600 | −0.3 | 22 px, 4.4 mm | Compact numerals; the medium slider's number. |
| `NUM_SECOND` | 26 | 600 | −0.2 | 18 px | A second reading beside the first (load), upload rate; a compact reading's numeral in a 106 px zone. |
| `NUM_SECOND_TIGHT` | 22 | 600 | −0.2 | 15.5 px | A medium card's second reading when 26 would not leave room for its icon beside the widest first. |
| `NUM_SECOND_SMALL` | 20 | 600 | −0.2 | 14 px | The step under that: a 200 px card with "212°" and "100%" and the icon. |
| `TITLE_BIG` | 26 | 560 | −0.2 | 18 px | The track title in the now-playing takeover. |
| `UNIT_HERO` | 22 | 560 | 0 | | "%" after a hero numeral. |
| `TITLE` | 20 | 560 | 0 | | Track title, weekday, weather condition, alarm headline, compact slider number. |
| `KEY_LABEL` | 18 | 560 | 0 | | Key labels; the short words on compact "set up" cards. |
| `UNIT` | 18 | 560 | 0 | 12.6 px, 2.5 mm | "%", "kB/s" after a numeral, on its baseline. |
| `BODY` | 18 | 480 | +0.1 | 12.6 px, 2.5 mm | Artist, date, second lines, hints, weather's range. |
| `LABEL` | 18 | 560 | +0.2 | 12.6 px, 2.5 mm | Reading names, times ("1:37 / 5:18"), "limit 167°", "Muted", the change "+15". |
| `TEXT_FLOOR` | 18 | | | | Nothing is smaller (asserted by `theme.rs`'s tests). |

**Secondary text is 18 px, provisionally.** The U3 review found 15–17 px
(2.1 mm caps) small for 60 cm, so every secondary style went to 18
(2.5 mm caps). That is the reviewer's ask, not a measurement: U0's
legibility card has not been read on the panel, and 18 may move. Where
18 does not fit at its old place, and what gives:

- A medium value card shows its name's first word ("CPU" of "CPU
  load", the icon says load) when the whole name does not fit beside a
  two-digit value. It is decided for two digits whatever the value, so
  the name does not change as the reading climbs into its warning.
- Medium weather closes its range up ("61°/49°"), and leaves it out
  before it would be cut.
- A hero media card shows only the elapsed time ("1:37") when the
  artist would be cut by "1:37 / 5:18", measured against the artist
  even while paused, so pausing does not change the times, and for the
  times' widest ("8:88 / 5:18"), so the seconds do not. The elapsed
  time goes too when it would leave the artist less than
  `MEDIA_ARTIST_MIN` 40 px.
- A seek slider's "9:26 / 43:12" drops "/ 43:12" where it does not fit
  (a medium card shows "9:26"), takes the next style down (`TITLE` to
  `BODY` at hero), then goes; the compact media card's elapsed time does
  the same from `TITLE`. Chosen for the track's widest elapsed time, so
  the form is fixed for the track, not changed at ten minutes. See
  Numbers are never cut.
- The empty music sleeve is 27 px at compact and medium (31 at hero),
  and the hero hint is "Play some music".
- A medium rate card whose down and up are in different units uses two
  rows (see Size classes).

The numeral is always the primary element; everything else backs it up.
"°" is part of the numeral; the unit letter (F or C) is not shown: it is
the owner's setting, and the hero card's "limit 167°" uses the same unit.
"%" and rates' units are a size smaller, on the same baseline.

Rubik has no TrueType hinting program (`fpgm`), unlike the shipped text
choices; it is used at 14 px and up only, where the unhinted outline is
clean in the frames. Whether it holds on the panel is a U0 check.

## Layout

### The strip

| Constant | Value | Meaning |
|---|---|---|
| `STRIP_W`, `STRIP_H` | 640, 48 | The panel. |
| `SAFE` | 2 | Rows and columns the bezel or angle may hide. Nothing essential in rows 0–1 and 46–47; card fills may run into them. |
| `EDGE` | 2 | Space at the strip's ends. |
| `GUTTER` | 4 | Space between cards (ground shows through). |
| `CARD_Y`, `CARD_H` | 6, 41 | Cards span rows 6–46. Rows 2–4 hold the page markers. |
| `PAD` | 9 | Inner padding at a card's sides. |
| `ICON_GAP` | 6 | Between an icon and its numeral. |

Baselines (strip rows): `BASE_HERO` 44 (50 px numeral, ink rows 9–44),
`BASE_MEDIUM` 41 (40 px, ink 13–41), `BASE_COMPACT` 36 (32 and 36 px
numerals above a bar band, clear of the limit notches), `BASE_LINE1` 24 and `BASE_LINE2` 42 for two
lines of text (descenders end at row 45).

A layout's widths are **zone** widths, and add up to 640 or less: each
zone holds its card inset `CARD_INSET` 2 px on both sides, which makes
the `GUTTER` between cards and the `EDGE` at the ends. A 128 px zone
draws a 124 px card. The widths below are cards unless they say zones.

### Size classes

By the widget's width: **compact** up to `COMPACT_MAX` 149 px, **medium**
150 to `MEDIUM_MAX` 299, **hero** 300 and up. Two widgets add detail inside
a class: a key shows its label from `KEY_LABEL_MIN` 96 px, and media shows
prev and next beside play/pause from `MEDIA_KEYS_MIN` 400 px. More width
always shows more or shows it bigger, and a bar is never shorter in a
wider card (a hero reading trades `NUM_HERO` for `NUM_HERO_BANDED` so
its bar can run the card: 290 px at 312, against 178 at 200); nothing is drawn narrow with empty
card beside it (content centres in its room when it has spare width).

| Widget | Compact (≤149) | Medium (150–299) | Hero (≥300) |
|---|---|---|---|
| Clock | 10:53 at 32, centred | 10:53 at 40 + "Sun" / "Nov 22" | 10:53 at 50 + "Sunday" / "November 22" |
| Value (temperature) | icon + 115°, at the largest step that holds the widest value ("212°") beside the icon: 32 with icon 22 from a 125 px zone, 28 / 22, 28 / 17, 26 / 17 (a 106 px zone: six widgets on a page), 22 / 17; narrower, the name ("CPU") above the numeral at 20; bar band below | icon + 115° at 36, name top right; bar band | icon 30 + 115° at 40; the name over the limit the reading is nearest ("limit 167°", the warning one; the critical one once past it); the bar band runs the **whole card** under them |
| Value + second reading | as compact (second hidden) | Laid out for the widest both readings can be (three digits each, "212°" / "100°" and "100%", from the units alone, before the first reading), never for the values now: nothing goes or moves as they change. Icon + 115°, the second's own 17 px icon (a gauge for load) just before "6%" at the right; two bars in the band, split where the widest second's icon starts. Sizes step down until that fits: 36 / 26, 36 / 22, 32 / 22, 32 / 20 (a 204 px zone, the System page), 28 / 20 (a 200 px zone), the first's icon 22 from the third step | icon + 115° at 40, name; the second named on the numeral's baseline at the right ("load 6%" at 26); the first reading's bar runs the whole card, the second has none (a percent is its own scale) |
| Load, memory | icon + 62%, sized for "100%" as the temperature is (32 / 22 at 128, 22 / 17 at 108); bar | icon + 62% at 36, name; bar | icon + 62% at 40, name over "limit 85%"; bar the whole card |
| Network rate | ↓ + 12 kB/s, sized for the widest rate on its scale ("888 MB/s"), at the largest step that fits on one line (20 / 17 at 124 px); narrower, ↓ and "MB/s" above the number at 20 (the 106 px zone); log bar | ↓12 kB/s at 32 + ↑3 at 26; two log bars. Down and up in different units: two rows, ↓48 MB/s and ↑310 kB/s at 20, each with its log bar beside it | ↓12 kB/s at 36 + ↑3 kB/s at 26; two log bars |
| Weather | icon + 58°, sized for "888°": 40 / 26, then the value card's steps (32 / 22 at 124 px, 28 / 22 at 107), half a pad at the sides; narrower, the icon above a 20 px number | icon + 58° at 40 + "Cloudy" / "61° / 49°" | icon 30 + 58° at 50 + condition / high and low |
| Media | play/pause key + elapsed + progress line | artwork + title / artist + a raised `MEDIA_KEY_W` 36 × 33 play/pause key at the right, `PAD` in from the card's edge, accent-filled (the whole card is the hit zone; the key says so) | artwork + title / artist + times + play/pause key; from 400 px prev and next too |
| Slider (volume) | icon + "40%" at 20; full-width track and thumb | icon + 40% at 32 + track and thumb | icon + 40% at 50 + track with 0–100 ticks |
| Key | icon (label from 96 px) | icon + label | icon + label + second line ("Hold for a new window") |
| News | the whole card a key: newspaper `ICON_KEY` 22 in the accent + the headline's age ("12 min") at `TITLE` 20, centred; no headline, which would be two words at this width | the whole card a key: the headline alone, over two lines at `LABEL` 18 in TEXT (baselines `NEWS_BASE1` 23.5 and `NEWS_BASE2` 41.5), as many whole words as fit on the first, the second cut by `fit`; one line with nothing under it is centred (`NEWS_BASE_ONE` 33). About 30 characters at 200 px | the same two lines; after the headline's end on line two, `NEWS_FROM_GAP` 12 px on, the newspaper at `ICON_SMALL` 17 in the accent + "Harbour Gazette · 12 min" in TEXT2, when it fits there; else left out. About 60 characters at 320 px, 80 at 440, 100 at 512 |

Frames: `specimen-*.png` draws every widget in zones of 128, 200 and
312 px (cards of 124, 196 and 308) in each state.

**A value card always says what it reads** (provisional, 2026-11-22).
The owner's layout put six widgets on a page; two compact temperatures
in 106 and 107 px zones showed "111°" and "120°" with no icon, and the
CPU could not be told from the GPU. A compact value card now keeps its
icon and steps its numeral and icon down together (`COMPACT_STEPS` in
`widget.rs`: 32 / 22, 28 / 22, 28 / 17, 26 / 17, 22 / 17, 20 / 17, the
numeral giving way first), decided by the reading's widest value for its
unit ("212°" in °F, "100°" in °C, "100%"), never the value now. Where
even 20 / 17 does not fit (a zone under about 96 px) the name goes above
a 20 px numeral in place of the icon: the widget's label, else the
signal's short name ("CPU", "GPU load", "Memory"); where no number fits
(about 70 px), the icon alone over the bar. The numeral sits at the left
of its widest box, so "45°" and "212°" start at the same column. Frames:
`live-p1-compact-temps` (the owner's page), `narrow-values` (one
temperature at 125, 112, 106, 96, 88, 72 and 41 px).

Compact rate and weather cards follow the same rule (provisional,
2026-11-22). A rate is laid out for "888" in its longest unit ("MB/s",
up to the scale's 1 GB/s); where that does not fit on one line even at
20 / 17, the arrow and the unit go above the number. The unit is never
shortened: "12 kB" (no "/s") would read as an amount, not a rate, so no
shorter form is unambiguous. Weather is laid out for "888°" (a hot
day in °F; "-88°" is narrower). Frames: `live-p1-rate` ("512 kB/s" in
the owner's 106 px zone), `overflow-sweep-rate`, `-load`, `-memory` and
`-weather` (each at its worst value in zones of 124, 112, 106, 102, 100
and 96 px).

### The default layout

Five pages; controls get whole pages; no page is another page stretched.

| Page | Widgets (zone widths) | Why |
|---|---|---|
| 1 Home | clock 196, weather 124, CPU 124, media 196 | The glance: time, weather, the hottest part, and music under a finger. |
| 2 Music | media 428 (three transport keys), volume slider 212 | Control: play, skip, set the volume. |
| 3 System | CPU + load 204, GPU + load 204, memory 108, network 124 | The machine, every reading with its fixed bar. Memory's 108 px zone holds "100%" beside its icon only at 22 / 17 (open question 17). |
| 4 Launch | clock 128, keys Browser, Files, Settings, Music (128 each) | Control: launchers, with the time kept. Settings opens the settings window (no terminal by default). Music shows the Music page (a long press plays or pauses): a games launcher is not on every desktop, and a key that opens nothing is worse than none. |
| 5 Desk | clock 332, weather 308 | Across the room: the biggest figures, full date, the day's high and low. |

Frames: `default-p1-home.png` … `default-p5-desk.png`, and
`default-night-desk.png`.

## Elevation & Depth

Flat and tonal. Depth has three steps, all by lightness: ground (L 0.18),
card (0.24), key (0.37) with a 1 px lighter top edge (`KEY_LIFT`), the
only "raised" signal. No shadows except the volume takeover, which sits on
a 3 px rim of ground colour so it reads as on top of the page. No glows,
except the soft accent halo around a held slider thumb.

## Shapes

- Cards: `CARD_R` 10 px on all four corners, floating with ground around
  them (not flush to an edge like C's keycaps).
- Keys: `KEY_R` 9 px, `KEY_H` 33 px, centred in the card (`KEY_Y`).
- Bars, slider tracks, progress lines, page markers: pills (radius half
  the height).
- Thumbs: discs. Artwork: 7–8 px rounded squares.
- "Not set up": the card's outline only, dashed (`DASH` 5 on, 4 off,
  `DASH_W` 1.5 px) in the accent. Nothing else on the strip is dashed
  except the empty music sleeve, which is a quiet TEXT3 dash.

## Components

### The bar (fixed scale)

A pill track `BAR_H_BANDED` 5 px at `BAR_Y_COMPACT` row 40 in every
size class, its warning zone tinted WARN_ZONE and its
critical zone CRIT_ZONE, with 2 px notches (`NOTCH_W`) at each limit
sticking out `NOTCH_OUT` 2 px above and below. The fill grows from the
left in FILL, or WARN on a warning card, or white on a critical card. The
bar always has its own band and **never crosses its number**. Scales are
fixed; the same length means the same value every minute:

| Reading | Scale | Limits drawn |
|---|---|---|
| Temperature (CPU, GPU, any °) | 30–100 °C (86–212 °F), linear, `TEMP_LO`/`TEMP_HI` | The reading's `warn` and `crit` (CPU 75/90 °C = 167/194 °F; GPU crit 90 °C, warn assumed 80 °C = 176 °F) |
| Load | 0–100 %, linear | None by default (a GPU at 97 % in a game is normal); drawn if the user sets them |
| Memory | 0–100 %, linear | Defaults warn 85 %, crit 95 % (`MEM_WARN`, `MEM_CRIT`) |
| Network | 1 kB/s – 1 GB/s, logarithmic (`NET_LO`, `NET_HI`) | None |
| Volume | 0–100 %, linear; a slider, not a bar (thumb, accent fill) | None |
| Media progress | 0 – track length; a 3 px line, no thumb (information) | None |

There are no history graphs on the strip in this system: at 48 px tall a
graph either crosses the number or is too faint to read from the seat
(U1's own finding). See Open questions.

### Keys and touch affordances

What can be touched looks like it; what cannot does not.

| Looks like | Means | Where |
|---|---|---|
| A raised face (KEY, lit top edge) with an accent glyph | Tap | Transport keys, launch keys |
| A face filled with the accent, dark glyph | The main tap here | Play/pause (the alarm's OK is white on red instead) |
| A cream disc with an accent ring on a pill track | Drag sideways | Sliders |
| A whole card raised (KEY, lit top edge) with an accent glyph | Tap | Launch keys; a media card with nothing playing that can open the player; a news card with a headline (the newspaper is the accent glyph where the source fits; otherwise the raised card alone says it; the headline itself stays cream) |
| A dashed accent outline with accent words | Tap to set this up (opens the settings window there) | Any widget not set up |
| Flat card, no accent | Inert: swipe passes through | Readings, clock, weather |

- **Minimum target**: `TARGET_MIN` 44 px wide (8.7 mm), the full card
  height. Transport keys are 44 × 33; in the now-playing takeover 50 × 33.
- **Pressed** (`MS_PRESS` 60 ms in): the face drops 1 px (glyph and label
  move down 1 px), turns `Accent::soft` (the primary key: accent mixed 25 %
  toward ink) with a 1.5 px accent rim. Shown in `specimen-key-pressed.png`,
  `specimen-media-pressed.png`, `takeover-alarm-ok-pressed.png`.
- **Holding**: a long press fires at 0.5 s (`MS_HOLD`) while still held.
  On the way, the key's face fills from the left with KEY mixed
  `HOLD_MIX` 45 % toward the accent, linearly in time; when full it fires
  and the key flashes `base` for one frame-pair. `specimen-key-holding.png`
  shows 60 %.
- **Sliders are relative**: landing changes nothing; dragging moves the
  value by the finger's travel (full track width = 0–100). At rest the
  thumb is `THUMB_R` 7.5 px; while held it grows to `THUMB_R_HELD` 10 px,
  fills with the accent and gets a soft `THUMB_HALO` glow; the number
  stays cream (a value is information). `specimen-slider-dragging.png`.

### States

| State | How it looks | Frame |
|---|---|---|
| Normal | CARD, cream numeral, FILL bar | `specimen-value-normal` |
| Warning | Whole card WARN_SURFACE; the warning reading's numeral, icon and bar WARN, a second reading that is fine stays cream (so an amber numeral always means warning); page marker WARN and longer | `specimen-value-warning`, `specimen-value2-warning`, `3-hot` |
| Critical | Whole card CRIT; everything white; track CRIT_DEEP, and limit notches under the white fill CRIT_DEEP so they stay visible; a hero card names the critical limit, as the alarm does; page marker CRIT and longer; plus the alarm | `specimen-value-critical`, `takeover-after-ok-system` |
| No data (source works, no reading yet) | Normal card; the numeral is two TEXT3 dashes; "No reading" (medium) / "No reading yet" (hero); empty bar | `specimen-value-no-data`, `specimen-weather-no-data` |
| Source lost / stale | The last value kept in TEXT3; the reading's own icon kept (at compact and medium it is the card's name) in TEXT3 with a broken-link badge on its lower right, cut out by a ring of the card (weather: the icon becomes a cloud-slash); "Lost at 10:41" / "From 9:30" where there is room (medium alone, hero); the broken link always. A second reading on a lost card is drawn lost too (TEXT3, its own badge, an empty bar), so a lost card never carries a bright number. Dimmed is never mistaken for lost (Idle and night) | `specimen-value-lost`, `specimen-weather-stale`, `default-night-system-lost`, `default-day-system-lost` |
| Not set up | No fill; dashed accent outline; plus or pin icon; "Weather / Set up" (compact), "Weather / Add your town" | `specimen-*-not-set-up`, `6-first-run` |
| Empty page | A page with no widgets: no card, outline, icon or accent; "Empty page" (`LABEL`) / "Add widgets in Settings" (`BODY`), both TEXT2, centred on the strip at `BASE_LINE1` and `BASE_LINE2` − 1. The page markers as on any page. Quiet, so it is taken for neither a fault nor a control (before, the page drew nothing and looked broken) | `live-p6-empty-page` |
| Pressed, holding, dragging | See Keys | |
| Paused | Play glyph in the key, artwork dimmed, title in TEXT2, "Paused" in place of the artist, progress line TEXT3 | `specimen-media-paused` |
| Nothing playing | When the layout says how to open the music player (the media widget's own `tap`, or a Music key, by the `music-notes` icon or the label "Music", whose tap runs or opens something; `Config::music_action`): the whole card is a key, the note in the accent, "Nothing playing" / "Tap to open music" ("Open music" where that is cut; "Open" / "music" at compact), and a tap does that. A Music key that only shows a page opens no player, so with the default layout nothing resolves and the card stays quiet: a dashed TEXT3 sleeve with a note, "Nothing playing" / "Play some music", no key look | `specimen-media-nothing-playing`, `specimen-media-nothing-playing-opens`, `-opens-pressed` |
| Muted | Speaker with a cross; "Muted" in place of the number (compact, medium) or the number in TEXT3 (hero); fill and ring grey | `specimen-slider-muted` |
| News: a headline | The whole card a key (see the size classes); tap opens the article, hold shows the next headline, with the usual press and hold fill | `specimen-news-live`, `-pressed`, `-holding`, `-short`, `news-page` |
| News: no headline yet | The feeds work (or are being read for the first time) and nothing has come: a flat card, the newspaper in outline in TEXT3, "No news yet" in TEXT2 (`BODY` at medium, `TITLE` at hero, "No news" / "yet" at compact). Not a key: there is nothing to open | `specimen-news-no-data` |
| News: lost | Every feed failing and the last headline older than `max_age`: that headline kept, flat card, everything TEXT3. Line one the headline's start, cut by `fit`; line two always the newspaper with the broken-link badge and "Harbour Gazette · 2 d", the age still growing, so the broken link shows at every width. Not a key. Never an error message: what failed goes to the log | `specimen-news-lost` |
| News: not set up | No feeds: the dashed outline, plus icon, "News / Add a feed in Settings" ("Add a feed" where that is cut; "News / Set up" at compact) | `specimen-news-not-set-up` |

"No data", "not set up" and "source lost" never look alike: dashes on a
normal card, a dashed outline with no card, and a grey number with a
broken-link icon.

### Long titles

A title that does not fit is **never cut by accident**, and no text
anywhere ends in a letter cut through. At rest it ends in an ellipsis,
as every other text that does not fit (`fit`): after a whole word when
that keeps at least 60 % of what a cut between letters would ("Weird…"
in a medium media card, "Weird Fishes…" in a hero one), else between
letters ("Radiohe…"), separators ("/", "–", "·") dropped before it.
Only while it moves does it fade into the card at the edge it slides
under (`text_fade`, 18 px). It scrolls as a marquee: after `MS_MARQUEE_REST` 2.5 s it scrolls left at
`MARQUEE_PX_PER_S` 28 px/s until its end shows, rests 2.5 s, fades back
to the start, and does this **once per track change and once per tap**,
then stays at rest (so it never moves in the corner of the eye for
long). The full title is always shown in the now-playing takeover
(`TITLE_BIG` 26 px; 22 characters fit with room to spare; a longer one
uses the same marquee there).

A **news headline never moves**: it wraps onto a second line, which
ends in an ellipsis by the same `fit` rule, and the next one replaces it
after `rotate`. A marquee runs once per change; with a new headline
every 12 s it would never rest. Tap the card to read the article.

**Two lines, not one (provisional, 2026-11-22).** Variant A put the
headline on one line at `TITLE` 20 and "Source · age" under it; even a
512 px card showed about 45 characters, and headlines run 60 to 90.
Variant B, built, sets the headline over both lines at `LABEL` 18 and
gives the source the room left after it, on hero cards only. Compared
at 200, 320, 440 and 512 px with headlines of 40 to 110 characters, B
shows about twice as much (a whole 60-character headline from 320 px,
up to 90 at 512) at the strip's smallest legible size, and still reads
as one card: one colour, one size, no motion. What it costs: the source
and the accent newspaper go when the headline needs the room, so a
medium card, and a hero card with a long headline, say "tap me" by the
raised card alone. To be confirmed on the panel.

### Numbers are never cut

A number or a time never ends in an ellipsis: "43…" or "19…" is a
different number. `fit` never cuts through a word with a figure in it
(it cuts before that word), and every place a number can meet the edge
shortens it in whole forms instead (`first_fit`, laid out for the
widest form so it does not flip as the value ticks):

| Where | Shortened as |
|---|---|
| A value card's numeral | Never shortened: sized for its widest (above) |
| Seek slider, compact media, hero media times | "9:26 / 43:12", then "9:26", then a style smaller, then none |
| Weather's high and low | "61° / 49°", "61°/49°", then none |
| "From 9:30" (stale weather, hero) | Whole or none; the cloud-slash still says it |
| "Lost at 10:41" (hero reading) | "Lost 10:41", then "Lost" |
| A headline's age ("12 min") | Whole or none (compact card, source line) |

Frames: `live-p5-seek`, `live-p5-seek-hours`, `specimen-slider-seek`,
`specimen-slider-seek-hours`, `specimen-media-hours`.

### Page markers

Small pills along the top edge, rows 2–3 (`MARKER_Y`, `MARKER_H` 2: at
3 they crowded the margin over the cards, open question 11),
centred, `MARKER_GAP` 6 apart: one per page, `MARKER_W` 14 px, the
current one `MARKER_W_CURRENT` 26 px and TEXT; the others TEXT3 at 55 %.
On the current page, trouble does not recolour the cream: it is a
separate WARN or CRIT segment `MARKER_SPLIT` 2 px after it, filling the
alert's extra length.
A page holding a warning reading has a WARN marker and one holding a
critical reading a CRIT marker, each `MARKER_W_ALERT` 16 px longer and
`MARKER_H_ALERT` 3 px tall (rows 2–4, row 5 still clear), never dimmed.
Takeovers leave the markers visible; the alarm covers them.

**Trouble on another page** (the U3 review: a 2 px marker alone was too
easy to miss). Critical already raises the alarm, and a warning may not
take the strip over (Principle 2), so the signal stays quiet but gains
shape: besides the bigger marker, an **edge flag** stands at the end of
the strip on the side of the page in trouble, a pill `EDGE_FLAG_W` 4 ×
`EDGE_FLAG_H` 28 px from row `EDGE_FLAG_Y` 13, WARN for a warning, CRIT
for a critical reading after its alarm was acknowledged. It covers
columns 0–3 (or 636–639): the strip's edge and the end card's outer
padding, where no widget draws (content starts `PAD` 9 px in). No other
state uses that shape, so it does not rely on hue, and it says which way
to swipe. Frames: `3-hot` (Home, to the left, is warning too),
`takeover-after-ok-home`, `severity`.

*Not done, and why:* a chip naming the reading ("CPU 172°") does not fit.
The marker margin is rows 2–5 (rows 0–1 are `SAFE`), four rows against
13 px for 18 px text, and a chip anywhere lower covers a card's
content.

### Takeovers

- **Now playing** (`takeover-now-playing.png`): when the track changes,
  the whole card band (rows 6–46) becomes B's media scene: artwork 35 px,
  the artist, a progress line, "1:37 / 5:18", the full title at 26 px,
  and prev / play-pause / next keys 50 px wide. The artist and times are
  TEXT2 and the progress line cream (information, not touch). Markers
  stay.
  Same timing as today's takeovers (`seconds`, kept up while touched).
- **Volume** (`takeover-volume-right.png`, `takeover-volume-left-held.png`):
  a card over **part** of the page, aligned `right` (default) or `left`,
  with a set `width` snapped to the card edges of the page under it (the
  nearest edge within 40 % of the width, `TAKEOVER_SNAP` in `engine.rs`,
  but never narrower than `TAKEOVER_MIN` 320 px or the width given, so
  its slider keeps a track of about 125 px or more;
  `takeover-volume-snapped-system` shows 320 snapping out to 436 over
  System, since 232 would leave a 45 px slider),
  so the clock and weather stay where the eye left them and no card is
  cut in two. Speaker, the new level at 40 px, the change in TEXT2
  ("+15"), and a slider whose part from the old level (a cream
  ghost mark at 40) to the new is bright and the rest calm. It has an
  accent rim, because it can be dragged. Where the page already shows the
  slider, there is no takeover; the slider moves.
- **Critical alarm** (`4-critical.png`, `takeover-alarm.png`): when a
  reading with a `crit` rises past it, the **whole strip** turns CRIT:
  warning icon, "GPU is too hot" with the reading's icon, "Over its 194°
  limit · check the fans", the reading at 50 px, and one white **OK** key
  (84 × 36). It stays until the user taps OK or swipes it away, or the
  reading has been back under its limit for 10 s. The OK key's touch
  zone runs from x 546 to the strip's right end (`alarm_ok_span`), wider
  than the key drawn, so a hurried tap at the edge still lands. After OK the page
  returns; that reading's card stays in its critical style and its page's
  marker stays red (`takeover-after-ok-system.png`). The alarm re-arms only
  after the reading has stayed below the limit for 30 s. While up, it
  breathes: its red eases between 100 % and 88 % brightness over
  `MS_ALARM_BREATHE` 2.4 s, the only looping motion in the system.

### Motion

Calm and quick, never bouncy. All eases are ease-out (decelerating, like
something settling), except where noted.

| Event | Duration | Feel |
|---|---|---|
| Key press in / out | `MS_PRESS` 60 ms in, 120 ms out | The face sinks at once and lifts gently |
| Hold fill | `MS_HOLD` 500 ms, linear | Steady, so the user can time it |
| Value change (numeral, bar) | `MS_VALUE` 400 ms | Bars glide to the new length; numerals change at once (no rolling digits) |
| Page swipe | follows the finger; release settles in `MS_PAGE` 280 ms | Existing gesture physics; ease-out |
| Takeover in | `MS_TAKEOVER_IN` 220 ms | Fades and rises 3 px into place |
| Takeover out | `MS_TAKEOVER_OUT` 320 ms | Fades; slower out than in |
| Alarm in | immediate cut | Never animated in: it must not be missed |
| Alarm breathing | `MS_ALARM_BREATHE` 2.4 s, sine | Slow, like breathing, not flashing |
| Thumb grow on hold | `MS_PRESS` | |
| Marquee | see Long titles | |
| New headline | immediate | Replaced at once, no fade or slide: every `rotate` (12 s) is too often for motion in the corner of the eye. Never a takeover |

### Idle and night

- **Idle**: after `IDLE_DIM_AFTER_S` 300 s with no touch and nothing at
  warning, the whole frame dims to `IDLE_DIM` 60 %. A touch, a takeover or
  a warning brings it back at once.
- **Night**: between the user's night hours (setting), the frame dims to
  `NIGHT_DIM` 35 % (`default-night-desk.png`); nothing is redrawn smaller.
  Warning and critical cards are not dimmed and the alarm is always full
  brightness.
- **How it dims** (`theme::dimmed`): not evenly. The ground and cards keep
  the dim level; the lightest pixels lose only `DIM_FIGURES` (half) of
  what they do, eased by lightness so the order of tones never changes.
  At night a cream numeral keeps about 68 % and a lost reading's TEXT3
  about 59 %, so live figures stay brighter than a lost reading is even
  by day, and at twice its contrast in the same frame. Before the U3
  polish the whole frame scaled to 35 % and a live number at night
  looked lost. Lost keeps its own marks besides contrast: the broken
  link, and its time where there is room. Frames:
  `default-night-system-lost` beside `default-day-system-lost`.
- When nothing matters (idle, nothing playing), the strip shows the user's
  first page as it is; it never invents a screensaver.

### Icons

**Phosphor** 2.1.2 (MIT, `@phosphor-icons/web`), two weights subset into
`assets/fonts/Phosphor-Fill.ttf` and `Phosphor-Bold.ttf` (67 glyphs,
`pyftsubset`; licence in `assets/fonts/licenses/Phosphor-LICENSE.txt`).
Phosphor's rounded joins and terminals match Rubik; Font Awesome (still
bundled for the shipped widgets) is blockier and is not used here.

| Glyph | Code point | Weight | For |
|---|---|---|---|
| cpu | U+E610 | Fill | CPU temperature |
| graphics-card | U+E612 | Fill | GPU temperature |
| memory | U+E9C4 | Fill | Memory |
| gauge | U+E628 | Fill | Load (as its own widget) |
| arrow-down / arrow-up | U+E03E / U+E08E | Bold | Network down / up |
| cloud (and cloud-sun, cloud-rain, cloud-moon, sun, moon) | U+E1AA, E540, E1B4, E53E, E472, E330 | Fill | Weather condition |
| cloud-slash | U+E1B6 | Fill | Weather with no data or stale |
| speaker-high / speaker-low / speaker-x | U+E44A / E44C / E45C | Fill | Volume; muted |
| play / pause / skip-back / skip-forward | U+E3D0 / E39E / E5A4 / E5A6 | Fill | Transport |
| music-note | U+E33C | Bold | Nothing playing |
| map-pin-plus | U+E314 | Fill | Weather not set up |
| plus | U+E3D4 | Fill | Anything else not set up |
| link-break | U+E2E4 | Fill | Source lost |
| newspaper | U+E344 | Bold | News (Bold: the card is a key) |
| warning | U+E4E0 | Fill | The alarm |
| globe-simple, folder-simple, gear, game-controller | U+E28E, E25A, E270, E26E | Bold | Default launch keys |

The subset also carries icons for user keys (house, gear, camera,
envelope, chat, bell, code, calculator, rocket, browser, wifi, lightning,
fan, drop, thermometer, clock, calendar, headphones, power, image and
others). Fill is for things, Bold for actions on keys and for empty
states. Icon sizes: `ICON_HERO` 30, `ICON_MEDIUM` 26, `ICON_COMPACT` 22,
`ICON_KEY` 22, `ICON_SMALL` 17.

### Artwork

Album artwork when the player gives it (`mpris:artUrl`, where the
layout's `[media] album_art` allows it; docs/LAYOUT.md); otherwise a
generated placeholder: a warm two-colour sky, a sun and two soft hills,
seeded by the title (`draw::art`). The placeholder also shows while the
artwork is on its way and whenever it cannot be read; the change to the
artwork is instant (nothing on the strip fades in place).

Artwork is prepared once per track (`nexus_core::art`), off the render
thread, at exactly the sizes drawn: `ART` 33 px on medium and hero
cards, `ART_SCENE` 35 px in the media scene and the now-playing
takeover. The middle square of the image (a wide or tall one is cropped
to it), scaled by area averaging in linear light, so fine detail
becomes its true average tone and never moiré or a darkened edge. The
corners are rounded as it is drawn, as the placeholder's: `ART_R` 7 px,
8 in the scene. Paused artwork, real or generated, is dimmed 55 %
toward the card. A compact card shows none.

Never real artwork in mockups or frames: the frames with artwork
(`specimen-media-art-playing`, `-paused`, `takeover-now-playing-art`,
`-paused`) draw `art::test_card`, a two-colour geometric card with a
checker, a diagonal, a ring and a border that make a scaling fault show.

## Layout format additions

All optional; existing layouts keep working unchanged.

| Key | Where | Value | Meaning |
|---|---|---|---|
| `accent` | `[settings]` | `"#b6a2ff"` or a preset name (`lilac`, `sky`, `teal`, `mint`, `rose`, `lemon`) | The user's accent; default `lilac`; hues in amber and red's band move out of it |
| `night` | `[settings]` | `{ from = "22:30", to = "07:00" }` | Night dimming hours; none by default |
| `idle_dim` | `[settings]` | `true` / `false` | Idle dimming; default `true` |
| `second` | `value` widget | a signal name, e.g. `"cpu.load"` | A second reading beside the first, from medium up |
| `second_label` | `value` widget | text | The second reading's name where it is named: the alarm says "GPU load". Default: the signal's last word. Hero cards show only its number, as the frames draw it |
| `scale` | `value` widget | `[min, max]` in base units | Override the fixed scale (defaults per signal kind above) |
| `icon` | any widget | a Phosphor name, e.g. `"fan"` | Override the default icon (which comes from the signal) |
| `align` | `[[takeover]]` | `"full"` (default), `"left"`, `"right"` | A takeover narrower than the strip covers only that side |
| `width` | `[[takeover]]` | px | Its width when `align` is `left` or `right`; snapped to the nearest card edge of the page under it when that is within 40 % of it |
| `type = "key"` | widget | with `icon`, optional `label`, `sub`, `tap`, `hold` | The new key widget: runs `tap` (any existing action: `run`, `open`, `page`, `media`, `publish`); `hold` fires at 0.5 s |
| `critical` | `[[takeover]]`-like rule, built in | – | The alarm needs no key: any widget with `crit` arms it |
| `[news]` | top level | `feeds`, `rotate`, `max_age` | The feeds to read; see docs/LAYOUT.md |
| `album_art` | `[media]` | `"all"` (default), `"local"`, `"off"` | Where the media widget's artwork may come from; `"all"` fetches covers from the player's image host. See docs/LAYOUT.md |
| `type = "news"` | widget | optional `tap`, `hold` | One headline at a time; tap opens it, hold shows the next |
| `news = "open"` / `"next"` | action | – | Open the headline showing; show the next one |

`graph = "area" | "line"` on existing `value` widgets is accepted and drawn
as the fixed-scale bar (see Open questions). `warn` and `crit` stay in the
signal's base unit (°C).

## Do's and Don'ts

- Do put the accent only on things a finger can act on. Don't use it for
  a reading, a page marker, a heading or decoration.
- Do change area and brightness for severity. Don't signal trouble with
  colour on a number alone, or with a dot.
- Do keep every bar on a fixed scale, in its own band. Don't autoscale,
  and don't let a bar or graph touch its numeral.
- Do write plain words in sentence case ("Add your town"). Don't show
  signal identifiers, shell commands or uppercase labels on the strip.
- Do keep every size in `theme.rs`. Don't put a number in a widget's code
  that a measurement on the panel might change.
- Don't draw anything essential in rows 0–1 or 46–47.
- Don't add a second text face, monospace, glows on black, or a second
  accent.

## Open questions

1. **Every size**, until U0's legibility card is read on the panel: is 15
   px LABEL readable at 60 cm, are KEY and TRACK (1.3–1.4:1 on CARD)
   visible, does WARN_SURFACE (L 0.35) read as "lit" on the LCD, does the
   bezel hide rows 46–47.
2. **History graphs**: dropped from the strip. If wanted back, the only
   honest place is a hero card at 480 px or more, in its own band above
   the bar, on the same fixed scale. Matt to decide.
3. **GPU warning limit**: assumed 80 °C (176 °F); the brief gives only the
   critical limit.
4. **Accent from album art** (PRODUCT.md, undecided): the derivation above
   would take a hue from the art just as from a setting; whether to offer
   it is open.
5. **Rubik's lack of hinting** on the real panel; and whether Rubik should
   also replace the five font choices or sit beside them.
6. **The Desk page** shows the same two things as Home (clock, weather),
   bigger and with more detail. It passes "not stretched", narrowly.
7. Whether the alarm should also sound or notify the desktop (not drawn).
8. Weather conditions beyond "cloudy" are mapped to Phosphor icons but not
   drawn in frames.
9. **Accents near amber.** The isolated review (25/36) read a marigold
   slider beside a warning card as "a second warning". Settled in U3: a
   hue in the band 15°–105° moves to its nearer end (see Colour), and the
   presets are all outside it. The settings window should still say so
   when a pick is moved.
10. **Secondary text size.** The same review found 15–17 px labels (2.1 mm
    caps), including "limit 167°", small for 60 cm. Raised to 18 in the
    U3 polish (Typography lists what it costs); provisional until U0's
    card is read on the panel.
11. **Page markers** sat one row above the cards (rows 2–4, cards from 6)
    and read as crowded into the top margin. Settled in U3 by making them
    2 px tall (rows 2–3), leaving two clear rows; the cards stay put, so
    the hero numeral keeps its room.
12. **The hero reading's scale.** A hero reading's numeral is 40 px, not
    50, so its bar can run the card (the U3 review: a hero bar was
    shorter than a medium one). Clock, weather and the alarm keep 50.
    Provisional, Matt's to overturn; the other way is 50 px with no bar
    at hero.
13. **Edge flags** sit partly in columns 0–1, which the bezel may hide
    (`SAFE`); 2 px of each is on the visible part even so. A U0 check.
14. *Closed (U3 polish 2, provisional, Matt to overturn):* a card's
    layout never changes with its value. The medium value card with a
    second reading is laid out for both at their widest and its sizes
    step down (to 28 / 20 px at 200 px) so the second's icon always fits.
15. **Opening the music player on the default layout.** The default
    Music key shows the Music page, which opens no player, so the idle
    media card stays quiet out of the box. Linux has no generic "open the
    music player" command; the daemon could resolve one (the desktop file
    `xdg-mime query default audio/mpeg` names, launched with
    `gtk-launch`), or the default layout could name a player. Matt to
    choose.
16. **The System page does not name CPU and GPU.** (A compact card now
    always names its reading, by icon or by name; this question is about
    the medium card with a second reading.) At medium a value
    card with a second reading has no room for its name, so the two
    cards differ by icon alone (the U3 polish 2 review). A user can
    already name it (`label`, shown at hero) or give it a hero card;
    whether medium cards name themselves, and at what cost to the
    numerals, is Matt's to decide.
17. **What "widest" costs on the owner's and the default layouts**
    (provisional, 2026-11-22, Matt to overturn). Laying compact cards
    out for "212°" and "100%" means: memory on the default System page
    (108 px) draws "54%" at 22 px with a 17 px icon, where it was 32 /
    22 until it reached 100% (and then lost its icon); a compact media
    card in a 107 px zone has no room for "9:26" and shows only its
    progress line (before, the time ran into the card's padding, and
    past an hour off the card). Rate and weather, laid out the same
    way: network on the default System page (124 px) is "12 kB/s" at
    20 px, where it was 32 and ran off the card at "999 MB/s"; the
    owner's 106 px rate stacks its unit over a 20 px number; weather on
    Home (124 px) is 32 px, not 40, and in the owner's 107 px zone 28.
    Ways out: wider zones for those widgets; or "kB" without "/s",
    if Matt reads it as a rate.

## Shared identity

What the desktop settings window (Slint, `crates/nexus-ui`) must take from
this system so the two surfaces belong together:

- **Accent handling**: the same setting, the same OKLCH derivation
  (`Accent::from`), the same rule: accent means "you can act on this"
  (primary buttons, toggles, sliders, focus rings, the selected item), never
  data, never warnings. The window's colour picker previews the four roles.
- **Neutrals**: the same warm browns and creams. In dark mode GROUND,
  CARD, KEY and TEXT map directly to window background, surface, raised
  control and text. A light mode inverts the lightness ladder at the same
  hue (about 60) and chroma: paper cream ground, walnut text, never
  blue-grey.
- **Severity**: the same WARN and CRIT, and the same rule (area and
  brightness, plus words, never hue alone).
- **Typeface**: Rubik, with the same weights (600 figures and titles,
  480–560 words), sentence case, tabular figures for any number.
- **Icon set**: Phosphor, Fill for things, Bold for actions, the same
  glyph for the same thing (the CPU chip on the strip is the CPU chip in
  the signal picker).
- **Corner language**: rounded throughout: 10 px surfaces, 9 px controls,
  pills for small chips and tracks, discs for thumbs; no sharp corners, no
  one-sided borders.
- **Tone of voice**: plain words that say what happened and what to do
  next ("Add your town", "The strip is in use by another program. Close
  it and press Retry"), never a shell command as the only fix.
- **The strip itself**: wherever the window shows the strip, it shows these
  exact pixels (the preview is the live frame), so the window should frame
  it like an object on a walnut desk, not redraw it.
