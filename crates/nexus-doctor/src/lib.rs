//! The checks behind `nexusctl doctor` and the settings window's
//! onboarding: everything the strip needs, in the order a problem would
//! show up. Each failure names the problem and the exact fix.

#[cfg(target_os = "linux")]
use std::path::Path;

use nexus_control::{Client, Status};
#[cfg(target_os = "linux")]
use nexus_device::DeviceError;

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
    other_devices(&mut r);
    r.0
}

/// A Corsair device that is plugged in, and where it stands.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DeviceLine {
    pub name: String,
    pub id: u16,
    /// What the app does with it, or will.
    pub support: String,
    /// Ready, or what blocks it.
    pub state: String,
    /// What to do about it, if anything.
    pub fix: String,
}

/// Every Corsair device plugged in. Nothing is sent to any of them.
#[cfg(target_os = "linux")]
pub fn devices() -> Vec<DeviceLine> {
    use nexus_device::catalog::{self, Support};
    use nexus_device::hidraw::{self, Missing};

    let found = hidraw::scan();
    // A device whose interfaces were all taken from the HID driver has no
    // hidraw node, so the scan cannot see it; the USB bus can.
    let hidden: Vec<DeviceLine> = catalog::PRODUCTS
        .iter()
        .filter(|p| !found.iter().any(|f| f.product == p.id))
        .filter_map(|p| {
            let missing = hidraw::missing(p.id, p.interfaces[0]);
            (missing != Missing::Unplugged).then(|| {
                let e = DeviceError::from(missing);
                DeviceLine {
                    name: p.name.into(),
                    id: p.id,
                    support: support(p.support),
                    state: blocked_state(&e),
                    fix: blocked_fix(&e, thing(p.id)),
                }
            })
        })
        .collect();

    fn support(s: Support) -> String {
        match s {
            Support::Supported(what) => format!("supported: {what}"),
            Support::Planned(what) => format!("planned: {what}"),
        }
    }
    fn thing(id: u16) -> &'static str {
        if id == 0x1b8e {
            "the strip"
        } else {
            "the device"
        }
    }

    found
        .into_iter()
        .map(|f| {
            let known = catalog::product(f.product);
            // An interface the app opens that has no hidraw node.
            let blocked = known.and_then(|p| {
                p.interfaces
                    .iter()
                    .find(|i| !f.nodes.iter().any(|n| n.interface == Some(**i)))
                    .map(|i| DeviceError::from(hidraw::missing(p.id, *i)))
            });
            let name = known.map_or(f.name.clone(), |p| p.name.to_string());
            let support = known.map_or("not supported".into(), |p| support(p.support));
            let used: Vec<&nexus_device::hidraw::Node> = f
                .nodes
                .iter()
                .filter(|n| {
                    known.is_some_and(|p| n.interface.is_some_and(|i| p.interfaces.contains(&i)))
                })
                .collect();
            let others: Vec<String> = used
                .iter()
                .flat_map(|n| &n.holders)
                .filter(|(_, who)| who != "nexusd")
                .map(|(pid, who)| format!("{who} (pid {pid})"))
                .collect();
            let (state, fix) = if known.is_none() {
                ("nothing to do".to_string(), String::new())
            } else if let Some(e) = &blocked {
                (blocked_state(e), blocked_fix(e, thing(f.product)))
            } else if used.iter().any(|n| !n.access) {
                (
                    "no access".to_string(),
                    "Install the udev rule, then unplug the device and plug it back in:\n\
                     sudo install -m 644 data/udev/70-nexus.rules /etc/udev/rules.d/\n\
                     sudo udevadm control --reload"
                        .to_string(),
                )
            } else if !others.is_empty() {
                (
                    format!("in use by {}", others.join(", ")),
                    "Two programs driving one device fight over it; quit the other one.".into(),
                )
            } else {
                ("ready".to_string(), String::new())
            };
            DeviceLine {
                name,
                id: f.product,
                support,
                state,
                fix,
            }
        })
        .chain(hidden)
        .collect()
}

/// How `nexusctl devices` puts why a device cannot be opened.
#[cfg(target_os = "linux")]
fn blocked_state(e: &DeviceError) -> String {
    match e {
        DeviceError::InUse(holders) if holders.is_empty() => "in use by another program".into(),
        DeviceError::InUse(holders) => format!("in use by {}", nexus_device::names(holders)),
        DeviceError::Detached => "left without its driver".into(),
        DeviceError::NotReady => "starting".into(),
        DeviceError::NotFound => "not plugged in".into(),
        e => e.to_string(),
    }
}

/// What to do when `thing` ("the strip") cannot be opened because another
/// program took it, or let go of it without giving it back.
///
/// The old Go Nexus Open claims the strip through usbfs, detaching the HID
/// driver with USBDEVFS_DISCONNECT, and on exit only releases the
/// interface: it never reattaches the driver (USBDEVFS_CONNECT), and the
/// kernel does not rebind by itself. So after quitting it the strip stays
/// dark until it is replugged; rebinding by hand needs root. A libusb
/// program that set auto-detach does reattach, so for other programs the
/// replug is only needed if the strip stays dark.
#[cfg(target_os = "linux")]
fn blocked_fix(e: &DeviceError, thing: &str) -> String {
    let replug = format!("unplug {thing} and plug it back in");
    let after = "nexusd picks it up again by itself.";
    match e {
        DeviceError::InUse(holders) if holders.iter().any(|(_, n)| n == "nexus-open") => format!(
            "The old Nexus Open has taken {thing}. Quit it: click its tray icon and choose Quit.\n\
             Then {replug}: the old app does not hand {thing} back when it quits. {after}"
        ),
        DeviceError::InUse(holders) if !holders.is_empty() => format!(
            "Quit {}. If {thing} stays dark after that, {replug}. {after}",
            nexus_device::names(holders)
        ),
        DeviceError::InUse(_) => format!(
            "Another program has taken {thing}, but this user cannot see which; it may run \
             as another user. Quit it (the old Nexus Open: from its tray icon).\n\
             If {thing} stays dark after that, {replug}. {after}"
        ),
        DeviceError::Detached => {
            let mut replug = replug;
            replug[..1].make_ascii_uppercase();
            format!(
                "{replug}; {after}\n\
                 A program that used it, such as the old Nexus Open, let go without \
                 giving it back to its driver."
            )
        }
        DeviceError::NotReady => format!(
            "Normal for a few seconds after {thing} is plugged in or the computer wakes.\n\
             If it lasts, {replug}."
        ),
        _ => String::new(),
    }
}

#[cfg(not(target_os = "linux"))]
pub fn devices() -> Vec<DeviceLine> {
    Vec::new()
}

/// Corsair devices other than the strip: a note, since none is needed
/// for the strip and their support is still to come.
fn other_devices(r: &mut Report) {
    let others: Vec<DeviceLine> = devices().into_iter().filter(|d| d.id != 0x1b8e).collect();
    if !others.is_empty() {
        let names: Vec<String> = others
            .iter()
            .map(|d| format!("{} ({})", d.name, d.state))
            .collect();
        r.note(
            "other devices",
            &format!("{}; see nexusctl devices", names.join(", ")),
        );
    }
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

    let node = match nexus_device::hidraw::locate() {
        Ok(node) => {
            r.ok("device", &format!("iCUE Nexus found at {}", node.display()));
            node
        }
        Err(DeviceError::NotFound) => {
            r.fail(
                "device",
                "no iCUE Nexus found",
                "Check it is plugged in. `lsusb | grep 1b1c:1b8e` should list it.",
            );
            return;
        }
        Err(e) => {
            r.fail("device", &e.to_string(), &blocked_fix(&e, "the strip"));
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

    let holders = nexus_device::hidraw::holders(&node);
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

    /// The old app never gives the strip back to its driver, so quitting
    /// it is not enough: the fix must say to replug.
    #[cfg(target_os = "linux")]
    #[test]
    fn a_strip_taken_by_the_old_app_needs_a_replug_after_quitting_it() {
        let old = DeviceError::InUse(vec![(42, "nexus-open".into())]);
        let fix = blocked_fix(&old, "the strip");
        assert!(fix.contains("tray icon"), "{fix}");
        assert!(
            fix.contains("Then unplug the strip and plug it back in"),
            "{fix}"
        );
        let left = blocked_fix(&DeviceError::Detached, "the strip");
        assert!(
            left.starts_with("Unplug the strip and plug it back in"),
            "{left}"
        );
        let unseen = blocked_fix(&DeviceError::InUse(vec![]), "the strip");
        assert!(unseen.contains("cannot see which"), "{unseen}");
        assert_eq!(blocked_state(&old), "in use by nexus-open (pid 42)");
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
        // A Windows checkout may have CRLF line endings.
        let page = include_str!("../../../docs/TROUBLESHOOTING.md").replace("\r\n", "\n");
        // Whitespace flattened: rustfmt may put a check's name on the line
        // after `r.note(`.
        let source = include_str!("lib.rs")
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ")
            .replace("( \"", "(\"");
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
