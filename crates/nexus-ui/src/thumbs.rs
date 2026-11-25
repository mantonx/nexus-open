//! Pictures of strips the daemon is not showing: every page and takeover
//! of the layout in the storyboard, each widget in the gallery, each
//! starter layout. Drawn here with `nexus-core`'s own engine, from the
//! layout and the latest readings, so they are the same pixels the strip
//! would show. Only the page on the strip comes from the daemon.
//!
//! The engine loads its fonts once, on its own thread; a request is
//! answered with RGBA rows, 640×48 each, turned into images on the UI
//! thread. The track's artwork comes from the daemon as the strip draws
//! it (`Job::Art`), so a media widget here shows the same cover.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use jiff::Zoned;
use nexus_core::art::Art;
use nexus_core::config::{Config, Page, Widget};
use nexus_core::engine::{Engine, Input};
use nexus_core::signal::{Unit, Value};

/// A strip's pixels, 640×48 RGBA.
pub type Pixels = Vec<u8>;

/// What to draw.
pub enum Job {
    /// Every page, then every takeover (each over the first page, as it
    /// would cover it).
    Layout {
        config: Box<Config>,
        readings: Vec<(String, Value)>,
        /// Answered with this, so an old answer can be told apart.
        epoch: u64,
    },
    /// One strip per widget, each alone at the left of the strip.
    Widgets {
        widgets: Vec<Widget>,
        config: Box<Config>,
        readings: Vec<(String, Value)>,
    },
    /// The first page of each layout.
    Starters {
        configs: Vec<Config>,
        readings: Vec<(String, Value)>,
    },
    /// The track's artwork, as the daemon prepared it: drawn from now on
    /// wherever `media.art` in the readings names it. Draws nothing.
    Art(Arc<Art>),
    /// One widget alone, as an example: the form shows it while the
    /// strip has nothing real to show for it.
    Example {
        widget: Box<Widget>,
        config: Box<Config>,
        readings: Vec<(String, Value)>,
    },
}

pub enum Done {
    Layout {
        pages: Vec<Pixels>,
        takeovers: Vec<Pixels>,
        epoch: u64,
    },
    Widgets(Vec<Pixels>),
    Starters(Vec<Pixels>),
    Example(Pixels),
}

/// Starts the drawing thread; `done` is called on it with each answer.
pub fn start(done: impl Fn(Done) + Send + 'static) -> Sender<Job> {
    let (tx, rx) = mpsc::channel::<Job>();
    let _ = thread::Builder::new()
        .name("thumbs".into())
        .spawn(move || run(&rx, done));
    tx
}

fn run(jobs: &Receiver<Job>, done: impl Fn(Done)) {
    let mut drawer: Option<Drawer> = None;
    let mut art: Option<Arc<Art>> = None;
    while let Ok(first) = jobs.recv() {
        let mut pending = vec![first];
        pending.extend(jobs.try_iter());
        // Only the newest layout matters; older ones waiting are skipped.
        let newest = pending
            .iter()
            .rposition(|j| matches!(j, Job::Layout { .. }));
        for (i, job) in pending.into_iter().enumerate() {
            if matches!(job, Job::Layout { .. }) && Some(i) != newest {
                continue;
            }
            if let Job::Art(a) = job {
                art = Some(a);
                continue;
            }
            handle(&mut drawer, job, art.as_ref(), &done);
        }
    }
}

fn handle(drawer: &mut Option<Drawer>, job: Job, art: Option<&Arc<Art>>, done: &impl Fn(Done)) {
    match job {
        Job::Layout {
            config,
            readings,
            epoch,
        } => {
            let d = Drawer::for_job(drawer, *config, &readings);
            d.readings(&readings, art);
            let pages = (0..d.engine.config().pages.len())
                .map(|i| d.page(i))
                .collect();
            let names: Vec<String> = d
                .engine
                .config()
                .takeovers
                .iter()
                .map(|t| t.name.clone())
                .collect();
            let takeovers = names.into_iter().map(|n| d.takeover(&n)).collect();
            done(Done::Layout {
                pages,
                takeovers,
                epoch,
            });
        }
        Job::Widgets {
            widgets,
            config,
            readings,
        } => {
            let mut out = Vec::new();
            for w in widgets {
                let mut c = (*config).clone();
                c.pages = vec![Page {
                    name: "Gallery".into(),
                    widgets: vec![w],
                }];
                c.takeovers.clear();
                let d = Drawer::for_job(drawer, c, &readings);
                d.readings(&readings, art);
                out.push(d.page(0));
            }
            done(Done::Widgets(out));
        }
        Job::Example {
            widget,
            config,
            readings,
        } => {
            let mut c = *config;
            c.pages = vec![Page {
                name: "Example".into(),
                widgets: vec![*widget],
            }];
            c.takeovers.clear();
            let d = Drawer::for_job(drawer, c, &readings);
            d.readings(&readings, art);
            done(Done::Example(d.page(0)));
        }
        Job::Starters { configs, readings } => {
            let mut out = Vec::new();
            for c in configs {
                let d = Drawer::for_job(drawer, c, &readings);
                d.readings(&readings, art);
                out.push(d.page(0));
            }
            done(Done::Starters(out));
        }
        Job::Art(_) => {}
    }
}

struct Drawer {
    engine: Engine,
    mono: f64,
    /// The readings the engine holds, by name.
    known: HashSet<String>,
}

impl Drawer {
    /// The drawer, given `config`, for a job with `readings`. Made once
    /// (fonts load here) and kept while each job's readings cover the
    /// last's; made again when one would leave a reading behind (the
    /// gallery's examples, then the layout's own pictures), as an engine
    /// keeps every reading it was given.
    fn for_job<'a>(
        slot: &'a mut Option<Drawer>,
        config: Config,
        readings: &[(String, Value)],
    ) -> &'a mut Drawer {
        let names: HashSet<String> = readings.iter().map(|(n, _)| n.clone()).collect();
        match slot {
            Some(d) if d.known.is_subset(&names) => {
                d.engine.handle(Input::Config(Box::new(config)));
                d.known = names;
            }
            _ => {
                let mono = slot.as_ref().map_or(0.0, |d| d.mono);
                *slot = Some(Drawer {
                    engine: Engine::new(config),
                    mono,
                    known: names,
                });
            }
        }
        slot.as_mut().expect("just set")
    }

    /// The readings, and the artwork beside them: the engine draws it
    /// while `media.art` names it, as the daemon's does.
    fn readings(&mut self, readings: &[(String, Value)], art: Option<&Arc<Art>>) {
        if let Some(art) = art {
            self.engine.handle(Input::Art(Arc::clone(art)));
        }
        for (name, value) in readings {
            self.engine
                .handle(Input::Signal(name.clone(), value.clone()));
        }
    }

    fn draw(&mut self) -> Pixels {
        // A picture of a page, not of the moment: no alarm over it, and no
        // takeover a reading's change set off.
        self.engine.handle(Input::Acknowledge);
        self.engine.handle(Input::Redraw);
        self.mono += 1.0;
        self.engine
            .frame(&Zoned::now(), self.mono)
            .map(|c| c.rgba().to_vec())
            .unwrap_or_else(|| vec![0; (nexus_core::WIDTH * nexus_core::HEIGHT * 4) as usize])
    }

    fn page(&mut self, index: usize) -> Pixels {
        self.engine.handle(Input::EndTakeover);
        self.engine.handle(Input::ShowPage(index));
        self.draw()
    }

    fn takeover(&mut self, name: &str) -> Pixels {
        self.engine.handle(Input::EndTakeover);
        self.engine.handle(Input::ShowPage(0));
        self.engine.handle(Input::ShowTakeover {
            name: name.into(),
            seconds: 60.0,
        });
        let pixels = self.draw();
        self.engine.handle(Input::EndTakeover);
        pixels
    }
}

/// A reading as the engine takes it, from what the daemon reported.
pub fn value(text: &str, number: Option<f64>, unit: Option<&str>) -> Value {
    match number {
        Some(n) => Value::Number(n, unit.and_then(Unit::from_name).unwrap_or(Unit::None)),
        None => Value::Text(text.into()),
    }
}

/// Readings for pictures: the live ones, and the demo's for anything not
/// published, so a gallery picture is never empty. With no forecast yet,
/// an example one (a cloudy day with rain from 15:00), whole, so a
/// forecast's picture shows what it would.
pub fn with_demo(live: Vec<(String, Value)>) -> Vec<(String, Value)> {
    let has = |list: &[(String, Value)], name: &str| list.iter().any(|(n, _)| n == name);
    let example = if has_forecast(&live) {
        Vec::new()
    } else {
        example_forecast()
    };
    let mut out: Vec<(String, Value)> = nexus_core::demo::readings(0.0)
        .into_iter()
        .map(|(n, v)| (n.to_string(), v))
        .filter(|(name, _)| !has(&live, name) && !has(&example, name))
        .collect();
    out.extend(example.into_iter().filter(|(name, _)| !has(&live, name)));
    out.extend(live);
    out
}

/// Whether there is a real forecast to show.
pub fn has_forecast(live: &[(String, Value)]) -> bool {
    live.iter().any(|(n, _)| n == nexus_core::forecast::NEXT)
}

/// The example forecast's readings: the design's rainy afternoon.
fn example_forecast() -> Vec<(String, Value)> {
    nexus_core::scenes::outlooks()
        .into_iter()
        .find(|o| o.name == "rainy-afternoon")
        .map(|o| {
            o.signals()
                .into_iter()
                .map(|(n, v)| (n.to_string(), v))
                .collect()
        })
        .unwrap_or_default()
}

/// The left `width` columns of a strip's pixels.
pub fn crop(pixels: &[u8], width: u32) -> (Vec<u8>, u32) {
    let full = nexus_core::WIDTH as usize;
    let w = (width as usize).clamp(1, full);
    let mut out = Vec::with_capacity(w * 4 * nexus_core::HEIGHT as usize);
    for row in pixels.chunks_exact(full * 4) {
        out.extend_from_slice(&row[..w * 4]);
    }
    (out, w as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crop_keeps_the_left_columns_of_every_row() {
        let full = nexus_core::WIDTH as usize;
        let rows = nexus_core::HEIGHT as usize;
        let pixels: Vec<u8> = (0..full * rows * 4).map(|i| (i / 4 % full) as u8).collect();
        let (out, w) = crop(&pixels, 3);
        assert_eq!(w, 3);
        assert_eq!(out.len(), 3 * 4 * rows);
        assert_eq!(&out[..12], &[0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2]);
        assert_eq!(&out[12..16], &[0, 0, 0, 0]);
    }

    #[test]
    fn the_demo_fills_in_only_what_is_not_live() {
        let live = vec![("cpu.temp".to_string(), Value::Number(99.0, Unit::Celsius))];
        let all = with_demo(live);
        let cpu: Vec<_> = all.iter().filter(|(n, _)| n == "cpu.temp").collect();
        assert_eq!(cpu.len(), 1);
        assert_eq!(cpu[0].1, Value::Number(99.0, Unit::Celsius));
        assert!(all.iter().any(|(n, _)| n == "gpu.temp"));
    }

    #[test]
    fn every_page_and_takeover_is_drawn() {
        let (tx, rx) = mpsc::channel();
        let jobs = start(move |d| {
            let _ = tx.send(d);
        });
        let config = Config::default_layout();
        let (pages, takeovers) = (config.pages.len(), config.takeovers.len());
        jobs.send(Job::Layout {
            config: Box::new(config),
            readings: with_demo(Vec::new()),
            epoch: 7,
        })
        .unwrap();
        match rx.recv().unwrap() {
            Done::Layout {
                pages: p,
                takeovers: t,
                epoch,
            } => {
                assert_eq!(epoch, 7);
                assert_eq!(p.len(), pages);
                assert_eq!(t.len(), takeovers);
                assert!(p.iter().chain(&t).all(|px| px.len() == 640 * 48 * 4));
                // Something is drawn, not a blank strip.
                assert!(p[0].chunks(4).any(|c| c[..3] != p[0][..3]));
            }
            _ => panic!("wrong answer"),
        }
    }

    /// A picture shows its own readings only: the gallery's examples never
    /// reach the pictures of the layout drawn after them.
    #[test]
    fn a_picture_shows_only_its_own_readings() {
        let config = Config::parse(
            "[[page]]\nname = \"A\"\n[[page.widget]]\ntype = \"value\"\nwidth = 200\nlabel = \"Weather\"\nsignal = \"weather.temp\"\n",
        )
        .unwrap();
        let draw = |slot: &mut Option<Drawer>, readings: Vec<(String, Value)>| {
            let d = Drawer::for_job(slot, config.clone(), &readings);
            d.readings(&readings, None);
            d.page(0)
        };
        let fresh = draw(&mut None, Vec::new());
        let mut slot = None;
        let example = draw(&mut slot, with_demo(Vec::new()));
        assert_ne!(example, fresh, "the example shows a temperature");
        assert_eq!(draw(&mut slot, Vec::new()), fresh);
    }

    /// With no forecast, pictures get an example one; a real one is never
    /// mixed with it.
    #[test]
    fn a_forecast_picture_has_an_example_until_a_real_one_comes() {
        let none = with_demo(Vec::new());
        assert!(has_forecast(&none), "the example is there");
        let real = vec![(
            nexus_core::forecast::NEXT.to_string(),
            Value::Text("dry".into()),
        )];
        let mixed = with_demo(real.clone());
        let next: Vec<_> = mixed
            .iter()
            .filter(|(n, _)| n == nexus_core::forecast::NEXT)
            .collect();
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].1, real[0].1);
        assert!(
            !mixed.iter().any(|(n, _)| n.starts_with("weather.hour")),
            "no example hours beside the real forecast"
        );
    }

    /// The pictures show the track's artwork once the daemon has sent it,
    /// exactly where `media.art` names it, as the strip does.
    #[test]
    fn artwork_reaches_the_pictures() {
        let (tx, rx) = mpsc::channel();
        let jobs = start(move |d| {
            let _ = tx.send(d);
        });
        let mut readings = with_demo(Vec::new());
        readings.extend([
            ("media.title".to_string(), Value::Text("Song".into())),
            ("media.status".to_string(), Value::Text("playing".into())),
            ("media.art".to_string(), Value::Text("art-test".into())),
        ]);
        let layout = |epoch| Job::Layout {
            config: Box::new(Config::default_layout()),
            readings: readings.clone(),
            epoch,
        };
        let pages = |d: Done| match d {
            Done::Layout { pages, .. } => pages,
            _ => panic!("wrong answer"),
        };
        jobs.send(layout(1)).unwrap();
        let without = pages(rx.recv().unwrap());
        let rgba = nexus_core::art::test_card(80, 60);
        let art = Art::prepare("art-test", 80, 60, &rgba).unwrap();
        jobs.send(Job::Art(Arc::new(art))).unwrap();
        jobs.send(layout(2)).unwrap();
        let with = pages(rx.recv().unwrap());
        // The Music page: no clock on it, so only the artwork can differ.
        assert_eq!(Config::default_layout().pages[1].name, "Music");
        assert_ne!(without[1], with[1], "the media card shows the art");
    }
}
