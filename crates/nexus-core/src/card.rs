//! The legibility card: test frames that are read on the real strip, from
//! the seat, to set the type scale and colours on measurement rather than on
//! a screenshot (design plan, phase U0).
//!
//! Each frame asks one question. Its number is drawn large in a blue badge
//! at the left so answers can be given as "frame 3: 22". Everything that
//! identifies a sample (a size, a patch number) is drawn large and in the
//! same blue, so reading the tag never depends on reading the thing tested.

use crate::canvas::Canvas;
use crate::fonts::FontChoice;
use crate::text::{Font, TextEngine};
use crate::theme::{Rgb, TextStyle, Theme};
use crate::{HEIGHT, WIDTH};

/// Numeral sizes on the numeral ladders, in pixels.
pub const NUMERAL_SIZES: [u32; 7] = [14, 18, 22, 26, 30, 34, 38];
/// Mixed-case text sizes on the word ladder, two per frame.
pub const WORD_SIZES: [u32; 6] = [10, 12, 14, 16, 18, 20];
/// The contrast ladder's greys, dimmest first; patch N is entry N − 1.
pub const GREYS: [u8; 12] = [
    0x0c, 0x12, 0x18, 0x22, 0x2c, 0x38, 0x48, 0x5c, 0x72, 0x8a, 0xb8, 0xec,
];
/// Rows and columns lit by the edge frame, as distance from the edge, each
/// in its own colour: outermost first.
pub const EDGE_COLOURS: [(Rgb, &str); 4] = [
    (Rgb::hex(0xff3030), "red"),
    (Rgb::hex(0xffd800), "yellow"),
    (Rgb::hex(0x30d040), "green"),
    (Rgb::hex(0x3c8cff), "blue"),
];

/// IBM Plex Sans is variable from 100 to 700; Roboto from 100 to 900.
const PLEX_BOLD: f32 = 700.0;
const ROBOTO_BLACK: f32 = 900.0;

/// The badge and every tag: the theme's info blue.
const TAG: Rgb = Rgb::hex(0x5aa0e0);
const TAG_SIZE: f32 = 28.0;
/// Where the badge sits. Kept clear of the four outermost rows and columns,
/// so it never covers what the edge frame tests.
const BADGE: (u32, u32, u32, u32) = (8, 6, 46, 36);
/// Samples start right of the badge.
const CONTENT_LEFT: u32 = 66;
const CONTENT_RIGHT: u32 = WIDTH - 4;

/// What a frame builder returns: name, question and key (see `CardFrame`).
type Built = (&'static str, &'static str, String);

/// One test frame.
pub struct CardFrame {
    /// Short id, used for file names and in the terminal.
    pub name: &'static str,
    /// What to ask, printed while the frame is showing.
    pub question: &'static str,
    /// What the samples are, printed under the question.
    pub key: String,
    pub canvas: Canvas,
}

/// Every frame of the card, in showing order. Frame N (1-based) carries
/// the number N in its badge.
pub fn frames() -> Vec<CardFrame> {
    let mut d = Draw::new();
    let theme = Theme::default();
    let p = &theme.palette;
    let builders: [fn(&mut Draw, &mut Canvas) -> Built; 10] = [
        edges,
        |d, c| numerals(d, c, Face::Plex, 400.0),
        |d, c| numerals(d, c, Face::Plex, PLEX_BOLD),
        |d, c| numerals(d, c, Face::Roboto, ROBOTO_BLACK),
        |d, c| words(d, c, [WORD_SIZES[0], WORD_SIZES[1]]),
        |d, c| words(d, c, [WORD_SIZES[2], WORD_SIZES[3]]),
        |d, c| words(d, c, [WORD_SIZES[4], WORD_SIZES[5]]),
        contrast,
        severity,
        lines,
    ];
    builders
        .iter()
        .enumerate()
        .map(|(i, build)| {
            let mut canvas = Canvas::new(WIDTH, HEIGHT, p.background);
            let (name, question, key) = build(&mut d, &mut canvas);
            d.badge(&mut canvas, i + 1);
            CardFrame {
                name,
                question,
                key,
                canvas,
            }
        })
        .collect()
}

#[derive(Clone, Copy)]
enum Face {
    Plex,
    Roboto,
}

/// Text drawing for the card: the two faces it uses and one engine.
struct Draw {
    engine: TextEngine,
    plex: Vec<Font>,
    roboto: Vec<Font>,
    value: Rgb,
    background: Rgb,
}

impl Draw {
    fn new() -> Self {
        let stack = |choice: FontChoice| {
            let f = choice.load();
            let mut v = vec![f.text];
            v.extend(f.fallback);
            v
        };
        let theme = Theme::default();
        Self {
            engine: TextEngine::new(),
            plex: stack(FontChoice::IbmPlexSans),
            roboto: stack(FontChoice::Roboto),
            value: theme.palette.value,
            background: theme.palette.background,
        }
    }

    fn width(&mut self, face: Face, size: f32, weight: f32, text: &str) -> f32 {
        let fonts = match face {
            Face::Plex => &self.plex,
            Face::Roboto => &self.roboto,
        };
        self.engine.measure(fonts, &style(size, weight), text)
    }

    /// Draws `text` with its left edge at `x` and its baseline at `baseline`.
    /// Returns the width drawn.
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        canvas: &mut Canvas,
        face: Face,
        size: f32,
        weight: f32,
        text: &str,
        (x, baseline): (f32, f32),
        color: Rgb,
    ) -> f32 {
        let fonts = match face {
            Face::Plex => &self.plex,
            Face::Roboto => &self.roboto,
        };
        self.engine.draw(
            canvas,
            fonts,
            &style(size, weight),
            text,
            (x, baseline),
            color,
        )
    }

    /// Draws `text` centred on `cx`, its digits/capitals centred on `cy`.
    #[allow(clippy::too_many_arguments)]
    fn centred(
        &mut self,
        canvas: &mut Canvas,
        face: Face,
        size: f32,
        weight: f32,
        text: &str,
        (cx, cy): (f32, f32),
        color: Rgb,
    ) {
        let w = self.width(face, size, weight, text);
        let baseline = cy + cap_height(size) / 2.0;
        self.text(
            canvas,
            face,
            size,
            weight,
            text,
            ((cx - w / 2.0).round(), baseline.round()),
            color,
        );
    }

    /// A large tag in the card's blue, left edge at `x`, centred on `cy`.
    fn tag(&mut self, canvas: &mut Canvas, text: &str, x: f32, cy: f32) -> f32 {
        let baseline = (cy + cap_height(TAG_SIZE) / 2.0).round();
        self.text(
            canvas,
            Face::Plex,
            TAG_SIZE,
            PLEX_BOLD,
            text,
            (x, baseline),
            TAG,
        )
    }

    /// The frame number: dark on a blue block, so it never reads as a sample.
    fn badge(&mut self, canvas: &mut Canvas, n: usize) {
        let (x, y, w, h) = BADGE;
        canvas.fill_rect(x, y, w, h, TAG);
        let bg = self.background;
        self.centred(
            canvas,
            Face::Plex,
            TAG_SIZE,
            PLEX_BOLD,
            &n.to_string(),
            ((x + w / 2) as f32, (y + h / 2) as f32),
            bg,
        );
    }
}

fn style(size: f32, weight: f32) -> TextStyle {
    TextStyle {
        size,
        weight,
        tracking: 0.0,
    }
}

/// Height of digits and capitals: about 0.7 em in both faces (Plex 698,
/// Roboto 711 units per 1000).
fn cap_height(size: f32) -> f32 {
    size * 0.7
}

/// Splits the content area into `n` equal columns; returns each one's left
/// edge and width.
fn columns(n: u32) -> impl Iterator<Item = (u32, u32)> {
    let w = (CONTENT_RIGHT - CONTENT_LEFT) / n;
    (0..n).map(move |i| (CONTENT_LEFT + i * w, w))
}

const MID: f32 = HEIGHT as f32 / 2.0;

/// Edges: the four outermost rows and columns on every side, each depth in
/// its own colour, with a legend of the colours in the middle.
fn edges(d: &mut Draw, c: &mut Canvas) -> Built {
    for (depth, &(colour, _)) in EDGE_COLOURS.iter().enumerate() {
        let depth = depth as u32;
        c.fill_rect(0, depth, WIDTH, 1, colour);
        c.fill_rect(0, HEIGHT - 1 - depth, WIDTH, 1, colour);
        // Columns stop short of the edge rows, so every edge row is one
        // colour from end to end.
        let rows = EDGE_COLOURS.len() as u32;
        c.fill_rect(depth, rows, 1, HEIGHT - 2 * rows, colour);
        c.fill_rect(WIDTH - 1 - depth, rows, 1, HEIGHT - 2 * rows, colour);
    }
    // Legend: four swatches numbered 1 (outermost) to 4.
    let (size, gap) = (28u32, 12u32);
    let total = 4 * size + 3 * gap;
    let left = (WIDTH - total) / 2;
    let bg = d.background;
    for (i, &(colour, _)) in EDGE_COLOURS.iter().enumerate() {
        let x = left + i as u32 * (size + gap);
        let y = (HEIGHT - size) / 2;
        c.fill_rect(x, y, size, size, colour);
        d.centred(
            c,
            Face::Plex,
            20.0,
            PLEX_BOLD,
            &(i + 1).to_string(),
            ((x + size / 2) as f32, MID),
            bg,
        );
    }
    let key = EDGE_COLOURS
        .iter()
        .enumerate()
        .map(|(i, (_, name))| {
            format!(
                "{} {name}: row {} and row {}, column {} and column {}",
                i + 1,
                i,
                HEIGHT as usize - 1 - i,
                i,
                WIDTH as usize - 1 - i
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    (
        "edges",
        "On each side (top, bottom, left, right), which colours can you see? \
         Is the outermost (red, 1) line visible along the whole bottom edge?",
        key,
    )
}

/// A numeral ladder: each sample is its own size in pixels.
fn numerals(d: &mut Draw, c: &mut Canvas, face: Face, weight: f32) -> Built {
    let value = d.value;
    for ((x, w), size) in columns(NUMERAL_SIZES.len() as u32).zip(NUMERAL_SIZES) {
        d.centred(
            c,
            face,
            size as f32,
            weight,
            &size.to_string(),
            ((x + w / 2) as f32, MID),
            value,
        );
    }
    let (name, font) = match (face, weight as u32) {
        (Face::Plex, 400) => ("numerals-regular", "IBM Plex Sans Regular (400)"),
        (Face::Plex, _) => ("numerals-bold", "IBM Plex Sans Bold (700, its heaviest)"),
        (Face::Roboto, _) => ("numerals-black", "Roboto Black (900, the heaviest bundled)"),
    };
    (
        name,
        "Each number is drawn at that many pixels. What is the smallest number \
         you can read at a glance, without leaning in?",
        format!("{font}, sizes {NUMERAL_SIZES:?} px, colour #ececec (value)"),
    )
}

const WORD_LINES: [&str; 2] = ["Playing  Radiohead", "Volume  Weather"];
/// Rows between the four outermost at the top and at the bottom.
const SAFE_ROWS: f32 = (HEIGHT - 8) as f32;

/// A word ladder frame: one size per half, tagged large on the left.
fn words(d: &mut Draw, c: &mut Canvas, sizes: [u32; 2]) -> Built {
    let value = d.value;
    for ((x, _), size) in columns(2).zip(sizes) {
        let tag_w = d.tag(c, &size.to_string(), x as f32, MID);
        let s = size as f32;
        // Two lines 1.2 em apart, centred as a block, while they fit inside
        // the four outermost rows (which the bezel may hide); one line above
        // that. The first line has descenders, so it is the one kept.
        let line = (s * 1.2).round();
        let count = if s * 2.4 <= SAFE_ROWS { 2 } else { 1 };
        let first = (MID - (count - 1) as f32 * line / 2.0 + cap_height(s) / 2.0).round();
        for (i, text) in WORD_LINES.iter().take(count).enumerate() {
            d.text(
                c,
                Face::Plex,
                s,
                400.0,
                text,
                (x as f32 + tag_w + 12.0, first + i as f32 * line),
                value,
            );
        }
    }
    let name = match sizes[0] {
        10 => "words-10-12",
        14 => "words-14-16",
        _ => "words-18-20",
    };
    (
        name,
        "The blue number is the size of the words beside it. Which halves can \
         you read at a glance?",
        format!(
            "IBM Plex Sans Regular, {} px and {} px, colour #ececec; today's labels are 10 px",
            sizes[0], sizes[1]
        ),
    )
}

const PATCH_TOP: u32 = 20;
const PATCH_HEIGHT: u32 = 12;

/// The contrast ladder: a patch and a word in each grey, numbered above.
fn contrast(d: &mut Draw, c: &mut Canvas) -> Built {
    for (i, ((x, w), grey)) in columns(GREYS.len() as u32).zip(GREYS).enumerate() {
        let colour = Rgb {
            r: grey,
            g: grey,
            b: grey,
        };
        let cx = (x + w / 2) as f32;
        d.centred(
            c,
            Face::Plex,
            18.0,
            PLEX_BOLD,
            &(i + 1).to_string(),
            (cx, 11.0),
            TAG,
        );
        // Everything stays inside rows 4–43, which the bezel cannot hide
        // unless the edge frame says otherwise.
        c.fill_rect(x + 5, PATCH_TOP, w - 10, PATCH_HEIGHT, colour);
        d.centred(c, Face::Plex, 14.0, 400.0, "Aa", (cx, 38.0), colour);
    }
    let key = GREYS
        .iter()
        .enumerate()
        .map(|(i, &g)| {
            let role = match g {
                0x22 => " (divider)",
                0x8a => " (unit)",
                0xb8 => " (label)",
                0xec => " (value)",
                _ => "",
            };
            format!("{:>2} #{g:02x}{g:02x}{g:02x}{role}", i + 1)
        })
        .collect::<Vec<_>>()
        .join("\n");
    (
        "contrast",
        "On the background (#050505): what is the dimmest patch you can see at \
         all, and the dimmest grey whose \"Aa\" you can read?",
        key,
    )
}

/// Severity: white, warn and critical, each as text on black and as a
/// filled block with dark text.
fn severity(d: &mut Draw, c: &mut Canvas) -> Built {
    let p = Theme::default().palette;
    let bg = d.background;
    for ((x, _), colour) in columns(3).zip([p.value, p.warn, p.crit]) {
        let x = x as f32;
        d.text(c, Face::Plex, 22.0, 400.0, "92°", (x, MID + 8.0), colour);
        let (bx, bw) = (x as u32 + 70, 96);
        c.fill_rect(bx, 8, bw, 32, colour);
        d.centred(
            c,
            Face::Plex,
            22.0,
            PLEX_BOLD,
            "92°",
            ((bx + bw / 2) as f32, MID),
            bg,
        );
    }
    (
        "severity",
        "Look at the keys, not the strip: which of the six catches your eye? \
         Can you tell orange (warn) from red (critical), as text and as blocks?",
        "left to right: white #ececec (normal), orange #f0a830 (warn), red #e2625f (critical); \
         each as 22 px text on black, then a filled block with dark text"
            .to_string(),
    )
}

/// Lines: 1, 2 and 3 px, horizontal and vertical, and a block of bars of
/// that width with gaps of the same width.
fn lines(d: &mut Draw, c: &mut Canvas) -> Built {
    let value = d.value;
    for ((x, _), k) in columns(3).zip(1u32..=3) {
        let tag_w = d.tag(c, &k.to_string(), x as f32, MID) as u32;
        let left = x + tag_w + 10;
        // Horizontal line, centred vertically.
        c.fill_rect(left, 24 - k / 2 - k % 2, 70, k, value);
        // Vertical line.
        let vx = left + 84;
        c.fill_rect(vx, 6, k, 36, value);
        // Bars: four of width k, k apart.
        let bars = vx + k + 14;
        for b in 0..4 {
            c.fill_rect(bars + b * 2 * k, 6, k, 36, value);
        }
    }
    (
        "lines",
        "For 1, 2 and 3 px: can you see the horizontal line, the vertical line, \
         and the bars as separate bars?",
        "each group: blue width tag, a 70 px horizontal line, a 36 px vertical line, \
         then four bars of that width with gaps of the same width; colour #ececec"
            .to_string(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pixel(c: &Canvas, x: u32, y: u32) -> Rgb {
        let i = ((y * c.width() + x) * 4) as usize;
        let p = &c.rgba()[i..i + 3];
        Rgb {
            r: p[0],
            g: p[1],
            b: p[2],
        }
    }

    #[test]
    fn every_frame_is_panel_sized_and_numbered_in_order() {
        let frames = frames();
        assert_eq!(frames.len(), 10);
        for f in &frames {
            assert_eq!((f.canvas.width(), f.canvas.height()), (WIDTH, HEIGHT));
            assert_eq!(f.canvas.rgba().len(), (WIDTH * HEIGHT * 4) as usize);
            assert!(!f.question.is_empty() && !f.key.is_empty());
            // The badge's top-left corner is blue on every frame.
            assert_eq!(pixel(&f.canvas, BADGE.0, BADGE.1), TAG, "{}", f.name);
        }
        let mut names: Vec<_> = frames.iter().map(|f| f.name).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), frames.len(), "names are unique");
    }

    #[test]
    fn edge_frame_lights_the_outer_four_rows_and_columns() {
        let frames = frames();
        let edges = frames.iter().find(|f| f.name == "edges").unwrap();
        let c = &edges.canvas;
        for (depth, &(colour, name)) in EDGE_COLOURS.iter().enumerate() {
            let d = depth as u32;
            for x in 0..WIDTH {
                assert_eq!(pixel(c, x, d), colour, "row {d} ({name}) at x={x}");
                let bottom = HEIGHT - 1 - d;
                assert_eq!(pixel(c, x, bottom), colour, "row {bottom} at x={x}");
            }
            for y in 4..HEIGHT - 4 {
                assert_eq!(pixel(c, d, y), colour, "column {d} at y={y}");
                let right = WIDTH - 1 - d;
                assert_eq!(pixel(c, right, y), colour, "column {right} at y={y}");
            }
        }
        // The next row and column in are not edge colours.
        let bg = Theme::default().palette.background;
        assert_eq!(pixel(c, 300, 4), bg);
        assert_eq!(pixel(c, 300, HEIGHT - 5), bg);
        assert_eq!(pixel(c, 4, 24), bg);
        assert_eq!(pixel(c, WIDTH - 5, 24), bg);
    }

    #[test]
    fn only_the_edge_frame_draws_in_the_outer_rows_and_columns() {
        // Whether the bezel hides them is what frame 1 asks, so no other
        // frame may put anything there.
        let bg = Theme::default().palette.background;
        for f in frames().iter().filter(|f| f.name != "edges") {
            for y in (0..4).chain(HEIGHT - 4..HEIGHT) {
                for x in 0..WIDTH {
                    assert_eq!(pixel(&f.canvas, x, y), bg, "{} row {y} x={x}", f.name);
                }
            }
            for x in (0..4).chain(WIDTH - 4..WIDTH) {
                for y in 0..HEIGHT {
                    assert_eq!(pixel(&f.canvas, x, y), bg, "{} column {x} y={y}", f.name);
                }
            }
        }
    }

    #[test]
    fn badge_fits_two_digits() {
        let mut d = Draw::new();
        let w = d.width(Face::Plex, TAG_SIZE, PLEX_BOLD, "10");
        assert!(
            w + 6.0 <= BADGE.2 as f32,
            "\"10\" is {w} px in a {} px badge",
            BADGE.2
        );
    }

    #[test]
    fn badge_stays_clear_of_the_edge_rows_and_columns() {
        let (x, y, w, h) = BADGE;
        assert!(x >= 4 && y >= 4 && x + w <= WIDTH - 4 && y + h <= HEIGHT - 4);
        assert!(x + w < CONTENT_LEFT);
    }

    #[test]
    fn contrast_ladder_includes_todays_greys() {
        let p = Theme::default().palette;
        for c in [p.divider, p.unit, p.label] {
            assert!(c.r == c.g && c.g == c.b);
            assert!(GREYS.contains(&c.r), "#{:02x} missing", c.r);
        }
        assert!(GREYS.windows(2).all(|w| w[0] < w[1]), "dimmest first");
    }

    #[test]
    fn contrast_patches_are_their_grey() {
        let frames = frames();
        let f = frames.iter().find(|f| f.name == "contrast").unwrap();
        for ((x, w), grey) in columns(GREYS.len() as u32).zip(GREYS) {
            let p = pixel(&f.canvas, x + w / 2, PATCH_TOP + PATCH_HEIGHT / 2);
            assert_eq!((p.r, p.g, p.b), (grey, grey, grey));
        }
    }

    #[test]
    fn numerals_and_words_fit_on_the_panel() {
        // Every sample ends inside its column (no overlap with the next).
        let mut d = Draw::new();
        for face_weight in [
            (Face::Plex, 400.0),
            (Face::Plex, PLEX_BOLD),
            (Face::Roboto, ROBOTO_BLACK),
        ] {
            for ((_, w), size) in columns(NUMERAL_SIZES.len() as u32).zip(NUMERAL_SIZES) {
                let tw = d.width(face_weight.0, size as f32, face_weight.1, &size.to_string());
                assert!(tw + 8.0 < w as f32, "{size} px numeral too wide");
            }
        }
        for ((_, w), size) in columns(2).zip([WORD_SIZES[5], WORD_SIZES[5]]) {
            let tag = d.width(Face::Plex, TAG_SIZE, PLEX_BOLD, "20");
            for line in WORD_LINES {
                let tw = d.width(Face::Plex, size as f32, 400.0, line);
                assert!(tag + 12.0 + tw + 8.0 < w as f32, "{line:?} too wide");
            }
        }
    }
}
