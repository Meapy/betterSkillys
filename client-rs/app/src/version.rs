//! This build's version (`client-rs/Cargo.toml`), the commit it was built from
//! (`BSK_COMMIT`, set by `build_client_rs.py`), and the changelog built into it.

use bsk_update::Entry;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// "v1.1.0 (88b1b6d)", or "v1.1.0" without a commit.
pub fn label() -> String {
    match option_env!("BSK_COMMIT").filter(|c| !c.is_empty()) {
        Some(commit) => format!("v{VERSION} ({commit})"),
        None => format!("v{VERSION}"),
    }
}

/// `client-rs/CHANGELOG.md` as it was when this was built, newest first.
pub fn changelog() -> Vec<Entry> {
    bsk_update::changelog::parse(include_str!("../../CHANGELOG.md"))
}

#[cfg(test)]
mod tests {
    /// The version being built has its own section in the changelog.
    #[test]
    fn changelog_has_this_version() {
        assert!(super::changelog().iter().any(|e| e.version == super::VERSION), "CHANGELOG.md has no ## {} section", super::VERSION);
    }
}
