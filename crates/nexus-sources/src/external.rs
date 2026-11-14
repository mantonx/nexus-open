//! The external source protocol: what a program prints for the daemon to
//! publish. The full description, with examples, is
//! docs/EXTERNAL_SOURCES.md.
//!
//! Each line is one JSON object whose keys are signal names:
//!
//! ```text
//! {"disk.used": {"value": 71.5, "unit": "percent"}, "backup.state": "ok"}
//! ```
//!
//! A value is a number, a string, or `{"value": number, "unit": name}`.
//! Running the program is the daemon's job; this only reads lines.

use nexus_core::signal::{Unit, Value};
use serde_json::Value as Json;

/// Longest signal name accepted, so a runaway program cannot fill the
/// signal list with megabyte-long keys.
pub const MAX_NAME: usize = 64;

/// The readings on one line. A blank line has none. Any mistake rejects
/// the whole line, so a line is published entirely or not at all.
pub fn parse_line(line: &str) -> Result<Vec<(String, Value)>, String> {
    let line = line.trim();
    if line.is_empty() {
        return Ok(Vec::new());
    }
    let Json::Object(map) =
        serde_json::from_str::<Json>(line).map_err(|e| format!("not JSON: {e}"))?
    else {
        return Err("not a JSON object; write {\"signal.name\": value}".into());
    };
    map.into_iter()
        .map(|(name, json)| {
            check_name(&name)?;
            let value = reading(&json).map_err(|m| format!("\"{name}\": {m}"))?;
            Ok((name, value))
        })
        .collect()
}

fn check_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > MAX_NAME || name.chars().any(char::is_whitespace) {
        return Err(format!(
            "\"{name}\" is not a signal name: 1 to {MAX_NAME} characters, no spaces"
        ));
    }
    Ok(())
}

fn reading(json: &Json) -> Result<Value, String> {
    match json {
        Json::String(text) => Ok(Value::Text(text.clone())),
        Json::Number(_) => Ok(Value::Number(number(json)?, Unit::None)),
        Json::Object(fields) => {
            let n = number(fields.get("value").ok_or("needs a \"value\"")?)?;
            let unit = match fields.get("unit") {
                None => Unit::None,
                Some(Json::String(name)) => Unit::from_name(name).ok_or_else(|| {
                    format!(
                        "unknown unit \"{name}\"; use none, celsius, percent or bytes-per-second"
                    )
                })?,
                Some(_) => return Err("\"unit\" must be a string".into()),
            };
            if let Some(extra) = fields.keys().find(|k| *k != "value" && *k != "unit") {
                return Err(format!("unknown field \"{extra}\"; only value and unit"));
            }
            Ok(Value::Number(n, unit))
        }
        _ => Err("must be a number, a string, or {\"value\": …, \"unit\": …}".into()),
    }
}

fn number(json: &Json) -> Result<f64, String> {
    json.as_f64()
        .filter(|n| n.is_finite())
        .ok_or_else(|| "\"value\" must be a number".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_numbers_text_and_units() {
        let got = parse_line(
            r#"{"disk.used": {"value": 71.5, "unit": "percent"}, "backup.state": "ok", "n": 3}"#,
        )
        .unwrap();
        assert_eq!(
            got,
            [
                ("backup.state".into(), Value::Text("ok".into())),
                ("disk.used".into(), Value::Number(71.5, Unit::Percent)),
                ("n".into(), Value::Number(3.0, Unit::None)),
            ]
        );
        assert_eq!(parse_line("  \n").unwrap(), []);
    }

    #[test]
    fn mistakes_reject_the_whole_line_and_say_why() {
        for (line, says) in [
            ("disk.used 71", "not JSON"),
            ("[1, 2]", "not a JSON object"),
            (r#"{"a b": 1}"#, "no spaces"),
            (r#"{"x": true}"#, "must be a number"),
            (r#"{"ok": 1, "x": {"value": 1, "unit": "kelvin"}}"#, "kelvin"),
            (r#"{"x": {"unit": "percent"}}"#, "needs a \"value\""),
            (r#"{"x": {"value": 1, "units": "percent"}}"#, "unknown field \"units\""),
        ] {
            let e = parse_line(line).unwrap_err();
            assert!(e.contains(says), "{line}: {e}");
        }
    }
}
