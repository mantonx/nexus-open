# Nexus Open design plan

Status: opened 2026-11-22, after Matt said the UI "sucks" and asked for a
brutal assessment and a plan for a beautiful, well-thought-out, useful app.
Written for an agent to execute phase by phase, like `REDESIGN_PLAN.md`.
Stop and ask Matt at every **GATE**.

## Why this plan exists

An Impeccable critique on 2026-11-22 (two isolated reviewers; snapshots in
`.impeccable/critique/`) scored:

- **The strip: 15/36 (Poor).** Only the value digits (3 mm tall) can be read
  from a seat 60 cm away; labels, titles, rates and both takeovers are
  1.4–1.6 mm. Every widget is one template stamped at any width, so the
  full-width Clock page uses 6% of the strip. Graphs autoscale and run
  through the digits. Critical red is the dimmest number shown. Sliders look
  like progress bars and jump on touch. No icons, artwork, theme or identity.
- **The settings window: 20/40 (bottom of Acceptable).** Stock Slint theme;
  one grey button plays tab, toggle, action and delete; the Signals list
  takes 27% of the window; layouts are buried under the font picker; the
  form you work in is cut off; the first "Add widget" errors and always
  inserts a Clock; failure states give shell commands and, on 2026-11-22,
  the wrong cause.

Both reviewers: the engineering is careful, and neither surface has been
designed. The 9/10 bar in `REDESIGN_PLAN.md` let this through because it
measures consistency ("one type scale", "crisp text"), not legibility at
arm's length or whether anything looks good.

## Decisions (Matt, 2026-11-22)

- The strip is redesigned first; the window second.
- The strip's direction is chosen by looking: three directions are mocked up
  as real 640×48 frames.
- The window gets a new concept, not a restyle.
- The redesign comes before the first release. Packaging (Phase 5 "ship")
  waits. The daily-driver trial to 2026-11-28 continues on the current build
  and still decides whether takeovers stay.

## What is kept, and what is replaced

Kept: the daemon, the control protocol, the store and its undo, the gesture
physics, sources and signals, takeover timing rules, the sandboxed service,
the doctor, and the idea that the preview is the live strip and the editing
canvas. None of the redesign throws engineering away.

Replaced: how widgets are drawn (`nexus-core` `theme.rs`, `widget.rs`), the
default layout, and the structure and look of `nexus-ui/ui/app.slint`.

## The design bar (added to the 9/10 bar)

Each is pass or fail with evidence.

- `[U3]` Every widget has one primary element readable at Matt's seat,
  proven with the legibility card on the real strip, not by calculation.
- `[U3]` No text on the strip is smaller than the smallest size Matt could
  read on that card.
- `[U3]` A widget given more width shows more or shows it bigger; no widget
  is a narrow one with black beside it.
- `[U3]` Warning and critical states are noticeable without looking at the
  strip directly: they change filled area or brightness, not only hue.
- `[U3]` A graph never crosses its number, and its scale means the same
  thing from one minute to the next.
- `[U3]` You can tell what a touch will do before touching: sliders, taps
  and inert zones look different, and a brush cannot set volume.
- `[U3]` "No data", "not set up" and "source lost" look different.
- `[U3]` Strip critique of 28/36 or better, by an isolated reviewer.
- `[U5]` Someone who never opens a terminal can go from first open to a
  strip they chose, and can recover from "strip busy" and "daemon stopped"
  with buttons in the window.
- `[U5]` Adding a widget never errors on a full page and never needs a
  pixel number.
- `[U5]` Signal identifiers (`cpu.load`) are never the primary name of
  anything.
- `[U5]` Window critique of 30/40 or better, by an isolated reviewer.
- `[U5]` The Phase 4 exit test: Matt rebuilds his layout from empty using
  only the window, on native Wayland.

## Phases

### U0 — Ground truth

Design for the real panel and the real seat, not for a screenshot.

1. The strip must be free: the old Go app (`/usr/bin/nexus-open --tray`)
   took it on 2026-11-22. Matt quits it. Separately, fix the doctor so a
   strip claimed through libusb is reported as "held by <program>", not
   "not plugged in".
2. **Legibility card.** A `nexusd preview` mode that shows test frames on
   the strip: a ladder of numeral and text sizes (10 to 40 px, regular to
   bold), a contrast ladder of greys on the background, the warn and
   critical colours as text and as fills, and 1, 2 and 3 px lines. Matt
   reads it from where he sits and reports the smallest size he reads at a
   glance, the dimmest grey he sees, and whether the bottom two rows are
   hidden by the bezel or the angle. Record the answers here; U2's type
   scale is built from them.
3. **PRODUCT.md** through `impeccable init`: who the strip is for, where it
   sits, what it is for while gaming, working and idle, and what must stay
   true. **GATE:** Matt answers the interview.

### U1 — Three strip directions, as real frames

Render each direction through the real renderer (a throwaway branch or an
example binary; no layout or store changes yet), with the same six scenes:
idle desk, music playing, a game running hot, a critical temperature, a
volume change, and first run with nothing set up.

- **Instrument.** State is carried by large fills, bars and colour that
  can be read without reading; numerals second.
- **Fewer, bigger, by context.** One or two things at full height
  (numerals about 7.5 mm), and the strip chooses what to show: temperatures
  when a game runs, a media hero with artwork when music plays, a big clock
  when idle. This generalises today's takeovers into scenes.
- **Better cards.** Pages of widgets stay, with a readable type scale,
  icons, compact/medium/hero sizes and honest graphs. Least change to how
  layouts work.

Show them on the strip itself (the preview mode cycles them) and as one
contact sheet printed at true size, 127 mm wide. **GATE:** Matt picks one,
or names the parts to combine. Record what the choice means for the layout
format: "by context" needs scene rules; the other two do not.

### U2 — The strip's design system

Write `DESIGN.md` (through `impeccable`'s new-work flow) for the chosen
direction:

- Type scale from U0's measurements; one display numeral face with real
  weight; where labels become icons.
- Size classes per widget and what each shows.
- Colour: background, neutrals, one accent, severity. Where the accent
  comes from (a setting, album art, time of day) is decided here.
- Icons: one set, drawn or licensed for MIT distribution.
- Graph rules: fixed ranges with a threshold mark, a band of their own,
  severity colouring, inset from the zone edge.
- States: nothing playing, not set up, no data, source lost, stale.
- Touch: slider thumb at rest and relative drag, a visible pressed state,
  a cue at the hold threshold, page ticks, minimum target width.
- Motion: takeover entry and exit, page change, value change.
- Idle and night: dimming, and what the strip shows when nothing matters.

**GATE:** Matt signs off `DESIGN.md` from rendered examples of every
widget in every size and state.

### U3 — Build the strip

Implement in `nexus-core`; migrate stored layouts (the config has a schema
version); redo the default layout so no page only stretches another; add
image tests for every widget, size and state. Deploy to Matt's strip as it
lands (`scripts/nexusd-dev-install.sh`).

Exit test: every `[U3]` bar item passes, and Matt has used it for three
days without wanting the old one back.

### U4 — The window's concept

`impeccable shape`, then two or three structural comps as real Slint
screens (not prose), using the new strip rendering. Ideas to test, from
the critique:

- A storyboard: every page and takeover as a live strip, stacked,
  reorderable, edited where you click.
- A gallery of rendered widget thumbnails to add from, including presets
  ("CPU temperature", "Volume"), which makes room by itself.
- Settings for the selected widget attached to it, not in a pane that gets
  the leftovers.
- A signal picker on the field, with human names and live values; the raw
  list becomes a drawer for tinkerers.
- Layouts in the header; app preferences out of the editor.
- A one-line status that opens to in-app fixes ("Start the service",
  "Retry", "Open the log"), with the true cause.
- A first run that offers good-looking starter layouts.
- The product's own name and mark; the About dialog with the Slint badge
  in proportion.

**GATE:** Matt picks the concept.

### U5 — Build the window

An authored Slint style (tokens, surfaces, three button tiers, real tabs
and segmented controls) in place of the stock theme. Keep what the audit
on 2026-11-15 established: keyboard reach, screen-reader labels, light and
dark, 1024×640. Exit test: every `[U5]` bar item passes.

### U6 — Back to shipping

Resume `REDESIGN_PLAN.md` Phase 5: packaging, the install guide, the
README clip (now of the new strip), release verification.

## Standing rules

- Judge rendered results at the size they ship: frames on the strip, the
  window at 1024×640 and at its default size. A description is not a
  design.
- Before calling any phase's design done, run an isolated Impeccable
  critique and record the score here, unsoftened.
- An approved mock is evidence, not protection: if the build shows the
  mock was wrong, say so and bring Matt the replacement as a decision.
- Never test on Matt's live layout; use a throwaway daemon with its own
  store and no device access, as the 2026-11-22 captures did.

## Decision log

- 2026-11-22 — Plan opened. Critique: strip 15/36, window 20/40. Matt:
  strip first, three directions mocked up, new window concept, redesign
  before release. Not judged in the critique: the physical panel
  (brightness, angle, colour), the light theme, drag feel, HiDPI.
- 2026-11-22 — U0 tools built, by delegated agents, reviewed and merged.
  The doctor, `status` and the window's banner now tell apart: not plugged
  in; in use by a named program; left without its driver (what the old Go
  app does when it quits: it detaches usbhid and never reattaches, so a
  replug is needed). Seen live on Matt's machine. `nexusd preview --card`
  shows a ten-frame legibility card and `--frames DIR` shows any folder of
  640×48 PNGs; both need `nexusd.service` stopped first, since opening the
  strip does not lock it. Neither has been run on the panel yet.
- 2026-11-22 — U1 rendered: three directions, the same six scenes each,
  as frames from the real drawing stack. The scene frames are in
  `docs/design/u1/` (named so each scene shows A instrument, B by context,
  C better cards); the code is on local branches `design/u1-instrument`,
  `design/u1-context` and `design/u1-cards`, not merged until one is
  chosen. Provisional sizes (30 px numerals, nothing under 14 px), since
  the card has not been read yet. Weak points as their authors and the
  review found them. A: densest (32 px numerals), media cramped, bars say
  little at idle. B: largest figures (52 px) but depends on knowing a game
  is running, which the daemon cannot tell; the edge key's readouts are
  small. C: least change to layouts, most conventional; long titles cut
  off; the current page's marker is hard to see. All three needed Plex's
  width axis and tabular figures, which `TextEngine` does not set yet.
  **GATE open:** Matt picks.
- 2026-11-22 — **U1 GATE passed.** Matt chose, agreeing with the review:
  C "better cards" as the structure (pages the user arranges; existing
  layouts keep working), with three things from B (the full-strip critical
  alarm, the media scene as the now-playing takeover with transport keys
  and the full title, and B's scale for hero-size widgets) and A's
  fixed-scale bar with its limits marked for system widgets. B as a whole
  was not chosen because it depends on knowing a game is running, which
  the daemon cannot tell, and it hides the machine at idle; A as a whole
  was the densest and says little at idle.
- 2026-11-22 — **U0 step 3 done: `PRODUCT.md`.** Matt's answers: the strip
  is a glance display and a control surface equally; the character is warm
  and friendly (the mockups' cool, technical look was the agents' choice
  and is not kept); the accent colour is the user's choice with a good
  default; the first release is for anyone with the strip, no terminal.
  Still open in U0: the legibility card has not been read (the strip needs
  a replug first), so U2's sizes stay provisional.
- 2026-11-22 — Matt set the goal "finish the redesign" and the work
  continues without stopping at each gate. Decisions the plan marked as
  his are taken provisionally by the agent, recorded here, and remain his
  to overturn. The legibility card is still unread, so every type size
  stays a named constant that can be re-tuned in minutes.
- 2026-11-22 — Behaviour rules fixed for U3 (provisional, see above), so
  the engine work can start before the look is final. Sliders are
  relative: a touch landing changes nothing, a drag moves the value by
  the distance travelled. A long press fires at the threshold while still
  held, and shows its progress. A reading that rises past its critical
  threshold raises a full-strip alarm until OK is tapped, it is swiped
  away, or the reading has been back under for 10 s; it re-arms after 30 s
  under. A takeover may cover part of the strip (`align`, `width`). New:
  a `key` widget (icon, label, tap and hold actions), a second reading on
  a value widget, an `accent` setting, and page state for the markers.
- 2026-11-22 — Groundwork merged. Text can be set condensed and with
  tabular figures (90 frames byte-identical before and after). The strip's
  new behaviour is in the engine with tests: relative sliders with a 4 px
  dead zone, hold at the threshold, the critical alarm, partial takeovers,
  the `key` widget, a second reading, `accent`, page state. The store's
  schema moves to 2; an old database loads identically and keeps its undo.
  Not deployed to Matt's machine until the new drawing is in.
- 2026-11-22 — **U2 signed off provisionally by the agent** (Matt's to
  overturn). `DESIGN.md`: "lamplight on walnut": brown-black ground,
  cream type, Rubik 600 numerals, rounded cards, Phosphor icons, a lilac
  default accent; any accent keeps only its hue (fixed lightness and capped
  chroma per role). Warning fills the card amber, critical fills it red,
  the alarm fills the strip. Reference frames: `docs/design/u2/`; all 74
  come from `cargo run -p nexus-core --example design_system -- DIR`. An
  isolated reviewer scored the frames 25/36 before the last fixes (U1 was
  15/36); the 28 target is measured again on the built strip. Known weak
  points: an accent near amber sits close to a warning bar; compact cards
  need about 120 px for three digits; a medium media card fades a long
  title; Rubik has no hinting; nothing is measured on the panel.
  `DESIGN.md` names keys the engine does not have yet (`night`,
  `idle_dim`, `second_label`, `scale`, accent preset names, `icon` on any
  widget, a key's `sub`); the renderer build reconciles them.
- 2026-11-22 — **Mistake, caught and fixed:** while testing a capture rig,
  a throwaway daemon opened Matt's real strip for about four minutes and
  drew on it alongside his own daemon. Root cause: the rig relied on
  `DevicePolicy=closed` to keep it off the device, and a user unit's device
  policy is not enforced (the daemon opened hidraw with no `DeviceAllow`
  for it). `nexusd run --no-device` now exists; `scripts/ui-rig/rig.sh`
  uses it and refuses to continue if its daemon reports the strip
  connected. The same fact means `nexusd.service`'s `DevicePolicy` and
  `DeviceAllow` lines do nothing as a user unit: the hardening claim in
  `REDESIGN_PLAN.md` (device access limited to hidraw and NVIDIA) is wrong
  and needs correcting in Phase 5.
- 2026-11-22 — **The window's Appearance** (U5, at Matt's request): Match
  the system (the default), Light or Dark, in Preferences under "This
  window", applied live. It is kept in the window's own file,
  `$XDG_CONFIG_HOME/nexus-open/window` (`appearance = "system"`), not in the
  layout or the store's settings, because it changes only the window: the
  strip stays dark whatever it says, it must not be exported with a
  layout, and choosing it must not enter the layout's undo history (it does
  not; checked in the capture rig). A config file rather than the state
  folder beside the remembered window size, since it is a choice the user
  makes, not something the window notices. "Match the system" follows a
  desktop switch without a restart: Slint 1.18 watches the
  `org.freedesktop.appearance color-scheme` portal setting, and the window
  leaves Slint's palette on `unknown` so it keeps tracking it. Checked in
  the rig's own session (its own bus, dconf and portal), both directions.

- 2026-11-22 — **Matt: light and dark mode for the window.** The window
  follows the desktop's colour scheme by default, with a "Match the
  system / Light / Dark" choice saved as a window preference (not in the
  strip layout, so it never enters undo). Light mode inverts the walnut
  ladder at the same hue (cream ground, walnut text) and must pass the same
  U5 bar as dark, judged from captures of both. The strip stays dark.
- 2026-11-22 — **Matt: news on the strip.** Provisional shape (Claude,
  his to overturn): headlines from RSS, Atom or JSON feeds the user adds;
  no feed is on by default, so nothing is fetched until the user adds a
  feed (as with weather). One headline at a time, rotating every 12 s,
  cut at a word with an ellipsis, source and age as a second label; no
  scrolling marquee. Tap opens the article; hold shows the next one. News
  never raises a takeover (Product Principle 2). Phase 1, the source
  (`crates/nexus-sources/src/news/`, `docs/design/news.md`), is merged;
  phase 2 (the `[news]` section, the widget, the window's feed settings
  and presets whose terms are checked) waits for U3 and U5. Before tap
  opens a link on Windows, the open-URL action must stop going through
  `cmd /C start`, which would run `&`, `|` and `^` in an untrusted link.
- 2026-11-22 — **U3 polish, after the isolated review of the built strip
  (25/36, target 28).** Five findings fixed on `feat/strip-polish`; each
  decision is **provisional, Matt to overturn**:
  1. *Hero bars were shorter than medium ones.* A hero reading's numeral
     steps down to 40 px (`NUM_HERO_BANDED`) so its bar runs the whole
     card (290 px at 312, against 178 at 200); a second reading is named
     on the numeral's baseline at the right and has no bar of its own at
     hero. Clock, weather and the alarm keep 50 px.
  2. *Accent on information.* The volume change ("+15"), the now-playing
     artist, the progress line and a dragged slider's number are now in
     the text ramp; a partial takeover is rimmed in the accent only when
     it holds a control. Every other accent use was checked and is on a
     hit zone (keys, thumbs, set-up cards, the medium media badge, whose
     whole card is the key).
  3. *Trouble off the page showed only as a 2 px marker.* The alert
     marker is now 16 px longer and 3 px tall, and an edge flag (a 4 × 28
     px pill, amber or red) stands at the strip's end on that page's
     side. A chip naming the reading does not fit: the marker margin is
     four rows, and anywhere lower covers a card.
  4. *Secondary text small.* LABEL, UNIT and BODY are 18 px (the floor);
     provisional until the legibility card is read. DESIGN.md lists the
     few places 18 does not fit and what gives.
  5. *Night dim looked lost.* Dimming spares figures: surfaces keep the
     level, cream text half the loss (night: 35 % and 68 %), so a live
     number stays above a lost one; lost keeps its broken link and time.
  Also: partial takeovers snap to the page's card edges (within 40 %),
  and a medium rate card with down and up in different units shows both
  in two rows.

  **Rescored in isolation: 26/36, still under 28.** The reviewer's top
  findings were an unnamed medium second reading ("6%" of what, with a
  near-empty bar read as ".6%") and a 45 px slider after the snap. Fixed
  in one more round (also provisional): the second reading carries its
  own icon with its bar starting under it; a snap never leaves a partial
  takeover under 320 px (`TAKEOVER_MIN`); a lost reading keeps its icon
  with a broken-link badge; the current page's marker stays cream with
  the alert as a segment after it; a medium name is laid out for two
  digits and hero media times do not move on pause.

  **Rescored once more, by a fresh reviewer: 26/36 again** (Good, bottom
  of the band; recognition rather than recall 2, everything else 3). Its
  top findings, not addressed (the brief allowed one fix round):
  1. *P1, the medium media card does not look tappable*: its only sign
     is a 14 px badge, while every other key has a raised face.
  2. *P1, page markers and edge flags are faint at 60 cm*: 2 px
     markers, and an amber edge flag beside an amber end card reads as
     the card's lip.
  3. *P1, the idle media slot is a dead end*: "Play something" with
     nothing to tap (resume the last player, or open one).
  Also P2: the second reading's icon still goes beside three-digit
  firsts with a two-digit second (DESIGN.md open question 14), and lost
  and no-data states use drifting words and cannot be acted on.
- 2026-11-22 — **U3 polish 2** (`feat/strip-polish-2`), each decision
  **provisional, Matt to overturn**:
  1. *A card's layout never changes with its value* (closes DESIGN.md
     open question 14). A medium value card with a second reading is
     laid out for both at their widest (three digits, from the units
     alone, before any reading), and its sizes step down until the
     second's icon fits: 32 / 20 px in the System page's 204 px zones,
     28 / 20 in a 200 px zone. The bars split at a fixed column.
  2. *The medium media card did not look tappable.* A raised 36 px
     play/pause key, accent-filled, at its right replaces the 14 px
     badge on the artwork; the whole card is still the hit zone.
  3. *The idle media slot was a dead end.* With nothing playing, a media
     widget whose layout says how to open the player (its own `tap`, or
     a Music key that runs or opens something) is a key: "Nothing
     playing" / "Tap to open music". The default Music key shows a page,
     which opens no player, so on the default layout the slot stays quiet
     (DESIGN.md open question 15).
  Not done: faint page markers and edge flags need the real panel (U0).

  **Rescored by a fresh isolated reviewer: 25/36** (recognition and
  error recovery 2, the rest 3). Top findings: System never names CPU or
  GPU (icons alone); "lost" barely registers at medium and compact, and
  a lost card's second reading stays cream; the Home media title is cut
  to "Weird Fis"; and the new medium media key ends 4 px from the card's
  edge, so a red edge flag at 636–639 reads as stuck to it (a regression
  here: the key should keep the full `PAD`).

  Then, without a rescore: the medium media key keeps the full `PAD`; no
  text ends in a letter cut through (a title at rest ends in an ellipsis
  after a whole word, or between letters; it fades only while moving);
  a lost card's second reading is drawn lost too. "System does not name
  CPU and GPU" is DESIGN.md open question 16.
- 2026-11-22 — **U3 closed on renders, provisionally (Claude; Matt to
  overturn).** Four isolated reviews scored the built strip 25, 26, 26
  and 25/36, against a target of 28. The 28 bar is **not met**. The
  scores swing with each fresh reviewer while the strip measurably
  improves (hero bars full width, accent only on controls, off-page
  flags, 18 px secondary text, night distinct from lost, fixed layouts,
  a real media key, ellipses), so more rounds on renders are measuring
  the reviewer, not the strip. Every open finding but the first
  needs the panel or Matt:
  - the System page names CPU and GPU only by icon (open question 16);
  - markers and edge flags may be faint at 60 cm;
  - 18 px secondary text is unmeasured;
  - Rubik is unhinted;
  - the empty media slot opens nothing on the default layout (open
    question 15).

  The last gate for U3 is the deployed build read on the real strip with
  the legibility card, and Matt's verdict on the frames. Whatever he
  finds there is fixed before release.
- 2026-11-22 — **U5 window review fixes, provisional (Claude; Matt to
  overturn).** After two isolated rescores at 28/40 in each mode:
  - Selection is a 2 px neutral outline (the page being edited in the
    window's text-2; the selected widget, on the always-dark strip, in the
    strip's cream). The accent ring, 2 px clear of the control, is
    keyboard focus only, shown when focus arrives by Tab or arrow keys, not
    by a click. The strip's dashed "Set up" outline is unchanged.
  - A button on a warning or problem banner is a neutral key, never the
    accent, whatever the accent is.
  - A failed "Start the service" is its own state: the chip reads "Did not
    start", the banner turns to a problem with the reason in plain words.
  - In Edit, the page being edited shows its own widgets, not the strip's
    live frame, so an alarm or takeover never sits under the selection
    outlines; Try it and the other rows show the live frame.
  - Add takes room only down to where each kind still reads (key 40 px,
    reading, rate and clock 72, slider 80, music 120) and is off, with the
    reason, below that. At 40 px a not-set-up key shows its icon alone
    (nexus-core, new reference frame `narrow-keys`).
  - Dark mode's ground and surfaces moved a step above the strip's ground
    (#1c1815, #251f1b, #2f2823); disabled words have their own token,
    3:1 or better on every surface in both modes.
  - Icons are chosen from a searchable list of the strip's icons (a list,
    not a grid); a bar's scale is two number fields in the shown unit.
- 2026-11-22 — News widget, provisional: the headline wraps onto two
  lines at `LABEL` 18 (variant B) rather than one line at `TITLE` 20 with
  the source under it (variant A). Side by side at 200 to 512 px with
  headlines of 40 to 110 characters, B shows about twice the headline
  and still reads as one calm card; the source is shown only after the
  headline's end on a hero card, when it fits. Costs the accent newspaper
  on most cards (the raised card carries the tap). DESIGN.md, Long
  titles. To be read on the panel.
