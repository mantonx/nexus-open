//! `nexusctl doctor`: checks everything the strip needs, in the order a
//! problem would show up, and prints the fix for anything wrong.
//!
//! The settings window's onboarding will run the same checks, so each one
//! names the problem and the exact command or step that fixes it.

#[cfg(target_os = "linux")]
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitCode;

use nexus_control::{Client, Status};
use nexus_core::config::Config;

struct Report {
    failed: bool,
}

impl Report {
    fn ok(&self, what: &str, detail: &str) {
        println!("  ok    {what}: {detail}");
    }

    fn fail(&mut self, what: &str, detail: &str, fix: &str) {
        self.failed = true;
        println!("  FAIL  {what}: {detail}");
        for line in fix.lines() {
            println!("        {line}");
        }
    }

    fn note(&self, what: &str, detail: &str) {
        println!("  --    {what}: {detail}");
    }
}

pub fn run() -> ExitCode {
    let mut r = Report { failed: false };
    println!("nexusctl {}", env!("CARGO_PKG_VERSION"));

    let status = daemon(&mut r);
    device(&mut r);
    config(&mut r, status.as_ref());

    if r.failed {
        println!("\nSomething needs fixing; see FAIL above.");
        ExitCode::FAILURE
    } else {
        println!("\nEverything looks right.");
        ExitCode::SUCCESS
    }
}

fn daemon(r: &mut Report) -> Option<Status> {
    let status = Client::connect()
        .ok()
        .and_then(|mut c| c.request("status").ok()?.ok())
        .and_then(|data| serde_json::from_value::<Status>(data).ok());
    match &status {
        Some(s) => {
            r.ok(
                "daemon",
                &format!("nexusd {} running (pid {})", s.version, s.pid),
            );
            match &s.device_error {
                None => r.ok("daemon device", "connected and showing frames"),
                Some(why) => r.note("daemon device", why),
            }
        }
        None => r.fail(
            "daemon",
            "nexusd is not running",
            "Start it with: nexusd run",
        ),
    }
    status
}

#[cfg(target_os = "linux")]
fn device(r: &mut Report) {
    use std::fs::OpenOptions;

    for rule in [
        "/usr/lib/udev/rules.d/99-corsair-nexus.rules",
        "/etc/udev/rules.d/99-corsair-nexus.rules",
    ] {
        if Path::new(rule).exists() {
            r.fail(
                "old udev rule",
                &format!("{rule} is installed"),
                &format!(
                    "It sorts after 73-seat-late.rules, so it never grants access. Remove it:\n\
                     sudo rm {rule} && sudo udevadm control --reload"
                ),
            );
        }
    }
    let rule_installed = ["/etc/udev/rules.d", "/usr/lib/udev/rules.d"]
        .iter()
        .any(|d| Path::new(d).join("70-nexus.rules").exists());

    let node = match nexus_device::hidraw::find() {
        Ok(Some(node)) => {
            r.ok("device", &format!("iCUE Nexus found at {}", node.display()));
            node
        }
        Ok(None) => {
            r.fail(
                "device",
                "no iCUE Nexus found",
                "Check it is plugged in. `lsusb | grep 1b1c:1b8e` should list it.",
            );
            return;
        }
        Err(e) => {
            r.fail("device", &e.to_string(), "");
            return;
        }
    };

    match OpenOptions::new().read(true).write(true).open(&node) {
        Ok(_) => r.ok("access", "this user can open the device"),
        Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
            let fix = if rule_installed {
                "The udev rule is installed but not applied to this device yet.\n\
                 Unplug the Nexus and plug it back in."
            } else {
                "Install the udev rule, then unplug and replug the Nexus:\n\
                 sudo install -m 644 data/udev/70-nexus.rules /etc/udev/rules.d/\n\
                 sudo udevadm control --reload"
            };
            r.fail("access", "permission denied", fix);
        }
        Err(e) => r.fail("access", &e.to_string(), ""),
    }

    let holders = holders(&node);
    let others: Vec<&(u32, String)> = holders
        .iter()
        .filter(|(_, name)| name != "nexusd")
        .collect();
    if others.is_empty() {
        let detail = if holders.is_empty() {
            "no process has it open".to_string()
        } else {
            "held by nexusd only".to_string()
        };
        r.ok("holders", &detail);
    } else {
        let list: Vec<String> = others
            .iter()
            .map(|(pid, n)| format!("{n} (pid {pid})"))
            .collect();
        let fix = if others.iter().any(|(_, n)| n == "nexus-open") {
            "The old Nexus Open service is running and fights over the device:\n\
             systemctl --user disable --now nexus-open"
        } else {
            "Stop the other program so only nexusd draws on the strip."
        };
        r.fail("holders", &format!("also open in {}", list.join(", ")), fix);
    }
}

/// Processes with `node` open, found through /proc/*/fd. Processes of
/// other users are not visible, which is fine: they could not open it
/// either without the same access.
#[cfg(target_os = "linux")]
fn holders(node: &Path) -> Vec<(u32, String)> {
    let mut found = Vec::new();
    let Ok(procs) = std::fs::read_dir("/proc") else {
        return found;
    };
    for p in procs.flatten() {
        let Some(pid) = p.file_name().to_str().and_then(|s| s.parse::<u32>().ok()) else {
            continue;
        };
        let Ok(fds) = std::fs::read_dir(p.path().join("fd")) else {
            continue;
        };
        if fds
            .flatten()
            .any(|fd| std::fs::read_link(fd.path()).is_ok_and(|target| target == node))
        {
            let name = std::fs::read_to_string(p.path().join("comm"))
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            found.push((pid, name));
        }
    }
    found
}

#[cfg(not(target_os = "linux"))]
fn device(r: &mut Report) {
    r.note("device", "device checks are not implemented on this OS yet");
}

fn config(r: &mut Report, status: Option<&Status>) {
    let path: Option<PathBuf> = status
        .map(|s| PathBuf::from(&s.config_path))
        .or_else(nexus_control::default_config_path);
    let Some(path) = path else {
        r.fail("config", "cannot find the user's config directory", "");
        return;
    };
    match std::fs::read_to_string(&path) {
        Ok(text) => match Config::parse(&text) {
            Ok(c) => r.ok(
                "config",
                &format!("{} is valid ({} pages)", path.display(), c.pages.len()),
            ),
            Err(e) => r.fail(
                "config",
                &format!("{} is invalid", path.display()),
                &format!("{e}\nThe strip keeps the last valid layout until this is fixed."),
            ),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => r.note(
            "config",
            &format!("{} does not exist yet; nexusd creates it", path.display()),
        ),
        Err(e) => r.fail("config", &format!("{}: {e}", path.display()), ""),
    }
}
