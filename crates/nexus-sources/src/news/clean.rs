//! Turning feed text into a plain line the strip can draw.
//!
//! Feed text is written by strangers. Publishers escape it inconsistently
//! (HTML inside RSS titles, entities escaped twice), and some of it is
//! hostile: bidi overrides that make a headline read differently from what
//! it is, control characters, a title the length of an article. Everything
//! here turns text into text; nothing is ever treated as markup to render or
//! a command to run.

/// The longest headline kept, in characters. The strip shows far less; the
/// cap only stops one feed from filling memory and logs with a novel.
pub const MAX_HEADLINE: usize = 300;
/// The longest source name kept. A feed title is a label, not a sentence.
pub const MAX_SOURCE: usize = 80;

/// The separators publishers put between a headline and their own name, as
/// in "Storm closes schools - BBC News".
const SUFFIX_SEPARATORS: &[&str] = &[" - ", " – ", " — ", " | ", " · ", " :: "];

/// A headline from a feed: tags stripped, entities decoded, invisible and
/// direction-changing characters removed, whitespace collapsed, the
/// publisher's name trimmed off the end when it equals `source` exactly, and
/// capped at [`MAX_HEADLINE`]. Empty if nothing printable is left.
pub fn headline(raw: &str, source: &str) -> String {
    let text = plain(raw);
    let text = trim_source(&text, source);
    cap(text, MAX_HEADLINE)
}

/// A feed's own title, cleaned the same way and capped at [`MAX_SOURCE`].
pub fn source(raw: &str) -> String {
    cap(&plain(raw), MAX_SOURCE)
}

/// Text with markup and unprintable characters removed and whitespace
/// collapsed to single spaces.
pub fn plain(raw: &str) -> String {
    // Tags first, then entities: an entity that decodes to "<" is then a
    // literal character, not the start of a tag to strip.
    let decoded = decode_entities(&strip_tags(raw));
    let mut out = String::with_capacity(decoded.len());
    let mut space = false;
    for c in decoded.chars() {
        if c.is_whitespace() {
            space = !out.is_empty();
        } else if !unwanted(c) {
            if space {
                out.push(' ');
                space = false;
            }
            out.push(c);
        }
    }
    out
}

/// Characters that are never shown: controls, and the invisible formatting
/// characters that reorder or hide text. Bidi overrides and isolates are
/// the dangerous ones (they can make "gro.evil" read as "live.org");
/// zero-width spaces and soft hyphens only make two equal headlines differ.
/// The joiners are kept, because emoji and Persian need them.
fn unwanted(c: char) -> bool {
    c.is_control()
        || matches!(
            c,
            '\u{00AD}'
                | '\u{061C}'
                | '\u{180E}'
                | '\u{200B}'
                | '\u{200E}'
                | '\u{200F}'
                | '\u{202A}'..='\u{202E}'
                | '\u{2060}'..='\u{2064}'
                | '\u{2066}'..='\u{2069}'
                | '\u{FEFF}'
                | '\u{FFF9}'..='\u{FFFB}'
        )
}

/// Removes HTML tags. A `<` only starts a tag when a letter, `/`, `!` or
/// `?` follows and a `>` closes it, so "3 < 4" survives. Tags that break a
/// line in HTML become a space, so "one<br>two" does not read "onetwo".
fn strip_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let tag = &rest[at..];
        let opens = tag[1..]
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || matches!(c, '/' | '!' | '?'));
        match tag.find('>') {
            Some(end) if opens => {
                if breaks_line(&tag[1..end]) {
                    out.push(' ');
                }
                rest = &tag[end + 1..];
            }
            _ => {
                out.push('<');
                rest = &tag[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

fn breaks_line(tag: &str) -> bool {
    let name: String = tag
        .trim_start_matches('/')
        .chars()
        .take_while(char::is_ascii_alphanumeric)
        .collect::<String>()
        .to_ascii_lowercase();
    matches!(
        name.as_str(),
        "br" | "p" | "div" | "li" | "tr" | "td" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
    )
}

/// Decodes `&name;`, `&#123;` and `&#x1F4F0;`. The XML parser has already
/// decoded one level; this is for the HTML a publisher escaped inside it.
/// An unknown name is left as written, and a number that is not a
/// character becomes U+FFFD rather than vanishing silently.
fn decode_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        // Entities are short; looking further only finds the next one's `;`.
        let decoded = after
            .char_indices()
            .take(12)
            .find(|&(_, c)| c == ';')
            .and_then(|(end, _)| entity(&after[..end]).map(|c| (c, end)));
        match decoded {
            Some((c, end)) => {
                out.push(c);
                rest = &after[end + 1..];
            }
            None => {
                out.push('&');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn entity(name: &str) -> Option<char> {
    if let Some(number) = name.strip_prefix('#') {
        let code = match number.strip_prefix(['x', 'X']) {
            Some(hex) => u32::from_str_radix(hex, 16),
            None => number.parse::<u32>(),
        }
        .ok()?;
        return Some(
            char::from_u32(code)
                .filter(|&c| c != '\0')
                .unwrap_or('\u{FFFD}'),
        );
    }
    Some(match name {
        "amp" => '&',
        "lt" => '<',
        "gt" => '>',
        "quot" => '"',
        "apos" => '\'',
        "nbsp" | "ensp" | "emsp" | "thinsp" => ' ',
        "ndash" => '–',
        "mdash" => '—',
        "hellip" => '…',
        "lsquo" => '‘',
        "rsquo" => '’',
        "sbquo" => '‚',
        "ldquo" => '“',
        "rdquo" => '”',
        "bdquo" => '„',
        "laquo" => '«',
        "raquo" => '»',
        "lsaquo" => '‹',
        "rsaquo" => '›',
        "bull" => '•',
        "middot" => '·',
        "prime" => '′',
        "Prime" => '″',
        "copy" => '©',
        "reg" => '®',
        "trade" => '™',
        "deg" => '°',
        "plusmn" => '±',
        "times" => '×',
        "divide" => '÷',
        "frac12" => '½',
        "frac14" => '¼',
        "frac34" => '¾',
        "euro" => '€',
        "pound" => '£',
        "yen" => '¥',
        "cent" => '¢',
        "sect" => '§',
        "para" => '¶',
        "shy" => '\u{00AD}',
        "zwj" => '\u{200D}',
        "zwnj" => '\u{200C}',
        "lrm" => '\u{200E}',
        "rlm" => '\u{200F}',
        "aacute" => 'á',
        "agrave" => 'à',
        "acirc" => 'â',
        "auml" => 'ä',
        "atilde" => 'ã',
        "aring" => 'å',
        "aelig" => 'æ',
        "ccedil" => 'ç',
        "eacute" => 'é',
        "egrave" => 'è',
        "ecirc" => 'ê',
        "euml" => 'ë',
        "iacute" => 'í',
        "igrave" => 'ì',
        "icirc" => 'î',
        "iuml" => 'ï',
        "ntilde" => 'ñ',
        "oacute" => 'ó',
        "ograve" => 'ò',
        "ocirc" => 'ô',
        "ouml" => 'ö',
        "otilde" => 'õ',
        "oslash" => 'ø',
        "uacute" => 'ú',
        "ugrave" => 'ù',
        "ucirc" => 'û',
        "uuml" => 'ü',
        "yacute" => 'ý',
        "szlig" => 'ß',
        "Aacute" => 'Á',
        "Auml" => 'Ä',
        "Aring" => 'Å',
        "Ccedil" => 'Ç',
        "Eacute" => 'É',
        "Ntilde" => 'Ñ',
        "Oacute" => 'Ó',
        "Ouml" => 'Ö',
        "Oslash" => 'Ø',
        "Uuml" => 'Ü',
        _ => return None,
    })
}

/// "Storm closes schools - BBC News" → "Storm closes schools" when the
/// feed is called "BBC News". Only an exact match (ignoring case) is
/// trimmed: "Ten things - a list" from a feed called "Lists" is a headline,
/// not a suffix.
fn trim_source<'a>(text: &'a str, source: &str) -> &'a str {
    if source.is_empty() {
        return text;
    }
    for sep in SUFFIX_SEPARATORS {
        if let Some((head, tail)) = text.rsplit_once(sep)
            && !head.trim().is_empty()
            && tail.trim().to_lowercase() == source.to_lowercase()
        {
            return head.trim_end();
        }
    }
    text
}

/// At most `max` characters, cut at a word boundary where one is near and
/// ended with "…" so a cut never looks like the whole text.
fn cap(text: &str, max: usize) -> String {
    let Some((cut, _)) = text.char_indices().nth(max) else {
        return text.to_string();
    };
    // Leave room for the ellipsis.
    let (limit, _) = text.char_indices().nth(max - 1).unwrap_or((cut, ' '));
    let head = &text[..limit];
    let head = match head.rfind(' ') {
        Some(space) if head[space..].chars().count() < 40 => &head[..space],
        _ => head,
    };
    format!("{}…", head.trim_end())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_tags_but_not_comparisons() {
        assert_eq!(plain("<b>Bold</b> move"), "Bold move");
        assert_eq!(plain("one<br/>two"), "one two");
        assert_eq!(plain("3 < 4 and 5 > 2"), "3 < 4 and 5 > 2");
        assert_eq!(plain("a <unclosed tag"), "a <unclosed tag");
        assert_eq!(plain("<!-- note -->Kept"), "Kept");
    }

    #[test]
    fn decodes_entities_once_and_leaves_unknown_ones() {
        assert_eq!(plain("Fish &amp; chips"), "Fish & chips");
        assert_eq!(plain("&#8220;Quoted&#x201D;"), "“Quoted”");
        assert_eq!(plain("Caf&eacute; &hellip;"), "Café …");
        assert_eq!(plain("&notathing; stays"), "&notathing; stays");
        assert_eq!(plain("AT&T"), "AT&T");
        // Decoded "<" is text: it is not stripped as a tag afterwards.
        assert_eq!(plain("&lt;script&gt;alert(1)"), "<script>alert(1)");
        assert_eq!(plain("&#0;&#xD800;x"), "\u{FFFD}\u{FFFD}x");
    }

    #[test]
    fn removes_controls_and_bidi_and_collapses_space() {
        assert_eq!(
            plain("  Safe\u{202E}gnp.exe\u{202C}  \t\n here\u{0007} "),
            "Safegnp.exe here"
        );
        assert_eq!(plain("a\u{2067}b\u{2069}c\u{200B}d"), "abcd");
        assert_eq!(plain("x&#x202E;y&rlm;z"), "xyz");
        assert_eq!(
            plain("non&nbsp;breaking\u{00A0}space"),
            "non breaking space"
        );
        // Joiners hold emoji together, so they stay.
        assert_eq!(plain("👩\u{200D}💻"), "👩\u{200D}💻");
    }

    #[test]
    fn trims_only_the_feeds_own_name() {
        assert_eq!(
            headline("Storm closes schools - BBC News", "BBC News"),
            "Storm closes schools"
        );
        assert_eq!(
            headline("Storm closes schools | bbc news", "BBC News"),
            "Storm closes schools"
        );
        assert_eq!(
            headline("Ten things - a list", "Lists"),
            "Ten things - a list"
        );
        assert_eq!(headline("BBC News", "BBC News"), "BBC News");
        assert_eq!(headline(" - BBC News", "BBC News"), "- BBC News");
        assert_eq!(headline("Anything - BBC", ""), "Anything - BBC");
    }

    #[test]
    fn caps_at_a_word_with_an_ellipsis() {
        let long = "word ".repeat(100);
        let capped = headline(&long, "");
        assert!(capped.chars().count() <= MAX_HEADLINE);
        assert!(capped.ends_with("word…"), "{capped}");
        let unbroken = "x".repeat(400);
        let capped = headline(&unbroken, "");
        assert_eq!(capped.chars().count(), MAX_HEADLINE);
        assert!(capped.ends_with('…'));
        assert_eq!(headline("short", ""), "short");
        let exact = "é".repeat(MAX_HEADLINE);
        assert_eq!(headline(&exact, ""), exact);
    }
}
