//! The Nexus Open settings window, built with Slint.
//!
//! The window is a storyboard of the layout: every page and takeover as a
//! strip. The one on the strip is the daemon's own frame, streamed over the
//! control socket, so it is always exactly what the strip shows; the others
//! are drawn here with the strip's renderer (`thumbs`). The strip being
//! edited is also the editor (`editor`): its changes are edits sent over
//! the same socket. If the daemon is not running the window says so, offers
//! to start it, and keeps retrying.

mod accent;
mod appearance;
mod editor;
mod files;
mod first_run;
mod gallery;
mod geometry;
mod i18n;
mod inspector;
mod layouts;
mod names;
mod settings;
mod signals;
mod simulate;
mod status;
mod takeovers;
mod thumbs;
mod tray;
mod window_size;

use std::io;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::Duration;

use nexus_control::{Client, SignalEntry, Status};
use nexus_doctor::Check;
use slint::{
    ComponentHandle, Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel, Weak,
};

slint::include_modules!();

/// Matches the old app's ID, so the existing desktop file and icon apply.
pub(crate) const APP_ID: &str = "io.nexus.open";
/// Strip size in pixels.
const WIDTH: u32 = 640;
const HEIGHT: u32 = 48;
/// Wait between reconnection attempts while the daemon is down.
const RETRY: Duration = Duration::from_secs(1);
/// How often the page, takeover and revision are checked: a swipe on the
/// strip or `nexusctl edit` changes them without the window knowing, and
/// the outlines over the preview must follow quickly.
const STATUS_POLL: Duration = Duration::from_millis(250);
/// Signals are fetched every this many status polls.
const SIGNALS_EVERY: u64 = 4;
/// How often the checks run again: often while something is wrong, so
/// the problem goes away soon after it is fixed, and rarely otherwise.
const CHECK_FAILING: Duration = Duration::from_secs(5);
const CHECK_HEALTHY: Duration = Duration::from_secs(30);

/// Whether the frame stream is up; a change runs the checks again.
static STREAMING: AtomicBool = AtomicBool::new(false);
/// Whether the window is on screen. Hidden in the tray it asks the daemon
/// for nothing: no frames, no polling.
static VISIBLE: AtomicBool = AtomicBool::new(true);
/// Whether the tray icon is up, so closing the window can hide it.
static IN_TRAY: AtomicBool = AtomicBool::new(false);
/// Polling while hidden only checks, this often, whether it is shown.
const HIDDEN_WAIT: Duration = Duration::from_millis(250);

/// Asks the checks to run again now.
static RECHECK: OnceLock<Sender<()>> = OnceLock::new();

fn recheck() {
    if let Some(tx) = RECHECK.get() {
        let _ = tx.send(());
    }
}

/// Opens the window and runs until it is closed, or quit from the tray.
/// If a window is already open, shows that one instead.
pub fn run() -> Result<(), slint::PlatformError> {
    if nexus_control::ui_raise() {
        return Ok(());
    }
    // Creating the window also creates Slint's platform (the winit backend).
    let app = AppWindow::new()?;
    APP.with(|a| *a.borrow_mut() = Some(app.as_weak()));
    app.set_version(env!("CARGO_PKG_VERSION").into());
    app.set_homepage(env!("CARGO_PKG_REPOSITORY").into());
    // On Wayland the app ID is how the desktop finds the window's icon and
    // groups it in the task bar. It needs the platform to exist, and must be
    // set before the window is shown by `run`.
    slint::set_xdg_app_id(APP_ID)?;

    // Test-only, for captures: light or dark whatever the desktop says.
    // The user's Appearance; for captures, NEXUS_UI_SCHEME overrides it
    // without saving.
    let chosen = match std::env::var("NEXUS_UI_SCHEME").as_deref() {
        Ok("light") => appearance::Appearance::Light,
        Ok("dark") => appearance::Appearance::Dark,
        _ => appearance::load(),
    };
    app.global::<Theme>().set_forced(chosen.index());
    app.invoke_apply_scheme();
    let weak = app.as_weak();
    app.on_set_appearance(move |i| {
        let chosen = appearance::Appearance::from_index(i);
        appearance::save(chosen);
        if let Some(app) = weak.upgrade() {
            app.global::<Theme>().set_forced(chosen.index());
            app.invoke_apply_scheme();
        }
    });
    set_accent(None);
    // Each swatch as the strip draws it (its key face), not the raw preset.
    let swatches: Vec<slint::Color> = accent::PRESETS
        .iter()
        .map(|(_, c)| accent::slint(accent::roles(*c).base))
        .collect();
    app.set_accent_swatches(ModelRc::new(VecModel::from(swatches)));
    let names: Vec<SharedString> = accent::PRESETS
        .iter()
        .map(|(n, _)| {
            let mut name = i18n::shown(n);
            if let Some(first) = name.get_mut(..1) {
                first.make_ascii_uppercase();
            }
            name.into()
        })
        .collect();
    app.set_accent_names(ModelRc::new(VecModel::from(names)));
    show_status(&app);

    let editor = editor::Editor::new(&app);
    editor::Editor::connect(&editor, &app);
    editor::install(&editor);
    editor.borrow_mut().set_thumbs(thumbs::start(|done| {
        let _ = slint::invoke_from_event_loop(move || editor::pictures(done));
    }));

    let (tx, rx) = mpsc::channel();
    let _ = RECHECK.set(tx);
    app.on_check_again(recheck);
    app.on_start_service(start_service);
    let weak = app.as_weak();
    app.on_open_log(move || {
        if let Some(app) = weak.upgrade() {
            app.set_log_text(SharedString::new());
        }
        let weak = weak.clone();
        let _ = thread::Builder::new().name("log".into()).spawn(move || {
            let text = status::log();
            let _ = weak.upgrade_in_event_loop(move |app| app.set_log_text(text.into()));
        });
    });
    let weak = app.as_weak();
    thread::Builder::new()
        .name("doctor".into())
        .spawn(move || run_checks(&weak, &rx))
        .map_err(|e| slint::PlatformError::Other(e.to_string()))?;

    window_size::restore(&app);
    app.window().on_close_requested(|| {
        with_app(window_size::save);
        if IN_TRAY.load(Ordering::Relaxed) {
            VISIBLE.store(false, Ordering::Relaxed);
        } else {
            let _ = slint::quit_event_loop();
        }
        slint::CloseRequestResponse::HideWindow
    });
    IN_TRAY.store(tray::start(tray_command), Ordering::Relaxed);
    listen_for_raise();

    spawn("frames", app.as_weak(), stream_frames)?;
    spawn("status", app.as_weak(), poll_status)?;

    app.show()?;
    // Not `app.run()`: that returns once the window is hidden, and hidden
    // in the tray the window must keep running.
    let ran = slint::run_event_loop_until_quit();
    nexus_control::ui_unlisten();
    let _ = app.hide();
    ran
}

thread_local! {
    static APP: std::cell::RefCell<Option<Weak<AppWindow>>> = const { std::cell::RefCell::new(None) };
    /// The daemon's latest status (`None` while it cannot be reached) and
    /// the doctor's latest checks, for the status chip.
    static LAST: std::cell::RefCell<(Option<Status>, Vec<Check>)> =
        const { std::cell::RefCell::new((None, Vec::new())) };
    /// Why the last try to start the service failed, if it did.
    static START_FAILED: std::cell::RefCell<Option<String>> =
        const { std::cell::RefCell::new(None) };
}

/// The user's accent, derived as the strip derives it, into the theme.
pub(crate) fn set_accent(user: Option<nexus_core::theme::Rgb>) {
    let roles = accent::roles(user.unwrap_or(accent::DEFAULT));
    with_app(|app| {
        let t = app.global::<Theme>();
        t.set_accent_base(accent::slint(roles.base));
        t.set_accent_ink(accent::slint(roles.ink));
        t.set_accent_text(accent::slint(roles.text));
        t.set_accent_fill(accent::slint(roles.fill));
        t.set_accent_soft(accent::slint(roles.soft));
        t.set_accent_deep(accent::slint(roles.deep));
    });
}

/// Puts the status chip right, from the latest status and checks.
fn show_status(app: &AppWindow) {
    let view = LAST.with(|l| {
        let l = l.borrow();
        let view = status::view(l.0.as_ref(), &l.1);
        // A try to start it that failed holds until the service answers.
        match (&l.0, START_FAILED.with(|f| f.borrow().clone())) {
            (None, Some(why)) => status::did_not_start(view, &why),
            _ => view,
        }
    });
    app.set_start_failure(
        START_FAILED
            .with(|f| f.borrow().clone())
            .filter(|_| LAST.with(|l| l.borrow().0.is_none()))
            .unwrap_or_default()
            .into(),
    );
    app.set_status(StatusView {
        tone: view.tone.into(),
        chip: view.chip.into(),
        headline: view.headline.into(),
        body: view.body.into(),
        start: view.start,
        retry: view.retry,
        log: view.log,
        detail: view.detail.into(),
    });
}

/// "Start the service": runs it off the UI thread, then checks again.
fn start_service() {
    let _ = thread::Builder::new().name("start".into()).spawn(|| {
        let result = status::start();
        let _ = slint::invoke_from_event_loop(move || {
            // A new try replaces what the last one said.
            START_FAILED.with(|f| *f.borrow_mut() = result.err());
            with_app(show_status);
        });
        recheck();
    });
}

/// Runs `f` with the window, on the UI thread.
fn with_app(f: impl FnOnce(&AppWindow)) {
    let app = APP.with(|a| a.borrow().as_ref().and_then(Weak::upgrade));
    if let Some(app) = app {
        f(&app);
    }
}

/// From the tray's thread: show, hide or quit the window.
fn tray_command(command: tray::Command) {
    let _ = slint::invoke_from_event_loop(move || match command {
        tray::Command::Show => show_window(),
        tray::Command::Hide => hide_window(),
        tray::Command::Toggle => {
            if VISIBLE.load(Ordering::Relaxed) {
                hide_window();
            } else {
                show_window();
            }
        }
        tray::Command::Quit => {
            with_app(window_size::save);
            let _ = slint::quit_event_loop();
        }
    });
}

fn show_window() {
    with_app(|app| {
        let _ = app.show();
    });
    if !VISIBLE.swap(true, Ordering::Relaxed) {
        recheck();
    }
}

fn hide_window() {
    with_app(|app| {
        window_size::save(app);
        let _ = app.hide();
    });
    VISIBLE.store(false, Ordering::Relaxed);
}

/// A second `nexusd ui` asks this window to show itself.
fn listen_for_raise() {
    let Ok(listener) = nexus_control::ui_listen() else {
        return; // then a second window opens; nothing worse
    };
    let _ = thread::Builder::new().name("raise".into()).spawn(move || {
        use interprocess::local_socket::traits::ListenerExt;
        for _ in listener.incoming().flatten() {
            if slint::invoke_from_event_loop(show_window).is_err() {
                return;
            }
        }
    });
}

fn spawn(
    name: &str,
    app: Weak<AppWindow>,
    body: fn(&Weak<AppWindow>),
) -> Result<(), slint::PlatformError> {
    thread::Builder::new()
        .name(name.into())
        .spawn(move || body(&app))
        .map(drop)
        .map_err(|e| slint::PlatformError::Other(e.to_string()))
}

/// Runs the `nexusctl doctor` checks now and again, and whenever asked,
/// for the onboarding banner and "Copy diagnostics". Off the UI thread:
/// they read /proc and the socket, and the window must open at once.
fn run_checks(app: &Weak<AppWindow>, again: &Receiver<()>) {
    loop {
        while !VISIBLE.load(Ordering::Relaxed) {
            thread::sleep(HIDDEN_WAIT);
        }
        let checks = nexus_doctor::run();
        let report = nexus_doctor::report(&checks);
        let failing = checks.iter().any(|c| c.state == nexus_doctor::State::Fail);
        let wait = if failing {
            CHECK_FAILING
        } else {
            CHECK_HEALTHY
        };
        let shown = app.upgrade_in_event_loop(move |app| {
            LAST.with(|l| l.borrow_mut().1 = checks);
            show_status(&app);
            app.set_diagnostics(report.into());
        });
        if shown.is_err() {
            return; // window closed
        }
        match again.recv_timeout(wait) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => while again.try_recv().is_ok() {},
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

/// Shows every frame the daemon sends, reconnecting whenever the
/// connection drops. Returns once the window has closed.
fn stream_frames(app: &Weak<AppWindow>) {
    loop {
        // Hidden in the tray: no frames, so the daemon has no viewer.
        while !VISIBLE.load(Ordering::Relaxed) {
            thread::sleep(HIDDEN_WAIT);
        }
        let _why = watch_frames(app);
        if !VISIBLE.load(Ordering::Relaxed) {
            // Dropped on purpose; it is picked up again when shown.
            STREAMING.store(false, Ordering::Relaxed);
            continue;
        }
        // Only when it was up: retrying every second while the daemon is
        // down must not run the checks every second too.
        if STREAMING.swap(false, Ordering::Relaxed) {
            recheck();
        }
        // Fails only when the event loop has ended: the window is gone.
        let lost = app.upgrade_in_event_loop(|app| {
            app.set_connected(false);
            editor::connected(false);
            LAST.with(|l| l.borrow_mut().0 = None);
            show_status(&app);
        });
        if lost.is_err() {
            return;
        }
        thread::sleep(RETRY);
    }
}

/// Streams frames until the connection fails; returns why.
fn watch_frames(app: &Weak<AppWindow>) -> io::Error {
    let mut client = match Client::connect() {
        Ok(c) => c,
        Err(e) => return e,
    };
    match client.request("frames") {
        Ok(Ok(_)) => {}
        Ok(Err(message)) => return io::Error::other(message),
        Err(e) => return e,
    }
    // The daemon is back: the checks say so at once, not in 30 s, and
    // the layout is fetched afresh.
    if !STREAMING.swap(true, Ordering::Relaxed) {
        recheck();
        let _ = app.upgrade_in_event_loop(|_| editor::reconnected());
    }
    loop {
        let rgba = match client.next_frame() {
            Ok(f) => f,
            Err(e) => return e,
        };
        if !VISIBLE.load(Ordering::Relaxed) {
            return io::Error::other("window hidden");
        }
        if rgba.len() != (WIDTH * HEIGHT * 4) as usize {
            return io::Error::new(io::ErrorKind::InvalidData, "frame of the wrong size");
        }
        // The pixel buffer can cross threads; the Image built from it
        // cannot, so it is made on the UI thread.
        let pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&rgba, WIDTH, HEIGHT);
        let shown = app.upgrade_in_event_loop(move |app| {
            app.set_frame(Image::from_rgba8(pixels));
            if !app.get_connected() {
                app.set_connected(true);
                editor::connected(true);
            }
        });
        if shown.is_err() {
            return io::Error::other("window closed");
        }
    }
}

fn poll_status(app: &Weak<AppWindow>) {
    for tick in 0u64.. {
        if !VISIBLE.load(Ordering::Relaxed) {
            thread::sleep(HIDDEN_WAIT);
            continue;
        }
        // Signals once a second: the inspector's "Now 52 °C" lines.
        if tick % SIGNALS_EVERY == 0 {
            let entries = Client::connect()
                .ok()
                .and_then(|mut c| c.request("signals").ok()?.ok())
                .and_then(|data| serde_json::from_value::<Vec<SignalEntry>>(data).ok());
            if let Some(entries) = entries
                && app
                    .upgrade_in_event_loop(move |_| editor::signals(entries))
                    .is_err()
            {
                return;
            }
        }
        let status = Client::connect()
            .ok()
            .and_then(|mut c| c.request("status").ok()?.ok())
            .and_then(|data| serde_json::from_value::<Status>(data).ok());
        if let Some(status) = status {
            let shown = app.upgrade_in_event_loop(move |app| {
                app.set_daemon_version(status.version.as_str().into());
                editor::status(&status);
                let changed = LAST.with(|l| {
                    let mut l = l.borrow_mut();
                    let changed = l.0.as_ref().is_none_or(|old| {
                        old.device_error != status.device_error
                            || old.store_error != status.store_error
                    });
                    l.0 = Some(status);
                    changed
                });
                if changed {
                    show_status(&app);
                }
            });
            if shown.is_err() {
                return; // window closed
            }
        }
        thread::sleep(STATUS_POLL);
    }
}
