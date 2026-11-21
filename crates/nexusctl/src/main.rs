//! nexusctl: control and diagnose the Nexus Open daemon.

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use nexus_control::{Client, SignalEntry, Status};

/// Like `print!`, but a reader that goes away (`nexusctl export | head`)
/// ends the program quietly, where `print!` would panic.
macro_rules! say {
    ($($arg:tt)*) => {
        $crate::out(&format!($($arg)*))
    };
}

mod doctor;
mod edit;
mod legacy;

#[derive(Parser)]
#[command(version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Show the daemon's state: device, page, layout, store.
    Status,
    /// Check everything the strip needs and say how to fix what is wrong.
    Doctor,
    /// List the Corsair devices plugged in: what each is for, and what
    /// blocks it. Sends nothing to them.
    Devices,
    /// List every signal and its latest value.
    Signals,
    /// Show a page: next, prev, a number counting from 1, or a name.
    Page { target: String },
    /// Re-read the store now.
    Reload,
    /// Show a takeover now, for some seconds; or --end to put the page
    /// back.
    Takeover {
        #[arg(required_unless_present = "end")]
        name: Option<String>,
        /// How long, up to 60.
        #[arg(long, default_value_t = 10.0)]
        seconds: f64,
        #[arg(long, conflicts_with = "name")]
        end: bool,
    },
    /// Edit the layout, settings and sources as TOML in $VISUAL or $EDITOR.
    /// Saving and closing applies it; comments directly above a table are
    /// kept as notes.
    Edit {
        /// Another layout than the one on the strip.
        #[arg(long)]
        layout: Option<String>,
    },
    /// Print a layout, the settings and sources as TOML, for sharing or
    /// backing up.
    Export {
        #[arg(long)]
        layout: Option<String>,
    },
    /// Add a layout from a TOML file, as `export` writes it.
    Import {
        file: std::path::PathBuf,
        /// Its name. Default: the file name without its extension.
        #[arg(long)]
        name: Option<String>,
        /// Put it on the strip.
        #[arg(long)]
        activate: bool,
    },
    /// Add a layout from the old Go and Flutter Nexus Open: its database,
    /// or a YAML file its Export wrote. Says what could not be carried over.
    ImportLegacy {
        /// Default: the old app's database, nexus-open/nexus.db in the
        /// config folder.
        path: Option<std::path::PathBuf>,
        /// Its name. Default: the old layout's name, or "Old layout".
        #[arg(long)]
        name: Option<String>,
        /// Put it on the strip.
        #[arg(long)]
        activate: bool,
        /// Only print the converted layout and what was left out.
        #[arg(long, conflicts_with_all = ["name", "activate"])]
        print: bool,
    },
    /// Take back the last change.
    Undo,
    /// Make the last undone change again.
    Redo,
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
    /// Drag along the strip from one x to another (0–639): a swipe, or a
    /// slide on a slider.
    Drag { from: f32, to: f32 },
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    if let Command::Edit { layout } = &cli.command {
        let mut client = match Client::connect() {
            Ok(c) => c,
            Err(_) => {
                eprintln!("nexusctl: nexusd is not running. Start it with: nexusd run");
                return ExitCode::FAILURE;
            }
        };
        return match edit::run(&mut client, layout.as_deref()) {
            Ok(()) => ExitCode::SUCCESS,
            Err(e) => {
                eprintln!("nexusctl: {e}");
                ExitCode::FAILURE
            }
        };
    }
    if let Command::ImportLegacy {
        path,
        name,
        activate,
        print,
    } = &cli.command
    {
        if *print {
            return legacy::run(None, path.as_deref(), None, false);
        }
        let Ok(mut client) = Client::connect() else {
            eprintln!("nexusctl: nexusd is not running. Start it with: nexusd run");
            return ExitCode::FAILURE;
        };
        return legacy::run(
            Some(&mut client),
            path.as_deref(),
            name.as_deref(),
            *activate,
        );
    }
    let line = match &cli.command {
        Command::Doctor => return doctor::run(),
        Command::Devices => return doctor::devices(),
        Command::ImportLegacy { .. } => unreachable!("handled above"),
        Command::Edit { .. } => unreachable!("handled above"),
        Command::Export { layout } => match layout {
            Some(name) => format!("export {name}"),
            None => "export".into(),
        },
        Command::Import {
            file,
            name,
            activate,
        } => {
            let text = match std::fs::read_to_string(file) {
                Ok(t) => t,
                Err(e) => {
                    eprintln!("nexusctl: {}: {e}", file.display());
                    return ExitCode::FAILURE;
                }
            };
            let name = name.clone().unwrap_or_else(|| {
                file.file_stem()
                    .map_or("Imported".into(), |s| s.to_string_lossy().into_owned())
            });
            let request = nexus_control::ImportRequest {
                name,
                text,
                activate: *activate,
            };
            format!(
                "import {}",
                serde_json::to_string(&request).unwrap_or_default()
            )
        }
        Command::Undo => "undo".into(),
        Command::Redo => "redo".into(),
        Command::Status => "status".to_string(),
        Command::Signals => "signals".to_string(),
        Command::Page { target } => format!("page {target}"),
        Command::Reload => "reload".to_string(),
        Command::Takeover { name, seconds, end } => match (end, name) {
            (true, _) | (_, None) => "end-takeover".into(),
            (false, Some(name)) => format!("show-takeover {seconds} {name}"),
        },
        Command::Publish { signal, value } => format!("publish {signal} {}", value.join(" ")),
        Command::Tap { x } => format!("tap {x}"),
        Command::Hold { x } => format!("hold {x}"),
        Command::Drag { from, to } => format!("drag {from} {to}"),
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
                say!("no signals yet\n");
            }
            let width = entries.iter().map(|e| e.name.len()).max().unwrap_or(0);
            for e in entries {
                say!("{:width$}  {}\n", e.name, e.value);
            }
        }
        Command::Export { .. } => match serde_json::from_value::<nexus_control::Export>(data) {
            Ok(export) => say!("{}", export.text),
            Err(e) => {
                eprintln!("nexusctl: unexpected answer from nexusd: {e}");
                return ExitCode::FAILURE;
            }
        },
        Command::Import { activate, .. } => say!(
            "Added the layout{}.\n",
            if activate {
                " and put it on the strip"
            } else {
                "; switch to it in the settings window"
            }
        ),
        Command::Undo => say!("Undid: {}\n", data["label"].as_str().unwrap_or("?")),
        Command::Redo => say!("Redid: {}\n", data["label"].as_str().unwrap_or("?")),
        _ => {}
    }
    ExitCode::SUCCESS
}

fn print_status(s: &Status) {
    say!("nexusd {} (pid {})\n", s.version, s.pid);
    match &s.device_error {
        None => say!("device   connected\n"),
        Some(why) => say!("device   not connected: {why}\n"),
    }
    let position = s
        .pages
        .iter()
        .position(|p| p == &s.page)
        .map_or(0, |i| i + 1);
    say!(
        "page     {} ({position} of {}: {})\n",
        s.page,
        s.pages.len(),
        s.pages.join(", ")
    );
    say!("layout   {}\n", s.layout);
    say!("store    {}\n", s.store_path);
    if let Some(e) = &s.store_error {
        say!("         cannot be used; showing the default layout, changes are not saved:\n");
        for line in e.lines() {
            say!("         {line}\n");
        }
    }
}

/// Writes to standard output; see `say!`.
fn out(text: &str) {
    use std::io::Write;
    let mut stdout = std::io::stdout().lock();
    if let Err(e) = stdout
        .write_all(text.as_bytes())
        .and_then(|()| stdout.flush())
    {
        if e.kind() == std::io::ErrorKind::BrokenPipe {
            std::process::exit(0);
        }
        eprintln!("nexusctl: {e}");
        std::process::exit(1);
    }
}
