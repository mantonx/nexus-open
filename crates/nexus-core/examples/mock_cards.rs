//! Design mockup (U1, "better cards"): renders the six comparison scenes,
//! a widget specimen set and the redesigned default pages as 640×48
//! frames, with the repo's canvas, tiny-skia and the bundled fonts.
//!
//! Usage:
//!   cargo run -p nexus-core --example mock_cards -- <out-dir>
//!
//! This is a mockup: the scene data is hard-coded and nothing here is
//! wired to the shipped widgets, theme or config.

#[path = "mock_cards/cards.rs"]
mod cards;
#[path = "mock_cards/gfx.rs"]
mod gfx;

use std::path::{Path, PathBuf};

use cards::*;
use gfx::{Gfx, mix, st};
use nexus_core::canvas::Canvas;
use nexus_core::theme::Rgb;
use nexus_core::{HEIGHT, WIDTH};

const SCALE: u32 = 3;

/// One state of the world, shared by every page that renders it.
#[derive(Clone)]
struct World {
    cpu: f32,
    gpu: f32,
    cpu_load: f32,
    gpu_load: f32,
    mem: f32,
    down: f32,
    up: f32,
    weather: Avail,
    track: Option<Track>,
    volume: f32,
    /// History shape for the hot scenes.
    hot: bool,
}

const CPU_WARN: f32 = 167.0;
const CPU_CRIT: f32 = 194.0;
const GPU_WARN: f32 = 176.0;
const GPU_CRIT: f32 = 194.0;

fn idle() -> World {
    World {
        cpu: 115.0,
        gpu: 106.0,
        cpu_load: 6.0,
        gpu_load: 2.0,
        mem: 31.0,
        down: 12_000.0,
        up: 3_000.0,
        weather: Avail::Live,
        track: None,
        volume: 40.0,
        hot: false,
    }
}

fn weird_fishes(playing: bool) -> Track {
    Track {
        title: "Weird Fishes / Arpeggi",
        artist: "Radiohead",
        pos: 97,
        len: 318,
        playing,
    }
}

fn music() -> World {
    World {
        track: Some(weird_fishes(true)),
        ..idle()
    }
}

fn hot() -> World {
    World {
        cpu: 172.0,
        gpu: 160.0,
        cpu_load: 62.0,
        gpu_load: 97.0,
        mem: 54.0,
        hot: true,
        ..music()
    }
}

fn critical() -> World {
    World {
        gpu: 199.0,
        ..hot()
    }
}

/// A plausible ten-minute history ending at `now`: flat and a little noisy
/// at idle, a climb when a game started.
fn hist(now: f32, hot: bool, seed: u32) -> Vec<f32> {
    let n = 40;
    (0..n)
        .map(|i| {
            let t = i as f32 / (n - 1) as f32;
            let wob = (((i as u32 * 7 + seed * 13) % 11) as f32 - 5.0) * 0.6;
            let base = if hot {
                // Idle until a third of the way in, then up to `now`.
                let start = now * 0.66;
                let k = ((t - 0.3) / 0.25).clamp(0.0, 1.0);
                start + (now - start) * (k * k * (3.0 - 2.0 * k))
            } else {
                now - 3.0 + 3.0 * (t * 9.0).sin().abs()
            };
            if i == n - 1 { now } else { base + wob }
        })
        .collect()
}

fn net_hist(now: f32, seed: u32) -> Vec<f32> {
    (0..40)
        .map(|i: u32| {
            let t = i as f32 + seed as f32 * 3.0;
            let k = 0.5 + 0.5 * (t * 0.45).sin() * (t * 0.13).cos();
            if i == 39 { now } else { now * (0.4 + 1.6 * k) }
        })
        .collect()
}

fn temp(
    name: &'static str,
    icon: Ico,
    v: f32,
    warn: f32,
    crit: f32,
    hot: bool,
    seed: u32,
) -> Reading {
    Reading {
        icon,
        name,
        value: v,
        unit: "°F",
        lo: 77.0,
        hi: 212.0,
        warn: Some(warn),
        crit: Some(crit),
        hist: hist(v, hot, seed),
        avail: Avail::Live,
        load: None,
    }
}

fn load(name: &'static str, icon: Ico, v: f32, hot: bool, seed: u32) -> Reading {
    Reading {
        icon,
        name,
        value: v,
        unit: "%",
        lo: 0.0,
        hi: 100.0,
        warn: None,
        crit: None,
        hist: hist(v, hot, seed)
            .into_iter()
            .map(|x| x.clamp(0.0, 100.0))
            .collect(),
        avail: Avail::Live,
        load: None,
    }
}

impl World {
    fn clock(&self) -> W {
        W::Clock {
            hh_mm: "10:53",
            weekday: "Sunday",
            day: 22,
            month: "November",
            minutes: 10 * 60 + 53,
        }
    }
    fn weather(&self) -> W {
        W::Weather {
            temp: 58.0,
            cond: "Cloudy",
            avail: self.weather,
        }
    }
    fn cpu_temp(&self) -> Reading {
        temp("CPU", I_CPU, self.cpu, CPU_WARN, CPU_CRIT, self.hot, 1)
    }
    fn gpu_temp(&self) -> Reading {
        temp("GPU", Ico::Gpu, self.gpu, GPU_WARN, GPU_CRIT, self.hot, 2)
    }
    fn cpu(&self) -> W {
        W::Value(self.cpu_temp())
    }
    fn gpu(&self) -> W {
        W::Value(self.gpu_temp())
    }
    /// Temperature with load beside it (the optional `load` key).
    fn cpu_dual(&self) -> W {
        W::Value(Reading {
            load: Some(self.cpu_load),
            ..self.cpu_temp()
        })
    }
    fn gpu_dual(&self) -> W {
        W::Value(Reading {
            load: Some(self.gpu_load),
            ..self.gpu_temp()
        })
    }
    fn mem(&self) -> W {
        W::Value(load("Memory", I_MEM, self.mem, self.hot, 3))
    }
    fn net(&self) -> W {
        W::Rate {
            down: self.down,
            up: self.up,
            hist_down: net_hist(self.down, 1),
            hist_up: net_hist(self.up, 2),
            avail: Avail::Live,
        }
    }
    fn media(&self) -> W {
        W::Media(self.track.clone())
    }
    fn volume(&self) -> W {
        W::Slider {
            level: self.volume,
            was: None,
            active: false,
            muted: false,
        }
    }
}

// ---- The redesigned default layout -------------------------------------

const PAGES: [&str; 4] = ["Home", "Music", "System", "Clock"];

fn page(w: &World, n: usize) -> Vec<Card> {
    match n {
        // Home: the time, the sky, the two temperatures that matter and
        // what is playing. Glanceable from across the desk.
        0 => vec![
            card(140, w.clock()),
            card(92, w.weather()),
            card(96, w.cpu()),
            card(96, w.gpu()),
            card(204, w.media()),
        ],
        // Music: the player at hero size with transport keys, and volume
        // as a slider you can grab.
        1 => vec![card(420, w.media()), card(217, w.volume())],
        // System: temperatures with load beside them, memory, network.
        2 => vec![
            card(218, w.cpu_dual()),
            card(218, w.gpu_dual()),
            card(98, w.mem()),
            card(97, w.net()),
        ],
        // Clock: a face, not a stretched clock.
        _ => vec![card(640, w.clock())],
    }
}

/// Worst severity on a page, for its tick on the rail.
fn page_sev(cards: &[Card]) -> Sev {
    let mut worst = Sev::Ok;
    for c in cards {
        if let W::Value(r) = &c.widget {
            match r.sev() {
                Sev::Crit => return Sev::Crit,
                Sev::Warn => worst = Sev::Warn,
                Sev::Ok => {}
            }
        }
    }
    worst
}

fn draw_cards(g: &mut Gfx, row: &[Card]) {
    let mut x = 0u32;
    for c in row {
        cards::draw(g, x as f32, c.w as f32, &c.widget);
        x += c.w + GAP;
    }
}

/// Page rail: one short segment per page across the top edge, the current
/// one lit. A page with a reading past its threshold shows it on its own
/// segment, so an alarm on another page is never invisible.
fn rail(g: &mut Gfx, current: usize, sevs: &[Sev]) {
    let (seg, gap) = (26.0, 5.0);
    let n = sevs.len() as f32;
    let total = n * seg + (n - 1.0) * gap;
    let x0 = ((WIDTH as f32 - total) / 2.0).round();
    for (i, s) in sevs.iter().enumerate() {
        let x = x0 + i as f32 * (seg + gap);
        let c = match (s, i == current) {
            (Sev::Crit, _) => CRIT,
            (Sev::Warn, _) => WARN,
            (Sev::Ok, true) => Rgb::hex(0xd4d9e0),
            (Sev::Ok, false) => Rgb::hex(0x3a414b),
        };
        let h = if i == current { 3.0 } else { 2.0 };
        g.rect(x, 0.0, seg, h, c);
    }
}

fn render_page(g: &mut Gfx, w: &World, n: usize) -> Canvas {
    g.begin(WIDTH, HEIGHT, GROUND);
    let row = page(w, n);
    draw_cards(g, &row);
    let sevs: Vec<Sev> = (0..PAGES.len()).map(|i| page_sev(&page(w, i))).collect();
    rail(g, n, &sevs);
    g.c.clone()
}

/// The volume takeover, made a card: it slides over the right of the page
/// (here 300 px) instead of blanking the whole strip, so the clock and
/// temperatures stay where the eye left them.
fn volume_overlay(g: &mut Gfx, was: f32, now: f32) {
    // Covers the GPU and media cards exactly, so no sliver peeks out.
    let x = (140 + 92 + 96 + 3 * GAP) as f32;
    let w = WIDTH as f32 - x;
    g.rect(x - GAP as f32, 4.0, w + GAP as f32, HEIGHT as f32, GROUND);
    slider(g, x, w, now, Some(was), false, false, Rgb::hex(0x171b21));
    g.rrect_line(
        x,
        CARD_Y,
        w,
        CARD_H + 6.0,
        RADIUS,
        mix(ACCENT, GROUND, 0.45),
        false,
    );
}

// ---- Specimens ---------------------------------------------------------------

/// A specimen frame: the same widget at compact, medium and hero widths.
fn specimen(g: &mut Gfx, widget: &W) -> Canvas {
    g.begin(WIDTH, HEIGHT, GROUND);
    let ws = [98u32, 200, 336];
    let mut x = 0;
    for w in ws {
        cards::draw(g, x as f32, w as f32, widget);
        x += w + GAP;
    }
    g.c.clone()
}

fn specimens(g: &mut Gfx) -> Vec<(String, Canvas)> {
    let i = idle();
    let h = hot();
    let c = critical();
    let mut out = Vec::new();
    let mut add = |name: &str, canvas: Canvas| out.push((format!("x-specimen-{name}"), canvas));

    add("clock", specimen(g, &i.clock()));
    add("weather", specimen(g, &i.weather()));
    add(
        "weather-no-data",
        specimen(
            g,
            &W::Weather {
                temp: 0.0,
                cond: "",
                avail: Avail::NoData,
            },
        ),
    );
    add(
        "weather-not-set-up",
        specimen(
            g,
            &W::Weather {
                temp: 0.0,
                cond: "",
                avail: Avail::NotSetUp,
            },
        ),
    );
    add("temp", specimen(g, &i.cpu()));
    add("temp-warn", specimen(g, &h.cpu()));
    add("temp-crit", specimen(g, &c.gpu()));
    let mut lost = i.cpu_temp();
    lost.avail = Avail::NoData;
    add("temp-no-data", specimen(g, &W::Value(lost)));
    add("temp-load", specimen(g, &h.gpu_dual()));
    add("temp-load-warn", specimen(g, &h.cpu_dual()));
    add("temp-load-crit", specimen(g, &c.gpu_dual()));
    add("load", specimen(g, &h.mem()));
    add("rate", specimen(g, &i.net()));
    add(
        "rate-no-data",
        specimen(
            g,
            &W::Rate {
                down: 0.0,
                up: 0.0,
                hist_down: vec![],
                hist_up: vec![],
                avail: Avail::NoData,
            },
        ),
    );
    add(
        "media-playing",
        specimen(g, &W::Media(Some(weird_fishes(true)))),
    );
    add(
        "media-paused",
        specimen(g, &W::Media(Some(weird_fishes(false)))),
    );
    add("media-nothing", specimen(g, &W::Media(None)));
    add(
        "slider",
        specimen(
            g,
            &W::Slider {
                level: 40.0,
                was: None,
                active: false,
                muted: false,
            },
        ),
    );
    add(
        "slider-dragging",
        specimen(
            g,
            &W::Slider {
                level: 55.0,
                was: Some(40.0),
                active: true,
                muted: false,
            },
        ),
    );
    add(
        "slider-muted",
        specimen(
            g,
            &W::Slider {
                level: 40.0,
                was: None,
                active: false,
                muted: true,
            },
        ),
    );
    out
}

// ---- Output ------------------------------------------------------------------

fn save(dir: &Path, name: &str, c: &Canvas) {
    assert_eq!((c.width(), c.height()), (WIDTH, HEIGHT));
    c.save_png(dir.join(format!("{name}.png")))
        .expect("save frame");
}

/// Stacks frames scaled 3× (nearest neighbour), each under a caption.
fn sheet(g: &mut Gfx, frames: &[(String, Canvas)], path: &Path) {
    let cap_h = 26u32;
    let gap = 10u32;
    let row = cap_h + HEIGHT * SCALE + gap;
    let (w, h) = (WIDTH * SCALE, row * frames.len() as u32 + gap);
    g.begin(w, h, Rgb::hex(0x1c1f24));
    for (i, (name, frame)) in frames.iter().enumerate() {
        let top = gap + i as u32 * row;
        g.text(
            &st(17.0, 500.0, 100.0),
            name,
            4.0,
            (top + 18) as f32,
            Rgb::hex(0x9aa1ab),
        );
        let src = frame.rgba();
        let mut img = Canvas::new(WIDTH * SCALE, HEIGHT * SCALE, GROUND);
        let data = img.pixmap_mut().data_mut();
        for y in 0..HEIGHT * SCALE {
            for x in 0..WIDTH * SCALE {
                let s = (((y / SCALE) * WIDTH + x / SCALE) * 4) as usize;
                let d = ((y * WIDTH * SCALE + x) * 4) as usize;
                data[d..d + 4].copy_from_slice(&src[s..s + 4]);
            }
        }
        g.c.copy_rows_from(&img, top + cap_h);
    }
    g.c.save_png(path).expect("save sheet");
}

trait CopyRows {
    fn copy_rows_from(&mut self, src: &Canvas, top: u32);
}

impl CopyRows for Canvas {
    /// Copies `src` (same width) into this canvas starting at row `top`.
    fn copy_rows_from(&mut self, src: &Canvas, top: u32) {
        let stride = (self.width() * 4) as usize;
        let start = top as usize * stride;
        let bytes = src.rgba();
        let data = self.pixmap_mut().data_mut();
        data[start..start + bytes.len()].copy_from_slice(bytes);
    }
}

fn main() {
    let Some(out) = std::env::args().nth(1).map(PathBuf::from) else {
        eprintln!("usage: mock_cards <out-dir>");
        std::process::exit(2);
    };
    std::fs::create_dir_all(&out).expect("create output directory");
    let mut g = Gfx::new(WIDTH, HEIGHT, GROUND);

    // The six scenes.
    let mut scenes: Vec<(String, Canvas)> = vec![
        ("1-idle".into(), render_page(&mut g, &idle(), 0)),
        ("2-music".into(), render_page(&mut g, &music(), 1)),
        ("3-hot".into(), render_page(&mut g, &hot(), 2)),
        ("4-critical".into(), render_page(&mut g, &critical(), 2)),
    ];
    let mut vol = music();
    vol.volume = 55.0;
    render_page(&mut g, &vol, 0);
    volume_overlay(&mut g, 40.0, 55.0);
    scenes.push(("5-volume".into(), g.c.clone()));
    let first = World {
        weather: Avail::NotSetUp,
        ..idle()
    };
    scenes.push(("6-first-run".into(), render_page(&mut g, &first, 0)));

    // Extras: the default pages, the alternative scene placements, the
    // specimens.
    let mut extras: Vec<(String, Canvas)> = Vec::new();
    for (n, title) in PAGES.iter().enumerate() {
        let name = format!("x-default-p{}-{}", n + 1, title.to_lowercase());
        extras.push((name, render_page(&mut g, &music(), n)));
    }
    extras.push(("x-3-hot-on-home".into(), render_page(&mut g, &hot(), 0)));
    extras.push((
        "x-4-critical-on-home".into(),
        render_page(&mut g, &critical(), 0),
    ));
    extras.push((
        "x-4-critical-on-music".into(),
        render_page(&mut g, &critical(), 1),
    ));
    let mut drag = music();
    drag.volume = 55.0;
    render_page(&mut g, &drag, 1);
    extras.push(("x-5-volume-on-music".into(), {
        let x = 420 + GAP;
        cards::slider(
            &mut g,
            x as f32,
            217.0,
            55.0,
            Some(40.0),
            false,
            false,
            CARD,
        );
        g.c.clone()
    }));
    extras.extend(specimens(&mut g));

    for (name, c) in scenes.iter().chain(extras.iter()) {
        save(&out, name, c);
    }
    sheet(&mut g, &scenes, &out.join("sheet.png"));
    sheet(&mut g, &extras, &out.join("sheet-specimens.png"));
    println!(
        "wrote {} scenes and {} extras to {}",
        scenes.len(),
        extras.len(),
        out.display()
    );
}
