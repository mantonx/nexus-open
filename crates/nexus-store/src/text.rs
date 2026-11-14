//! The text form of the store: TOML, for `nexusctl edit`, import, export
//! and starter layouts.
//!
//! Comments become notes. The comment lines directly above a `[settings]`,
//! `[[page]]`, `[[page.widget]]`, `[[takeover]]`, `[[takeover.widget]]` or
//! `[[source]]` line are that item's note, and writing puts them back in the
//! same place. Lines starting with `##` are help written at the top of the
//! file, and are never kept.

use std::fmt::Write as _;

use nexus_core::config::{Config, ConfigError, Widget};
use toml_edit::{Decor, DocumentMut, Item, Table};

/// The notes on a config's items, in the same order as the items. Missing
/// entries are empty notes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Notes {
    pub settings: String,
    pub pages: Vec<RowNotes>,
    pub takeovers: Vec<RowNotes>,
    pub sources: Vec<String>,
}

/// A page's or takeover's note and its widgets' notes.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct RowNotes {
    pub note: String,
    pub widgets: Vec<String>,
}

impl RowNotes {
    pub fn widget(&self, i: usize) -> &str {
        self.widgets.get(i).map_or("", String::as_str)
    }
}

impl Notes {
    pub fn page(&self, i: usize) -> &RowNotes {
        self.pages.get(i).unwrap_or(&EMPTY_ROW)
    }

    pub fn takeover(&self, i: usize) -> &RowNotes {
        self.takeovers.get(i).unwrap_or(&EMPTY_ROW)
    }

    pub fn source(&self, i: usize) -> &str {
        self.sources.get(i).map_or("", String::as_str)
    }
}

static EMPTY_ROW: RowNotes = RowNotes {
    note: String::new(),
    widgets: Vec::new(),
};

/// Written at the top of every file; `##` so it is never read back as a
/// note.
const HELP: &str = "\
## Nexus Open: what the strip shows. Saving applies it.
##
## The strip is 640 px wide; each page's widget widths add up to 640 or less.
## Widgets: clock, value, rate, slider, media. Tap and hold actions:
##   tap = { page = \"Clock\" }   next, prev or a page name
##   tap = { media = \"play-pause\" }   play-pause, next, previous, stop
##   tap = { run = [\"pavucontrol\"] }   a program and its arguments, no shell
##   hold = { open = \"https://example.org\" }   a URL or file
## A comment directly above a [[page]], [[page.widget]], [[takeover]] or
## [[source]] line is kept as that item's note. Other comments are not kept.
";

/// Reads a config and its notes.
pub fn parse(text: &str) -> Result<(Config, Notes), ConfigError> {
    let config = Config::parse(text)?;
    let doc: DocumentMut = text
        .parse()
        .map_err(|e: toml_edit::TomlError| ConfigError(e.to_string()))?;
    let notes = Notes {
        settings: doc
            .get("settings")
            .and_then(Item::as_table)
            .map(|t| note(t.decor()))
            .unwrap_or_default(),
        pages: rows(doc.get("page")),
        takeovers: rows(doc.get("takeover")),
        sources: tables(doc.get("source")).map(|t| note(t.decor())).collect(),
    };
    Ok((config, notes))
}

impl Notes {
    /// Drops note lines that also appear as comments in `help`, for files
    /// written before notes existed: they carry copies of the bundled
    /// layout's help, which are not the person's own notes.
    pub fn without_lines_in(mut self, help: &str) -> Notes {
        let known: std::collections::HashSet<&str> = help
            .lines()
            .filter_map(|l| l.trim().strip_prefix('#'))
            .map(|l| l.trim_start_matches('#').trim())
            .filter(|l| !l.is_empty())
            .collect();
        let clean = |note: &mut String| {
            let kept: Vec<&str> = note.lines().filter(|l| !known.contains(l.trim())).collect();
            // Collapse the blank lines the removed text leaves behind.
            let mut out: Vec<&str> = Vec::new();
            for line in kept {
                if !(line.trim().is_empty() && out.last().is_none_or(|l| l.trim().is_empty())) {
                    out.push(line);
                }
            }
            while out.last().is_some_and(|l| l.trim().is_empty()) {
                out.pop();
            }
            *note = out.join("\n");
        };
        clean(&mut self.settings);
        for row in self.pages.iter_mut().chain(self.takeovers.iter_mut()) {
            clean(&mut row.note);
            row.widgets.iter_mut().for_each(clean);
        }
        self.sources.iter_mut().for_each(clean);
        self
    }
}

fn tables(item: Option<&Item>) -> impl Iterator<Item = &Table> {
    item.and_then(Item::as_array_of_tables)
        .into_iter()
        .flat_map(|a| a.iter())
}

fn rows(item: Option<&Item>) -> Vec<RowNotes> {
    tables(item)
        .map(|t| RowNotes {
            note: note(t.decor()),
            widgets: tables(t.get("widget")).map(|w| note(w.decor())).collect(),
        })
        .collect()
}

/// The note in the comment lines before a table header. Blank lines
/// between comments become one empty line; `##` help lines are skipped.
fn note(decor: &Decor) -> String {
    let raw = decor.prefix().and_then(|r| r.as_str()).unwrap_or("");
    let mut lines: Vec<&str> = Vec::new();
    let mut gap = false;
    for line in raw.lines().map(str::trim) {
        if let Some(text) = line.strip_prefix('#').filter(|_| !line.starts_with("##")) {
            if gap && !lines.is_empty() {
                lines.push("");
            }
            gap = false;
            lines.push(text.strip_prefix(' ').unwrap_or(text).trim_end());
        } else {
            gap = true;
        }
    }
    lines.join("\n")
}

/// Writes a config and its notes as TOML that `parse` reads back to the
/// same config and notes.
pub fn write(config: &Config, notes: &Notes) -> String {
    let mut out = String::from(HELP);
    header(&mut out, &notes.settings, "[settings]");
    keys(&mut out, &config.settings, &[]);
    if let Some(weather) = &config.weather {
        header(&mut out, "", "[weather]");
        keys(&mut out, weather, &[]);
    }
    for (i, page) in config.pages.iter().enumerate() {
        let n = notes.page(i);
        header(&mut out, &n.note, "[[page]]");
        keys(&mut out, page, &["widget"]);
        widgets(&mut out, &page.widgets, n, "[[page.widget]]");
    }
    for (i, takeover) in config.takeovers.iter().enumerate() {
        let n = notes.takeover(i);
        header(&mut out, &n.note, "[[takeover]]");
        keys(&mut out, takeover, &["widget"]);
        widgets(&mut out, &takeover.widgets, n, "[[takeover.widget]]");
    }
    for (i, source) in config.sources.iter().enumerate() {
        header(&mut out, notes.source(i), "[[source]]");
        keys(&mut out, source, &[]);
    }
    out
}

fn widgets(out: &mut String, widgets: &[Widget], notes: &RowNotes, line: &str) {
    for (i, widget) in widgets.iter().enumerate() {
        header(out, notes.widget(i), line);
        keys(out, widget, &[]);
    }
}

/// A blank line, the note as comments, and the table's header line.
fn header(out: &mut String, note: &str, line: &str) {
    out.push('\n');
    for text in note.lines() {
        if text.is_empty() {
            out.push_str("#\n");
        } else {
            let _ = writeln!(out, "# {text}");
        }
    }
    out.push_str(line);
    out.push('\n');
}

/// One `key = value` line per field, in the order the type declares them.
/// Fields in `skip` are written separately (a page's widgets).
fn keys(out: &mut String, item: &impl serde::Serialize, skip: &[&str]) {
    let Ok(toml::Value::Table(table)) = toml::Value::try_from(item) else {
        unreachable!("config types serialize to tables");
    };
    for (key, value) in table.iter().filter(|(k, _)| !skip.contains(&k.as_str())) {
        let _ = writeln!(out, "{key} = {value}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notes_are_read_from_comments_above_headers() {
        let (_, notes) = parse(
            "## help, never kept\n\
             # Units for the whole strip.\n\
             [settings]\n\
             clock = \"12h\"\n\
             \n\
             # The one I look at most.\n\
             #\n\
             # Second paragraph.\n\
             [[page]]\n\
             name = \"A\"\n\
             \n\
             # Shows the time.\n\
             [[page.widget]]\n\
             type = \"clock\"\n\
             width = 100\n\
             [[page.widget]]\n\
             type = \"clock\"\n\
             width = 100\n",
        )
        .unwrap();
        assert_eq!(notes.settings, "Units for the whole strip.");
        assert_eq!(
            notes.page(0).note,
            "The one I look at most.\n\nSecond paragraph."
        );
        assert_eq!(notes.page(0).widgets, ["Shows the time.", ""]);
    }

    #[test]
    fn the_default_layout_round_trips_with_notes() {
        let config = Config::default_layout();
        let mut notes = Notes {
            settings: "Settings note.".into(),
            ..Notes::default()
        };
        notes.pages.push(RowNotes {
            note: "First page.\n\nWith a gap.".into(),
            widgets: vec!["".into(), "The media zone.".into()],
        });
        notes.takeovers.push(RowNotes {
            note: "Pops up on a new track.".into(),
            widgets: vec![],
        });
        let text = write(&config, &notes);
        let (again, read) = parse(&text).unwrap_or_else(|e| panic!("{e}\n{text}"));
        assert_eq!(again, config);
        // Trailing empty entries are not written, so compare what was set.
        assert_eq!(read.settings, notes.settings);
        assert_eq!(read.page(0).note, notes.page(0).note);
        assert_eq!(read.page(0).widget(1), "The media zone.");
        assert_eq!(read.takeover(0).note, "Pops up on a new track.");
        assert!(text.contains("type = \"clock\"\nwidth = 107\n"), "{text}");
    }

    #[test]
    fn copied_help_is_not_a_note_but_your_own_words_are() {
        let help = "## The strip is 640 px wide.\n## Swipe to change page.\n";
        let (_, notes) = parse(
            "# The strip is 640 px wide.\n\
             # Swipe to change page.\n\
             #\n\
             # Mine: Fahrenheit for the office.\n\
             [settings]\n\
             [[page]]\n\
             name = \"A\"\n",
        )
        .unwrap();
        let notes = notes.without_lines_in(help);
        assert_eq!(notes.settings, "Mine: Fahrenheit for the office.");
    }

    #[test]
    fn the_bundled_layout_has_notes_only_where_meant() {
        let (_, notes) = parse(nexus_core::config::DEFAULT).unwrap();
        assert_eq!(notes.settings, "");
        let with_notes: Vec<&str> = notes
            .pages
            .iter()
            .filter(|p| !p.note.is_empty())
            .map(|p| p.note.as_str())
            .collect();
        assert_eq!(with_notes.len(), 1, "only the Sound page: {with_notes:?}");
        assert!(notes.takeovers.iter().all(|t| t.note.is_empty()));
    }
}
