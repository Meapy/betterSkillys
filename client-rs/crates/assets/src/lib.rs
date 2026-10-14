//! The client's asset list and file access.
//!
//! The asset list is `client-rs/assets/manifest.json`, generated from the AIR client's sources by
//! `tools/gen_rs_manifest.py`. Its paths are relative to `client/src`, so the Rust client reads
//! the same sprite sheets and XML as the AIR client.

mod manifest;
mod source;

pub use manifest::*;
pub use source::AssetSource;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("{path}: {message}")]
    Io { path: String, message: String },
    #[error("{path}: {source}")]
    Json { path: String, source: serde_json::Error },
    #[error("{path}: {source}")]
    Image { path: String, source: image::ImageError },
    #[error("no assets found: {0}")]
    NotFound(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Decode a PNG or JPEG into straight (not premultiplied) RGBA.
pub fn decode_image(path: &str, bytes: &[u8]) -> Result<image::RgbaImage> {
    image::load_from_memory(bytes)
        .map(|img| img.into_rgba8())
        .map_err(|source| Error::Image { path: path.to_owned(), source })
}
