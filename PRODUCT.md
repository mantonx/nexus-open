# Product

<!-- impeccable:product-schema 1 -->

## Platform

desktop

Not a web product. Two surfaces: a hardware display (the strip, drawn by the
app's own renderer in `crates/nexus-core`) and a native desktop settings
window (Slint, `crates/nexus-ui`). Linux first; macOS and Windows later.

## Users

Anyone who owns a Corsair iCUE Nexus and runs Linux. The first release must
work for someone who installs a package and never opens a terminal; people
who like to tinker (config as text, their own data sources, a command line)
are served too, but are not the bar. Confirmed by Matt, 2026-11-22.

They sit at a desk with the strip at the top edge of the keyboard or in
front of the monitor, about 60 cm from their eyes, looking down at it at an
angle. They glance at it while working or gaming and reach for it without
looking closely.

## Product Purpose

Nexus Open makes the iCUE Nexus touch strip useful and good-looking on
Linux, where Corsair ships no software. The strip is a glance display and a
control surface in equal measure (Matt, 2026-11-22): the time and the
machine's state at a glance, and media, volume and launchers under a finger.

Success: the owner reads what matters from their seat without leaning in,
can tell what a touch will do before touching, sets it up without a
terminal, and likes how it looks on their desk.

## Positioning

An open (MIT) alternative to Corsair's iCUE for this device, and in time for
other Corsair devices (`docs/DEVICES_PLAN.md`). Native, not a web UI. No
root, no account, no network service: a small user-level daemon and a
settings window.

## Operating Context

- The strip: 640×48 px, about 127 × 9.5 mm (roughly 128 px per inch), colour,
  touch along one axis: tap, long press, swipe, drag.
- A background daemon (`nexusd`) draws the strip and keeps working with the
  settings window closed. The window edits what the strip shows, live.
- What the strip shows is a layout: pages of widgets the user arranges,
  swiped between, plus takeovers that cover the page for a moment when
  something changes (the track, the volume, a reading going critical).
- Readings come from the machine (temperatures, load, memory, network),
  the media player, the weather, and the user's own programs.

## Capabilities and Constraints

- The user arranges their own pages. The strip does not rearrange itself by
  guessing what the user is doing; it may interrupt only for events that are
  certain (a threshold crossed, the track or volume changed).
- The accent colour is the user's choice, with a good default
  (Matt, 2026-11-22). Warning and critical colours are separate from it.
- Temperatures in °C or °F, 12- or 24-hour time, strings translatable.
- Clean-room: nothing copied from Corsair's software or from the GPL
  projects for these devices. Fonts and icons must be licensed for MIT
  distribution (OFL, Apache, MIT, ISC).
- Not yet measured on the real panel: the smallest readable sizes,
  brightness, colour, and whether the bezel hides the edge rows
  (`docs/DESIGN_PLAN.md`, U0).
- Undecided: whether the accent may also follow album art.

## Brand Commitments

- The name is "Nexus Open". It is not made or endorsed by Corsair and must
  not look like it is.
- Character: warm and friendly. Softer shapes and colour, less technical,
  more a small companion display than an instrument panel or gaming
  hardware (Matt, 2026-11-22).
- Voice: plain words, says what happened and what to do next, never a shell
  command as the only fix.

## Evidence on Hand

- The 2026-11-22 critique of both surfaces: `.impeccable/critique/`.
- Three mocked directions for the strip as real frames: `docs/design/u1/`.
  Chosen: pages of widgets ("better cards") as the structure, with the
  by-context direction's full-strip critical alarm, media scene and large
  hero scale, and the instrument direction's fixed-scale bar with its
  limits marked.
- No product mark or logo exists yet. No photographs of the real strip.

## Product Principles

1. Readable from the seat: if it cannot be read at 60 cm, it is not on the
   strip.
2. The user arranges; the strip interrupts only for what is certain.
3. A touch never surprises: what can be pressed or dragged looks like it.
4. No terminal required, ever, for someone who just owns the strip.
5. Quiet at rest, unmistakable when something needs attention.

## Accessibility & Inclusion

- Warning and critical states never rely on hue alone.
- The settings window is fully usable from the keyboard and with a screen
  reader, in light and dark, at 1024×640 and up.
