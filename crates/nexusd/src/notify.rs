//! Tells systemd the daemon is ready and that its thread is still turning
//! (the `sd_notify` protocol: one datagram to the socket in `$NOTIFY_SOCKET`).
//!
//! Run outside systemd, or on an OS without it, every call does nothing.

use std::time::{Duration, Instant};

pub struct Notifier {
    #[cfg(target_os = "linux")]
    socket: Option<linux::Socket>,
    /// Half of `WatchdogSec`: the longest the daemon thread may go without
    /// pinging. `None` when the unit has no watchdog.
    interval: Option<Duration>,
    last_ping: Option<Instant>,
}

impl Notifier {
    pub fn from_env() -> Self {
        Notifier {
            #[cfg(target_os = "linux")]
            socket: linux::Socket::from_env(),
            interval: watchdog_interval(
                std::env::var("WATCHDOG_USEC").ok().as_deref(),
                std::env::var("WATCHDOG_PID").ok().as_deref(),
                std::process::id(),
            ),
            last_ping: None,
        }
    }

    /// The control socket is listening and the store is open: `systemctl
    /// start` can return, and `nexusctl` will find the daemon.
    pub fn ready(&mut self) {
        self.send("READY=1");
        self.last_ping = Some(Instant::now());
    }

    /// Called on every turn of the daemon loop. Sends a ping at most every
    /// quarter of the watchdog time, so a 60 fps animation does not cost a
    /// syscall per frame.
    pub fn alive(&mut self) {
        let Some(interval) = self.interval else {
            return;
        };
        if self.last_ping.is_some_and(|at| at.elapsed() < interval / 2) {
            return;
        }
        self.send("WATCHDOG=1");
        self.last_ping = Some(Instant::now());
    }

    /// The longest the loop may sleep and still ping in time.
    pub fn max_wait(&self) -> Option<Duration> {
        self.interval
    }

    #[cfg(target_os = "linux")]
    fn send(&self, message: &str) {
        if let Some(socket) = &self.socket {
            socket.send(message);
        }
    }

    #[cfg(not(target_os = "linux"))]
    fn send(&self, _: &str) {}
}

/// Half the watchdog time systemd asked for, if it asked this process.
/// `WATCHDOG_PID` names the process meant, so a child that inherited the
/// variables does not take them as its own.
fn watchdog_interval(usec: Option<&str>, pid: Option<&str>, own_pid: u32) -> Option<Duration> {
    if let Some(pid) = pid
        && pid.parse::<u32>().ok() != Some(own_pid)
    {
        return None;
    }
    let usec: u64 = usec?.parse().ok().filter(|&u| u > 0)?;
    Some(Duration::from_micros(usec) / 2)
}

#[cfg(target_os = "linux")]
mod linux {
    use std::os::linux::net::SocketAddrExt;
    use std::os::unix::net::{SocketAddr, UnixDatagram};

    pub struct Socket {
        socket: UnixDatagram,
        addr: SocketAddr,
    }

    impl Socket {
        pub fn from_env() -> Option<Self> {
            let path = std::env::var_os("NOTIFY_SOCKET")?;
            let path = path.to_str()?;
            // A leading '@' names the abstract namespace.
            let addr = match path.strip_prefix('@') {
                Some(name) => SocketAddr::from_abstract_name(name.as_bytes()).ok()?,
                None => SocketAddr::from_pathname(path).ok()?,
            };
            let socket = match UnixDatagram::unbound() {
                Ok(socket) => socket,
                Err(e) => {
                    eprintln!("nexusd: cannot reach systemd's notify socket: {e}");
                    return None;
                }
            };
            Some(Socket { socket, addr })
        }

        /// A lost datagram costs at worst one restart by the watchdog, and
        /// there is no one to tell, so a failure is only logged.
        pub fn send(&self, message: &str) {
            if let Err(e) = self.socket.send_to_addr(message.as_bytes(), &self.addr) {
                eprintln!("nexusd: cannot notify systemd: {e}");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_the_watchdog_time() {
        assert_eq!(
            watchdog_interval(Some("20000000"), None, 7),
            Some(Duration::from_secs(10))
        );
        assert_eq!(
            watchdog_interval(Some("20000000"), Some("7"), 7),
            Some(Duration::from_secs(10))
        );
    }

    #[test]
    fn not_for_another_process() {
        assert_eq!(watchdog_interval(Some("20000000"), Some("8"), 7), None);
        assert_eq!(watchdog_interval(Some("20000000"), Some("x"), 7), None);
    }

    #[test]
    fn no_watchdog() {
        assert_eq!(watchdog_interval(None, None, 7), None);
        assert_eq!(watchdog_interval(Some("0"), None, 7), None);
        assert_eq!(watchdog_interval(Some("soon"), None, 7), None);
    }
}
