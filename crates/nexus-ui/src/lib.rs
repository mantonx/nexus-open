//! The Nexus Open settings window, built with Slint.
//!
//! This is the shell from milestone 3: a live strip preview drawn by the same
//! renderer the strip uses, plus a font picker. Frames come from the
//! renderer in-process for now; once the core exists (milestone 4) they
//! arrive from the daemon over the control socket instead, so the preview
//! always shows exactly what the strip shows.

use nexus_core::canvas::Canvas;
use nexus_core::demo::system_page;
use nexus_core::fonts::FontChoice;
use nexus_core::theme::Theme;
use nexus_core::widget::Renderer;
use slint::{Image, ModelRc, Rgba8Pixel, SharedPixelBuffer, SharedString, VecModel};

slint::include_modules!();

/// Matches the old app's ID, so the existing desktop file and icon apply.
const APP_ID: &str = "io.nexus.open";

/// Opens the window and runs until it is closed.
pub fn run() -> Result<(), slint::PlatformError> {
    // Creating the window also creates Slint's platform (the winit backend).
    let app = AppWindow::new()?;
    // On Wayland the app ID is how the desktop finds the window's icon and
    // groups it in the task bar. It needs the platform to exist, and must be
    // set before the window is shown by `run`.
    slint::set_xdg_app_id(APP_ID)?;
    let names: Vec<SharedString> = FontChoice::ALL
        .iter()
        .map(|f| f.display_name().into())
        .collect();
    app.set_fonts(ModelRc::new(VecModel::from(names)));
    app.set_frame(render(FontChoice::default()));

    // The callback must not own `app`, or the window would keep itself
    // alive; a weak handle breaks that cycle.
    let weak = app.as_weak();
    app.on_font_chosen(move |index| {
        let Some(app) = weak.upgrade() else { return };
        let font = usize::try_from(index)
            .ok()
            .and_then(|i| FontChoice::ALL.get(i).copied())
            .unwrap_or_default();
        app.set_frame(render(font));
    });

    app.run()
}

fn render(font: FontChoice) -> Image {
    let mut renderer = Renderer::new(Theme::default(), font.load());
    to_image(&renderer.render_page(&system_page()))
}

/// Copies a rendered frame into a Slint image. The canvas is always opaque,
/// so its premultiplied RGBA bytes are also plain RGBA.
pub fn to_image(canvas: &Canvas) -> Image {
    Image::from_rgba8(SharedPixelBuffer::<Rgba8Pixel>::clone_from_slice(
        canvas.rgba(),
        canvas.width(),
        canvas.height(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::theme::Rgb;

    #[test]
    fn image_keeps_size_and_pixels() {
        let mut canvas = Canvas::new(nexus_core::WIDTH, nexus_core::HEIGHT, Rgb::hex(0x000000));
        canvas.fill_rect(10, 5, 3, 2, Rgb::hex(0xC86432));
        let image = to_image(&canvas);
        let size = image.size();
        assert_eq!((size.width, size.height), (640, 48));
        let pixels = image.to_rgba8().expect("RGBA image");
        assert_eq!(pixels.as_bytes(), canvas.rgba());
    }
}
