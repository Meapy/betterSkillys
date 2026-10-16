//! TLS for the desktop client: the account server over https and the game over wss, both through
//! the site's Caddy and gateway, as the browser client reaches them. One rustls configuration
//! serves both (and the updater): the usual public roots, plus the certificates in the PEM file
//! named by `BSK_CA`, for a test stack with a certificate of its own.

use std::sync::{Arc, OnceLock};

pub fn config() -> Arc<rustls::ClientConfig> {
    static CONFIG: OnceLock<Arc<rustls::ClientConfig>> = OnceLock::new();
    CONFIG.get_or_init(build).clone()
}

/// A ureq agent builder that uses [`config`].
pub fn agent_builder() -> ureq::AgentBuilder {
    ureq::AgentBuilder::new().tls_config(config())
}

fn build() -> Arc<rustls::ClientConfig> {
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    if let Some(path) = std::env::var_os("BSK_CA") {
        use rustls::pki_types::pem::PemObject;
        match rustls::pki_types::CertificateDer::pem_file_iter(&path) {
            Ok(certs) => {
                for cert in certs {
                    match cert.map_err(|e| e.to_string()).and_then(|c| roots.add(c).map_err(|e| e.to_string())) {
                        Ok(()) => log::info!("tls: trusting a certificate from {}", path.to_string_lossy()),
                        Err(e) => log::warn!("tls: {}: {e}", path.to_string_lossy()),
                    }
                }
            }
            Err(e) => log::warn!("tls: {}: {e}", path.to_string_lossy()),
        }
    }
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .expect("TLS versions")
        .with_root_certificates(roots)
        .with_no_client_auth();
    Arc::new(config)
}
