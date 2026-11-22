//! The design's reference frames (`DESIGN.md`, `examples/design_system`),
//! rebuilt from real layouts and signal values and drawn by the shipped
//! renderer: whole pages through the `Engine`, single widgets through
//! `layout` and `Renderer`. Shared by the `design_system` example, which
//! writes them out, and the image tests, which compare them with stored
//! frames.
//!
//! The readings are the reference's: a Sunday at 10:53, in °F, on a
//! machine whose CPU idles at 115°.

use jiff::Zoned;

use crate::canvas::Canvas;
use crate::config::{Align, Config, Night, Temperature, Weather};
use crate::engine::{Engine, Input};
use crate::layout::{Bind, bind_page, slider};
use crate::signal::{Signals, Unit, Value};
use crate::theme::{GROUND, Rgb, Theme};
use crate::widget::{MediaKey, Renderer, Touch, WidgetView, media_keys};
use crate::{HEIGHT, WIDTH, demo};

/// One frame, named as the reference names it, in its sheet's group.
pub struct Frame {
    pub group: &'static str,
    pub name: String,
    pub canvas: Canvas,
}

/// °F as the °C the signals carry.
fn f(fahrenheit: f64) -> f64 {
    (fahrenheit - 32.0) * 5.0 / 9.0
}

/// What the machine is doing, in the reference's numbers.
#[derive(Clone)]
struct World {
    cpu: f64,
    gpu: f64,
    cpu_load: f64,
    gpu_load: f64,
    mem: f64,
    down: f64,
    up: f64,
    weather: bool,
    /// The track, playing or paused.
    track: Option<bool>,
    volume: f64,
}

fn idle() -> World {
    World {
        cpu: 115.0,
        gpu: 106.0,
        cpu_load: 6.0,
        gpu_load: 2.0,
        mem: 31.0,
        down: 12_000.0,
        up: 3_000.0,
        weather: true,
        track: None,
        volume: 40.0,
    }
}

fn music() -> World {
    World {
        track: Some(true),
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

pub const TITLE: &str = "Weird Fishes / Arpeggi";

fn track(playing: bool) -> Vec<(&'static str, Value)> {
    vec![
        ("media.title", Value::Text(TITLE.into())),
        ("media.artist", Value::Text("Radiohead".into())),
        (
            "media.status",
            Value::Text(if playing { "playing" } else { "paused" }.into()),
        ),
        ("media.position", Value::Number(97.0, Unit::None)),
        ("media.length", Value::Number(318.0, Unit::None)),
    ]
}

fn weather_signals() -> Vec<(&'static str, Value)> {
    vec![
        ("weather.temp", Value::Number(f(58.0), Unit::Celsius)),
        ("weather.code", Value::Number(3.0, Unit::None)),
        ("weather.condition", Value::Text("Cloudy".into())),
        ("weather.high", Value::Number(f(61.0), Unit::Celsius)),
        ("weather.low", Value::Number(f(49.0), Unit::Celsius)),
        ("weather.day", Value::Number(1.0, Unit::None)),
    ]
}

impl World {
    fn signals(&self) -> Vec<(&'static str, Value)> {
        let mut s = vec![
            ("cpu.temp", Value::Number(f(self.cpu), Unit::Celsius)),
            ("gpu.temp", Value::Number(f(self.gpu), Unit::Celsius)),
            ("cpu.load", Value::Number(self.cpu_load, Unit::Percent)),
            ("gpu.load", Value::Number(self.gpu_load, Unit::Percent)),
            ("mem.used", Value::Number(self.mem, Unit::Percent)),
            ("net.down", Value::Number(self.down, Unit::BytesPerSecond)),
            ("net.up", Value::Number(self.up, Unit::BytesPerSecond)),
            ("volume.level", Value::Number(self.volume, Unit::Percent)),
            ("volume.muted", Value::Number(0.0, Unit::None)),
        ];
        if self.weather {
            s.extend(weather_signals());
        }
        match self.track {
            Some(playing) => s.extend(track(playing)),
            None => s.push(("media.status", Value::Text("stopped".into()))),
        }
        s
    }
}

/// The bundled default layout, in °F, with a weather location and the
/// given accent.
fn layout(accent: Option<Rgb>, weather: bool) -> Config {
    let mut c = Config::default_layout();
    c.settings.temperature = Temperature::Fahrenheit;
    c.settings.accent = accent;
    c.weather = weather.then(|| Weather {
        location: Some("Jersey City".into()),
        latitude: None,
        longitude: None,
    });
    c
}

/// An engine on `config` with the world's readings in, on page `page`.
fn engine(config: Config, world: &World, page: &str) -> Engine {
    let index = config.page_index(page).expect("the page exists");
    let mut e = Engine::new(config);
    for (name, value) in world.signals() {
        e.handle(Input::Signal(name.into(), value));
    }
    e.handle(Input::Acknowledge);
    e.handle(Input::ShowPage(index));
    e
}

/// The engine's frame at `mono`, drawn whether or not it changed.
fn shot(e: &mut Engine, now: &Zoned, mono: f64) -> Canvas {
    e.handle(Input::Redraw);
    e.frame(now, mono).expect("a redraw draws")
}

struct Out {
    frames: Vec<Frame>,
}

impl Out {
    fn save(&mut self, group: &'static str, name: impl Into<String>, canvas: Canvas) {
        self.frames.push(Frame {
            group,
            name: name.into(),
            canvas,
        });
    }
}

/// Every reference frame, drawn with the default accent, then the scenes
/// that show the accent with each of `accents`.
pub fn frames(accents: &[(&str, Rgb)]) -> Vec<Frame> {
    let mut out = Out { frames: Vec::new() };
    scenes(&mut out, None, "", "scenes", None);
    default_pages(&mut out);
    takeovers(&mut out);
    specimens(&mut out);
    for (name, rgb) in accents {
        scenes(
            &mut out,
            Some(*rgb),
            &format!("accent-{name}-"),
            "accents",
            Some(&["2-music", "severity", "6-first-run"]),
        );
    }
    out.frames
}

fn scenes(
    out: &mut Out,
    accent: Option<Rgb>,
    prefix: &str,
    group: &'static str,
    only: Option<&[&str]>,
) {
    let now = demo::sample_time();
    let want = |n: &str| only.is_none_or(|o| o.contains(&n));
    let name = |n: &str| format!("{prefix}{n}");
    if want("1-idle") {
        let mut e = engine(layout(accent, true), &idle(), "Home");
        out.save(group, name("1-idle"), shot(&mut e, &now, 1.0));
    }
    if want("2-music") {
        let mut e = engine(layout(accent, true), &music(), "Music");
        out.save(group, name("2-music"), shot(&mut e, &now, 1.0));
    }
    if want("3-hot") {
        let mut e = engine(layout(accent, true), &hot(), "System");
        out.save(group, name("3-hot"), shot(&mut e, &now, 1.0));
    }
    if want("4-critical") {
        let mut e = alarmed(accent);
        out.save(group, name("4-critical"), shot(&mut e, &now, 0.0));
    }
    if want("5-volume") {
        let mut e = volume_up(layout(accent, true));
        out.save(group, name("5-volume"), shot(&mut e, &now, 1.0));
    }
    if want("6-first-run") {
        let world = World {
            weather: false,
            ..idle()
        };
        let mut e = engine(layout(accent, false), &world, "Home");
        out.save(group, name("6-first-run"), shot(&mut e, &now, 1.0));
    }
    if want("severity") {
        // Accent controls right beside warning and critical cards, with
        // the markers of five pages, two of them critical.
        let mut c = layout(accent, true);
        let row = |w: &str| format!("[[page]]\nname = \"{w}\"\n");
        let text = format!(
            "{}{}{}{}{}",
            row("S")
                + &value("CPU", "cpu.temp", 124, "warn = 75\ncrit = 90\n")
                + &value("GPU", "gpu.temp", 124, "warn = 80\ncrit = 90\n")
                + "[[page.widget]]\ntype = \"media\"\nwidth = 260\n"
                + "[[page.widget]]\ntype = \"slider\"\nwidth = 132\nlabel = \"Volume\"\ncontrols = \"volume\"\n",
            row("B") + "[[page.widget]]\ntype = \"clock\"\nwidth = 640\n",
            row("C") + &value("GPU", "gpu.temp", 640, "crit = 90\n"),
            row("D") + "[[page.widget]]\ntype = \"clock\"\nwidth = 640\n",
            row("E") + "[[page.widget]]\ntype = \"clock\"\nwidth = 640\n",
        );
        let pages = Config::parse(&text).expect("a valid layout");
        c.pages = pages.pages;
        let mut e = engine(c, &critical(), "S");
        out.save(group, name("severity"), shot(&mut e, &now, 1.0));
    }
    if want("narrow-keys") {
        // Sixteen keys at the 40 px minimum, none set up: each shows its
        // icon in its outline, and no words run into the next.
        let mut c = layout(accent, true);
        let text = format!(
            "[[page]]\nname = \"K\"\n{}",
            "[[page.widget]]\ntype = \"key\"\nwidth = 40\n".repeat(16)
        );
        c.pages = Config::parse(&text).expect("a valid layout").pages;
        let mut e = engine(c, &idle(), "K");
        out.save(group, name("narrow-keys"), shot(&mut e, &now, 1.0));
    }
}

/// A value widget's TOML.
fn value(label: &str, signal: &str, width: u32, extra: &str) -> String {
    format!(
        "[[page.widget]]\ntype = \"value\"\nwidth = {width}\nlabel = \"{label}\"\nsignal = \"{signal}\"\n{extra}"
    )
}

/// The GPU at 199°: the alarm is up.
fn alarmed(accent: Option<Rgb>) -> Engine {
    let config = layout(accent, true);
    let index = config.page_index("Home").expect("Home");
    let mut e = Engine::new(config);
    e.handle(Input::ShowPage(index));
    for (name, value) in critical().signals() {
        e.handle(Input::Signal(name.into(), value));
    }
    assert!(e.alarm().is_some(), "the GPU raised the alarm");
    e
}

/// Home with music playing, and the volume just turned from 40 to 55.
fn volume_up(config: Config) -> Engine {
    let mut e = engine(config, &music(), "Home");
    e.handle(Input::Signal(
        "volume.level".into(),
        Value::Number(55.0, Unit::Percent),
    ));
    assert_eq!(e.takeover(), Some("Volume"));
    e
}

fn default_pages(out: &mut Out) {
    let now = demo::sample_time();
    let config = layout(None, true);
    for (i, page) in config.pages.iter().enumerate() {
        let mut e = engine(config.clone(), &music(), &page.name);
        let name = format!("default-p{}-{}", i + 1, page.name.to_lowercase());
        out.save("default", name, shot(&mut e, &now, 1.0));
    }
    // At night the whole frame dims; nothing is redrawn smaller.
    let mut night = layout(None, true);
    night.settings.night = Some(Night {
        from: "22:00".into(),
        to: "11:00".into(),
    });
    let mut e = engine(night.clone(), &idle(), "Desk");
    out.save("default", "default-night-desk", shot(&mut e, &now, 1.0));
    // Dimmed is not lost: at night, the GPU's source gone beside live
    // readings. The lost one keeps its broken link and its time; the live
    // figures stay brighter than it.
    let mut e = engine(night, &idle(), "System");
    e.handle(Input::Forget("gpu.temp".into()));
    out.save(
        "default",
        "default-night-system-lost",
        shot(&mut e, &now, 1.0),
    );
    let mut e = engine(layout(None, true), &idle(), "System");
    e.handle(Input::Forget("gpu.temp".into()));
    out.save(
        "default",
        "default-day-system-lost",
        shot(&mut e, &now, 1.0),
    );
}

/// The x of the middle of a media widget's key with this command.
fn key_x(left: f32, width: u32, key: impl Fn(&MediaKey) -> bool) -> f32 {
    let k = media_keys(width)
        .into_iter()
        .find(|k| key(k))
        .expect("the key is drawn");
    left + (k.span.start + k.span.end) / 2.0
}

fn takeovers(out: &mut Out) {
    let now = demo::sample_time();
    let show = |e: &mut Engine| {
        e.handle(Input::ShowTakeover {
            name: "Now playing".into(),
            seconds: 5.0,
        });
    };
    // Now playing, over Home.
    let mut e = engine(layout(None, true), &music(), "Home");
    show(&mut e);
    out.save("takeover", "takeover-now-playing", shot(&mut e, &now, 1.0));

    let world = World {
        track: Some(false),
        ..music()
    };
    let mut e = engine(layout(None, true), &world, "Home");
    show(&mut e);
    e.frame(&now, 1.0);
    let x = key_x(0.0, WIDTH, |k| {
        k.command == crate::config::MediaCommand::PlayPause
    });
    e.handle(Input::Touch {
        down: true,
        x,
        at: 1.1,
    });
    out.save(
        "takeover",
        "takeover-now-playing-paused-pressed",
        shot(&mut e, &now, 1.1),
    );

    // Volume over the right half, then the left, held.
    let mut e = volume_up(layout(None, true));
    out.save("takeover", "takeover-volume-right", shot(&mut e, &now, 1.0));
    let mut left = layout(None, true);
    let t = left
        .takeovers
        .iter_mut()
        .find(|t| t.name == "Volume")
        .expect("a volume takeover");
    t.align = Align::Left;
    let mut e = volume_up(left);
    e.frame(&now, 1.0);
    e.handle(Input::Touch {
        down: true,
        x: 200.0,
        at: 1.1,
    });
    out.save(
        "takeover",
        "takeover-volume-left-held",
        shot(&mut e, &now, 1.2),
    );

    // The same 320 px takeover over System, whose cards meet at 204 and
    // 408: 232 px would cramp its slider, so it snaps out to 204 (436 px)
    // rather than cut the GPU card in two.
    let mut e = engine(layout(None, true), &music(), "System");
    e.handle(Input::Signal(
        "volume.level".into(),
        Value::Number(55.0, Unit::Percent),
    ));
    out.save(
        "takeover",
        "takeover-volume-snapped-system",
        shot(&mut e, &now, 1.0),
    );

    // The alarm, and the OK key under a finger.
    let mut e = alarmed(None);
    out.save("takeover", "takeover-alarm", shot(&mut e, &now, 0.0));
    e.handle(Input::Touch {
        down: true,
        x: 590.0,
        at: 0.0,
    });
    out.save(
        "takeover",
        "takeover-alarm-ok-pressed",
        shot(&mut e, &now, 0.0),
    );

    // After OK: the page returns; the GPU card stays critical, its page
    // markers stay red.
    let mut e = alarmed(None);
    e.handle(Input::Acknowledge);
    let system = e.config().page_index("System").expect("System");
    e.handle(Input::ShowPage(system));
    out.save(
        "takeover",
        "takeover-after-ok-system",
        shot(&mut e, &now, 1.0),
    );
    e.handle(Input::ShowPage(0));
    out.save(
        "takeover",
        "takeover-after-ok-home",
        shot(&mut e, &now, 1.0),
    );
}

/// Zone widths of a specimen frame: compact, medium, hero.
pub const SPECIMEN_WIDTHS: [u32; 3] = [128, 200, 312];

/// How a specimen's widgets are touched.
#[derive(Clone, Copy)]
enum Finger {
    None,
    /// On the widget; for media, on its play/pause key.
    On,
    /// Holding, this far to the long press.
    Holding(f32),
}

/// The same widget at the three specimen widths, side by side. `widget`
/// is its TOML with `{w}` for the width.
#[allow(clippy::too_many_arguments)]
fn specimen(
    out: &mut Out,
    renderer: &mut Renderer,
    name: &str,
    widget: &str,
    signals: &Signals,
    weather: bool,
    mono: f64,
    finger: Finger,
    edit: impl Fn(&mut WidgetView),
) {
    let mut text = String::from("[[page]]\nname = \"S\"\n");
    for w in SPECIMEN_WIDTHS {
        text += &widget.replace("{w}", &w.to_string());
    }
    let mut config = Config::parse(&text).unwrap_or_else(|e| panic!("{name}: {e}"));
    config.settings.temperature = Temperature::Fahrenheit;
    let now = demo::sample_time();
    let ctx = Bind {
        signals,
        settings: &config.settings,
        weather,
        now: &now,
        mono,
        marquee: None,
        music: config.music_action(),
    };
    let zones = bind_page(&config.pages[0].widgets, &ctx);
    let mut frame = Canvas::new(WIDTH, HEIGHT, GROUND);
    let mut x = 0;
    for mut zone in zones {
        edit(&mut zone.view);
        let part = media_keys(zone.width)
            .iter()
            .position(|k| k.command == crate::config::MediaCommand::PlayPause)
            .filter(|_| matches!(zone.view, WidgetView::Media(Some(_))));
        let touch = match finger {
            Finger::None => None,
            Finger::On => Some(Touch {
                zone: 0,
                part,
                hold: None,
            }),
            Finger::Holding(p) => Some(Touch {
                zone: 0,
                part: None,
                hold: Some(p),
            }),
        };
        let width = zone.width;
        let card = renderer.render_row(&[zone], touch, width, false);
        frame.copy_shifted(&card, x as i32);
        x += width;
    }
    out.save("specimen", format!("specimen-{name}"), frame);
}

fn specimens(out: &mut Out) {
    let mut r = Renderer::new(Theme::default(), crate::fonts::FontChoice::default().load());
    let r = &mut r;
    let signals = |pairs: Vec<(&str, Value)>| {
        let mut s = Signals::new();
        for (name, v) in pairs {
            s.set_at(name, v, 1000.0);
        }
        s
    };
    let world = signals(idle().signals());
    let none = Signals::new();
    let same = |_: &mut WidgetView| {};
    let w = "{w}";

    let clock = format!("[[page.widget]]\ntype = \"clock\"\nwidth = {w}\n");
    specimen(
        out,
        r,
        "clock",
        &clock,
        &world,
        true,
        1000.0,
        Finger::None,
        same,
    );

    // A temperature with limits.
    let cpu =
        value("CPU", "cpu.temp", 0, "warn = 75\ncrit = 90\n").replace("width = 0", "width = {w}");
    let temp = |v: f64| signals(vec![("cpu.temp", Value::Number(f(v), Unit::Celsius))]);
    for (name, v) in [("normal", 115.0), ("warning", 172.0), ("critical", 199.0)] {
        let s = temp(v);
        let n = format!("value-{name}");
        specimen(out, r, &n, &cpu, &s, true, 1000.0, Finger::None, same);
    }
    specimen(
        out,
        r,
        "value-no-data",
        &cpu,
        &none,
        true,
        1000.0,
        Finger::None,
        same,
    );
    // Lost at 10:41: twelve minutes before the frame's 10:53.
    let mut lost = temp(115.0);
    lost.forget("cpu.temp", 1000.0 - 720.0);
    specimen(
        out,
        r,
        "value-lost",
        &cpu,
        &lost,
        true,
        1000.0,
        Finger::None,
        same,
    );
    let unset = value("CPU", "", 0, "").replace("width = 0", "width = {w}");
    specimen(
        out,
        r,
        "value-not-set-up",
        &unset,
        &none,
        true,
        1000.0,
        Finger::None,
        same,
    );

    // A value with a second reading beside it.
    let gpu = value(
        "GPU",
        "gpu.temp",
        0,
        "warn = 80\ncrit = 90\nsecond = \"gpu.load\"\nsecond_label = \"load\"\n",
    )
    .replace("width = 0", "width = {w}");
    let two = |t: f64, load: f64| {
        signals(vec![
            ("gpu.temp", Value::Number(f(t), Unit::Celsius)),
            ("gpu.load", Value::Number(load, Unit::Percent)),
        ])
    };
    for (name, t, load) in [
        ("normal", 106.0, 2.0),
        ("warning", 181.0, 97.0),
        ("critical", 199.0, 97.0),
    ] {
        let s = two(t, load);
        let n = format!("value2-{name}");
        specimen(out, r, &n, &gpu, &s, true, 1000.0, Finger::None, same);
    }
    specimen(
        out,
        r,
        "value2-no-data",
        &gpu,
        &none,
        true,
        1000.0,
        Finger::None,
        same,
    );

    // Load, with limits the user set (load has none by default).
    let load = value("CPU load", "cpu.load", 0, "warn = 85\ncrit = 95\n")
        .replace("width = 0", "width = {w}");
    for (name, v) in [("normal", 6.0), ("warning", 88.0), ("critical", 98.0)] {
        let s = signals(vec![("cpu.load", Value::Number(v, Unit::Percent))]);
        let n = format!("load-{name}");
        specimen(out, r, &n, &load, &s, true, 1000.0, Finger::None, same);
    }
    let mem = value("Memory", "mem.used", 0, "warn = 85\ncrit = 95\n")
        .replace("width = 0", "width = {w}");
    for (name, v) in [("normal", 31.0), ("warning", 89.0), ("critical", 97.0)] {
        let s = signals(vec![("mem.used", Value::Number(v, Unit::Percent))]);
        let n = format!("memory-{name}");
        specimen(out, r, &n, &mem, &s, true, 1000.0, Finger::None, same);
    }

    let net = format!(
        "[[page.widget]]\ntype = \"rate\"\nwidth = {w}\nlabel = \"Network\"\ndown = \"net.down\"\nup = \"net.up\"\n"
    );
    specimen(
        out,
        r,
        "network-normal",
        &net,
        &world,
        true,
        1000.0,
        Finger::None,
        same,
    );
    let busy = signals(vec![
        (
            "net.down",
            Value::Number(48_200_000.0, Unit::BytesPerSecond),
        ),
        ("net.up", Value::Number(1_300_000.0, Unit::BytesPerSecond)),
    ]);
    specimen(
        out,
        r,
        "network-busy",
        &net,
        &busy,
        true,
        1000.0,
        Finger::None,
        same,
    );
    // Down and up in different units: each keeps its own.
    let mixed = signals(vec![
        (
            "net.down",
            Value::Number(48_200_000.0, Unit::BytesPerSecond),
        ),
        ("net.up", Value::Number(310_000.0, Unit::BytesPerSecond)),
    ]);
    specimen(
        out,
        r,
        "network-mixed-units",
        &net,
        &mixed,
        true,
        1000.0,
        Finger::None,
        same,
    );
    specimen(
        out,
        r,
        "network-no-data",
        &net,
        &none,
        true,
        1000.0,
        Finger::None,
        same,
    );

    let wx = value("Weather", "weather.temp", 0, "").replace("width = 0", "width = {w}");
    let live = signals(weather_signals());
    specimen(
        out,
        r,
        "weather-normal",
        &wx,
        &live,
        true,
        1000.0,
        Finger::None,
        same,
    );
    specimen(
        out,
        r,
        "weather-no-data",
        &wx,
        &none,
        true,
        1000.0,
        Finger::None,
        same,
    );
    // The last forecast came at 9:30, 83 minutes before the frame.
    specimen(
        out,
        r,
        "weather-stale",
        &wx,
        &live,
        true,
        1000.0 + 83.0 * 60.0,
        Finger::None,
        same,
    );
    specimen(
        out,
        r,
        "weather-not-set-up",
        &wx,
        &none,
        false,
        1000.0,
        Finger::None,
        same,
    );

    let media = format!("[[page.widget]]\ntype = \"media\"\nwidth = {w}\n");
    let playing = signals(track(true));
    let paused = signals(track(false));
    specimen(
        out,
        r,
        "media-playing",
        &media,
        &playing,
        true,
        1000.0,
        Finger::None,
        same,
    );
    specimen(
        out,
        r,
        "media-paused",
        &media,
        &paused,
        true,
        1000.0,
        Finger::None,
        same,
    );
    specimen(
        out,
        r,
        "media-pressed",
        &media,
        &playing,
        true,
        1000.0,
        Finger::On,
        same,
    );
    specimen(
        out,
        r,
        "media-nothing-playing",
        &media,
        &none,
        true,
        1000.0,
        Finger::None,
        same,
    );
    // The same, in a layout that says how to open the music player (here
    // the widget's own tap; a Music key that runs a player does the same):
    // the card is a key that opens it.
    let media_open = format!(
        "[[page.widget]]\ntype = \"media\"\nwidth = {w}\ntap = {{ run = [\"rhythmbox\"] }}\n"
    );
    for (name, finger) in [
        ("media-nothing-playing-opens", Finger::None),
        ("media-nothing-playing-opens-pressed", Finger::On),
    ] {
        specimen(out, r, name, &media_open, &none, true, 1000.0, finger, same);
    }

    let volume = format!(
        "[[page.widget]]\ntype = \"slider\"\nwidth = {w}\nlabel = \"Volume\"\ncontrols = \"volume\"\n"
    );
    let vol = |muted: f64| {
        signals(vec![
            ("volume.level", Value::Number(40.0, Unit::Percent)),
            ("volume.muted", Value::Number(muted, Unit::None)),
        ])
    };
    let rest = vol(0.0);
    specimen(
        out,
        r,
        "slider-rest",
        &volume,
        &rest,
        true,
        1000.0,
        Finger::None,
        same,
    );
    let config = Config::parse(&format!(
        "[[page]]\nname = \"S\"\n{}",
        volume.replace("{w}", "200")
    ))
    .expect("a slider");
    let held = slider(&config.pages[0].widgets[0], &rest, Some(0.62), None, None);
    // A finger on it at 62: the view the engine shows while dragging.
    let drag = move |v: &mut WidgetView| *v = held.clone();
    specimen(
        out,
        r,
        "slider-dragging",
        &volume,
        &rest,
        true,
        1000.0,
        Finger::None,
        drag,
    );
    let muted = vol(1.0);
    specimen(
        out,
        r,
        "slider-muted",
        &volume,
        &muted,
        true,
        1000.0,
        Finger::None,
        same,
    );

    let key = format!(
        "[[page.widget]]\ntype = \"key\"\nwidth = {w}\nicon = \"globe-simple\"\nlabel = \"Browser\"\nsub = \"Hold for a new window\"\ntap = {{ open = \"https://duckduckgo.com\" }}\n"
    );
    specimen(
        out,
        r,
        "key-rest",
        &key,
        &none,
        true,
        1000.0,
        Finger::None,
        same,
    );
    specimen(
        out,
        r,
        "key-pressed",
        &key,
        &none,
        true,
        1000.0,
        Finger::On,
        same,
    );
    specimen(
        out,
        r,
        "key-holding",
        &key,
        &none,
        true,
        1000.0,
        Finger::Holding(0.6),
        same,
    );
    let bare = format!(
        "[[page.widget]]\ntype = \"key\"\nwidth = {w}\nicon = \"globe-simple\"\nlabel = \"Browser\"\n"
    );
    specimen(
        out,
        r,
        "key-not-set-up",
        &bare,
        &none,
        true,
        1000.0,
        Finger::None,
        same,
    );
}
