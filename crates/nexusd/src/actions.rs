//! Carries out the tap and hold actions the engine cannot do itself.
//!
//! Nothing here waits: a program or a URL is handed off and the daemon
//! carries on drawing. Failures are logged, since a tap has nowhere else to
//! report them.

use std::process::{Command, Stdio};
use std::thread;

use nexus_core::config::Action;

pub fn carry_out(action: Action) {
    match action {
        Action::Run(argv) => launch(&argv),
        Action::Open(target) => launch(&opener(&target)),
        Action::Media(command) => {
            // A D-Bus call can stall on a hung player; keep it off the
            // daemon thread.
            let _ = thread::Builder::new()
                .name("media-command".into())
                .spawn(move || nexus_sources::media_command(command));
        }
        // Done by the engine before it gets here.
        Action::Page(_) | Action::Publish { .. } => {}
    }
}

/// Starts a program and lets it run on its own.
///
/// As a systemd service, nexusd runs sandboxed: a read-only system, no GPU,
/// a short device list. A program started directly would inherit all of
/// that, so a GUI app would fail. Under systemd it is started with
/// `systemd-run --user` instead, which asks the user's service manager to
/// run it as its own unit, outside this sandbox, as if from the launcher.
fn launch(argv: &[String]) {
    let Some((program, args)) = argv.split_first() else {
        return;
    };
    let under_systemd = cfg!(target_os = "linux") && std::env::var_os("INVOCATION_ID").is_some();
    let mut command = if under_systemd {
        let mut c = Command::new("systemd-run");
        c.args(["--user", "--collect", "--quiet", "--"])
            .arg(program)
            .args(args);
        c
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    };
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    match command.spawn() {
        // Reap it when it exits, so it does not linger as a zombie.
        Ok(mut child) => {
            let _ = thread::Builder::new()
                .name("launched".into())
                .spawn(move || child.wait());
        }
        Err(e) => eprintln!("nexusd: tap action: cannot start {program}: {e}"),
    }
}

/// The command that opens a URL or file with the desktop's default app.
fn opener(target: &str) -> Vec<String> {
    let argv: &[&str] = if cfg!(target_os = "macos") {
        &["open"]
    } else if cfg!(target_os = "windows") {
        // `start` is a cmd built-in; the empty string is its window title.
        &["cmd", "/C", "start", ""]
    } else {
        &["xdg-open"]
    };
    argv.iter()
        .map(|s| s.to_string())
        .chain([target.to_string()])
        .collect()
}
