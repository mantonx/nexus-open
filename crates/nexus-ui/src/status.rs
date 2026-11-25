//! The header's status chip: what is wrong, if anything, said plainly,
//! with buttons that fix it. The cause comes from the daemon (why it has
//! no strip) and the doctor's checks; a shell command is only ever an
//! extra detail for those who want it.
//!
//! "Start the service" and "Open the log" run `systemctl` and
//! `journalctl` on the user's `nexusd.service`; a start first clears a
//! failed state (`reset-failed`), which is what stops systemd from
//! starting a service that failed too often. When starting it fails
//! twice, the window offers to run the daemon itself for the session
//! (`here`), and to report the problem with the diagnostics. The commands are built
//! here, apart from running them, so tests check what would run; and
//! `NEXUS_UI_SERVICE_STUB` (for the capture rig and tests only) puts a
//! harmless program in front of them, so a test never touches the real
//! service.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use nexus_control::Status;
use nexus_doctor::{Check, State};

/// The service's unit.
pub const UNIT: &str = "nexusd.service";
/// Test-only: a program run with the real command as its arguments, in
/// place of the command.
pub const STUB_ENV: &str = "NEXUS_UI_SERVICE_STUB";
/// Test-only: pretend the daemon reports this about the strip
/// (`in-use`, `old-app`, `detached`, `unplugged`, `no-access`).
pub const FAKE_DEVICE_ENV: &str = "NEXUS_UI_FAKE_DEVICE";

/// What the chip and its panel show.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct View {
    /// "ok", "note" (works, worth knowing), "app" (the app cannot show
    /// the layout on the strip: a state to fix, not a reading, so never
    /// amber), "stopped", "starting", or "failed" (a try to start it
    /// failed: the one real fault, shown critical).
    pub tone: &'static str,
    /// What the chip's icon means: "ok", "info", "plug", "busy", "lock",
    /// "save", "power", "wait" or "fail".
    pub icon: &'static str,
    /// The chip's words.
    pub chip: String,
    pub headline: String,
    pub body: String,
    /// What can be done, in the order shown, the first the main one:
    /// "start", "retry", "run-here", "report" or "log". The banner and the
    /// chip's popover show the same, in this order. There is no "Try
    /// again" for the strip: the service retries it by itself (every
    /// second at most) and the window follows within a second, so such a
    /// button would do nothing.
    pub actions: Vec<&'static str>,
    /// The daemon runs as this window's child: the banner says so even
    /// while the pictures are live.
    pub here: bool,
    /// The strip is not showing this layout, though the service runs: the
    /// pictures are dimmed and the banner says why. Editing still works.
    pub away: bool,
    /// The banner under the header, when there is one: its first line and
    /// what follows it. Empty when all is well.
    pub banner: String,
    pub banner_body: String,
    /// The fix as commands, for those who want it; may be empty.
    pub detail: String,
}

/// The status for the chip. `status` is the daemon's latest answer, or
/// `None` while it cannot be reached; `checks` are the doctor's.
pub fn view(status: Option<&Status>, checks: &[Check]) -> View {
    let failed: Vec<&Check> = checks.iter().filter(|c| c.state == State::Fail).collect();
    let fix_of = |what: &str| {
        failed
            .iter()
            .find(|c| c.what == what)
            .map(|c| c.fix.clone())
            .unwrap_or_default()
    };
    let Some(status) = status else {
        return View {
            tone: "stopped",
            icon: "power",
            chip: "Not running".into(),
            headline: "The service is not running".into(),
            body: STOPPED.into(),
            actions: vec!["start", "log"],
            banner: "The service is not running".into(),
            banner_body: STOPPED.into(),
            detail: fix_of("daemon"),
            ..View::default()
        };
    };
    if let Some(e) = &status.store_error {
        return View {
            tone: "app",
            icon: "save",
            chip: "Cannot save".into(),
            headline: "Changes cannot be saved".into(),
            body: format!(
                "The strip shows the default layout, and nothing changed here is kept, because the file that holds your layouts cannot be used: {e}. The log may say more."
            ),
            actions: vec!["log"],
            detail: fix_of("store"),
            ..View::default()
        };
    }
    let device_error = fake_device().or_else(|| status.device_error.clone());
    let Some(why) = device_error else {
        let others: Vec<String> = failed
            .iter()
            .map(|c| format!("{}: {}", c.what, c.detail))
            .collect();
        if !others.is_empty() {
            return View {
                tone: "note",
                icon: "info",
                chip: "Strip connected".into(),
                headline: "The strip is working".into(),
                body: "It shows your layout and follows every change here. One check found something worth fixing; the details are below.".into(),
                detail: others.join("\n"),
                ..View::default()
            };
        }
        return View {
            tone: "ok",
            icon: "ok",
            chip: "Strip connected".into(),
            headline: "The strip is working".into(),
            body: "It shows your layout and follows every change made here at once.".into(),
            ..View::default()
        };
    };
    device_view(&why, &fix_of("device"))
}

/// Why the daemon has no strip, in plain words.
fn device_view(why: &str, fix: &str) -> View {
    let warn = |icon: &'static str, chip: &str, headline: &str, body: String| View {
        tone: "app",
        icon,
        chip: chip.into(),
        headline: headline.into(),
        banner: headline.into(),
        banner_body: format!("{body} {STILL_EDITABLE}"),
        body,
        away: true,
        detail: fix.to_string(),
        ..View::default()
    };
    if why.contains("--no-device") {
        return View {
            tone: "note",
            icon: "info",
            chip: "Running, no strip".into(),
            headline: "Running without the strip".into(),
            body: "The service was started for testing and leaves the strip alone. Everything here works; the pictures come from the service.".into(),
            ..View::default()
        };
    }
    if let Some(who) = why.strip_prefix("the strip is in use by ") {
        let who = who.trim();
        if who.starts_with("another program") {
            return warn(
                "busy",
                "Strip in use",
                "Another program is using the strip",
                "Quit it (an older Nexus Open: from its tray icon) and Nexus Open takes the strip back by itself. If the strip stays dark after that, unplug it and plug it back in.".into(),
            );
        }
        if who.contains("nexus-open") {
            return warn(
                "busy",
                "Strip in use",
                "The old Nexus Open app has the strip",
                "Quit it from its tray icon, then unplug the strip and plug it back in: the old app does not hand it back by itself. Nexus Open picks it up as soon as it is free.".into(),
            );
        }
        let name = program_name(who);
        return warn(
            "busy",
            "Strip in use",
            &format!("{name} is using the strip"),
            format!(
                "Quit {name} and Nexus Open takes the strip back by itself. If the strip stays dark after that, unplug it and plug it back in."
            ),
        );
    }
    if why.contains("left without its driver") {
        return warn(
            "plug",
            "Replug the strip",
            "The strip needs to be plugged in again",
            "A program that used it let go without handing it back. Unplug the strip and plug it back in; Nexus Open picks it up by itself.".into(),
        );
    }
    if why.contains("not there yet") {
        return View {
            tone: "note",
            icon: "wait",
            chip: "Starting".into(),
            headline: "The strip is getting ready".into(),
            body: "It was just plugged in, or the computer just woke. It shows your layout in a few seconds.".into(),
            ..View::default()
        };
    }
    if why.contains("no permission") {
        return warn(
            "lock",
            "No access",
            "Nexus Open may not use the strip",
            "The rule that lets it reach the strip is missing. Reinstalling Nexus Open puts it back; then unplug the strip and plug it back in.".into(),
        );
    }
    if why.contains("no iCUE Nexus found") || why.contains("unplugged") {
        return warn(
            "plug",
            "No strip",
            "The strip is not plugged in",
            "Plug in the iCUE Nexus. Nexus Open finds it by itself and shows your layout.".into(),
        );
    }
    let mut body = why.to_string();
    if let Some(first) = body.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    let mut v = warn(
        "plug",
        "Strip problem",
        "The strip cannot be used",
        format!("{body}. Nexus Open keeps trying by itself; the log may say more."),
    );
    v.actions = vec!["log"];
    v
}

/// "steam (pid 4242), obs (pid 7)" as "steam and obs".
fn program_name(holders: &str) -> String {
    let names: Vec<&str> = holders
        .split(", ")
        .map(|h| h.split(" (pid").next().unwrap_or(h).trim())
        .filter(|n| !n.is_empty())
        .collect();
    let names: Vec<String> = names.into_iter().map(known_name).collect();
    match names.as_slice() {
        [] => "Another program".into(),
        [one] => one.clone(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
    }
}

/// A process name as people know the program: "steam" is "Steam".
fn known_name(process: &str) -> String {
    match process.to_ascii_lowercase().as_str() {
        "icue" | "icue.exe" => "iCUE".into(),
        "obs" => "OBS".into(),
        "openrgb" => "OpenRGB".into(),
        "steamwebhelper" | "steam" => "Steam".into(),
        _ => {
            let mut c = process.chars();
            c.next()
                .map_or_else(String::new, |f| f.to_uppercase().chain(c).collect())
        }
    }
}

/// Test-only: the device problem to pretend, from `NEXUS_UI_FAKE_DEVICE`.
fn fake_device() -> Option<String> {
    let v = std::env::var(FAKE_DEVICE_ENV).ok()?;
    Some(
        match v.as_str() {
            "in-use" => "the strip is in use by steam (pid 4242)",
            "old-app" => "the strip is in use by nexus-open (pid 1700)",
            "detached" => "the strip was left without its driver by another program; unplug it and plug it back in",
            "unplugged" => "no iCUE Nexus found; is it plugged in?",
            "no-access" => "no permission to open /dev/hidraw3",
            _ => return None,
        }
        .into(),
    )
}

/// Whether the service is installed for this user, as the doctor checks.
pub fn installed() -> bool {
    let home = std::env::var_os("HOME").map(PathBuf::from);
    let user = home.map(|h| h.join(".config/systemd/user").join(UNIT));
    user.is_some_and(|p| p.exists())
        || ["/usr/lib/systemd/user", "/etc/systemd/user"]
            .iter()
            .any(|d| Path::new(d).join(UNIT).exists())
}

/// What "Start the service" runs: the service when it is installed, else
/// this program's own daemon, in the background.
pub fn start_command(installed: bool, exe: &Path) -> Vec<String> {
    if installed {
        ["systemctl", "--user", "start", UNIT]
            .map(String::from)
            .to_vec()
    } else {
        vec![exe.display().to_string(), "run".into()]
    }
}

/// What clears the service's failed state before a start: systemd will
/// not start a service again that failed too often until it is cleared.
pub fn reset_command() -> Vec<String> {
    ["systemctl", "--user", "reset-failed", UNIT]
        .map(String::from)
        .to_vec()
}

/// What "Open the log" runs: the service's recent log, as text.
pub fn log_command() -> Vec<String> {
    [
        "journalctl",
        "--user",
        "-u",
        UNIT,
        "-n",
        "200",
        "--no-pager",
        "-o",
        "short",
    ]
    .map(String::from)
    .to_vec()
}

/// `argv` behind the stub, if one is set.
pub fn stubbed(argv: Vec<String>, stub: Option<String>) -> Vec<String> {
    match stub {
        Some(stub) if !stub.is_empty() => std::iter::once(stub).chain(argv).collect(),
        _ => argv,
    }
}

fn stub() -> Option<String> {
    std::env::var(STUB_ENV).ok()
}

/// Starts the daemon. Blocks until `systemctl` answers (a few seconds at
/// most); a daemon started directly is left running in the background.
/// Returns what went wrong, if anything.
pub fn start() -> Result<(), String> {
    let installed = installed();
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if installed {
        // Its answer does not matter: a service that has not failed has
        // nothing to clear, and the start says what is wrong.
        let reset = stubbed(reset_command(), stub());
        let _ = Command::new(&reset[0])
            .args(&reset[1..])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let argv = stubbed(start_command(installed, &exe), stub());
    let mut cmd = Command::new(&argv[0]);
    cmd.args(&argv[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if !installed && std::env::var_os(STUB_ENV).is_none() {
        return cmd.spawn().map(drop).map_err(|e| could_not_run(&e));
    }
    let out = cmd.output().map_err(|e| could_not_run(&e))?;
    if out.status.success() {
        Ok(())
    } else {
        let why = String::from_utf8_lossy(&out.stderr).trim().to_string();
        Err(if why.is_empty() {
            "The system gave no reason.".into()
        } else {
            format!("The system said: {why}")
        })
    }
}

/// The service's recent log, or why there is none, and what the daemon
/// run from this window wrote, if it ran.
pub fn log() -> String {
    let service = service_log();
    let here = crate::here::output();
    if here.is_empty() {
        service
    } else {
        format!("{service}\n\nRun from this window:\n{here}")
    }
}

fn service_log() -> String {
    let argv = stubbed(log_command(), stub());
    match Command::new(&argv[0]).args(&argv[1..]).output() {
        Ok(out) if out.status.success() => {
            let text = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if text.is_empty() || text.starts_with("-- No entries --") {
                "The service has not written anything to its log yet.".into()
            } else {
                text
            }
        }
        Ok(out) => format!(
            "The log could not be read: {}",
            String::from_utf8_lossy(&out.stderr).trim()
        ),
        Err(e) => format!("The log could not be read: {e}"),
    }
}

/// Why the command that starts the service did not run, in plain words.
fn could_not_run(e: &std::io::Error) -> String {
    match e.kind() {
        std::io::ErrorKind::NotFound => {
            "The program that starts it was not found on this computer.".into()
        }
        std::io::ErrorKind::PermissionDenied => {
            "This computer did not allow the program that starts it to run.".into()
        }
        _ => format!("The program that starts it did not run: {e}."),
    }
}

/// The stopped view after a try to start it failed: the one real fault
/// here, so it is shown critical, chip and banner alike, with the reason.
/// `tries` is how many tries have failed: after the first, Try again;
/// after more, starting it again is not the way forward, so the window
/// offers to run it itself and to report the problem.
pub fn did_not_start(v: View, why: &str, tries: u32) -> View {
    failed(v, "Did not start", "The service did not start", why, tries)
}

/// A start that ran, but the service has not answered since: not the
/// same story as one that did not start, so not the same words.
pub fn not_answering(v: View, tries: u32) -> View {
    failed(
        v,
        "Not answering",
        "The service is not answering",
        "It was started, but has not answered since.",
        tries,
    )
}

fn failed(mut v: View, chip: &str, headline: &str, why: &str, tries: u32) -> View {
    v.tone = "failed";
    v.icon = "fail";
    if tries < 2 {
        v.chip = chip.into();
        v.headline = headline.into();
        v.body = format!("{why} Your layouts are safe.");
        v.actions = vec!["retry", "log"];
    } else {
        // Trying again is not the way forward any more: the heading says
        // so, and the words say what is.
        v.chip = "Failed to start".into();
        v.headline = "Starting the service failed twice".into();
        v.body = "This window can run it for now, until you quit it. Report the problem too, so it can be fixed.".into();
        v.actions = vec!["run-here", "report", "log"];
    }
    v.banner = v.headline.clone();
    v.banner_body = v.body.clone();
    v.detail = failed_detail(installed());
    v
}

/// The fix as commands after a failed start, for those who use a
/// terminal: clear the failure, start it, read why.
pub fn failed_detail(installed: bool) -> String {
    if installed {
        "Clear the failure: systemctl --user reset-failed nexusd\nStart it: systemctl --user start nexusd\nWhy it stopped: journalctl --user -u nexusd -e".into()
    } else {
        "nexusd run".into()
    }
}

/// The daemon run from this window stopped, or could not start: the end
/// of what the window can do. What is left is the report.
pub fn here_failed(mut v: View, why: &str) -> View {
    v.tone = "failed";
    v.icon = "fail";
    if why == crate::here::STILL_RUNNING {
        // Refused, not failed: the service is there, it does not answer.
        v.chip = "Not answering".into();
        v.headline = "The service runs, but does not answer".into();
        v.body =
            "This window does not run a second one. Report the problem, so it can be fixed.".into();
    } else {
        v.chip = "Did not run".into();
        v.headline = "It did not run from this window either".into();
        v.body = format!("{why} Report the problem, so it can be fixed.");
    }
    v.banner = v.headline.clone();
    v.banner_body = v.body.clone();
    v.actions = vec!["report", "log"];
    v.detail = failed_detail(installed());
    v
}

/// The service answers, and it is the daemon this window runs: the strip
/// works, for as long as the window does. Said in a banner, as the pictures
/// are live.
pub fn from_window(mut v: View) -> View {
    let note = "The strip works until you quit the window; closed to the tray, it keeps running. Report the problem, so the service can be fixed.";
    v.here = true;
    if v.away {
        // The strip's own problem leads; this is said after it.
        v.body = format!("{} This window runs the service for now.", v.body);
        v.banner_body = format!("{} This window runs the service for now.", v.banner_body);
    } else {
        v.tone = "note";
        v.icon = "info";
        v.chip = "Running from this window".into();
        v.headline = "Running from this window for now".into();
        v.body = note.into();
        v.banner = v.headline.clone();
        v.banner_body = note.into();
    }
    v.actions = vec!["report", "log"];
    v.detail = failed_detail(installed());
    v
}

/// The stopped view while a try to start it is under way: never the same
/// as stopped, so a press of Start or Try again always shows it was heard.
/// The chip says "Starting…"; the banner says what it waits for, with a
/// moving mark instead of a button, as there is nothing to press.
pub fn starting(mut v: View) -> View {
    v.tone = "starting";
    v.icon = "wait";
    v.chip = "Starting…".into();
    v.headline = "Waiting for the service to answer".into();
    v.body = "This takes a few seconds. Nothing here can be changed until then.".into();
    v.banner = v.headline.clone();
    v.banner_body = v.body.clone();
    v.actions = Vec::new();
    v
}

/// Report the problem: the project's page for a new issue, in the
/// browser, opened through the same checked opener as Help (or the stub,
/// in the rig). The window copies the diagnostics as it does this.
pub fn report() -> Result<(), String> {
    let url = report_address();
    if let Some(stub) = stub().filter(|s| !s.is_empty()) {
        let checked = nexus_control::links::checked_link(&url).map_err(String::from)?;
        let argv = stubbed(
            nexus_control::links::link_opener(&checked, std::env::consts::OS),
            Some(stub),
        );
        return Command::new(&argv[0])
            .args(&argv[1..])
            .status()
            .map(drop)
            .map_err(|e| e.to_string());
    }
    nexus_control::links::open(&url)
}

/// Where a problem is reported.
pub fn report_address() -> String {
    format!("{}/issues/new", env!("CARGO_PKG_REPOSITORY"))
}

/// What a start that ran but has not answered is recorded as (shown by
/// `not_answering`, never as a reason of its own).
pub const NO_ANSWER: &str = "no answer";

/// The stopped service, in the popover.
const STOPPED: &str = "Nothing here can be changed until it runs. Your layouts are safe.";

/// Said with every state where the strip is not showing the layout but the
/// service runs.
const STILL_EDITABLE: &str = "You can keep editing meanwhile.";

#[cfg(test)]
mod tests {
    use super::*;

    fn status(device_error: Option<&str>) -> Status {
        Status {
            version: "0.1.0".into(),
            pid: 1,
            device_error: device_error.map(String::from),
            page: "Home".into(),
            pages: vec!["Home".into()],
            layout: "Main".into(),
            store_path: "/tmp/x.db".into(),
            store_error: None,
            takeover: None,
            alarm: None,
            revision: 1,
        }
    }

    #[test]
    fn a_stopped_daemon_offers_to_start_it() {
        let v = view(None, &[]);
        assert_eq!(v.tone, "stopped");
        assert_eq!(v.actions, ["start", "log"]);
        assert!(!v.body.contains("systemctl"), "{}", v.body);
    }

    #[test]
    fn a_strip_in_use_names_the_program_in_plain_words() {
        let v = view(
            Some(&status(Some("the strip is in use by steam (pid 4242)"))),
            &[],
        );
        assert_eq!(v.chip, "Strip in use");
        assert_eq!(v.headline, "Steam is using the strip");
        assert!(
            v.away && v.actions.is_empty(),
            "dimmed, with nothing to press"
        );
        assert!(v.banner_body.contains("keep editing"), "{}", v.banner_body);
        let old = view(
            Some(&status(Some("the strip is in use by nexus-open (pid 9)"))),
            &[],
        );
        assert!(old.body.contains("unplug the strip"), "{}", old.body);
        assert_eq!(
            program_name("a (pid 1), b (pid 2), c (pid 3)"),
            "A, B and C"
        );
    }

    #[test]
    fn a_strip_left_without_its_driver_asks_for_a_replug() {
        let v = view(
            Some(&status(Some(
                "the strip was left without its driver by another program; unplug it and plug it back in",
            ))),
            &[],
        );
        assert_eq!(v.chip, "Replug the strip");
        assert!(v.body.starts_with("A program that used it"));
    }

    #[test]
    fn app_states_are_never_amber_and_a_failed_start_is_critical() {
        for why in [
            "the strip is in use by steam (pid 4242)",
            "no iCUE Nexus found; is it plugged in?",
            "no permission to open /dev/hidraw3",
        ] {
            let v = view(Some(&status(Some(why))), &[]);
            assert_eq!(v.tone, "app", "{why}");
        }
        assert_eq!(view(Some(&status(Some("no permission"))), &[]).icon, "lock");
        let failed = did_not_start(view(None, &[]), "No reason.", 1);
        assert_eq!((failed.tone, failed.icon), ("failed", "fail"));
        assert_eq!(failed.actions, ["retry", "log"]);
        let starting = starting(view(None, &[]));
        assert_eq!(starting.tone, "starting");
        assert_ne!(starting.chip, view(None, &[]).chip);
        assert!(
            starting.actions.is_empty(),
            "nothing to press twice while it starts"
        );
        // "Starting…" is the chip's alone.
        for words in [
            &starting.headline,
            &starting.body,
            &starting.banner,
            &starting.banner_body,
        ] {
            assert!(!words.contains("Starting"), "{words}");
        }
    }

    /// The way forward never needs a terminal: Try again once; after
    /// that, run it from this window, or report it.
    #[test]
    fn a_start_that_keeps_failing_offers_a_way_without_a_terminal() {
        let once = not_answering(view(None, &[]), 1);
        assert_eq!(once.actions[0], "retry");
        let twice = not_answering(view(None, &[]), 2);
        assert_eq!(twice.actions, ["run-here", "report", "log"]);
        assert!(twice.body.contains("run it for now"), "{}", twice.body);
        assert_ne!(
            once.headline, twice.headline,
            "the heading says it failed again"
        );
        let refused = here_failed(view(None, &[]), crate::here::STILL_RUNNING);
        assert_eq!(
            refused.chip, "Not answering",
            "the chip and the heading agree"
        );
        let again = did_not_start(view(None, &[]), "No reason.", 3);
        assert_eq!(again.actions[0], "run-here");
        let gone = here_failed(view(None, &[]), "It stopped.");
        assert_eq!(gone.actions, ["report", "log"]);
        assert_eq!(gone.body.matches("Report the problem").count(), 1);
        let stopped = view(None, &[]);
        let wait = starting(view(None, &[]));
        let here = from_window(view(Some(&status(None)), &[]));
        for v in [
            &once, &twice, &again, &gone, &refused, &stopped, &wait, &here,
        ] {
            assert_eq!(v.body, v.banner_body, "one story");
            assert_eq!(v.headline, v.banner, "one heading");
            // Two short lines at most, at about 70 characters each.
            assert!(v.body.chars().count() <= 140, "{}", v.body);
            for word in ["systemctl", "journalctl", "terminal"] {
                assert!(!v.body.contains(word), "{}", v.body);
            }
        }
        assert!(
            failed_detail(true)
                .starts_with("Clear the failure: systemctl --user reset-failed nexusd\n")
        );
        assert!(report_address().ends_with("/issues/new"));
        assert!(nexus_control::links::checked_link(&report_address()).is_ok());
    }

    #[test]
    fn running_from_the_window_says_so_with_live_pictures() {
        let v = from_window(view(Some(&status(None)), &[]));
        assert!(v.here && !v.away);
        assert_eq!(v.tone, "note");
        assert!(
            v.banner_body.contains("until you quit the window"),
            "{}",
            v.banner_body
        );
        assert_eq!(v.actions, ["report", "log"]);
        let away = from_window(view(
            Some(&status(Some("no iCUE Nexus found; is it plugged in?"))),
            &[],
        ));
        assert_eq!(away.chip, "No strip", "the strip's own problem leads");
        assert!(away.here && away.banner_body.contains("This window runs the service"));
    }

    #[test]
    fn running_without_the_strip_reads_as_a_state() {
        let v = view(Some(&status(Some("started with --no-device"))), &[]);
        assert_eq!(v.chip, "Running, no strip");
        assert_eq!(v.tone, "note");
    }

    #[test]
    fn all_well_is_quiet() {
        let v = view(Some(&status(None)), &[]);
        assert_eq!(v.tone, "ok");
        assert_eq!(v.chip, "Strip connected");
        assert!(v.actions.is_empty() && !v.away && v.banner.is_empty());
    }

    #[test]
    fn no_shell_command_is_ever_the_body() {
        for why in [
            "no iCUE Nexus found; is it plugged in?",
            "no permission to open /dev/hidraw3; install data/udev/70-nexus.rules",
            "the strip is in use by another program; `nexusctl doctor` names it",
        ] {
            let v = view(Some(&status(Some(why))), &[]);
            for word in ["sudo", "systemctl", "nexusctl", "udevadm", "`"] {
                assert!(!v.body.contains(word), "{why}: {}", v.body);
            }
        }
    }

    #[test]
    fn the_fixes_run_the_users_service_and_can_be_stubbed() {
        let exe = Path::new("/usr/bin/nexusd");
        assert_eq!(
            start_command(true, exe),
            ["systemctl", "--user", "start", "nexusd.service"]
        );
        assert_eq!(start_command(false, exe), ["/usr/bin/nexusd", "run"]);
        assert_eq!(
            reset_command(),
            ["systemctl", "--user", "reset-failed", "nexusd.service"]
        );
        assert_eq!(
            log_command()[..4],
            ["journalctl", "--user", "-u", "nexusd.service"]
        );
        assert_eq!(
            stubbed(
                vec!["systemctl".into(), "--user".into()],
                Some("/tmp/stub".into())
            ),
            ["/tmp/stub", "systemctl", "--user"]
        );
        assert_eq!(stubbed(vec!["a".into()], None), ["a"]);
    }
}
