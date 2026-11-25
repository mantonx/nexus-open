//! The design's reference frames (`DESIGN.md`, `examples/design_system`),
//! rebuilt from real layouts and signal values and drawn by the shipped
//! renderer: whole pages through the `Engine`, single widgets through
//! `layout` and `Renderer`. Shared by the `design_system` example, which
//! writes them out, and the image tests, which compare them with stored
//! frames.
//!
//! The readings are the reference's: a Sunday at 10:53, in °F, on a
//! machine whose CPU idles at 115°.

use std::sync::Arc;

use jiff::Zoned;

use crate::art::{self, Art};
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

/// Stand-in artwork for frames: `art::test_card`, wider than tall so the
/// centre crop shows, never a real cover.
pub fn test_art() -> Arc<Art> {
    let (w, h) = (360, 300);
    Arc::new(Art::prepare("art-test", w, h, &art::test_card(w, h)).expect("a test card"))
}

/// The engine given `test_art` and told it is the track's.
fn with_art(e: &mut Engine) {
    e.handle(Input::Art(test_art()));
    e.handle(Input::Signal(
        "media.art".into(),
        Value::Text("art-test".into()),
    ));
}

/// A headline, as the news source publishes it.
pub const HEADLINE: &str = "Harbour ferry returns to service after a two-week repair";

fn news_signals(title: &str, age: f64, lost: bool) -> Vec<(&'static str, Value)> {
    vec![
        ("news.title", Value::Text(title.into())),
        ("news.source", Value::Text("Harbour Gazette".into())),
        (
            "news.link",
            Value::Text("https://news.example/ferry-returns".into()),
        ),
        ("news.age", Value::Number(age, Unit::None)),
        ("news.count", Value::Number(14.0, Unit::None)),
        ("news.lost", Value::Number(f64::from(lost), Unit::None)),
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
    news_page(&mut out);
    forecast_frames(&mut out);
    live_pages(&mut out);
    specimens(&mut out);
    layout_switch(&mut out);
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

/// The optional News page of docs/LAYOUT.md, with a feed and a headline:
/// a wide news card beside the clock.
fn news_page(out: &mut Out) {
    let now = demo::sample_time();
    let mut c = layout(None, true);
    let page = Config::parse(NEWS_PAGE).expect("the News page is valid");
    c.pages.extend(page.pages);
    c.news = page.news;
    let mut e = engine(c, &idle(), "News");
    for (name, value) in news_signals(HEADLINE, 12.0 * 60.0, false) {
        e.handle(Input::Signal(name.into(), value));
    }
    out.save("news", "news-page", shot(&mut e, &now, 1.0));
}

/// The example News page in docs/LAYOUT.md.
pub const NEWS_PAGE: &str = r#"[news]
feeds = ["https://news.example/rss.xml"]

[[page]]
name = "News"

[[page.widget]]
type = "clock"
width = 128

[[page.widget]]
type = "news"
width = 512
"#;

/// An hour-long track 9 min 26 s in, and one past its first hour.
const LONG_TRACK: (f64, f64) = (566.0, 2592.0);
const HOURS_TRACK: (f64, f64) = (3753.0, 7800.0);

/// The track, paused, at `at` seconds of `length`.
fn track_at(e: &mut Engine, (at, length): (f64, f64)) {
    e.handle(Input::Signal(
        "media.position".into(),
        Value::Number(at, Unit::None),
    ));
    e.handle(Input::Signal(
        "media.length".into(),
        Value::Number(length, Unit::None),
    ));
}

/// The owner's layout as the strip first drew it, where it went wrong
/// (2026-11-22): six widgets on a page, so every zone is 106 or 107 px,
/// two of them compact temperatures in °F; a media page with a seek
/// slider and an hour-long track; and a page with no widgets yet.
fn live_pages(out: &mut Out) {
    let now = demo::sample_time();
    let row = |w: &str| format!("[[page]]\nname = \"{w}\"\n");
    let text = format!(
        "{}{}{}",
        row("Glance")
            + "[[page.widget]]\ntype = \"clock\"\nwidth = 107\n"
            + "[[page.widget]]\ntype = \"media\"\nwidth = 107\n"
            + &value("Weather", "weather.temp", 107, "")
            + &value("CPU", "cpu.temp", 107, "warn = 75\ncrit = 90\n")
            + &value("GPU", "gpu.temp", 106, "warn = 80\ncrit = 90\n")
            + "[[page.widget]]\ntype = \"rate\"\nwidth = 106\nlabel = \"Network\"\ndown = \"net.down\"\nup = \"net.up\"\n",
        row("Media")
            + "[[page.widget]]\ntype = \"media\"\nwidth = 214\n"
            + "[[page.widget]]\ntype = \"slider\"\nwidth = 213\nlabel = \"Seek\"\ncontrols = \"seek\"\n"
            + "[[page.widget]]\ntype = \"slider\"\nwidth = 213\nlabel = \"Volume\"\ncontrols = \"volume\"\n",
        row("Empty"),
    );
    let mut c = layout(None, true);
    c.pages
        .extend(Config::parse(&text).expect("a valid layout").pages);
    let world = World {
        cpu: 111.0,
        gpu: 120.0,
        down: 8_000.0,
        track: Some(false),
        ..idle()
    };
    let mut e = engine(c.clone(), &world, "Glance");
    track_at(&mut e, LONG_TRACK);
    out.save("live", "live-p1-compact-temps", shot(&mut e, &now, 1.0));
    // The same page with a three-digit rate: "512 kB/s" in 106 px.
    let busy = World {
        down: 512_000.0,
        ..world.clone()
    };
    let mut e = engine(c.clone(), &busy, "Glance");
    track_at(&mut e, LONG_TRACK);
    out.save("live", "live-p1-rate", shot(&mut e, &now, 1.0));
    let mut e = engine(c.clone(), &world, "Media");
    track_at(&mut e, LONG_TRACK);
    out.save("live", "live-p5-seek", shot(&mut e, &now, 1.0));
    let mut e = engine(c.clone(), &world, "Media");
    track_at(&mut e, HOURS_TRACK);
    out.save("live", "live-p5-seek-hours", shot(&mut e, &now, 1.0));
    let mut e = engine(c, &world, "Empty");
    out.save("live", "live-p6-empty-page", shot(&mut e, &now, 1.0));

    // One compact temperature at every width a layout may give it, widest
    // first: each step of the ladder, the name in place of the icon, and
    // the icon alone where no number fits.
    let widths = [125, 112, 106, 96, 88, 72, 41];
    let mut text = row("Narrow");
    for w in widths {
        text += &value("CPU", "cpu.temp", w, "warn = 75\ncrit = 90\n");
    }
    let mut c = layout(None, true);
    c.pages = Config::parse(&text).expect("a valid layout").pages;
    let mut e = engine(c, &world, "Narrow");
    out.save("live", "narrow-values", shot(&mut e, &now, 1.0));

    // Every compact card that shows a number, at its worst value, in the
    // zones a layout gives compact cards: nothing may run past its card.
    let widths = [124, 112, 106, 102, 100, 96];
    let worst = World {
        cpu_load: 100.0,
        mem: 100.0,
        down: 999_000_000.0,
        ..world.clone()
    };
    let rate = |w: u32| {
        format!(
            "[[page.widget]]\ntype = \"rate\"\nwidth = {w}\nlabel = \"Network\"\ndown = \"net.down\"\nup = \"net.up\"\n"
        )
    };
    let load = |w: u32| value("CPU load", "cpu.load", w, "");
    let mem = |w: u32| value("Memory", "mem.used", w, "warn = 85\ncrit = 95\n");
    let wx = |w: u32| value("Weather", "weather.temp", w, "");
    let kinds: [(&str, &dyn Fn(u32) -> String); 4] = [
        ("rate", &rate),
        ("load", &load),
        ("memory", &mem),
        ("weather", &wx),
    ];
    for (kind, widget) in kinds {
        let mut text = row("Sweep");
        for w in widths {
            text += &widget(w);
        }
        let mut c = layout(None, true);
        c.pages = Config::parse(&text).expect("a valid layout").pages;
        let mut e = engine(c, &worst, "Sweep");
        if kind == "weather" {
            // 104 °F, the hottest three digits a forecast gives.
            e.handle(Input::Signal(
                "weather.temp".into(),
                Value::Number(f(104.0), Unit::Celsius),
            ));
        }
        out.save(
            "live",
            format!("overflow-sweep-{kind}"),
            shot(&mut e, &now, 1.0),
        );
    }
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

    // The same with the player's artwork, playing and paused.
    let mut e = engine(layout(None, true), &music(), "Home");
    with_art(&mut e);
    show(&mut e);
    out.save(
        "takeover",
        "takeover-now-playing-art",
        shot(&mut e, &now, 1.0),
    );
    let paused = World {
        track: Some(false),
        ..music()
    };
    let mut e = engine(layout(None, true), &paused, "Home");
    with_art(&mut e);
    show(&mut e);
    out.save(
        "takeover",
        "takeover-now-playing-art-paused",
        shot(&mut e, &now, 1.0),
    );

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
/// is its TOML with `{w}` for the width. `set_up` says whether the layout
/// gives a weather location and a news feed.
#[allow(clippy::too_many_arguments)]
fn specimen(
    out: &mut Out,
    renderer: &mut Renderer,
    name: &str,
    widget: &str,
    signals: &Signals,
    set_up: bool,
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
        weather: set_up,
        news: set_up,
        now: &now,
        mono,
        marquee: None,
        music: config.music_action(),
        art: &[],
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

    // Times are never cut: "/ 43:12" goes first, then a step smaller,
    // then the time; past an hour too.
    let seek = format!(
        "[[page.widget]]\ntype = \"slider\"\nwidth = {w}\nlabel = \"Seek\"\ncontrols = \"seek\"\n"
    );
    let at = |(pos, len): (f64, f64), playing: bool| {
        let mut t = track(playing);
        t.retain(|(n, _)| !matches!(*n, "media.position" | "media.length"));
        t.push(("media.position", Value::Number(pos, Unit::None)));
        t.push(("media.length", Value::Number(len, Unit::None)));
        signals(t)
    };
    for (name, times) in [
        ("slider-seek", LONG_TRACK),
        ("slider-seek-hours", HOURS_TRACK),
    ] {
        let s = at(times, true);
        specimen(out, r, name, &seek, &s, true, 1000.0, Finger::None, same);
    }
    let hours = at(HOURS_TRACK, true);
    specimen(
        out,
        r,
        "media-hours",
        &media,
        &hours,
        true,
        1000.0,
        Finger::None,
        same,
    );
    // The player's artwork in place of the placeholder (compact cards
    // show none), playing and paused.
    let art = test_art();
    let with_art = |v: &mut WidgetView| {
        if let WidgetView::Media(Some(t)) = v {
            t.art = Some(Arc::clone(&art));
        }
    };
    for (name, s) in [
        ("media-art-playing", &playing),
        ("media-art-paused", &paused),
    ] {
        specimen(
            out,
            r,
            name,
            &media,
            s,
            true,
            1000.0,
            Finger::None,
            with_art,
        );
    }

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
    news_specimens(out, r, &signals);

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

/// Turns readings into the signals a specimen shows.
type ToSignals = dyn Fn(Vec<(&str, Value)>) -> Signals;

/// The news widget in each of its states, at the three widths.
fn news_specimens(out: &mut Out, r: &mut Renderer, signals: &ToSignals) {
    let same = |_: &mut WidgetView| {};
    let news = "[[page.widget]]\ntype = \"news\"\nwidth = {w}\n";
    let live = signals(news_signals(HEADLINE, 12.0 * 60.0, false));
    for (name, finger) in [
        ("news-live", Finger::None),
        ("news-pressed", Finger::On),
        ("news-holding", Finger::Holding(0.6)),
    ] {
        specimen(out, r, name, news, &live, true, 1000.0, finger, same);
    }
    // A short headline that fits, published under a minute ago.
    let short = signals(news_signals("Ferry back in service", 20.0, false));
    specimen(
        out,
        r,
        "news-short",
        news,
        &short,
        true,
        1000.0,
        Finger::None,
        same,
    );
    let none = Signals::new();
    specimen(
        out,
        r,
        "news-no-data",
        news,
        &none,
        true,
        1000.0,
        Finger::None,
        same,
    );
    // Every feed failing, and the headline kept is two days old.
    let lost = signals(news_signals(HEADLINE, 2.0 * 86_400.0 + 600.0, true));
    specimen(
        out,
        r,
        "news-lost",
        news,
        &lost,
        true,
        1000.0,
        Finger::None,
        same,
    );
    specimen(
        out,
        r,
        "news-not-set-up",
        news,
        &none,
        false,
        1000.0,
        Finger::None,
        same,
    );
}

// ---------------------------------------------------------------- forecast

/// One generated forecast: the weather now, eight three-hour columns,
/// five days and the next change, with the units and clock it is shown
/// in. Never a live fetch.
pub struct Outlook {
    pub name: &'static str,
    temperature: Temperature,
    clock: crate::config::ClockFormat,
    /// °C, WMO code, condition, high, low.
    now: (f64, f64, &'static str, f64, f64),
    /// Hour of the day, °C, WMO code, chance of rain, day.
    hours: [(u8, f64, f64, f64, bool); 8],
    /// Day key, high and low °C, WMO code, chance of rain.
    days: [(&'static str, f64, f64, f64, f64); 5],
    next: crate::forecast::Change,
    /// The deciding hour's WMO code and day flag: the next change's icon.
    next_icon: (f64, bool),
}

impl Outlook {
    fn signals(&self) -> Vec<(&'static str, Value)> {
        use crate::forecast::{self, DAYS, HOURS};
        let n = |v: f64| Value::Number(v, Unit::None);
        let c = |v: f64| Value::Number(v, Unit::Celsius);
        let p = |v: f64| Value::Number(v, Unit::Percent);
        let b = |v: bool| n(f64::from(u8::from(v)));
        let (temp, code, cond, high, low) = self.now;
        let mut s = vec![
            ("weather.temp", c(temp)),
            ("weather.code", n(code)),
            ("weather.condition", Value::Text(cond.into())),
            ("weather.high", c(high)),
            ("weather.low", c(low)),
            ("weather.day", n(1.0)),
            (forecast::NEXT, Value::Text(self.next.key())),
            (forecast::NEXT_CODE, n(self.next_icon.0)),
            (forecast::NEXT_DAY, b(self.next_icon.1)),
        ];
        if let Some(at) = self.next.at() {
            s.push((forecast::NEXT_AT, n(at.into())));
        }
        for (names, (hour, t, code, rain, day)) in HOURS.iter().zip(self.hours) {
            s.extend([
                (names.hour, n(hour.into())),
                (names.temp, c(t)),
                (names.code, n(code)),
                (names.rain, p(rain)),
                (names.day, b(day)),
            ]);
        }
        for (names, (key, hi, lo, code, rain)) in DAYS.iter().zip(self.days) {
            s.extend([
                (names.day, Value::Text(key.into())),
                (names.high, c(hi)),
                (names.low, c(lo)),
                (names.code, n(code)),
                (names.rain, p(rain)),
            ]);
        }
        s
    }
}

/// The generated forecasts the variants are drawn with, for a Sunday at
/// 10:53: a dry day (°F, 12 h), a rainy afternoon (°C, 24 h), a cold week
/// with snow tonight (°C, 12 h), and rain now that clears (°F, 24 h).
pub fn outlooks() -> Vec<Outlook> {
    use crate::config::ClockFormat::{H12, H24};
    use crate::forecast::{Change, Wet};
    vec![
        Outlook {
            name: "dry-day",
            temperature: Temperature::Fahrenheit,
            clock: H12,
            now: (f(52.0), 1.0, "Mostly clear", f(57.0), f(41.0)),
            hours: [
                (12, f(56.0), 1.0, 0.0, true),
                (15, f(57.0), 2.0, 5.0, true),
                (18, f(50.0), 1.0, 0.0, false),
                (21, f(46.0), 0.0, 0.0, false),
                (0, f(44.0), 0.0, 0.0, false),
                (3, f(42.0), 0.0, 0.0, false),
                (6, f(41.0), 1.0, 0.0, false),
                (9, f(47.0), 0.0, 0.0, true),
            ],
            days: [
                ("mon", f(58.0), f(43.0), 2.0, 10.0),
                ("tue", f(61.0), f(47.0), 3.0, 20.0),
                ("wed", f(55.0), f(44.0), 61.0, 45.0),
                ("thu", f(49.0), f(38.0), 3.0, 20.0),
                ("fri", f(52.0), f(40.0), 1.0, 0.0),
            ],
            next: Change::Dry { night: false },
            next_icon: (1.0, true),
        },
        Outlook {
            name: "rainy-afternoon",
            temperature: Temperature::Celsius,
            clock: H24,
            now: (12.0, 3.0, "Cloudy", 14.0, 7.0),
            hours: [
                (12, 13.0, 3.0, 25.0, true),
                (15, 12.0, 63.0, 85.0, true),
                (18, 10.0, 61.0, 60.0, false),
                (21, 9.0, 3.0, 20.0, false),
                (0, 8.0, 2.0, 10.0, false),
                (3, 7.0, 1.0, 0.0, false),
                (6, 6.0, 2.0, 5.0, false),
                (9, 9.0, 3.0, 15.0, true),
            ],
            days: [
                ("mon", 11.0, 5.0, 3.0, 20.0),
                ("tue", 9.0, 4.0, 61.0, 65.0),
                ("wed", 7.0, 2.0, 80.0, 40.0),
                ("thu", 8.0, 3.0, 2.0, 10.0),
                ("fri", 10.0, 4.0, 3.0, 20.0),
            ],
            next: Change::Starts {
                wet: Wet::Rain,
                at: 15,
            },
            next_icon: (63.0, true),
        },
        Outlook {
            name: "snow-week",
            temperature: Temperature::Celsius,
            clock: H12,
            now: (-3.0, 3.0, "Cloudy", -1.0, -7.0),
            hours: [
                (12, -2.0, 3.0, 20.0, true),
                (15, -2.0, 3.0, 30.0, true),
                (18, -4.0, 71.0, 60.0, false),
                (21, -5.0, 73.0, 80.0, false),
                (0, -6.0, 75.0, 85.0, false),
                (3, -7.0, 73.0, 70.0, false),
                (6, -8.0, 71.0, 40.0, false),
                (9, -6.0, 3.0, 20.0, true),
            ],
            days: [
                ("mon", -4.0, -11.0, 73.0, 80.0),
                ("tue", -6.0, -14.0, 3.0, 20.0),
                ("wed", -9.0, -17.0, 0.0, 0.0),
                ("thu", -5.0, -12.0, 71.0, 55.0),
                ("fri", -1.0, -8.0, 85.0, 60.0),
            ],
            next: Change::Tonight {
                wet: Wet::Snow,
                at: 18,
            },
            next_icon: (71.0, false),
        },
        Outlook {
            name: "rain-now",
            temperature: Temperature::Fahrenheit,
            clock: H24,
            now: (f(47.0), 63.0, "Rain", f(50.0), f(41.0)),
            hours: [
                (12, f(48.0), 63.0, 90.0, true),
                (15, f(49.0), 61.0, 70.0, true),
                (18, f(46.0), 2.0, 20.0, false),
                (21, f(44.0), 1.0, 10.0, false),
                (0, f(43.0), 0.0, 0.0, false),
                (3, f(42.0), 0.0, 0.0, false),
                (6, f(41.0), 1.0, 0.0, false),
                (9, f(45.0), 2.0, 10.0, true),
            ],
            days: [
                ("mon", f(52.0), f(40.0), 2.0, 10.0),
                ("tue", f(55.0), f(43.0), 3.0, 15.0),
                ("wed", f(50.0), f(39.0), 95.0, 70.0),
                ("thu", f(45.0), f(34.0), 45.0, 10.0),
                ("fri", f(44.0), f(33.0), 1.0, 0.0),
            ],
            next: Change::Clears { at: 17 },
            next_icon: (2.0, false),
        },
    ]
}

/// The three ways a forecast widget shows.
pub const FORECAST_SHOWS: [&str; 3] = ["hours", "days", "next"];

/// A forecast widget's TOML.
fn forecast_widget(show: &str, width: u32) -> String {
    format!("[[page.widget]]\ntype = \"forecast\"\nwidth = {width}\nshow = \"{show}\"\n")
}

/// A page of `widgets` (TOML) on the default layout, in the outlook's
/// units and clock, with its readings in: drawn by the engine, as the
/// strip would draw it.
fn outlook_frame(o: &Outlook, widgets: &str, set_up: bool) -> Canvas {
    let now = demo::sample_time();
    let mut c = layout(None, set_up);
    c.settings.temperature = o.temperature;
    c.settings.clock = o.clock;
    let text = format!("[[page]]\nname = \"F\"\n{widgets}");
    c.pages = Config::parse(&text).expect("a valid layout").pages;
    let world = World {
        weather: false,
        ..idle()
    };
    let mut e = engine(c, &world, "F");
    if set_up {
        for (name, value) in o.signals() {
            e.handle(Input::Signal(name.into(), value));
        }
    }
    shot(&mut e, &now, 1.0)
}

/// The rows the variants are drawn in: every medium and hero width a
/// layout is likely to give (200, 320, 440, 640) and the compact one.
const VARIANT_ROWS: [&[u32]; 3] = [&[200, 440], &[320, 128, 192], &[640]];

/// Every variant in every outlook at 200, 320, 440 and 640 px (and a
/// compact 128 and a medium 192), for the comparison sheet; and each
/// variant on a Home page beside the clock and the weather now.
pub fn forecast_sheets() -> (Vec<Frame>, Vec<Frame>) {
    let mut variants = Vec::new();
    let mut home = Vec::new();
    for show in FORECAST_SHOWS {
        for o in outlooks() {
            for widths in VARIANT_ROWS {
                let widgets: String = widths.iter().map(|w| forecast_widget(show, *w)).collect();
                let label: Vec<String> = widths.iter().map(u32::to_string).collect();
                variants.push(Frame {
                    group: "forecast-variants",
                    name: format!("{show} · {} · {}", o.name, label.join(" + ")),
                    canvas: outlook_frame(&o, &widgets, true),
                });
            }
            home.push(Frame {
                group: "forecast-home",
                name: format!("home · {show} · {}", o.name),
                canvas: outlook_frame(&o, &home_page(show), true),
            });
        }
    }
    (variants, home)
}

/// Home with a forecast: the clock, the weather now, and the forecast in
/// the 320 px the CPU and media cards had.
fn home_page(show: &str) -> String {
    let (_, widgets) = FORECAST_PAGE.split_once("\n\n").expect("a page");
    widgets.replace("show = \"next\"", &format!("show = \"{show}\""))
}

/// The example forecast page in docs/LAYOUT.md: the forecast beside the
/// clock and the weather now, as `forecast-home-next` draws it.
pub const FORECAST_PAGE: &str = r#"[[page]]
name = "Weather"

[[page.widget]]
type = "clock"
width = 196

[[page.widget]]
type = "value"
label = "Weather"
signal = "weather.temp"
width = 124

[[page.widget]]
type = "forecast"
show = "next"
width = 320
"#;

/// The forecast's checked-in frames: each variant at the specimen widths
/// (its compact form in the 128 px zone) and across the strip, the compact
/// form down to the narrowest zones, the states the weather card shares,
/// and each variant on Home.
fn forecast_frames(out: &mut Out) {
    let all = outlooks();
    let rainy = &all[1];
    let snow = &all[2];
    let specimen_row = |show: &str| -> String {
        SPECIMEN_WIDTHS
            .iter()
            .map(|w| forecast_widget(show, *w))
            .collect()
    };
    for show in FORECAST_SHOWS {
        let name = format!("forecast-{show}");
        out.save(
            "forecast",
            &name,
            outlook_frame(rainy, &specimen_row(show), true),
        );
        let wide = forecast_widget(show, 640);
        out.save(
            "forecast",
            format!("{name}-wide"),
            outlook_frame(snow, &wide, true),
        );
        let home = format!("forecast-home-{show}");
        out.save(
            "forecast",
            home,
            outlook_frame(rainy, &home_page(show), true),
        );
    }
    // The compact form in every zone a layout may give it, widest first.
    let compact: String = [128, 112, 106, 96, 72, 64, 62]
        .iter()
        .map(|w| forecast_widget("hours", *w))
        .collect();
    out.save(
        "forecast",
        "forecast-compact",
        outlook_frame(rainy, &compact, true),
    );
    out.save(
        "forecast",
        "forecast-compact-12h",
        outlook_frame(snow, &compact, true),
    );
    // The weather card's states: not set up, no forecast yet, and lost.
    let row = specimen_row("next");
    out.save(
        "forecast",
        "forecast-not-set-up",
        outlook_frame(rainy, &row, false),
    );
    let now = demo::sample_time();
    let mut c = layout(None, true);
    let text = format!("[[page]]\nname = \"F\"\n{row}");
    c.pages = Config::parse(&text).expect("a valid layout").pages;
    let world = World {
        weather: false,
        ..idle()
    };
    let mut e = engine(c.clone(), &world, "F");
    out.save("forecast", "forecast-no-data", shot(&mut e, &now, 1.0));
    let mut e = engine(c, &world, "F");
    for (name, value) in rainy.signals() {
        e.handle(Input::Signal(name.into(), value));
    }
    // Twelve minutes after the last forecast came, the source stopped.
    e.handle(Input::Forget("weather.temp".into()));
    out.save("forecast", "forecast-lost", shot(&mut e, &now, 1.0));
}

/// A still finger at `from` past the long press, then a slide to `to` and
/// a rest there, from monotonic time `t`, with the finger still down.
/// Returns the time after the last sample.
fn hold_slide(e: &mut Engine, mut t: f64, from: f32, to: f32) -> f64 {
    let mut touch = |x: f32, t: &mut f64| {
        e.handle(Input::Touch {
            down: true,
            x,
            at: *t,
        });
        *t += 0.01;
    };
    for _ in 0..70 {
        touch(from, &mut t);
    }
    for i in 1..=30 {
        touch(from + (to - from) * i as f32 / 30.0, &mut t);
    }
    for _ in 0..40 {
        touch(to, &mut t);
    }
    t
}

/// The layout picker (`DESIGN.md`, Layout picker) and the new layout's
/// name after a switch from the strip.
fn layout_switch(out: &mut Out) {
    const GROUP: &str = "layout-switch";
    let now = demo::sample_time();
    let names = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    let on_home = |list: &[String], current: usize| {
        let mut e = engine(layout(None, true), &idle(), "Home");
        e.handle(Input::Layouts {
            names: list.to_vec(),
            current: Some(current),
        });
        e
    };

    // Opening: held on Home, slid just past the threshold.
    let three = names(&["Desk", "Gaming", "Night"]);
    let mut e = on_home(&three, 1);
    let t = hold_slide(&mut e, 10.0, 300.0, 330.0);
    out.save(GROUP, "picker-open", shot(&mut e, &now, t));

    // Mid-slide on each card of five, from the middle one; the first and
    // the last are the ends, where it stops.
    let mut r = Renderer::new(Theme::default(), crate::fonts::FontChoice::default().load());
    let five = names(&["Desk", "Gaming", "Movies", "Night", "Work"]);
    for selected in 0..five.len() {
        let canvas = r.render_picker(&five, 2, selected);
        out.save(GROUP, format!("picker-select-{selected}"), canvas);
    }
    out.save(
        GROUP,
        "picker-two",
        r.render_picker(&names(&["Desk", "Gaming"]), 0, 1),
    );
    let long = names(&["Desk", "Late-night streaming with chat", "Night"]);
    out.save(GROUP, "picker-long-name", r.render_picker(&long, 0, 1));
    // Nine layouts: more than fit, so the row scrolls with the selection.
    let nine = names(&[
        "Coding", "Desk", "Focus", "Gaming", "Home", "Movies", "Music", "Night", "Work",
    ]);
    for (name, selected) in [("many", 4), ("many-scrolled", 7), ("many-end", 8)] {
        out.save(
            GROUP,
            format!("picker-{name}"),
            r.render_picker(&nine, 4, selected),
        );
    }

    // After the switch: the new layout's first page under its name, as
    // it fades in, and standing.
    let fade_in = f64::from(crate::theme::MS_TAKEOVER_IN) / 1000.0;
    for (name, list, to, at) in [
        ("layout-name", three.clone(), 2, fade_in + 0.4),
        ("layout-name-fading-in", three.clone(), 2, fade_in / 2.0),
        ("layout-name-long", long.clone(), 1, fade_in + 0.4),
    ] {
        let mut e = on_home(&list, 0);
        e.handle(Input::ShowPage(2));
        let x = 300.0 + 24.0 + 72.0 * to as f32 + 6.0;
        let t = hold_slide(&mut e, 10.0, 300.0, x);
        e.handle(Input::Touch {
            down: false,
            x: 0.0,
            at: t,
        });
        let asked = e.take_effects();
        assert_eq!(
            asked,
            [crate::engine::Effect::SwitchLayout(list[to].clone())],
            "{name}"
        );
        // What the daemon does: the new layout, then the list.
        e.handle(Input::Config(Box::new(layout(None, true))));
        e.handle(Input::Layouts {
            names: list.clone(),
            current: Some(to),
        });
        e.frame(&now, t);
        out.save(GROUP, name, shot(&mut e, &now, t + at));
    }
}
