# News headlines

Status: phase 1 (the data source) is done in
`crates/nexus-sources/src/news/`. Phase 2a is done: the `[news]` layout
section (store, undo, text), the daemon hook-up, the `news` widget and
the `news = "open"` / `"next"` actions; they are described in
docs/LAYOUT.md and DESIGN.md. Phase 2b is done: the window's Settings has
a News section (the feeds, added by pasting an address and checked once;
remove; reorder; how long each headline shows, 8, 12 or 20 s; how old one
may be, 1 day, 2 days or a week), and a news widget's editor shows the
feed count with "Edit feeds…". No presets are offered (see below).

## Privacy

Nexus Open fetches only the feeds you add, sends nothing but a request for
the feed, and shows no news until you add one.

In detail: each request is a plain GET with the same user agent as the
weather (`nexus-open/<version>`) and, after the first answer, the feed's
own ETag and Last-Modified sent back so an unchanged feed answers 304. No
cookies, no added query, no account. Feed addresses are written to the
log without their query string or user name and password, because private
feeds carry keys there.

## Config

`NewsConfig` lives in `nexus_core::config` as the `[news]` section, in
the shape phase 1 gave it. It derives `Deserialize` and `Serialize`, with
`deny_unknown_fields` like the other sections.

| Key       | Default | Meaning |
|-----------|---------|---------|
| `feeds`   | `[]`    | Feed addresses: RSS 2.0, RSS 1.0 (RDF), Atom 1.0 or JSON Feed. `https://` or `http://` only; others are logged and skipped. In the user's order: on a tie, the earlier feed's copy of a story wins. |
| `rotate`  | `"12s"` | How long each headline shows. At least 2 s. |
| `max_age` | `"48h"` | Headlines published longer ago are not shown. |

Durations are seconds as a number (`rotate = 12`) or a number with a unit
(`"30s"`, `"15m"`, `"48h"`, `"2d"`), and are written back the same way.
`nexus_sources::news::check_url` gives the window the same error text the
log uses. When a feed is added, `nexus_sources::news::check_feed` fetches it
once, off the window's UI thread, with the daemon's limits: the feed's title
and newest headline, or why not by kind (`Problem`: not a web address, no
answer, too many redirects, not found, another error status, not a feed,
too big), which the window says in plain words. For no answer or an error
status it offers "Add anyway", since the site may only be down.

Fixed, not settings: each feed is asked every 15 min; a failing feed is
tried again after 1 min, doubling to at most 30 min, and keeps its last
good headlines meanwhile. A request has 10 s, may follow 5 redirects and
may read 2 MB.

## Signals

| Signal        | Kind   | Meaning |
|---------------|--------|---------|
| `news.title`  | text   | The headline showing: plain text, at most 300 characters, the publisher's name trimmed off the end when it equals the feed title. |
| `news.source` | text   | The feed's title (at most 80 characters), or its host name if it has none. |
| `news.link`   | text   | The article, always an absolute `http(s)://` address with no spaces or control characters. |
| `news.age`    | number | Seconds since it was published (else updated, else first seen), as of when it was published to the strip. `signal::Unit` has no unit for time spans, so this is `Unit::None`, as `media.position` is. |
| `news.count`  | number | Headlines kept across all feeds, at most 50. |
| `news.lost`   | number | 1 while every feed is failing and the headline shown is older than `max_age` (kept rather than blanked), else 0. Added in phase 2a for the widget's lost state. |

Headlines are newest first, each story once (matched by guid/id, then
link, then title ignoring case and punctuation), and the source moves to
the next every `rotate`, wrapping to the newest. A refresh does not jump:
the headline showing stays until its turn ends.

With no feeds, or none read yet, nothing is published, so the widget sees
no signals at all. If headlines were shown and then go (the last feed
removed, all too old), the text signals are set to `""` and the numbers to
0, as the media source does when nothing is playing. Errors only go to the
log, once per feed each time its state changes.

`nexus_sources::spawn_news(publish)` returns a `News` handle:
`configure(Option<NewsConfig>)` (feeds new to the list are fetched at
once; `None` stops and blanks) and `next()` (show the next headline now,
for a full turn). Dropping the last handle stops the source.

## The widget: `news`

The proposal phase 2a started from. DESIGN.md has what was built; where
it differs, it says so below.

- One headline in `TITLE` (20 px), truncated at a word boundary with an
  ellipsis; below it, the source and age in `LABEL` (15 px) and TEXT2:
  "BBC News · 3 h". Age is "now" under a minute, then minutes, hours,
  days.
- No marquee. DESIGN.md's long-title marquee suits a track title that
  changes every few minutes; a headline changes every 12 s, and a moving
  line every 12 s cannot be read at a glance and breaks "quiet at rest".
  This is an exception phase 2 should write into DESIGN.md's "Long
  titles".
- The headline changes with a short fade, never a slide, so it does not
  read as a page swipe. Built differently: it changes at once, with no
  fade, so nothing on a news card moves every 12 s (DESIGN.md, Motion).
- Tap opens `news.link` in the default browser through the existing
  `Action::Open` path (`nexusd/src/actions.rs`, `xdg-open` on Linux).
  That action takes a fixed target today; the widget needs it to take the
  link showing at the moment of the tap (resolve the signal when the touch
  lands, not when it lifts, so a turn mid-tap cannot open a different
  story). Built as a separate `news = "open"` action and
  `Effect::OpenLink`, so a feed's link is checked (http(s) only, no
  spaces, control or invisible characters) and handed to the opener as
  one argument, never to a shell: `xdg-open` on Linux, `open` on macOS,
  `rundll32 url.dll,FileProtocolHandler` on Windows in place of
  `cmd /C start`, which treats `&`, `|` and `^` in a link as commands.
- Hold shows the next headline (`News::next`).
- News never raises a takeover and never colours anything warning or
  critical: no headline is certain enough to interrupt.
- States, following DESIGN.md's table: not set up (no `[news]` or no
  feeds): dashed accent outline, "News / Add a feed"; no headlines yet:
  a quiet normal card, "No news yet"; never an error string. Every feed
  failing with the last headline too old: the lost look, from the
  `news.lost` signal phase 2a added.

## Proposed feed presets

Not offered (2026-11-22): the window has no presets. A proposal only. None of these addresses was fetched or checked; the
tests use local fixtures. Before the window offers any of them, someone
must check that each address works and that its terms of use allow a
desktop app to show its headlines (several publishers allow personal,
non-commercial use only).

| Topic       | Name           | Feed address (unchecked) |
|-------------|----------------|--------------------------|
| World news  | BBC News, World | `https://feeds.bbci.co.uk/news/world/rss.xml` |
| World news  | NPR News       | `https://feeds.npr.org/1001/rss.xml` |
| Technology  | Ars Technica   | `https://feeds.arstechnica.com/arstechnica/index` |
| Technology  | The Verge      | `https://www.theverge.com/rss/index.xml` |
| Linux       | LWN.net        | `https://lwn.net/headlines/rss` |
| Linux       | Phoronix       | `https://www.phoronix.com/rss.php` |

## Known limits

- An entry without a date counts as published when first seen, and that
  is kept in memory only, so after a restart it counts as new again.
- Relative article links are dropped rather than resolved against the
  feed address.
- A body is capped at 2 MB after decompression; a feed bigger than that
  is reported and skipped.
