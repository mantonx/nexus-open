//! The header's status chip: what is wrong, if anything, said plainly,
//! with buttons that fix it. The cause comes from the daemon (why it has
//! no strip) and the doctor's checks; a shell command is only ever an
//! extra detail for those who want it.
//!
//! "Start the service" and "Open the log" run `systemctl` and
//! `journalctl` on the user's `nexusd.service`. The commands are built
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
    /// Which fixes apply.
    pub start: bool,
    pub retry: bool,
    pub log: bool,
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
            headline: "Nexus Open's service is not running".into(),
            body: "The strip keeps its last picture until the service runs again. Your layouts are safe, and a change made here waits and is sent once it is back.".into(),
            start: true,
            retry: false,
            log: true,
            detail: fix_of("daemon"),
        };
    };
    if let Some(e) = &status.store_error {
        return View {
            tone: "app",
            icon: "save",
            chip: "Cannot save".into(),
            headline: "Changes cannot be saved".into(),
            body: format!(
                "The strip shows the default layout, and nothing changed here is kept, because the file that holds your layouts cannot be used: {e}."
            ),
            start: false,
            retry: true,
            log: true,
            detail: fix_of("store"),
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
                retry: true,
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
        body,
        start: false,
        retry: true,
        log: false,
        detail: fix.to_string(),
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
                "While it does, Nexus Open cannot draw on it. Quit the other program (an older Nexus Open: from its tray icon), then press Try again. If the strip stays dark, unplug it and plug it back in.".into(),
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
                "While it does, Nexus Open cannot draw on it. Quit {name}, then press Try again. If the strip stays dark, unplug it and plug it back in."
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
            retry: true,
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
            "Plug in the iCUE Nexus. It shows your layout as soon as it is found; nothing else to do.".into(),
        );
    }
    let mut body = why.to_string();
    if let Some(first) = body.get_mut(..1) {
        first.make_ascii_uppercase();
    }
    warn(
        "plug",
        "Strip problem",
        "The strip cannot be used",
        format!("{body}."),
    )
}

/// "steam (pid 4242), obs (pid 7)" as "steam and obs".
fn program_name(holders: &str) -> String {
    let names: Vec<&str> = holders
        .split(", ")
        .map(|h| h.split(" (pid").next().unwrap_or(h).trim())
        .filter(|n| !n.is_empty())
        .collect();
    match names.as_slice() {
        [] => "Another program".into(),
        [one] => (*one).into(),
        [rest @ .., last] => format!("{} and {last}", rest.join(", ")),
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

/// The service's recent log, or why there is none.
pub fn log() -> String {
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
/// here, so it is shown critical, chip and banner alike, with the reason
/// and the log as the next step.
pub fn did_not_start(mut v: View, why: &str) -> View {
    v.tone = "failed";
    v.icon = "fail";
    v.chip = "Did not start".into();
    v.headline = "The service did not start".into();
    v.body = format!(
        "{why} The log may say more. Your layouts are safe; copy the diagnostics to ask for help."
    );
    v.log = true;
    v.start = true;
    v
}

/// The stopped view while a try to start it is under way: never the same
/// as stopped, so a press of Start or Try again always shows it was heard.
pub fn starting(mut v: View) -> View {
    v.tone = "starting";
    v.icon = "wait";
    v.chip = "Starting…".into();
    v.headline = "Starting the service…".into();
    v.body = "This takes a few seconds. The window picks the strip up again by itself.".into();
    v.start = false;
    v.retry = false;
    v.log = false;
    v
}

/// Why a start that ran has still not answered.
pub const NO_ANSWER: &str = "It was started, but it has not answered after ten seconds.";

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
        assert!(v.start && v.log);
        assert!(!v.body.contains("systemctl"), "{}", v.body);
    }

    #[test]
    fn a_strip_in_use_names_the_program_in_plain_words() {
        let v = view(
            Some(&status(Some("the strip is in use by steam (pid 4242)"))),
            &[],
        );
        assert_eq!(v.chip, "Strip in use");
        assert_eq!(v.headline, "steam is using the strip");
        assert!(v.retry && !v.start);
        let old = view(
            Some(&status(Some("the strip is in use by nexus-open (pid 9)"))),
            &[],
        );
        assert!(old.body.contains("unplug the strip"), "{}", old.body);
        assert_eq!(
            program_name("a (pid 1), b (pid 2), c (pid 3)"),
            "a, b and c"
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
        let failed = did_not_start(view(None, &[]), "No reason.");
        assert_eq!((failed.tone, failed.icon), ("failed", "fail"));
        assert!(failed.log && failed.start);
        let starting = starting(view(None, &[]));
        assert_eq!(starting.tone, "starting");
        assert_ne!(starting.chip, view(None, &[]).chip);
        assert!(!starting.start, "nothing to press twice while it starts");
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
        assert!(!v.start && !v.retry);
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
