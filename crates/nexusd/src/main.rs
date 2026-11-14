//! nexusd: drives the Corsair iCUE Nexus touch strip.
//!
//! Only the `preview` command exists so far. It shows the demo System page on
//! the real strip, which is how the renderer and fonts are judged on hardware
//! and how replug recovery is measured.

use std::process::ExitCode;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};

use clap::{Parser, Subcommand};
use nexus_core::demo::system_page;
use nexus_core::fonts::FontChoice;
use nexus_core::theme::Theme;
use nexus_core::widget::Renderer;
use nexus_device::hidraw::{Input, Nexus};
use nexus_device::protocol::{TouchCalibration, parse_touch};
use nexus_device::{DeviceError, Surface};

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the demo System page on the strip.
    Preview {
        /// Font to draw with.
        #[arg(long, default_value_t = FontChoice::default())]
        font: FontChoice,
        /// Move to the next font every this many seconds.
        #[arg(long, value_name = "SECONDS")]
        cycle: Option<u64>,
        /// Print touch positions as you touch the strip.
        #[arg(long)]
        touch: bool,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result = match cli.command {
        Command::Preview { font, cycle, touch } => {
            preview(font, cycle.map(Duration::from_secs), touch)
        }
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nexusd: {e}");
            ExitCode::FAILURE
        }
    }
}

/// How often to look for the device while it is unplugged.
const RECONNECT_POLL: Duration = Duration::from_millis(100);

fn preview(mut font: FontChoice, cycle: Option<Duration>, touch: bool) -> Result<(), DeviceError> {
    let mut frame = render(font);
    let (tx, rx) = mpsc::channel();

    let mut device = Nexus::open()?;
    device.spawn_reader(tx.clone())?;
    println!("opened {}", device.path().display());
    show(&mut device, &frame, font)?;

    let mut calibration = TouchCalibration::default();
    let mut last_switch = Instant::now();
    loop {
        let wait = cycle.map_or(Duration::from_secs(3600), |c| {
            c.saturating_sub(last_switch.elapsed())
        });
        match rx.recv_timeout(wait) {
            Ok(Input::Report(report)) => {
                if touch && let Some(t) = parse_touch(&report) {
                    let x = calibration.to_panel_x(t.x);
                    println!(
                        "touch {} x={x:>3} (raw {})",
                        if t.down { "down" } else { "up  " },
                        t.x
                    );
                }
            }
            Ok(Input::Gone(e)) => {
                println!("device gone ({e}); waiting for it to come back");
                let lost = Instant::now();
                device = wait_for_device();
                device.spawn_reader(tx.clone())?;
                show(&mut device, &frame, font)?;
                println!("recovered in {} ms", lost.elapsed().as_millis());
            }
            Err(RecvTimeoutError::Timeout) => {
                font = next(font);
                frame = render(font);
                show(&mut device, &frame, font)?;
                last_switch = Instant::now();
            }
            Err(RecvTimeoutError::Disconnected) => unreachable!("main holds a sender"),
        }
    }
}

fn render(font: FontChoice) -> Vec<u8> {
    let mut renderer = Renderer::new(Theme::default(), font.load());
    renderer.render_page(&system_page()).rgba().to_vec()
}

fn show(device: &mut Nexus, frame: &[u8], font: FontChoice) -> Result<(), DeviceError> {
    let start = Instant::now();
    device.send_frame(frame)?;
    println!(
        "showing {:<22} frame sent in {:.1} ms",
        font.display_name(),
        start.elapsed().as_secs_f64() * 1000.0
    );
    Ok(())
}

/// Blocks until the Nexus can be opened again.
fn wait_for_device() -> Nexus {
    loop {
        match Nexus::open() {
            Ok(device) => return device,
            // Right after replug the node can exist before udev has applied
            // the ACL, so permission errors are retried too.
            Err(DeviceError::NotFound | DeviceError::PermissionDenied(_)) => {
                thread::sleep(RECONNECT_POLL);
            }
            Err(e) => {
                eprintln!("reconnect: {e}");
                thread::sleep(RECONNECT_POLL);
            }
        }
    }
}

fn next(font: FontChoice) -> FontChoice {
    let all = FontChoice::ALL;
    let i = all.iter().position(|&f| f == font).unwrap_or(0);
    all[(i + 1) % all.len()]
}
