//! `nexusctl import-legacy`: brings a layout over from the old Go and
//! Flutter Nexus Open.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use nexus_core::config::Temperature;
use nexus_store::Notes;
use nexus_store::legacy::{self, Conversion};

/// Where the old app kept its database: Go's `os.UserConfigDir()`, then
/// `nexus-open/nexus.db`.
pub fn default_path() -> Option<PathBuf> {
    let dir = if cfg!(target_os = "windows") {
        std::env::var_os("AppData").map(PathBuf::from)
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|h| Path::new(&h).join("Library/Application Support"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".config")))
    }?;
    Some(dir.join("nexus-open").join("nexus.db"))
}

/// Reads and converts `path`: a YAML export by its extension, else the
/// old database. The layout's name comes back with it.
fn read(path: &Path) -> Result<(Option<String>, Conversion), String> {
    let is_yaml = path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("yaml") || e.eq_ignore_ascii_case("yml"));
    let old = if is_yaml {
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        legacy::read_yaml(&text)?
    } else {
        legacy::read_db(path)?
    };
    let conversion = legacy::convert(&old)?;
    Ok((old.name, conversion))
}

pub fn run(
    client: Option<&mut nexus_control::Client>,
    path: Option<&Path>,
    name: Option<&str>,
    activate: bool,
) -> ExitCode {
    let Some(path) = path.map(Path::to_path_buf).or_else(default_path) else {
        eprintln!("nexusctl: cannot find the old database; give its path");
        return ExitCode::FAILURE;
    };
    let (old_name, conversion) = match read(&path) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("nexusctl: {e}");
            return ExitCode::FAILURE;
        }
    };
    let text = nexus_store::text::write(&conversion.config, &Notes::default());
    let Some(client) = client else {
        // --print: the layout, then what was left out, as comments.
        say!("{text}");
        for line in report(&conversion) {
            say!("# {line}\n");
        }
        return ExitCode::SUCCESS;
    };
    let name = name
        .map(str::to_string)
        .or(old_name)
        .unwrap_or_else(|| "Old layout".into());
    let request = nexus_control::ImportRequest {
        name: name.clone(),
        text,
        activate,
    };
    let line = format!(
        "import {}",
        serde_json::to_string(&request).unwrap_or_default()
    );
    match client.request(&line) {
        Ok(Ok(_)) => {}
        Ok(Err(e)) => {
            eprintln!("nexusctl: {e}");
            return ExitCode::FAILURE;
        }
        Err(e) => {
            eprintln!("nexusctl: {e}");
            return ExitCode::FAILURE;
        }
    }
    let pages = conversion.config.pages.len();
    say!(
        "Added the layout \"{name}\" with {pages} page{} from {}{}.\n",
        if pages == 1 { "" } else { "s" },
        path.display(),
        if activate {
            ", and put it on the strip"
        } else {
            "; switch to it in the settings window"
        }
    );
    for line in report(&conversion) {
        say!("  {line}\n");
    }
    ExitCode::SUCCESS
}

/// What the user should know: what was left out, and the old settings,
/// which an import does not change once the app has its own.
fn report(c: &Conversion) -> Vec<String> {
    let mut out = c.left_out.clone();
    if c.config.settings.temperature == Temperature::Fahrenheit {
        out.push("The old layout showed °F. If this app shows °C, change it in Settings.".into());
    }
    if let Some(place) = c
        .config
        .weather
        .as_ref()
        .and_then(|w| w.location.as_deref())
    {
        out.push(format!(
            "The old weather was for \"{place}\". If no weather shows, set it in Settings."
        ));
    }
    out
}
