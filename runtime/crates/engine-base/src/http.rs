//! HTTP client construction shared by every Engine crate.
//!
//! reqwest is built with `rustls-no-provider`, so a rustls crypto provider must
//! be installed before the first client exists. Building clients through this
//! module guarantees that, in production and in tests alike.

use std::sync::Once;

/// Installs the ring crypto provider as the process default, once.
pub fn ensure_crypto_provider() {
    static INSTALL: Once = Once::new();
    INSTALL.call_once(|| {
        // Err means another component already installed a default; keep it.
        let _ = rustls::crypto::ring::default_provider().install_default();
    });
}

pub fn client_builder() -> reqwest::ClientBuilder {
    ensure_crypto_provider();
    reqwest::Client::builder()
}

pub fn client() -> reqwest::Client {
    ensure_crypto_provider();
    reqwest::Client::new()
}

#[cfg(test)]
mod tests {
    #[test]
    fn clients_build_with_the_ring_provider() {
        let _ = super::client();
        assert!(super::client_builder().build().is_ok());
        assert!(rustls::crypto::CryptoProvider::get_default().is_some());
    }
}
