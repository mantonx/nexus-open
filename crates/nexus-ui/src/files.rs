//! Layouts as files: what Export writes and what Import accepts.

use std::path::Path;

use nexus_core::config::Config;
use nexus_store::Notes;

/// Larger than any real layout by far; a bigger file is not one.
const MAX_BYTES: u64 = 1 << 20;

/// The text of a layout file: its pages and takeovers, and the strip
/// settings. Not where the weather is for, nor the sources: a layout file
/// is made to be shared, and those say where the user lives and what is
/// in their home directory. Import ignores them anyway.
pub fn export_text(config: &Config, notes: &Notes) -> String {
    let mut shared = config.clone();
    shared.weather = None;
    shared.sources.clear();
    nexus_store::text::write(&shared, notes)
}

/// A file's name for a layout, without characters file systems refuse.
pub fn file_name(layout: &str) -> String {
    let safe: String = layout
        .chars()
        .map(|c| {
            if "/\\:*?\"<>|".contains(c) || c.is_control() {
                '-'
            } else {
                c
            }
        })
        .collect();
    format!("{}.toml", safe.trim())
}

/// Reads a layout file: its text and the name it suggests (the file name
/// without `.toml`), or what is wrong with it.
pub fn read(path: &Path) -> Result<(String, String), String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?
        .len();
    if size > MAX_BYTES {
        return Err(format!("{} is too large to be a layout.", path.display()));
    }
    let text = std::fs::read_to_string(path)
        .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
    nexus_store::text::parse(&text)
        .map_err(|e| format!("That file is not a layout Nexus Open can use: {e}"))?;
    let name = path
        .file_stem()
        .map(|s| s.to_string_lossy().trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Imported".into());
    Ok((name, text))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nexus_core::config::Weather;

    #[test]
    fn an_export_leaves_out_the_weather_place_and_sources() {
        let mut config = Config::default_layout();
        config.weather = Some(Weather {
            location: Some("Somewhere Private".into()),
            latitude: None,
            longitude: None,
        });
        let text = export_text(&config, &Notes::default());
        assert!(!text.contains("Somewhere Private"));
        let (back, _) = nexus_store::text::parse(&text).unwrap();
        assert_eq!(back.pages, config.pages);
        assert_eq!(back.takeovers, config.takeovers);
        assert_eq!(back.weather, None);
    }

    #[test]
    fn file_names_are_safe() {
        assert_eq!(file_name("Main"), "Main.toml");
        assert_eq!(file_name("Desk/Work: 2"), "Desk-Work- 2.toml");
    }

    #[test]
    fn reading_names_the_layout_after_the_file_and_refuses_others() {
        let dir = std::env::temp_dir().join(format!("nexus-files-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let good = dir.join("Desk.toml");
        std::fs::write(
            &good,
            export_text(&Config::default_layout(), &Notes::default()),
        )
        .unwrap();
        let (name, _) = read(&good).unwrap();
        assert_eq!(name, "Desk");
        let bad = dir.join("notes.toml");
        std::fs::write(&bad, "shopping = [\"milk\"]").unwrap();
        assert!(read(&bad).unwrap_err().contains("not a layout"));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
