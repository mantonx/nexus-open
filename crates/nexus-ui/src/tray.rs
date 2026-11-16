//! The tray icon: Show, Hide and Quit. With it, closing the window hides
//! it; the strip keeps working either way, since the daemon draws it.
//!
//! Linux only for now, through StatusNotifierItem (`ksni`), which KDE,
//! GNOME with the AppIndicator extension, and most panels host.

/// What a tray item asks of the window.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// Only the Linux tray makes them yet; macOS and Windows have no tray.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
pub enum Command {
    Show,
    Hide,
    /// Show if hidden, else hide: a click on the icon.
    Toggle,
    Quit,
}

/// Puts the icon in the tray. `act` runs on the tray's thread. False if
/// there is no tray to put it in.
#[cfg(target_os = "linux")]
pub fn start(act: fn(Command)) -> bool {
    use ksni::blocking::TrayMethods;

    let tray = Tray { act };
    match tray.spawn() {
        Ok(handle) => {
            // Lives as long as the process: the icon goes when it exits.
            std::mem::forget(handle);
            true
        }
        Err(_) => false,
    }
}

#[cfg(not(target_os = "linux"))]
pub fn start(_act: fn(Command)) -> bool {
    false
}

#[cfg(target_os = "linux")]
struct Tray {
    act: fn(Command),
}

#[cfg(target_os = "linux")]
impl ksni::Tray for Tray {
    fn id(&self) -> String {
        crate::APP_ID.into()
    }

    fn title(&self) -> String {
        "Nexus Open".into()
    }

    fn icon_name(&self) -> String {
        crate::APP_ID.into()
    }

    // For a build whose icon is not installed in the icon theme.
    fn icon_pixmap(&self) -> Vec<ksni::Icon> {
        icon().into_iter().collect()
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        (self.act)(Command::Toggle);
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        use ksni::menu::StandardItem;
        let item = |label: &str, command: Command| {
            StandardItem {
                label: label.into(),
                activate: Box::new(move |t: &mut Self| (t.act)(command)),
                ..Default::default()
            }
            .into()
        };
        vec![
            item("Show", Command::Show),
            item("Hide", Command::Hide),
            ksni::MenuItem::Separator,
            item("Quit", Command::Quit),
        ]
    }
}

/// The bundled icon at 64 px, as the tray wants it: ARGB, big-endian.
#[cfg(target_os = "linux")]
fn icon() -> Option<ksni::Icon> {
    const SIZE: u32 = 64;
    let png = include_bytes!("../assets/icon.png");
    let mut reader = png::Decoder::new(std::io::Cursor::new(&png[..]))
        .read_info()
        .ok()?;
    let mut rgba = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut rgba).ok()?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return None;
    }
    // Nearest neighbour is enough for a picture this simple.
    let mut data = Vec::with_capacity((SIZE * SIZE * 4) as usize);
    for y in 0..SIZE {
        for x in 0..SIZE {
            let sx = x * info.width / SIZE;
            let sy = y * info.height / SIZE;
            let i = ((sy * info.width + sx) * 4) as usize;
            let [r, g, b, a] = [rgba[i], rgba[i + 1], rgba[i + 2], rgba[i + 3]];
            data.extend_from_slice(&[a, r, g, b]);
        }
    }
    Some(ksni::Icon {
        width: SIZE as i32,
        height: SIZE as i32,
        data,
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    #[test]
    fn the_icon_decodes_to_64_pixels_square() {
        let icon = super::icon().expect("the bundled icon decodes");
        assert_eq!((icon.width, icon.height), (64, 64));
        assert_eq!(icon.data.len(), 64 * 64 * 4);
    }
}
