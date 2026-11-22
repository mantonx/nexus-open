//! Widgets and the page renderer.
//!
//! A page is a row of zones. Each zone has a width and a `WidgetView`: the
//! already-formatted data a widget shows. Turning live signals into views
//! (formatting, thresholds, history) happens before this point, so drawing
//! stays a pure function of view + theme + fonts.

use tiny_skia::{FillRule, Paint, PathBuilder, Stroke, Transform};

use crate::canvas::{Canvas, to_color};
use crate::text::{Font, TextEngine};
use crate::theme::{Rgb, TextStyle, Theme};
use crate::{HEIGHT, WIDTH};

/// The fonts a renderer draws with.
#[derive(Clone)]
pub struct Fonts {
    /// Used for all text.
    pub text: Font,
    /// Tried in order for characters `text` has no glyph for, such as arrows
    /// or symbols in a song title.
    pub fallback: Vec<Font>,
    /// Icon font (Font Awesome Solid). Icons are looked up by codepoint.
    pub icon: Font,
}

/// How a value is judged against its thresholds. Drives the value colour.
/// Ordered from calm to critical, so the worst of several is their `max`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    #[default]
    Ok,
    Warn,
    Crit,
}

/// A history graph drawn along the bottom of a zone.
#[derive(Clone, Debug, PartialEq)]
pub enum Graph {
    /// Filled area under a line. Values are 0.0–1.0, oldest first.
    Area(Vec<f32>),
    /// Line only. Values are 0.0–1.0, oldest first.
    Line(Vec<f32>),
}

/// What a zone shows, already formatted.
#[derive(Clone, Debug, PartialEq)]
pub enum WidgetView {
    /// Date as the label, time as the value, AM/PM as the unit.
    Clock {
        date: String,
        time: String,
        meridiem: Option<String>,
    },
    /// A labelled reading such as a temperature.
    Value {
        label: String,
        value: String,
        unit: String,
        icon: Option<char>,
        graph: Option<Graph>,
        severity: Severity,
        /// A second reading, shown beside the first.
        second: Option<Reading>,
    },
    /// A label with a small text readout and a graph, such as network rates.
    Rate {
        label: String,
        caption: String,
        graph: Option<Graph>,
    },
    /// The track playing, or `None` when nothing is.
    Media(Option<Track>),
    /// A label, a readout, and a bar filled to `fraction`.
    Slider {
        label: String,
        value: String,
        /// 0.0–1.0; `None` when there is nothing to show.
        fraction: Option<f32>,
        /// A finger is on it: the bar is drawn thicker, with a handle.
        active: bool,
        /// Shown dimmed (muted audio).
        muted: bool,
    },
    /// A control to press: an icon's name and a label, either optional.
    Key {
        icon: Option<String>,
        label: Option<String>,
    },
}

/// A formatted reading: the second one of a value widget.
#[derive(Clone, Debug, PartialEq)]
pub struct Reading {
    pub value: String,
    pub unit: String,
    pub severity: Severity,
}

/// A track as the media widget shows it.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub title: String,
    /// Empty when the player gives none.
    pub artist: String,
    /// 0.0–1.0 through the track; `None` when the length is unknown.
    pub progress: Option<f32>,
    /// False while paused: the title is dimmed.
    pub playing: bool,
}

/// Shown by the media widget when no player has a track.
pub const NOTHING_PLAYING: &str = "Nothing playing";

#[derive(Clone, Debug, PartialEq)]
pub struct Zone {
    pub width: u32,
    pub view: WidgetView,
}

/// Draws pages. Keep one per display: it caches rasterized glyphs.
pub struct Renderer {
    theme: Theme,
    /// The text font followed by its fallbacks.
    text_fonts: Vec<Font>,
    icon_fonts: Vec<Font>,
    text: TextEngine,
}

impl Renderer {
    pub fn new(theme: Theme, fonts: Fonts) -> Self {
        let mut text_fonts = vec![fonts.text];
        text_fonts.extend(fonts.fallback);
        Self {
            theme,
            text_fonts,
            icon_fonts: vec![fonts.icon],
            text: TextEngine::new(),
        }
    }

    /// Renders one page of zones, left to right, into a new 640×48 frame.
    /// The `pressed` zone gets a lit background: the instant answer to a
    /// finger landing on something that acts.
    pub fn render_page(&mut self, zones: &[Zone], pressed: Option<usize>) -> Canvas {
        self.render_row(zones, pressed, WIDTH, self.theme.palette.background)
    }

    /// The theme it draws with, the user's accent included.
    pub fn theme(&self) -> &Theme {
        &self.theme
    }

    /// Renders a row of zones into a new frame `width` pixels wide: a page
    /// is the whole strip, a takeover over one end of it less.
    pub fn render_row(
        &mut self,
        zones: &[Zone],
        pressed: Option<usize>,
        width: u32,
        background: Rgb,
    ) -> Canvas {
        let mut canvas = Canvas::new(width, HEIGHT, background);
        let mut x = 0u32;
        for (i, zone) in zones.iter().enumerate() {
            if pressed == Some(i) {
                let w = zone.width.min(width.saturating_sub(x));
                canvas.fill_rect(x, 0, w, HEIGHT, self.theme.palette.pressed);
            }
            if i > 0 {
                let g = &self.theme.grid;
                canvas.fill_rect(
                    x,
                    g.divider_top,
                    1,
                    g.divider_bottom - g.divider_top,
                    self.theme.palette.divider,
                );
            }
            self.render_zone(&mut canvas, x as f32, zone.width as f32, &zone.view);
            x += zone.width;
        }
        canvas
    }

    /// The critical alarm as a plain takeover: the strip filled with the
    /// critical colour, the reading on the left and an OK key on the right,
    /// `ok_width` wide. A placeholder until the strip is redrawn.
    pub fn render_alarm(
        &mut self,
        label: &str,
        value: &str,
        unit: &str,
        ok_width: u32,
        ok_pressed: bool,
    ) -> Canvas {
        let zones = [
            Zone {
                width: WIDTH.saturating_sub(ok_width),
                view: WidgetView::Value {
                    label: label.to_string(),
                    value: value.to_string(),
                    unit: unit.to_string(),
                    icon: None,
                    graph: None,
                    severity: Severity::Ok,
                    second: None,
                },
            },
            Zone {
                width: ok_width,
                view: WidgetView::Key {
                    icon: None,
                    label: Some("OK".into()),
                },
            },
        ];
        let background = self.theme.palette.crit;
        self.render_row(&zones, ok_pressed.then_some(1), WIDTH, background)
    }

    fn render_zone(&mut self, canvas: &mut Canvas, x: f32, width: f32, view: &WidgetView) {
        match view {
            WidgetView::Clock {
                date,
                time,
                meridiem,
            } => {
                self.label(canvas, x, date);
                let unit = meridiem.as_deref().unwrap_or("");
                let left = x + self.theme.grid.pad_left;
                self.value_and_unit(canvas, left, time, unit, Severity::Ok);
            }
            WidgetView::Value {
                label,
                value,
                unit,
                icon,
                graph,
                severity,
                second,
            } => {
                if let Some(graph) = graph {
                    self.graph(canvas, x, width, graph);
                }
                self.label(canvas, x, label);
                // A second reading follows the first after a gap.
                const GAP: f32 = 8.0;
                let first = self.value_and_unit_width(value, unit);
                let both = first
                    + second
                        .as_ref()
                        .map_or(0.0, |s| GAP + self.value_and_unit_width(&s.value, &s.unit));
                let left = if graph.is_some() {
                    // With a graph the reading is centred so it reads as one
                    // block above the trend line.
                    x + ((width - both) / 2.0).round()
                } else {
                    let mut left = x + self.theme.grid.pad_left;
                    if let Some(icon) = icon {
                        left += self.icon(canvas, left, *icon, *severity) + 3.0;
                    }
                    left
                };
                self.value_and_unit(canvas, left, value, unit, *severity);
                if let Some(s) = second {
                    let at = left + first + GAP;
                    self.value_and_unit(canvas, at, &s.value, &s.unit, s.severity);
                }
            }
            WidgetView::Key { icon, label } => {
                // The icon set is not chosen yet: its name stands in for it
                // on the label line, and the label is the key's text.
                let room = width - 2.0 * self.theme.grid.pad_left;
                let ts = self.theme.type_scale.clone();
                let p = self.theme.palette.clone();
                let g = self.theme.grid.clone();
                let (small, big) = match (icon, label) {
                    (Some(i), Some(l)) => (Some(i.as_str()), l.as_str()),
                    (Some(i), None) => (None, i.as_str()),
                    (None, Some(l)) => (None, l.as_str()),
                    (None, None) => (None, ""),
                };
                if let Some(small) = small {
                    let small = self.fit(&ts.label, small, room);
                    self.label(canvas, x, &small);
                }
                let big = self.fit(&ts.value, big, room);
                let at = (x + g.pad_left, g.value_baseline);
                self.text
                    .draw(canvas, &self.text_fonts, &ts.value, &big, at, p.value);
            }
            WidgetView::Rate {
                label,
                caption,
                graph,
            } => {
                if let Some(graph) = graph {
                    self.graph(canvas, x, width, graph);
                }
                self.label(canvas, x, label);
                let (ts, g, p) = (
                    &self.theme.type_scale,
                    &self.theme.grid,
                    &self.theme.palette,
                );
                self.text.draw(
                    canvas,
                    &self.text_fonts,
                    &ts.caption,
                    caption,
                    (x + g.pad_left, g.caption_baseline),
                    p.value,
                );
            }
            WidgetView::Media(track) => self.media(canvas, x, width, track.as_ref()),
            WidgetView::Slider {
                label,
                value,
                fraction,
                active,
                muted,
            } => self.slider(canvas, x, width, label, value, *fraction, *active, *muted),
        }
    }

    /// Artist on the label line, title below it, and a progress bar along
    /// the bottom edge. Text that does not fit ends in an ellipsis.
    fn media(&mut self, canvas: &mut Canvas, x: f32, width: f32, track: Option<&Track>) {
        let g = &self.theme.grid;
        let room = width - g.pad_left - g.pad_left;
        let (left, label_y, caption_y) = (x + g.pad_left, g.label_baseline, g.caption_baseline);
        let ts = self.theme.type_scale.clone();
        let p = self.theme.palette.clone();
        let Some(track) = track else {
            self.label(canvas, x, "Media");
            let text = self.fit(&ts.caption, NOTHING_PLAYING, room);
            let at = (left, caption_y);
            self.text
                .draw(canvas, &self.text_fonts, &ts.caption, &text, at, p.unit);
            return;
        };
        let artist = self.fit(&ts.label, &track.artist, room);
        let at = (left, label_y);
        self.text
            .draw(canvas, &self.text_fonts, &ts.label, &artist, at, p.label);
        let title = self.fit(&ts.caption, &track.title, room);
        let color = if track.playing { p.value } else { p.unit };
        let at = (left, caption_y);
        self.text
            .draw(canvas, &self.text_fonts, &ts.caption, &title, at, color);

        if let Some(progress) = track.progress {
            // Two pixels high, inside the zone but clear of the divider.
            let (bar_x, bar_w) = (x as u32 + 1, width as u32 - 1);
            let y = HEIGHT - 2;
            canvas.fill_rect(bar_x, y, bar_w, 2, p.divider);
            let done = (bar_w as f32 * progress).round() as u32;
            canvas.fill_rect(
                bar_x,
                y,
                done,
                2,
                if track.playing { p.info } else { p.unit },
            );
        }
    }

    /// Label and readout above a bar along the bottom. While a finger is on
    /// it the bar thickens and grows a handle, so the slider visibly has
    /// hold of the touch.
    #[allow(clippy::too_many_arguments)]
    fn slider(
        &mut self,
        canvas: &mut Canvas,
        x: f32,
        width: f32,
        label: &str,
        value: &str,
        fraction: Option<f32>,
        active: bool,
        muted: bool,
    ) {
        let ts = self.theme.type_scale.clone();
        let p = self.theme.palette.clone();
        let g = self.theme.grid.clone();
        let room = width - 2.0 * g.pad_left;
        let label = self.fit(&ts.label, label, room);
        self.label(canvas, x, &label);
        let value = self.fit(&ts.caption, value, room);
        let at = (x + g.pad_left, g.caption_baseline);
        let color = if muted { p.unit } else { p.value };
        self.text
            .draw(canvas, &self.text_fonts, &ts.caption, &value, at, color);

        let left = (x + g.pad_left).round() as u32;
        let len = room.round().max(1.0) as u32;
        let (y, thick) = if active { (38, 5) } else { (39, 3) };
        canvas.fill_rect(left, y, len, thick, p.divider);
        if let Some(f) = fraction {
            let filled = (len as f32 * f).round() as u32;
            canvas.fill_rect(left, y, filled, thick, if muted { p.unit } else { p.info });
            if active {
                let handle = (left + filled).saturating_sub(1).min(left + len - 2);
                canvas.fill_rect(handle, 34, 3, 13, p.value);
            }
        }
    }

    /// `text`, shortened with an ellipsis if it is wider than `room`.
    fn fit(&mut self, style: &TextStyle, text: &str, room: f32) -> String {
        if self.text.measure(&self.text_fonts, style, text) <= room {
            return text.to_string();
        }
        // Longest prefix that fits with the ellipsis, found by halving.
        let ends: Vec<usize> = text.char_indices().map(|(i, _)| i).skip(1).collect();
        let (mut lo, mut hi) = (0, ends.len());
        while lo < hi {
            let mid = (lo + hi).div_ceil(2);
            let candidate = format!("{}…", text[..ends[mid - 1]].trim_end());
            if self.text.measure(&self.text_fonts, style, &candidate) <= room {
                lo = mid;
            } else {
                hi = mid - 1;
            }
        }
        match lo {
            0 => "…".to_string(),
            n => format!("{}…", text[..ends[n - 1]].trim_end()),
        }
    }

    fn label(&mut self, canvas: &mut Canvas, x: f32, text: &str) {
        let (ts, g, p) = (
            &self.theme.type_scale,
            &self.theme.grid,
            &self.theme.palette,
        );
        self.text.draw(
            canvas,
            &self.text_fonts,
            &ts.label,
            text,
            (x + g.pad_left, g.label_baseline),
            p.label,
        );
    }

    fn value_and_unit_width(&mut self, value: &str, unit: &str) -> f32 {
        let ts = &self.theme.type_scale;
        let mut w = self.text.measure(&self.text_fonts, &ts.value, value);
        if !unit.is_empty() {
            w += self.theme.grid.unit_gap + self.text.measure(&self.text_fonts, &ts.unit, unit);
        }
        w
    }

    fn value_and_unit(
        &mut self,
        canvas: &mut Canvas,
        left: f32,
        value: &str,
        unit: &str,
        severity: Severity,
    ) {
        let (ts, g, p) = (
            &self.theme.type_scale,
            &self.theme.grid,
            &self.theme.palette,
        );
        let color = severity_color(&self.theme, severity);
        let vw = self.text.draw(
            canvas,
            &self.text_fonts,
            &ts.value,
            value,
            (left, g.value_baseline),
            color,
        );
        if !unit.is_empty() {
            self.text.draw(
                canvas,
                &self.text_fonts,
                &ts.unit,
                unit,
                (left + vw + g.unit_gap, g.value_baseline),
                p.unit,
            );
        }
    }

    /// Draws an icon on the value baseline and returns its width.
    fn icon(&mut self, canvas: &mut Canvas, left: f32, icon: char, severity: Severity) -> f32 {
        let (ts, g) = (&self.theme.type_scale, &self.theme.grid);
        let color = severity_color(&self.theme, severity);
        let mut buf = [0u8; 4];
        self.text.draw(
            canvas,
            &self.icon_fonts,
            &ts.icon,
            icon.encode_utf8(&mut buf),
            (left, g.value_baseline),
            color,
        )
    }

    fn graph(&mut self, canvas: &mut Canvas, x: f32, width: f32, graph: &Graph) {
        let (values, filled) = match graph {
            Graph::Area(v) => (v, true),
            Graph::Line(v) => (v, false),
        };
        if values.len() < 2 {
            return;
        }
        let (g, p) = (&self.theme.grid, &self.theme.palette);
        let bottom = HEIGHT as f32 - 1.0;
        let band = bottom - g.graph_top;
        // Leave the divider column alone.
        let left = x + 1.0;
        let span = width - 1.0;
        let step = span / (values.len() - 1) as f32;
        let point = |i: usize, v: f32| (left + i as f32 * step, bottom - v.clamp(0.0, 1.0) * band);

        let mut line = PathBuilder::new();
        for (i, v) in values.iter().enumerate() {
            let (px, py) = point(i, *v);
            if i == 0 {
                line.move_to(px, py);
            } else {
                line.line_to(px, py);
            }
        }
        let Some(line) = line.finish() else { return };

        let pixmap = canvas.pixmap_mut();
        if filled {
            let mut area = PathBuilder::new();
            area.move_to(left, HEIGHT as f32);
            for (i, v) in values.iter().enumerate() {
                let (px, py) = point(i, *v);
                area.line_to(px, py);
            }
            area.line_to(left + span, HEIGHT as f32);
            area.close();
            if let Some(area) = area.finish() {
                pixmap.fill_path(
                    &area,
                    &paint(p.info_area),
                    FillRule::Winding,
                    Transform::identity(),
                    None,
                );
            }
        }
        let stroke = Stroke {
            width: 1.25,
            ..Stroke::default()
        };
        pixmap.stroke_path(&line, &paint(p.info), &stroke, Transform::identity(), None);
    }
}

fn severity_color(theme: &Theme, severity: Severity) -> Rgb {
    match severity {
        Severity::Ok => theme.palette.value,
        Severity::Warn => theme.palette.warn,
        Severity::Crit => theme.palette.crit,
    }
}

fn paint(color: Rgb) -> Paint<'static> {
    let mut paint = Paint::default();
    paint.set_color(to_color(color));
    paint.anti_alias = true;
    paint
}
