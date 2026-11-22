//! nexusd: drives the Corsair iCUE Nexus touch strip.
//!
//! Commands so far:
//! - `run` is the daemon.
//! - `preview` shows the demo System page on the real strip, which is how the
//!   renderer and fonts are judged on hardware and how replug recovery is
//!   measured. With `--card` it shows the legibility card instead, and with
//!   `--frames` a folder of mockup frames.
//! - `trace` records raw touches for tuning gestures.
//! - `ui` opens the settings window. It is the same binary so installs stay
//!   one file; the window runs as its own process and the strip will keep
//!   working with it closed.

use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use nexus_core::fonts::FontChoice;

mod actions;
mod control;
mod notify;
#[cfg(target_os = "linux")]
mod preview;
mod run;
mod slides;
mod sources;
#[cfg(target_os = "linux")]
mod trace;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the demo System page on the strip, or test frames.
    ///
    /// Stop the daemon first (systemctl --user stop nexusd): both would draw
    /// on the strip and both would act on touches.
    Preview {
        /// Font to draw with.
        #[arg(long, default_value_t = FontChoice::default())]
        font: FontChoice,
        /// Move to the next font (or frame) every this many seconds.
        #[arg(long, value_name = "SECONDS")]
        cycle: Option<u64>,
        /// Print touch positions as you touch the strip.
        #[arg(long)]
        touch: bool,
        /// Show the legibility card. Tap the right half for the next frame,
        /// the left half for the previous one.
        #[arg(long, conflicts_with = "frames")]
        card: bool,
        /// Show every 640×48 *.png in this folder, in name order, stepped
        /// through like the card.
        #[arg(long, value_name = "DIR")]
        frames: Option<std::path::PathBuf>,
        /// With --card: write the card's frames and a 3× sheet as PNGs to
        /// this folder instead of showing them on the strip.
        #[arg(long, value_name = "DIR", requires = "card")]
        out: Option<std::path::PathBuf>,
    },
    /// Run the daemon: show the configured pages on the strip.
    Run {
        /// The store. Default: nexus-open/nexusd.db in the user's state
        /// directory, filled on first start from nexus-open/nexus.toml if
        /// there is one, else with the default layout.
        #[arg(long, value_name = "PATH")]
        db: Option<std::path::PathBuf>,
        /// Publish fake readings, for testing before real sources exist.
        #[arg(long)]
        demo: bool,
    },
    /// Open the settings window.
    Ui,
    /// Record raw touches to a file, alongside a running daemon, for tuning
    /// taps, long presses and accidental brushes.
    Trace {
        /// Where to write the samples.
        out: std::path::PathBuf,
        /// How long to record.
        #[arg(long, default_value_t = 30)]
        seconds: u64,
    },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result: Result<(), Box<dyn std::error::Error>> = match cli.command {
        Command::Preview {
            font,
            cycle,
            touch,
            card,
            frames,
            out,
        } => preview_command(
            font,
            cycle.map(Duration::from_secs),
            touch,
            card,
            frames,
            out,
        ),
        Command::Run { db, demo } => run::run(run::Options { db, demo }),
        Command::Ui => nexus_ui::run().map_err(Into::into),
        Command::Trace { out, seconds } => trace(&out, seconds),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nexusd: {e}");
            ExitCode::FAILURE
        }
    }
}

/// `nexusd preview`: picks what to show, or with `--out` writes the card to
/// files without opening the device.
fn preview_command(
    font: FontChoice,
    cycle: Option<Duration>,
    touch: bool,
    card: bool,
    frames: Option<std::path::PathBuf>,
    out: Option<std::path::PathBuf>,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(out) = out {
        for path in slides::write_card(&out)? {
            println!("wrote {}", path.display());
        }
        return Ok(());
    }
    let show = if card {
        Show::Slides(slides::card())
    } else if let Some(dir) = frames {
        let (found, skipped) =
            slides::from_dir(&dir).map_err(|e| format!("cannot read {}: {e}", dir.display()))?;
        for line in &skipped {
            println!("{line}");
        }
        if found.is_empty() {
            return Err(format!("no 640×48 PNG frames in {}", dir.display()).into());
        }
        Show::Slides(found)
    } else {
        Show::Demo(font)
    };
    preview(show, cycle, touch).map_err(Into::into)
}

#[cfg(target_os = "linux")]
use preview::{Show, preview};

#[cfg(target_os = "linux")]
use trace::trace;

#[cfg(not(target_os = "linux"))]
fn trace(_: &std::path::Path, _: u64) -> Result<(), Box<dyn std::error::Error>> {
    Err(nexus_device::DeviceError::Unsupported.into())
}

#[cfg(not(target_os = "linux"))]
#[allow(dead_code)] // built, then refused by the stub below
enum Show {
    Demo(FontChoice),
    Slides(Vec<slides::Slide>),
}

#[cfg(not(target_os = "linux"))]
fn preview(_: Show, _: Option<Duration>, _: bool) -> Result<(), nexus_device::DeviceError> {
    Err(nexus_device::DeviceError::Unsupported)
}
