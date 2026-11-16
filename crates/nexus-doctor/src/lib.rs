//! The checks behind `nexusctl doctor` and the settings window's
//! onboarding: everything the strip needs, in the order a problem would
//! show up. Each failure names the problem and the exact fix.

#[cfg(target_os = "linux")]
use std::path::Path;

use nexus_control::{Client, Status};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum State {
    Ok,
    Fail,
    /// Neither: worth knowing, nothing to fix here.
    Note,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub what: String,
    pub state: State,
    pub detail: String,
    /// What to do, for a failure; may be several lines.
    pub fix: String,
}

/// Every check, in order.
pub fn run() -> Vec<Check> {
    let mut r = Report(Vec::new());
    let status = daemon(&mut r);
    device(&mut r);
    store(&mut r, status.as_ref());
    r.0
}

/// The checks as text, the way `nexusctl doctor` prints them and "Copy
/// diagnostics" copies them.
pub fn text(checks: &[Check]) -> String {
    let mut out = String::new();
    for c in checks {
        let tag = match c.state {
            State::Ok => "ok  ",
            State::Fail => "FAIL",
            State::Note => "--  ",
        };
        out += &format!("  {tag}  {}: {}\n", c.what, c.detail);
        for line in c.fix.lines() {
            out += &format!("        {line}\n");
        }
    }
    out
}

/// What to paste into a bug report: versions, the system, and every
/// check. The home directory is shown as ~, so a report says nothing
/// about who made it. Nothing from the layout is included: it can hold
/// where the user lives.
pub fn report(checks: &[Check]) -> String {
    let mut out = format!("Nexus Open {}\n", env!("CARGO_PKG_VERSION"));
    out += &format!("System: {}\n", system());
    for var in ["XDG_CURRENT_DESKTOP", "XDG_SESSION_TYPE"] {
        if let Ok(v) = std::env::var(var) {
            out += &format!("{var}: {v}\n");
        }
    }
    out += "\n";
    out += &text(checks);
    match std::env::var("HOME") {
        Ok(home) if home.len() > 1 => out.replace(&home, "~"),
        _ => out,
    }
}

/// The OS by name and the kernel, where they can be read.
fn system() -> String {
    #[cfg(target_os = "linux")]
    {
        let name = std::fs::read_to_string("/etc/os-release")
            .ok()
            .and_then(|t| {
                t.lines()
                    .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                    .map(|v| v.trim_matches('"').to_string())
            })
            .unwrap_or_else(|| "Linux".into());
        let kernel = std::fs::read_to_string("/proc/sys/kernel/osrelease").unwrap_or_default();
        return format!("{name}, kernel {}", kernel.trim());
    }
    #[allow(unreachable_code)]
    std::env::consts::OS.to_string()
}

struct Report(Vec<Check>);

impl Report {
    fn push(&mut self, what: &str, state: State, detail: &str, fix: &str) {
        self.0.push(Check {
            what: what.into(),
            state,
            detail: detail.into(),
            fix: fix.into(),
        });
    }

    fn ok(&mut self, what: &str, detail: &str) {
        self.push(what, State::Ok, detail, "");
    }

    fn fail(&mut self, what: &str, detail: &str, fix: &str) {
        self.push(what, State::Fail, detail, fix);
    }

    fn note(&mut self, what: &str, detail: &str) {
        self.push(what, State::Note, detail, "");
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
        None => r.fail("daemon", "nexusd is not running", &start_fix()),
    }
    status
}

/// How to start the daemon: through its service where one is installed,
/// since a daemon started by hand stops with the terminal.
fn start_fix() -> String {
    #[cfg(target_os = "linux")]
    {
        let user_units = std::env::var_os("HOME")
            .map(|h| Path::new(&h).join(".config/systemd/user/nexusd.service"));
        let installed = user_units.is_some_and(|p| p.exists())
            || Path::new("/usr/lib/systemd/user/nexusd.service").exists();
        if installed {
            return "Start its service: systemctl --user start nexusd\n\
                    If it stops again, its log says why: journalctl --user -u nexusd -e"
                .into();
        }
    }
    "Start it with: nexusd run".into()
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

fn store(r: &mut Report, status: Option<&Status>) {
    match status {
        Some(s) => match &s.store_error {
            None => r.ok(
                "store",
                &format!("{} (layout \"{}\", {} pages)", s.store_path, s.layout, s.pages.len()),
            ),
            Some(e) => r.fail(
                "store",
                &format!("{} cannot be used", s.store_path),
                &format!("{e}\nThe strip shows the default layout and changes are not saved until this is fixed."),
            ),
        },
        None => match nexus_control::default_store_path() {
            Some(path) if path.exists() => r.note(
                "store",
                &format!("{} exists; start nexusd to check it", path.display()),
            ),
            Some(path) => r.note(
                "store",
                &format!("{} does not exist yet; nexusd creates it on first start", path.display()),
            ),
            None => r.fail("store", "cannot find the user's state directory", ""),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_lists_each_check_with_its_fix_indented() {
        let checks = [
            Check {
                what: "daemon".into(),
                state: State::Ok,
                detail: "running".into(),
                fix: String::new(),
            },
            Check {
                what: "access".into(),
                state: State::Fail,
                detail: "permission denied".into(),
                fix: "Unplug it.\nPlug it back in.".into(),
            },
        ];
        assert_eq!(
            text(&checks),
            "  ok    daemon: running\n  FAIL  access: permission denied\n        Unplug it.\n        Plug it back in.\n"
        );
    }

    #[test]
    fn the_report_hides_the_home_directory() {
        let Ok(home) = std::env::var("HOME") else {
            return;
        };
        let checks = [Check {
            what: "store".into(),
            state: State::Ok,
            detail: format!("{home}/.local/state/nexusd/nexusd.db"),
            fix: String::new(),
        }];
        let r = report(&checks);
        assert!(r.contains("~/.local/state/nexusd/nexusd.db"), "{r}");
        assert!(!r.contains(&home));
    }
}

#[cfg(test)]
mod troubleshooting {
    /// Every check the doctor can report has a section in the
    /// troubleshooting page, named after it.
    #[test]
    fn every_check_has_a_section() {
        let page = include_str!("../../../docs/TROUBLESHOOTING.md");
        let source = include_str!("lib.rs");
        let mut names: Vec<&str> = Vec::new();
        for call in [r#"r.ok(""#, r#"r.fail(""#, r#"r.note(""#, r#"what: ""#] {
            for part in source.split(call).skip(1) {
                let name = &part[..part.find('"').unwrap()];
                // Skip this test's own patterns and the unit test's data.
                if !name.is_empty() && !names.contains(&name) {
                    names.push(name);
                }
            }
        }
        for name in names {
            assert!(
                page.contains(&format!("\n## {name}\n")),
                "docs/TROUBLESHOOTING.md has no section for `{name}`"
            );
        }
    }
}
