//! The strip's design system (U2), proven in frames: the six comparison
//! scenes, the default layout's pages, the takeovers, every widget at every
//! size and state, the accent alternatives, and contact sheets.
//!
//! Usage:
//!   cargo run -p nexus-core --example design_system -- <out-dir>
//!
//! A mockup: data is hard-coded, and nothing here is wired to the shipped
//! widgets, theme or config. Every size and colour lives in
//! `design_system/tokens.rs`.

#[path = "design_system/gfx.rs"]
mod gfx;
#[path = "design_system/tokens.rs"]
mod tokens;
#[path = "design_system/widgets.rs"]
mod widgets;

use std::path::{Path, PathBuf};

use gfx::{Gfx, st};
use nexus_core::canvas::Canvas;
use nexus_core::theme::Rgb;
use nexus_core::{HEIGHT, WIDTH};
use tokens::*;
use widgets::*;

// ---------------------------------------------------------------- the world

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
}

const CPU_WARN: f32 = 167.0;
const CPU_CRIT: f32 = 194.0;
/// Assumed: the brief gives only the GPU's critical limit.
const GPU_WARN: f32 = 176.0;
const GPU_CRIT: f32 = 194.0;
/// Temperatures share one fixed scale: 86–212 °F (30–100 °C).
const TEMP_LO: f32 = 86.0;
const TEMP_HI: f32 = 212.0;
/// Memory has default limits; load has none unless the user sets them.
const MEM_WARN: f32 = 85.0;
const MEM_CRIT: f32 = 95.0;
/// Network: logarithmic, 1 kB/s to 1 GB/s (values in bytes per second).
const NET_LO: f32 = 1_000.0;
const NET_HI: f32 = 1_000_000_000.0;

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
        ..music()
    }
}

fn critical() -> World {
    World {
        gpu: 199.0,
        ..hot()
    }
}

fn first_run() -> World {
    World {
        weather: Avail::NotSetUp,
        ..idle()
    }
}

fn temp(name: &'static str, icon: char, v: f32, warn: f32, crit: f32) -> Reading {
    Reading {
        icon,
        name,
        value: v,
        text: format!("{}°", v.round() as i32),
        unit: "",
        scale: Scale {
            lo: TEMP_LO,
            hi: TEMP_HI,
            warn: Some(warn),
            crit: Some(crit),
            log: false,
        },
        avail: Avail::Live,
        limit_words: Some(format!("limit {}°", warn.round() as i32)),
    }
}

fn percent(
    name: &'static str,
    icon: char,
    v: f32,
    warn: Option<f32>,
    crit: Option<f32>,
) -> Reading {
    Reading {
        icon,
        name,
        value: v,
        text: format!("{}", v.round() as i32),
        unit: "%",
        scale: Scale {
            lo: 0.0,
            hi: 100.0,
            warn,
            crit,
            log: false,
        },
        avail: Avail::Live,
        limit_words: warn.map(|w| format!("limit {}%", w.round() as i32)),
    }
}

fn rate(name: &'static str, icon: char, bps: f32) -> Reading {
    // At most three figures: 12, 480, 1.3, 48.
    let (text, unit) = if bps >= 10_000_000.0 {
        (format!("{}", (bps / 1_000_000.0).round() as i32), "MB/s")
    } else if bps >= 1_000_000.0 {
        (format!("{:.1}", bps / 1_000_000.0), "MB/s")
    } else {
        (format!("{}", (bps / 1_000.0).round() as i32), "kB/s")
    };
    Reading {
        icon,
        name,
        value: bps,
        text,
        unit,
        scale: Scale {
            lo: NET_LO,
            hi: NET_HI,
            warn: None,
            crit: None,
            log: true,
        },
        avail: Avail::Live,
        limit_words: None,
    }
}

impl World {
    fn clock(&self) -> W {
        W::Clock {
            hh_mm: "10:53",
            weekday: "Sunday",
            wd_short: "Sun",
            date: "November 22",
            date_short: "Nov 22",
        }
    }
    fn weather(&self) -> W {
        W::Weather {
            temp: "58°",
            cond: "Cloudy",
            hi_lo: "61° / 49°",
            icon: ico::CLOUD,
            avail: self.weather,
        }
    }
    fn cpu_r(&self) -> Reading {
        temp("CPU", ico::CPU, self.cpu, CPU_WARN, CPU_CRIT)
    }
    fn gpu_r(&self) -> Reading {
        temp("GPU", ico::GPU, self.gpu, GPU_WARN, GPU_CRIT)
    }
    fn cpu(&self) -> W {
        W::Value {
            r: self.cpu_r(),
            second: None,
        }
    }
    fn gpu(&self) -> W {
        W::Value {
            r: self.gpu_r(),
            second: None,
        }
    }
    fn cpu_with_load(&self) -> W {
        W::Value {
            r: self.cpu_r(),
            second: Some(percent("load", ico::GAUGE, self.cpu_load, None, None)),
        }
    }
    fn gpu_with_load(&self) -> W {
        W::Value {
            r: self.gpu_r(),
            second: Some(percent("load", ico::GAUGE, self.gpu_load, None, None)),
        }
    }
    fn memory(&self) -> W {
        W::Value {
            r: percent(
                "Memory",
                ico::MEMORY,
                self.mem,
                Some(MEM_WARN),
                Some(MEM_CRIT),
            ),
            second: None,
        }
    }
    fn network(&self) -> W {
        W::Network {
            down: rate("Down", ico::ARROW_DOWN, self.down),
            up: rate("Up", ico::ARROW_UP, self.up),
        }
    }
    fn media(&self) -> W {
        W::Media {
            track: self.track.clone(),
            pressed: None,
        }
    }
    fn volume(&self) -> W {
        W::Slider {
            value: self.volume,
            muted: false,
            held: false,
        }
    }
}

// ---------------------------------------------------------------- pages

struct Page {
    name: &'static str,
    widgets: Vec<(f32, W)>,
}

fn launch_key(icon: char, label: &'static str) -> W {
    W::Key {
        icon,
        label,
        sub: "",
        state: KeyState::Rest,
    }
}

/// The default layout: five pages, none a stretched copy of another.
fn pages(w: &World) -> Vec<Page> {
    vec![
        Page {
            name: "home",
            widgets: vec![
                (192.0, w.clock()),
                (120.0, w.weather()),
                (120.0, w.cpu()),
                (192.0, w.media()),
            ],
        },
        Page {
            name: "music",
            widgets: vec![(424.0, w.media()), (208.0, w.volume())],
        },
        Page {
            name: "system",
            widgets: vec![
                (200.0, w.cpu_with_load()),
                (200.0, w.gpu_with_load()),
                (104.0, w.memory()),
                (120.0, w.network()),
            ],
        },
        Page {
            name: "launch",
            widgets: vec![
                (124.0, w.clock()),
                (124.0, launch_key(ico::GLOBE, "Browser")),
                (124.0, launch_key(ico::FOLDER, "Files")),
                (124.0, launch_key(ico::GEAR, "Settings")),
                (124.0, launch_key(ico::GAME, "Games")),
            ],
        },
        Page {
            name: "desk",
            widgets: vec![(328.0, w.clock()), (304.0, w.weather())],
        },
    ]
}

/// The worst severity among a page's readings.
fn page_sev(p: &Page) -> Sev {
    let mut worst = Sev::Normal;
    for (_, wd) in &p.widgets {
        let s = match wd {
            W::Value { r, second } => {
                let a = r.sev();
                let b = second.as_ref().map_or(Sev::Normal, Reading::sev);
                if a == Sev::Crit || b == Sev::Crit {
                    Sev::Crit
                } else if a == Sev::Warn || b == Sev::Warn {
                    Sev::Warn
                } else {
                    Sev::Normal
                }
            }
            _ => Sev::Normal,
        };
        if s == Sev::Crit || (s == Sev::Warn && worst == Sev::Normal) {
            worst = s;
        }
    }
    worst
}

/// Left edges of a row of cards of these widths; panics if they don't fill
/// the strip exactly.
fn positions(widths: &[f32]) -> Vec<f32> {
    let total: f32 =
        widths.iter().sum::<f32>() + GUTTER * (widths.len().saturating_sub(1)) as f32 + 2.0 * EDGE;
    assert!(
        (total - STRIP_W).abs() < 0.5,
        "widths {widths:?} fill {total}, not {STRIP_W}"
    );
    let mut x = EDGE;
    widths
        .iter()
        .map(|w| {
            let at = x;
            x += w + GUTTER;
            at
        })
        .collect()
}

fn draw_row(g: &mut Gfx, a: &Accent, row: &[(f32, W)]) {
    let widths: Vec<f32> = row.iter().map(|(w, _)| *w).collect();
    for ((w, wd), x) in row.iter().zip(positions(&widths)) {
        widgets::draw(g, a, x, *w, wd);
    }
}

fn draw_page(g: &mut Gfx, a: &Accent, all: &[Page], current: usize) {
    g.begin(WIDTH, HEIGHT, GROUND);
    let ms: Vec<Marker> = all
        .iter()
        .enumerate()
        .map(|(i, p)| Marker {
            current: i == current,
            sev: page_sev(p),
        })
        .collect();
    markers(g, &ms);
    draw_row(g, a, &all[current].widgets);
}

fn page_index(all: &[Page], name: &str) -> usize {
    all.iter()
        .position(|p| p.name == name)
        .expect("page exists")
}

// ---------------------------------------------------------------- output

struct Out {
    dir: PathBuf,
    groups: Vec<(&'static str, Vec<(String, Canvas)>)>,
}

impl Out {
    fn save(&mut self, group: &'static str, name: &str, c: &Canvas) {
        assert_eq!(
            (c.width(), c.height()),
            (WIDTH, HEIGHT),
            "{name} is not 640×48"
        );
        c.save_png(self.dir.join(format!("{name}.png")))
            .expect("write frame");
        if let Some(gr) = self.groups.iter_mut().find(|(n, _)| *n == group) {
            gr.1.push((name.to_string(), c.clone()));
        } else {
            self.groups
                .push((group, vec![(name.to_string(), c.clone())]));
        }
    }
}

/// The volume takeover on Home covers the CPU and media cards.
const HOME_VOLUME_X: f32 = EDGE + 192.0 + GUTTER + 120.0 + GUTTER;

const SHEET_SCALE: u32 = 3;
const SHEET_CAPTION: u32 = 34;
const SHEET_GAP: u32 = 10;

/// Stacks frames at 3×, nearest-neighbour, each under its caption.
fn sheet(g: &mut Gfx, frames: &[(String, Canvas)], path: &Path) {
    let fw = WIDTH * SHEET_SCALE;
    let fh = HEIGHT * SHEET_SCALE;
    let row = SHEET_CAPTION + fh + SHEET_GAP;
    let bg = Rgb::hex(0x0b0908);
    let mut out = Canvas::new(fw, row * frames.len() as u32, bg);
    for (i, (name, c)) in frames.iter().enumerate() {
        let y0 = i as u32 * row;
        // Caption, drawn on its own canvas and copied in.
        g.begin(fw, SHEET_CAPTION, bg);
        g.text(&st(22.0, 500.0, 0.3), name, 6.0, 25.0, Rgb::hex(0xb7a895));
        blit(&mut out, &g.c, 0, y0, 1);
        blit(&mut out, c, 0, y0 + SHEET_CAPTION, SHEET_SCALE);
    }
    out.save_png(path).expect("write sheet");
}

fn blit(dst: &mut Canvas, src: &Canvas, x0: u32, y0: u32, scale: u32) {
    let w = src.width();
    let px = src.rgba().to_vec();
    for y in 0..src.height() {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let c = Rgb {
                r: px[i],
                g: px[i + 1],
                b: px[i + 2],
            };
            dst.fill_rect(x0 + x * scale, y0 + y * scale, scale, scale, c);
        }
    }
}

// ---------------------------------------------------------------- frames

fn scenes(
    g: &mut Gfx,
    a: &Accent,
    out: &mut Out,
    prefix: &str,
    group: &'static str,
    only: Option<&[&str]>,
) {
    let want = |n: &str| only.is_none_or(|o| o.contains(&n));
    // 1 idle, on Home.
    if want("1-idle") {
        let w = idle();
        let ps = pages(&w);
        draw_page(g, a, &ps, page_index(&ps, "home"));
        out.save(group, &format!("{prefix}1-idle"), &g.c);
    }
    // 2 music, on the Music page.
    if want("2-music") {
        let w = music();
        let ps = pages(&w);
        draw_page(g, a, &ps, page_index(&ps, "music"));
        out.save(group, &format!("{prefix}2-music"), &g.c);
    }
    // 3 hot, on System.
    if want("3-hot") {
        let w = hot();
        let ps = pages(&w);
        draw_page(g, a, &ps, page_index(&ps, "system"));
        out.save(group, &format!("{prefix}3-hot"), &g.c);
    }
    // 4 critical: the alarm covers the strip.
    if want("4-critical") {
        g.begin(WIDTH, HEIGHT, GROUND);
        alarm(
            g,
            a,
            ico::GPU,
            "GPU is too hot",
            "Over its 194° limit · check the fans",
            "199°",
            false,
        );
        out.save(group, &format!("{prefix}4-critical"), &g.c);
    }
    // 5 volume: 40 → 55 on Home, a card over the right of the page.
    if want("5-volume") {
        let w = World {
            volume: 55.0,
            ..music()
        };
        let ps = pages(&w);
        draw_page(g, a, &ps, page_index(&ps, "home"));
        let x = HOME_VOLUME_X;
        volume_takeover(g, a, x, STRIP_W - EDGE - x, 40.0, 55.0, false);
        out.save(group, &format!("{prefix}5-volume"), &g.c);
    }
    // 6 first run: no weather location, nothing playing.
    if want("6-first-run") {
        let w = first_run();
        let ps = pages(&w);
        draw_page(g, a, &ps, page_index(&ps, "home"));
        out.save(group, &format!("{prefix}6-first-run"), &g.c);
    }
    // For the accent frames: accent controls right beside warning and
    // critical cards.
    if want("severity") {
        let w = critical();
        g.begin(WIDTH, HEIGHT, GROUND);
        let ms = [
            Marker {
                current: true,
                sev: Sev::Crit,
            },
            Marker {
                current: false,
                sev: Sev::Normal,
            },
            Marker {
                current: false,
                sev: Sev::Crit,
            },
            Marker {
                current: false,
                sev: Sev::Normal,
            },
            Marker {
                current: false,
                sev: Sev::Normal,
            },
        ];
        markers(g, &ms);
        draw_row(
            g,
            a,
            &[
                (120.0, w.cpu()),
                (120.0, w.gpu()),
                (256.0, w.media()),
                (128.0, w.volume()),
            ],
        );
        out.save(group, &format!("{prefix}severity"), &g.c);
    }
}

fn default_pages(g: &mut Gfx, a: &Accent, out: &mut Out) {
    let w = music();
    let ps = pages(&w);
    for i in 0..ps.len() {
        draw_page(g, a, &ps, i);
        out.save(
            "default",
            &format!("default-p{}-{}", i + 1, ps[i].name),
            &g.c,
        );
    }
    // At night the whole frame dims; nothing is redrawn smaller.
    let wn = World {
        track: None,
        ..idle()
    };
    let ps = pages(&wn);
    draw_page(g, a, &ps, page_index(&ps, "desk"));
    dim(&mut g.c, NIGHT_DIM);
    out.save("default", "default-night-desk", &g.c);
}

/// Scales every pixel's brightness, as the panel's backlight would.
fn dim(c: &mut Canvas, k: f32) {
    let px = c.rgba().to_vec();
    let w = c.width();
    for y in 0..c.height() {
        for x in 0..w {
            let i = ((y * w + x) * 4) as usize;
            let f = |v: u8| (f32::from(v) * k).round() as u8;
            let col = Rgb {
                r: f(px[i]),
                g: f(px[i + 1]),
                b: f(px[i + 2]),
            };
            c.fill_rect(x, y, 1, 1, col);
        }
    }
}

fn takeovers(g: &mut Gfx, a: &Accent, out: &mut Out) {
    // Now playing, over Home, when the track changes.
    let w = music();
    let ps = pages(&w);
    draw_page(g, a, &ps, page_index(&ps, "home"));
    now_playing(g, a, &weird_fishes(true), None);
    out.save("takeover", "takeover-now-playing", &g.c);

    draw_page(g, a, &ps, page_index(&ps, "home"));
    now_playing(g, a, &weird_fishes(false), Some(1));
    out.save("takeover", "takeover-now-playing-paused-pressed", &g.c);

    // Volume, right-aligned (default) and left-aligned, held.
    let wv = World {
        volume: 55.0,
        ..music()
    };
    let ps = pages(&wv);
    draw_page(g, a, &ps, page_index(&ps, "home"));
    let x = HOME_VOLUME_X;
    volume_takeover(g, a, x, STRIP_W - EDGE - x, 40.0, 55.0, false);
    out.save("takeover", "takeover-volume-right", &g.c);

    draw_page(g, a, &ps, page_index(&ps, "home"));
    let wl = 192.0 + GUTTER + 120.0;
    volume_takeover(g, a, EDGE, wl, 40.0, 55.0, true);
    out.save("takeover", "takeover-volume-left-held", &g.c);

    // The alarm, and the OK key under a finger.
    g.begin(WIDTH, HEIGHT, GROUND);
    alarm(
        g,
        a,
        ico::GPU,
        "GPU is too hot",
        "Over its 194° limit · check the fans",
        "199°",
        false,
    );
    out.save("takeover", "takeover-alarm", &g.c);
    g.begin(WIDTH, HEIGHT, GROUND);
    alarm(
        g,
        a,
        ico::GPU,
        "GPU is too hot",
        "Over its 194° limit · check the fans",
        "199°",
        true,
    );
    out.save("takeover", "takeover-alarm-ok-pressed", &g.c);

    // After OK: the page returns; the GPU card stays critical, its page
    // markers stay red.
    let wc = critical();
    let ps = pages(&wc);
    draw_page(g, a, &ps, page_index(&ps, "system"));
    out.save("takeover", "takeover-after-ok-system", &g.c);
    draw_page(g, a, &ps, page_index(&ps, "home"));
    out.save("takeover", "takeover-after-ok-home", &g.c);
}

/// Specimen widths: compact, medium, hero.
const SPEC_W: [f32; 3] = [124.0, 196.0, 308.0];

fn specimen(g: &mut Gfx, a: &Accent, out: &mut Out, name: &str, make: impl Fn(Class) -> W) {
    g.begin(WIDTH, HEIGHT, GROUND);
    let row: Vec<(f32, W)> = SPEC_W
        .iter()
        .map(|w| (*w, make(widgets::class(*w))))
        .collect();
    draw_row(g, a, &row);
    out.save("specimen", &format!("specimen-{name}"), &g.c);
}

fn specimens(g: &mut Gfx, a: &Accent, out: &mut Out) {
    let w = idle();
    specimen(g, a, out, "clock", |_| w.clock());

    // Temperature (a value with limits).
    let t = |v: f32, avail: Avail| W::Value {
        r: Reading {
            avail,
            ..temp("CPU", ico::CPU, v, CPU_WARN, CPU_CRIT)
        },
        second: None,
    };
    specimen(g, a, out, "value-normal", |_| t(115.0, Avail::Live));
    specimen(g, a, out, "value-warning", |_| t(172.0, Avail::Live));
    specimen(g, a, out, "value-critical", |_| t(199.0, Avail::Live));
    specimen(g, a, out, "value-no-data", |_| t(0.0, Avail::NoData));
    specimen(g, a, out, "value-lost", |_| t(115.0, Avail::Lost("10:41")));
    specimen(g, a, out, "value-not-set-up", |_| t(0.0, Avail::NotSetUp));

    // A value with a second reading beside it.
    let t2 = |v: f32, load: f32, avail: Avail| W::Value {
        r: Reading {
            avail,
            ..temp("GPU", ico::GPU, v, GPU_WARN, GPU_CRIT)
        },
        second: Some(Reading {
            avail,
            ..percent("load", ico::GAUGE, load, None, None)
        }),
    };
    specimen(g, a, out, "value2-normal", |_| t2(106.0, 2.0, Avail::Live));
    specimen(g, a, out, "value2-warning", |_| {
        t2(181.0, 97.0, Avail::Live)
    });
    specimen(g, a, out, "value2-critical", |_| {
        t2(199.0, 97.0, Avail::Live)
    });
    specimen(g, a, out, "value2-no-data", |_| t2(0.0, 0.0, Avail::NoData));

    // Load, with limits the user set (load has none by default).
    let ld = |v: f32| W::Value {
        r: percent("CPU load", ico::GAUGE, v, Some(85.0), Some(95.0)),
        second: None,
    };
    specimen(g, a, out, "load-normal", |_| ld(6.0));
    specimen(g, a, out, "load-warning", |_| ld(88.0));
    specimen(g, a, out, "load-critical", |_| ld(98.0));

    let mem = |v: f32| W::Value {
        r: percent("Memory", ico::MEMORY, v, Some(MEM_WARN), Some(MEM_CRIT)),
        second: None,
    };
    specimen(g, a, out, "memory-normal", |_| mem(31.0));
    specimen(g, a, out, "memory-warning", |_| mem(89.0));
    specimen(g, a, out, "memory-critical", |_| mem(97.0));

    specimen(g, a, out, "network-normal", |_| w.network());
    specimen(g, a, out, "network-busy", |_| W::Network {
        down: rate("Down", ico::ARROW_DOWN, 48_200_000.0),
        up: rate("Up", ico::ARROW_UP, 1_300_000.0),
    });
    specimen(g, a, out, "network-no-data", |_| W::Network {
        down: Reading {
            avail: Avail::NoData,
            ..rate("Down", ico::ARROW_DOWN, 0.0)
        },
        up: Reading {
            avail: Avail::NoData,
            ..rate("Up", ico::ARROW_UP, 0.0)
        },
    });

    let wx = |avail: Avail| W::Weather {
        temp: "58°",
        cond: "Cloudy",
        hi_lo: "61° / 49°",
        icon: ico::CLOUD,
        avail,
    };
    specimen(g, a, out, "weather-normal", |_| wx(Avail::Live));
    specimen(g, a, out, "weather-no-data", |_| wx(Avail::NoData));
    specimen(g, a, out, "weather-stale", |_| wx(Avail::Lost("9:30")));
    specimen(g, a, out, "weather-not-set-up", |_| wx(Avail::NotSetUp));

    let md = |track: Option<Track>, pressed: Option<usize>| W::Media { track, pressed };
    specimen(g, a, out, "media-playing", |_| {
        md(Some(weird_fishes(true)), None)
    });
    specimen(g, a, out, "media-paused", |_| {
        md(Some(weird_fishes(false)), None)
    });
    specimen(g, a, out, "media-pressed", |_| {
        md(Some(weird_fishes(true)), Some(1))
    });
    specimen(g, a, out, "media-nothing-playing", |_| md(None, None));

    let sl = |value: f32, muted: bool, held: bool| W::Slider { value, muted, held };
    specimen(g, a, out, "slider-rest", |_| sl(40.0, false, false));
    specimen(g, a, out, "slider-dragging", |_| sl(62.0, false, true));
    specimen(g, a, out, "slider-muted", |_| sl(40.0, true, false));

    let k = |state: KeyState| {
        move |c: Class| W::Key {
            icon: ico::GLOBE,
            label: "Browser",
            sub: if c == Class::Hero {
                "Hold for a new window"
            } else {
                ""
            },
            state,
        }
    };
    specimen(g, a, out, "key-rest", k(KeyState::Rest));
    specimen(g, a, out, "key-pressed", k(KeyState::Pressed));
    specimen(g, a, out, "key-holding", k(KeyState::Holding(0.6)));
    specimen(g, a, out, "key-not-set-up", k(KeyState::NotSetUp));
}

// ---------------------------------------------------------------- tokens

fn hex(c: Rgb) -> String {
    format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
}

/// Prints the colour tokens with their contrast ratios, for DESIGN.md.
fn print_tokens() {
    let named = [
        ("GROUND", GROUND),
        ("CARD", CARD),
        ("KEY", KEY),
        ("KEY_TOP", KEY_TOP),
        ("TRACK", TRACK),
        ("TICK", TICK),
        ("TEXT", TEXT),
        ("TEXT2", TEXT2),
        ("TEXT3", TEXT3),
        ("FILL", FILL),
        ("WARN", WARN),
        ("WARN_SURFACE", WARN_SURFACE),
        ("WARN_ZONE", WARN_ZONE),
        ("CRIT", CRIT),
        ("CRIT_INK", CRIT_INK),
        ("CRIT_DEEP", CRIT_DEEP),
        ("CRIT_ZONE", CRIT_ZONE),
    ];
    println!("| token | hex | on CARD | on GROUND | OKLCH L |");
    for (n, c) in named {
        println!(
            "| {n} | {} | {:.1} | {:.1} | {:.2} |",
            hex(c),
            contrast(c, CARD),
            contrast(c, GROUND),
            oklch(c).0
        );
    }
    println!();
    println!("WARN on WARN_SURFACE {:.1}", contrast(WARN, WARN_SURFACE));
    println!("CRIT_INK on CRIT {:.1}", contrast(CRIT_INK, CRIT));
    println!("CRIT on CRIT_INK (OK) {:.1}", contrast(CRIT, CRIT_INK));
    println!("TEXT on KEY {:.1}", contrast(TEXT, KEY));
    println!();
    println!(
        "| accent | user | base | ink on base | text | text on CARD | fill | soft | base on CARD |"
    );
    for (n, u) in std::iter::once(ACCENT_DEFAULT).chain(ACCENT_ALTERNATIVES) {
        let a = Accent::from(u);
        println!(
            "| {n} | {} | {} | {:.1} | {} | {:.1} | {} | {} | {:.1} |",
            hex(u),
            hex(a.base),
            contrast(a.ink, a.base),
            hex(a.text),
            contrast(a.text, CARD),
            hex(a.fill),
            hex(a.soft),
            contrast(a.base, CARD),
        );
    }
    println!(
        "motion ms: press {MS_PRESS}, value {MS_VALUE}, page {MS_PAGE}, takeover in {MS_TAKEOVER_IN} out {MS_TAKEOVER_OUT}, hold {MS_HOLD}, alarm breathe {MS_ALARM_BREATHE}, marquee rest {MS_MARQUEE_REST} at {MARQUEE_PX_PER_S} px/s; idle dim {IDLE_DIM} after {IDLE_DIM_AFTER_S} s"
    );
    let (wl, _, wh) = oklch(WARN);
    let (cl, _, ch) = oklch(CRIT);
    println!("WARN oklch L {wl:.2} h {wh:.0}; CRIT L {cl:.2} h {ch:.0}");
    for t in [
        NUM_HERO,
        NUM_MEDIUM,
        NUM_BANDED,
        NUM_COMPACT,
        NUM_SECOND,
        UNIT_HERO,
        UNIT,
        TITLE_BIG,
        TITLE,
        BODY,
        LABEL,
    ] {
        assert!(t.size >= TEXT_FLOOR, "a text style is below the floor");
    }
}

fn main() {
    let dir = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("design-system-frames"));
    std::fs::create_dir_all(&dir).expect("create output dir");
    let mut g = Gfx::new(WIDTH, HEIGHT, GROUND);
    let mut out = Out {
        dir: dir.clone(),
        groups: Vec::new(),
    };
    let a = Accent::from(ACCENT_DEFAULT.1);
    scenes(
        &mut g,
        &a,
        &mut out,
        "",
        "scenes",
        Some(&[
            "1-idle",
            "2-music",
            "3-hot",
            "4-critical",
            "5-volume",
            "6-first-run",
        ]),
    );
    default_pages(&mut g, &a, &mut out);
    takeovers(&mut g, &a, &mut out);
    specimens(&mut g, &a, &mut out);
    let accent_frames: &[&str] = &["2-music", "severity", "6-first-run"];
    for (n, u) in std::iter::once(ACCENT_DEFAULT).chain(ACCENT_ALTERNATIVES) {
        let acc = Accent::from(u);
        scenes(
            &mut g,
            &acc,
            &mut out,
            &format!("accent-{n}-"),
            "accents",
            Some(accent_frames),
        );
    }
    for (group, file) in [
        ("scenes", "sheet-scenes.png"),
        ("default", "sheet-default.png"),
        ("takeover", "sheet-takeovers.png"),
        ("specimen", "sheet-specimens.png"),
        ("accents", "sheet-accents.png"),
    ] {
        if let Some((_, frames)) = out.groups.iter().find(|(n, _)| *n == group) {
            let frames = frames.clone();
            sheet(&mut g, &frames, &dir.join(file));
        }
    }
    print_tokens();
}
