//! Opening a web address from an untrusted source (a feed's link) or from
//! the settings window (Help), safely: checked, made plain, and handed to
//! the desktop's opener as one argument, never to a shell. The daemon's
//! news action and the window's Help item both come through here.

/// A link from a feed, checked and made plain before it goes near another
/// program. Feeds are written by strangers, so the link is untrusted:
///
/// - it must be an absolute `http://` or `https://` address with a host;
/// - it may not hold whitespace, control characters or invisible
///   formatting characters (which could hide where it really goes);
/// - characters RFC 3986 does not allow in an address (`"`, `<`, `>`,
///   `\`, `^`, `` ` ``, `{`, `|`, `}`) are percent-encoded, as a browser
///   would, so no opener has anything to interpret.
///
/// Returns the address to open, or why it was refused.
pub fn checked_link(link: &str) -> Result<String, &'static str> {
    const MAX: usize = 4096;
    if link.len() > MAX {
        return Err("is too long");
    }
    let scheme = |s: &str| {
        link.get(..s.len())
            .is_some_and(|p| p.eq_ignore_ascii_case(s))
            .then(|| &link[s.len()..])
    };
    let Some(rest) = scheme("https://").or_else(|| scheme("http://")) else {
        return Err("is not an http or https address");
    };
    if rest.is_empty() || rest.starts_with(['/', '?', '#']) {
        return Err("has no host");
    }
    let hidden = |c: char| {
        matches!(c,
            '\u{00ad}' | '\u{061c}' | '\u{180e}' | '\u{200b}'..='\u{200f}'
            | '\u{202a}'..='\u{202e}' | '\u{2060}'..='\u{206f}' | '\u{feff}')
    };
    let mut out = String::with_capacity(link.len());
    for c in link.chars() {
        if c.is_control() || c.is_whitespace() || hidden(c) {
            return Err("holds spaces, control or invisible characters");
        }
        match c {
            '"' | '<' | '>' | '\\' | '^' | '`' | '{' | '|' | '}' => {
                out.push_str(&format!("%{:02X}", c as u32));
            }
            c => out.push(c),
        }
    }
    Ok(out)
}

/// The command that opens a checked web address in the default browser,
/// on `os` (as `std::env::consts::OS` names it). The address is one
/// argument, never read by a shell. On Windows `cmd /C start` would run
/// `&`, `|` and `^` in it as commands, so it goes to the URL handler
/// directly.
pub fn link_opener(url: &str, os: &str) -> Vec<String> {
    let argv: &[&str] = match os {
        "macos" => &["open"],
        "windows" => &["rundll32", "url.dll,FileProtocolHandler"],
        _ => &["xdg-open"],
    };
    argv.iter()
        .map(|s| s.to_string())
        .chain([url.to_string()])
        .collect()
}

/// Opens a web address in the default browser, from a program with no
/// systemd unit of its own to escape (the settings window), if
/// [`checked_link`] lets it through. Returns once the opener has started;
/// it is reaped on its own thread.
pub fn open(link: &str) -> Result<(), String> {
    let url = checked_link(link).map_err(|why| format!("The address {why}."))?;
    let argv = link_opener(&url, std::env::consts::OS);
    let mut child = std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|e| format!("The browser could not be opened: {e}."))?;
    let _ = std::thread::Builder::new()
        .name("opened".into())
        .spawn(move || child.wait());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_links_pass_unchanged() {
        for link in [
            "https://news.example/story?id=7&src=rss#top",
            "http://news.example:8080/a/b_(c)/d;e=f",
            "HTTPS://news.example/%7Euser/caf%C3%A9",
            "https://news.example/café",
        ] {
            assert_eq!(checked_link(link).as_deref(), Ok(link));
        }
    }

    #[test]
    fn hostile_links_are_refused_or_defused() {
        for link in [
            "file:///etc/passwd",
            "javascript:alert(1)",
            "ftp://news.example/x",
            "-h",
            "--help",
            "https://",
            "https:///etc",
            "http:news.example",
            " https://news.example/",
            "https://news.example/a b",
            "https://news.example/\nrm x",
            "https://news.example/\r\n",
            "https://news.example/\t",
            "https://news.example/\u{0}",
            "https://news.example/\u{1b}[2J",
            "https://news.example/\u{85}",
            "https://news.example/\u{a0}",
            "https://news.example/\u{2028}",
            "https://news.example/\u{202e}gpj.exe",
            "https://news.example/\u{200b}",
            "https://news.example/\u{feff}",
            "httpś://news.example/",
        ] {
            assert!(checked_link(link).is_err(), "{link:?} was let through");
        }
        let long = format!("https://news.example/{}", "a".repeat(5000));
        assert!(checked_link(&long).is_err());
        // What a shell or cmd would act on is encoded away; what an
        // address may hold (`&`, `;`, `$`, `'`, parentheses) stays, since
        // nothing here reads it as a command.
        assert_eq!(
            checked_link("https://news.example/?a=1&b=2|calc^x\"y\"<z>`w`{v}\\").as_deref(),
            Ok("https://news.example/?a=1&b=2%7Ccalc%5Ex%22y%22%3Cz%3E%60w%60%7Bv%7D%5C")
        );
        let kept = "https://news.example/$(touch_x);$HOME'q'&n=1";
        assert_eq!(
            checked_link(kept).as_deref(),
            Ok(kept),
            "kept, and only ever one argument"
        );
    }

    #[test]
    fn a_link_is_one_argument_on_every_system() {
        let url = checked_link("https://news.example/?a=1&b=2|x^y").unwrap();
        assert_eq!(link_opener(&url, "linux"), ["xdg-open", url.as_str()]);
        assert_eq!(link_opener(&url, "macos"), ["open", url.as_str()]);
        // Not `cmd /C start`: cmd would run what follows `&` or `|`.
        assert_eq!(
            link_opener(&url, "windows"),
            ["rundll32", "url.dll,FileProtocolHandler", url.as_str()]
        );
    }
}
