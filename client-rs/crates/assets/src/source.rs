//! Where asset files come from: the disk on desktop, HTTP in a browser.
//!
//! Layouts:
//! - **Development:** the manifest is `client-rs/assets/manifest.json` and the files are under
//!   `client/src`, found by walking up from the working directory or the executable.
//! - **Packaged (desktop and web):** `assets/manifest.json` and the files under `assets/`, beside
//!   the executable or the web page.

use crate::{Error, Result};

#[derive(Debug, Clone)]
pub struct AssetSource {
    manifest: String,
    files: String,
}

impl AssetSource {
    pub fn new(manifest: impl Into<String>, files: impl Into<String>) -> Self {
        let files = files.into();
        let files = if files.is_empty() || files.ends_with('/') { files } else { files + "/" };
        Self { manifest: manifest.into(), files }
    }

    pub async fn manifest(&self) -> Result<crate::Manifest> {
        crate::Manifest::parse(&fetch(&self.manifest).await?)
    }

    /// A file named by the manifest (a path relative to `client/src`).
    pub async fn load(&self, path: &str) -> Result<Vec<u8>> {
        fetch(&format!("{}{}", self.files, path)).await
    }

    pub fn describe(&self) -> String {
        format!("manifest {}, files {}", self.manifest, self.files)
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use super::*;
    use std::path::{Path, PathBuf};

    impl AssetSource {
        /// The packaged layout beside the executable, or the development layout in the repository.
        pub fn locate() -> Result<Self> {
            if let Ok(exe) = std::env::current_exe() {
                if let Some(dir) = exe.parent() {
                    let packaged = dir.join("assets");
                    if packaged.join("manifest.json").is_file() {
                        return Ok(Self::from_paths(&packaged.join("manifest.json"), &packaged));
                    }
                }
            }
            let starts = [std::env::current_dir().ok(), std::env::current_exe().ok()];
            for start in starts.into_iter().flatten() {
                for dir in start.ancestors() {
                    let manifest = dir.join("client-rs").join("assets").join("manifest.json");
                    let src = dir.join("client").join("src");
                    if manifest.is_file() && src.is_dir() {
                        return Ok(Self::from_paths(&manifest, &src));
                    }
                }
            }
            Err(Error::NotFound(
                "no assets/manifest.json beside the executable, and not inside the repository".into(),
            ))
        }

        fn from_paths(manifest: &Path, files: &Path) -> Self {
            let slash = |p: &Path| p.to_string_lossy().replace('\\', "/");
            Self::new(slash(manifest), slash(files))
        }
    }

    pub async fn fetch(path: &str) -> Result<Vec<u8>> {
        std::fs::read(PathBuf::from(path))
            .map_err(|e| Error::Io { path: path.to_owned(), message: e.to_string() })
    }
}

#[cfg(target_arch = "wasm32")]
mod imp {
    use super::*;
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    impl AssetSource {
        /// `assets/` beside the page.
        pub fn locate() -> Result<Self> {
            Ok(Self::new("assets/manifest.json", "assets/"))
        }
    }

    pub async fn fetch(url: &str) -> Result<Vec<u8>> {
        let err = |e: wasm_bindgen::JsValue| Error::Io { path: url.to_owned(), message: format!("{e:?}") };
        let window = web_sys::window().ok_or_else(|| Error::NotFound("no window".into()))?;
        let response: web_sys::Response = JsFuture::from(window.fetch_with_str(url))
            .await
            .map_err(err)?
            .dyn_into()
            .map_err(err)?;
        if !response.ok() {
            return Err(Error::Io { path: url.to_owned(), message: format!("HTTP {}", response.status()) });
        }
        let buffer = JsFuture::from(response.array_buffer().map_err(err)?).await.map_err(err)?;
        Ok(js_sys::Uint8Array::new(&buffer).to_vec())
    }
}

use imp::fetch;
