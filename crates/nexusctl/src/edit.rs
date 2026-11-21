//! `nexusctl edit`: the layout, settings and sources as TOML in your own
//! editor, applied when the editor closes. Comments directly above a table
//! are kept as that item's note (see docs/STORAGE.md).

use std::path::Path;
use std::process::Command;

use nexus_control::{Client, EditRequest, Export};
use nexus_store::{Edit, text};

/// Marks the lines this command adds at the top to explain why the file
/// came back. `##` lines are never kept, so they cannot leak into notes.
const MESSAGE: &str = "## ! ";

pub fn run(client: &mut Client, layout: Option<&str>) -> Result<(), String> {
    let mut export = export(client, layout)?;
    let path = std::env::temp_dir().join(format!("nexus-edit-{}.toml", std::process::id()));
    std::fs::write(&path, &export.text).map_err(|e| format!("{}: {e}", path.display()))?;
    let result = edit_until_done(client, &path, &mut export);
    let _ = std::fs::remove_file(&path);
    result
}

fn edit_until_done(client: &mut Client, path: &Path, export: &mut Export) -> Result<(), String> {
    let mut base = export.revision;
    loop {
        open_editor(path)?;
        let written = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
        let text = without_messages(&written);
        if text == export.text {
            say!("No changes.\n");
            return Ok(());
        }
        if text
            .lines()
            .all(|l| l.trim().is_empty() || l.trim().starts_with('#'))
        {
            say!("Emptied: nothing applied.\n");
            return Ok(());
        }
        let (config, notes) = match text::parse(&text) {
            Ok(parsed) => parsed,
            Err(e) => {
                reopen_with(path, &text, &e.to_string())?;
                continue;
            }
        };
        let request = EditRequest {
            edit: Edit::Replace {
                layout: Some(export.id),
                config: Box::new(config),
                notes,
            },
            base: Some(base),
        };
        let line = serde_json::to_string(&request).map_err(|e| e.to_string())?;
        match client.request(&format!("edit {line}")) {
            Ok(Ok(reply)) => {
                say!(
                    "Applied to layout \"{}\" (revision {}). Undo with: nexusctl undo\n",
                    export.layout,
                    reply["revision"]
                );
                for line in text::stray_comments(&text) {
                    eprintln!(
                        "nexusctl: the comment on line {line} was not kept: only comments directly above [settings], [[page]], [[page.widget]], [[takeover]], [[takeover.widget]] or [[source]] are"
                    );
                }
                return Ok(());
            }
            Ok(Err(message)) if message.contains("changed since") => {
                // Saving again applies this text over the other change.
                base = self::export(client, Some(&export.layout))?.revision;
                reopen_with(
                    path,
                    &text,
                    "The layout was changed elsewhere since this file was opened. Save again to apply this version over that change, or empty the file to cancel.",
                )?;
            }
            Ok(Err(message)) => reopen_with(path, &text, &message)?,
            Err(e) => {
                return Err(format!(
                    "lost the connection to nexusd: {e}; nothing was applied"
                ));
            }
        }
    }
}

fn export(client: &mut Client, layout: Option<&str>) -> Result<Export, String> {
    let line = match layout {
        Some(name) => format!("export {name}"),
        None => "export".into(),
    };
    let data = client
        .request(&line)
        .map_err(|e| format!("lost the connection to nexusd: {e}"))??;
    serde_json::from_value(data).map_err(|e| format!("unexpected answer from nexusd: {e}"))
}

/// Writes `text` back with `message` above it, for the next editor round.
fn reopen_with(path: &Path, text: &str, message: &str) -> Result<(), String> {
    let mut out = String::new();
    for line in message.lines() {
        out.push_str(MESSAGE);
        out.push_str(line);
        out.push('\n');
    }
    out.push_str(MESSAGE);
    out.push_str("Fix it and save, or empty the file to cancel.\n");
    out.push_str(text);
    std::fs::write(path, out).map_err(|e| e.to_string())
}

fn without_messages(text: &str) -> String {
    let mut lines = text.lines().peekable();
    while lines.peek().is_some_and(|l| l.starts_with(MESSAGE)) {
        lines.next();
    }
    let mut out: String = lines.collect::<Vec<_>>().join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// $VISUAL, then $EDITOR (either may have arguments, as in "code --wait"),
/// then the platform's plain editor. Waits for it to close.
fn open_editor(path: &Path) -> Result<(), String> {
    let chosen = std::env::var("VISUAL")
        .ok()
        .or_else(|| std::env::var("EDITOR").ok())
        .filter(|e| !e.trim().is_empty())
        .unwrap_or_else(|| if cfg!(windows) { "notepad" } else { "vi" }.into());
    let mut words = chosen.split_whitespace();
    let program = words.next().unwrap_or("vi");
    let status = Command::new(program)
        .args(words)
        .arg(path)
        .status()
        .map_err(|e| format!("cannot start the editor \"{chosen}\": {e}; set VISUAL or EDITOR"))?;
    if !status.success() {
        return Err(format!(
            "the editor exited with {status}; nothing was applied"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_are_added_and_removed_whole() {
        let dir = std::env::temp_dir().join(format!("nexusctl-test-{}.toml", std::process::id()));
        let text = "## help\n[settings]\n";
        reopen_with(&dir, text, "line 1\nline 2").unwrap();
        let written = std::fs::read_to_string(&dir).unwrap();
        assert!(written.starts_with("## ! line 1\n## ! line 2\n"));
        assert_eq!(without_messages(&written), text);
        let _ = std::fs::remove_file(&dir);
    }
}
