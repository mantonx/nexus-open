//! Adding a layout: what it can start from, and the text the daemon
//! imports for it. Every way of adding is one `import` request, so the
//! layout is added and put on the strip in one go.

use nexus_core::config::{Config, STARTERS};
use nexus_store::Notes;

/// A copy of the layout on the strip.
pub const COPY: &str = "A copy of this one";
/// One empty page.
pub const EMPTY: &str = "Empty";

/// What a new layout can start from, in the order offered.
pub fn choices() -> Vec<String> {
    let mut out = vec![COPY.to_string(), EMPTY.to_string()];
    out.extend(STARTERS.iter().map(|(name, _)| format!("Starter: {name}")));
    out
}

/// The name for a new layout: `base`, or `base 2`, `base 3`… if taken.
pub fn unique_name(base: &str, taken: &[&str]) -> String {
    if !taken.contains(&base) {
        return base.into();
    }
    (2..)
        .map(|n| format!("{base} {n}"))
        .find(|n| !taken.contains(&n.as_str()))
        .expect("some number is free")
}

/// The name and text to import for `choice`, given the layout on the
/// strip (its name, config and notes); `None` for an unknown choice.
pub fn start(choice: &str, current: (&str, &Config, &Notes)) -> Option<(String, String)> {
    let (name, config, notes) = current;
    if choice == COPY {
        return Some((
            format!("{name} copy"),
            nexus_store::text::write(config, notes),
        ));
    }
    if choice == EMPTY {
        return Some(("New layout".into(), "[[page]]\nname = \"Page 1\"\n".into()));
    }
    let starter = choice.strip_prefix("Starter: ")?;
    STARTERS
        .iter()
        .find(|(n, _)| *n == starter)
        .map(|(n, text)| (n.to_string(), text.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_count_up_past_the_taken_ones() {
        assert_eq!(unique_name("Music", &["Main"]), "Music");
        assert_eq!(unique_name("Music", &["Music", "Music 2"]), "Music 3");
    }

    #[test]
    fn every_choice_gives_a_layout_that_parses() {
        let config = Config::default_layout();
        let notes = Notes::default();
        for choice in choices() {
            let (name, text) = start(&choice, ("Main", &config, &notes))
                .unwrap_or_else(|| panic!("{choice} gives nothing"));
            assert!(!name.is_empty());
            nexus_store::text::parse(&text).unwrap_or_else(|e| panic!("{choice}: {e}"));
        }
        assert_eq!(start("Starter: Nope", ("Main", &config, &notes)), None);
    }

    #[test]
    fn a_copy_keeps_every_page() {
        let config = Config::default_layout();
        let (name, text) = start(COPY, ("Main", &config, &Notes::default())).unwrap();
        assert_eq!(name, "Main copy");
        let (copy, _) = nexus_store::text::parse(&text).unwrap();
        assert_eq!(copy.pages, config.pages);
    }
}
