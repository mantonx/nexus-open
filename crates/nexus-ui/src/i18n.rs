//! Where shown text will be translated. Choices are stored and compared
//! by their English text, which therefore never changes; only what the
//! window shows passes through `shown`, and a picked choice comes back as
//! its position, not its shown text. No translations exist yet, so a
//! string is shown as it is.

/// The text to show for `text`.
pub fn shown(text: &str) -> String {
    text.to_string()
}
