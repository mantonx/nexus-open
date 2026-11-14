//! nexusctl: control and diagnose the Nexus Open daemon.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use nexus_control::{Client, SignalEntry, Status};

mod doctor;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the daemon's state: device, page, config.
    Status,
    /// Check everything the strip needs and say how to fix what is wrong.
    Doctor,
    /// List every signal and its latest value.
    Signals,
    /// Show a page: next, prev, a number counting from 1, or a name.
    Page { target: String },
    /// Re-read the config file now.
    Reload,
    /// Set a signal: a number with an optional unit (celsius, percent,
    /// bytes-per-second), or any text.
    Publish {
        signal: String,
        #[arg(required = true, num_args = 1..)]
        value: Vec<String>,
    },
    /// Tap the strip at x (0–639), as a finger would: runs that widget's
    /// tap action.
    Tap { x: f32 },
    /// Long-press the strip at x (0–639): runs that widget's hold action.
    Hold { x: f32 },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let line = match &cli.command {
        Command::Doctor => return doctor::run(),
        Command::Status => "status".to_string(),
        Command::Signals => "signals".to_string(),
        Command::Page { target } => format!("page {target}"),
        Command::Reload => "reload".to_string(),
        Command::Publish { signal, value } => format!("publish {signal} {}", value.join(" ")),
        Command::Tap { x } => format!("tap {x}"),
        Command::Hold { x } => format!("hold {x}"),
    };
    let mut client = match Client::connect() {
        Ok(c) => c,
        Err(_) => {
            eprintln!("nexusctl: nexusd is not running. Start it with: nexusd run");
            return ExitCode::FAILURE;
        }
    };
    let data = match client.request(&line) {
        Ok(Ok(data)) => data,
        Ok(Err(message)) => {
            eprintln!("nexusctl: {message}");
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("nexusctl: lost the connection to nexusd: {e}");
            return ExitCode::FAILURE;
        }
    };
    match cli.command {
        Command::Status => match serde_json::from_value::<Status>(data) {
            Ok(status) => print_status(&status),
            Err(e) => {
                eprintln!("nexusctl: unexpected answer from nexusd: {e}");
                return ExitCode::FAILURE;
            }
        },
        Command::Signals => {
            let entries: Vec<SignalEntry> = serde_json::from_value(data).unwrap_or_default();
            if entries.is_empty() {
                println!("no signals yet");
            }
            let width = entries.iter().map(|e| e.name.len()).max().unwrap_or(0);
            for e in entries {
                println!("{:width$}  {}", e.name, e.value);
            }
        }
        _ => {}
    }
    ExitCode::SUCCESS
}

fn print_status(s: &Status) {
    println!("nexusd {} (pid {})", s.version, s.pid);
    match &s.device_error {
        None => println!("device   connected"),
        Some(why) => println!("device   not connected: {why}"),
    }
    let position = s
        .pages
        .iter()
        .position(|p| p == &s.page)
        .map_or(0, |i| i + 1);
    println!(
        "page     {} ({position} of {}: {})",
        s.page,
        s.pages.len(),
        s.pages.join(", ")
    );
    println!("config   {}", s.config_path);
    if let Some(e) = &s.config_error {
        println!("         rejected, previous layout in use:");
        for line in e.lines() {
            println!("         {line}");
        }
    }
}
