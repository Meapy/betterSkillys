//! `client-rs/CHANGELOG.md`: what changed in each version, for players.
//!
//! ```text
//! ## 1.1.0 (2026-10-15)
//! - A change, in plain words.
//! - Another one, which may go on
//!   over more than one line.
//! ```
//!
//! Anything before the first `## ` heading (a title, a note for whoever edits it) is ignored.

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    pub version: String,
    /// YYYY-MM-DD, or empty.
    pub date: String,
    pub changes: Vec<String>,
}

/// Every version in the file, in the file's order (newest first).
pub fn parse(text: &str) -> Vec<Entry> {
    let mut out: Vec<Entry> = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if let Some(head) = trimmed.strip_prefix("## ") {
            let (version, date) = match head.split_once('(') {
                Some((v, d)) => (v.trim(), d.trim_end_matches(')').trim()),
                None => (head.trim(), ""),
            };
            out.push(Entry { version: version.to_owned(), date: date.to_owned(), changes: Vec::new() });
            continue;
        }
        let Some(entry) = out.last_mut() else { continue };
        if let Some(change) = trimmed.strip_prefix("- ").or_else(|| trimmed.strip_prefix("* ")) {
            entry.changes.push(change.trim().to_owned());
        } else if !trimmed.is_empty() && line.starts_with(char::is_whitespace) {
            // A change carried on to the next line.
            if let Some(last) = entry.changes.last_mut() {
                last.push(' ');
                last.push_str(trimmed);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    #[test]
    fn parses_versions_and_wrapped_lines() {
        let text = "# Changelog\n\nIntro.\n\n## 1.1.0 (2026-10-15)\n- One.\n- Two goes\n  on here.\n\n## 1.0.0\n- First.\n";
        let e = super::parse(text);
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].version.as_str(), e[0].date.as_str()), ("1.1.0", "2026-10-15"));
        assert_eq!(e[0].changes, vec!["One.", "Two goes on here."]);
        assert_eq!((e[1].version.as_str(), e[1].date.as_str(), e[1].changes.len()), ("1.0.0", "", 1));
    }
}
