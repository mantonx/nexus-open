//! nexusd: drives the Corsair iCUE Nexus touch strip.
//!
//! Commands so far:
//! - `preview` shows the demo System page on the real strip, which is how the
//!   renderer and fonts are judged on hardware and how replug recovery is
//!   measured.
//! - `ui` opens the settings window. It is the same binary so installs stay
//!   one file; the window runs as its own process and the strip will keep
//!   working with it closed.

use std::process::ExitCode;
use std::time::Duration;

use clap::{Parser, Subcommand};
use nexus_core::fonts::FontChoice;

#[cfg(target_os = "linux")]
mod preview;

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
    /// Open the settings window.
    Ui,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let result: Result<(), Box<dyn std::error::Error>> = match cli.command {
        Command::Preview { font, cycle, touch } => {
            preview(font, cycle.map(Duration::from_secs), touch).map_err(Into::into)
        }
        Command::Ui => nexus_ui::run().map_err(Into::into),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("nexusd: {e}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(target_os = "linux")]
use preview::preview;

#[cfg(not(target_os = "linux"))]
fn preview(_: FontChoice, _: Option<Duration>, _: bool) -> Result<(), nexus_device::DeviceError> {
    Err(nexus_device::DeviceError::Unsupported)
}
