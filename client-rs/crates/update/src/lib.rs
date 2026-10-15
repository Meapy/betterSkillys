//! The client's published releases, shared by the client (which checks for and downloads them)
//! and `bsk-publish` (which makes them).
//!
//! A release is published under the account server's `/client/` folder:
//!
//! - `<platform>/latest.json`: a [`Release`]: the version, every file of the game with its
//!   SHA-256 and size, and the changelog.
//! - `<platform>/latest.json.sig`: the Ed25519 signature of those exact bytes, in hex. The client
//!   has the public key built in and ignores a release whose signature doesn't match, so nobody
//!   between the server and a player can hand them a different game.
//! - `blobs/<sha256>.gz`: each file's content, gzipped, named by the SHA-256 of the content
//!   itself (not of the gzip). A client downloads only the files it doesn't already have.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

pub mod changelog;
pub use changelog::Entry;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Release {
    pub version: String,
    /// The commit it was built from (short hash).
    pub commit: String,
    /// The day it was published, YYYY-MM-DD.
    pub date: String,
    /// "windows" (the only one published for now).
    pub platform: String,
    /// Every file of the game, relative to the folder the executable is in.
    pub files: Vec<FileEntry>,
    /// Every version's changes, newest first.
    pub changelog: Vec<Entry>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FileEntry {
    /// Forward slashes, relative to the executable's folder.
    pub path: String,
    /// SHA-256 of the content, lowercase hex.
    pub sha256: String,
    pub size: u64,
    /// The size of its gzipped blob, to show download progress.
    pub gz_size: u64,
}

impl FileEntry {
    /// The blob's path under the published folder.
    pub fn blob(&self) -> String {
        format!("blobs/{}.gz", self.sha256)
    }
}

/// The release list's path under the published folder.
pub fn latest_path(platform: &str) -> String {
    format!("{platform}/latest.json")
}

/// SHA-256 of `bytes`, lowercase hex.
pub fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    hex(&sha2::Sha256::digest(bytes))
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub fn unhex(text: &str) -> Option<Vec<u8>> {
    let text = text.trim();
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len()).step_by(2).map(|i| u8::from_str_radix(text.get(i..i + 2)?, 16).ok()).collect()
}

/// Check `json` against its signature (hex) with the public key (hex), then read it.
pub fn verify(json: &[u8], signature_hex: &str, public_key_hex: &str) -> Result<Release, String> {
    use ed25519_dalek::{Signature, Verifier, VerifyingKey};
    let key: [u8; 32] = unhex(public_key_hex).and_then(|k| k.try_into().ok()).ok_or("bad public key")?;
    let key = VerifyingKey::from_bytes(&key).map_err(|e| e.to_string())?;
    let sig: [u8; 64] = unhex(signature_hex).and_then(|s| s.try_into().ok()).ok_or("bad signature")?;
    key.verify(json, &Signature::from_bytes(&sig)).map_err(|_| "the release's signature doesn't match".to_owned())?;
    serde_json::from_slice(json).map_err(|e| e.to_string())
}

/// Sign `json` with the private key's 32-byte seed; returns the signature in hex.
pub fn sign(json: &[u8], seed: &[u8; 32]) -> String {
    use ed25519_dalek::Signer;
    hex(&ed25519_dalek::SigningKey::from_bytes(seed).sign(json).to_bytes())
}

/// The public key (hex) for a private key's seed.
pub fn public_key(seed: &[u8; 32]) -> String {
    hex(ed25519_dalek::SigningKey::from_bytes(seed).verifying_key().as_bytes())
}

/// Compare "1.2.10" with "1.2.9" as numbers, part by part (a missing part is 0).
pub fn compare_versions(a: &str, b: &str) -> Ordering {
    let parts = |v: &str| v.trim().split('.').map(|p| p.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>();
    let (a, b) = (parts(a), parts(b));
    for i in 0..a.len().max(b.len()) {
        match a.get(i).unwrap_or(&0).cmp(b.get(i).unwrap_or(&0)) {
            Ordering::Equal => {}
            other => return other,
        }
    }
    Ordering::Equal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions() {
        assert_eq!(compare_versions("1.2.10", "1.2.9"), Ordering::Greater);
        assert_eq!(compare_versions("1.2", "1.2.0"), Ordering::Equal);
        assert_eq!(compare_versions("0.9.9", "1.0.0"), Ordering::Less);
    }

    #[test]
    fn signed_release_round_trip() {
        let seed = [7u8; 32];
        let release = Release {
            version: "1.1.0".into(),
            commit: "abc1234".into(),
            date: "2026-10-15".into(),
            platform: "windows".into(),
            files: vec![FileEntry { path: "assets/a.png".into(), sha256: sha256_hex(b"a"), size: 1, gz_size: 21 }],
            changelog: Vec::new(),
        };
        let json = serde_json::to_vec(&release).unwrap();
        let sig = sign(&json, &seed);
        assert_eq!(verify(&json, &sig, &public_key(&seed)).unwrap(), release);
        // One byte changed, or another key: refused.
        let mut tampered = json.clone();
        tampered[10] ^= 1;
        assert!(verify(&tampered, &sig, &public_key(&seed)).is_err());
        assert!(verify(&json, &sig, &public_key(&[8u8; 32])).is_err());
    }

    #[test]
    fn hashes() {
        assert_eq!(sha256_hex(b"abc"), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
        assert_eq!(unhex("00ff10"), Some(vec![0, 255, 16]));
    }
}
