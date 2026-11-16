//! The Nexus Open settings window, built with Slint.
//!
//! The window draws nothing itself. It streams frames from the daemon over
//! the control socket and shows them, so the preview is always exactly
//! what the strip shows. The preview is also the editor (`editor`): its
//! changes are edits sent over the same socket. If the daemon is not
//! running the window says so and keeps retrying.

mod editor;
mod geometry;
mod inspector;
mod settings;
mod signals;
mod takeovers;

use std::io;
use std::thread;
use std::time::Duration;

use nexus_control::{Client, SignalEntry, Status};
use slint::{Image, Rgba8Pixel, SharedPixelBuffer, Weak};

slint::include_modules!();

/// Matches the old app's ID, so the existing desktop file and icon apply.
const APP_ID: &str = "io.nexus.open";
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

/// Opens the window and runs until it is closed.
pub fn run() -> Result<(), slint::PlatformError> {
    // Creating the window also creates Slint's platform (the winit backend).
    let app = AppWindow::new()?;
    // On Wayland the app ID is how the desktop finds the window's icon and
    // groups it in the task bar. It needs the platform to exist, and must be
    // set before the window is shown by `run`.
    slint::set_xdg_app_id(APP_ID)?;

    let editor = editor::Editor::new(&app);
    editor::Editor::connect(&editor, &app);
    editor::install(&editor);

    spawn("frames", app.as_weak(), stream_frames)?;
    spawn("status", app.as_weak(), poll_status)?;

    app.run()
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

/// Shows every frame the daemon sends, reconnecting whenever the
/// connection drops. Returns once the window has closed.
fn stream_frames(app: &Weak<AppWindow>) {
    loop {
        let _why = watch_frames(app);
        // Fails only when the event loop has ended: the window is gone.
        if app
            .upgrade_in_event_loop(|app| app.set_connected(false))
            .is_err()
        {
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
    loop {
        let rgba = match client.next_frame() {
            Ok(f) => f,
            Err(e) => return e,
        };
        if rgba.len() != (WIDTH * HEIGHT * 4) as usize {
            return io::Error::new(io::ErrorKind::InvalidData, "frame of the wrong size");
        }
        // The pixel buffer can cross threads; the Image built from it
        // cannot, so it is made on the UI thread.
        let pixels = SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(&rgba, WIDTH, HEIGHT);
        let shown = app.upgrade_in_event_loop(move |app| {
            app.set_frame(Image::from_rgba8(pixels));
            app.set_connected(true);
        });
        if shown.is_err() {
            return io::Error::other("window closed");
        }
    }
}

fn poll_status(app: &Weak<AppWindow>) {
    for tick in 0u64.. {
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
            let position = status
                .pages
                .iter()
                .position(|p| p == &status.page)
                .map_or(0, |i| i + 1);
            let label = format!("{position} of {}", status.pages.len());
            let shown = app.upgrade_in_event_loop(move |app| {
                editor::status(&status);
                app.set_page_name(status.page.into());
                app.set_page_position(label.into());
            });
            if shown.is_err() {
                return; // window closed
            }
        }
        thread::sleep(STATUS_POLL);
    }
}
