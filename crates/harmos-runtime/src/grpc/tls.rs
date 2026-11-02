/* ***
 *
 * Harmos
 *
 * SPDX-License-Identifier: Apache-2.0
 *
 * ***
 */

// Validated Certificate Replacement For New TLS Handshakes.

/* === Modules === */

/* === Re-Exports === */

/* === Imports === */

// Standard Library Imports
use std::sync::{Arc, RwLock};

// Third Party Imports
use rustls::{
    crypto::CryptoProvider,
    pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject},
    server::{ClientHello, ResolvesServerCert},
    sign::CertifiedKey,
};

// Local Imports
use crate::{Error, Result};

/* === Constants === */

/* === Macros === */

/* === Type Aliases === */

/* === Structs === */

#[derive(Clone, Debug)]
pub struct ServerCertificate {
    resolver: Arc<Resolver>,
    config: Arc<rustls::ServerConfig>,
    provider: Arc<CryptoProvider>,
}

#[derive(Debug)]
struct Resolver(RwLock<Arc<CertifiedKey>>);

/* === Enums === */

/* === Traits === */

/* === Implementations === */

impl ServerCertificate {
    pub fn from_pem(certificate: &[u8], private_key: &[u8]) -> Result<Self> {
        let provider = CryptoProvider::get_default().cloned().unwrap_or_else(|| Arc::new(rustls::crypto::ring::default_provider()));
        let key = certified_key(certificate, private_key, &provider)?;
        let resolver = Arc::new(Resolver(RwLock::new(Arc::new(key))));
        let mut config = rustls::ServerConfig::builder_with_provider(provider.clone())
            .with_safe_default_protocol_versions()
            .map_err(failure)?
            .with_no_client_auth()
            .with_cert_resolver(resolver.clone());
        config.alpn_protocols = vec![b"h2".to_vec()];
        // Every New Connection Authenticates With The Current Certificate.
        config.session_storage = Arc::new(rustls::server::NoServerSessionStorage {});
        config.send_tls13_tickets = 0;
        Ok(Self {
            resolver,
            config: Arc::new(config),
            provider,
        })
    }

    pub fn reload(&self, certificate: &[u8], private_key: &[u8]) -> Result<()> {
        let key = certified_key(certificate, private_key, &self.provider)?;
        *self.resolver.0.write().expect("gRPC certificate lock is healthy") = Arc::new(key);
        Ok(())
    }

    pub(super) fn acceptor(&self) -> tokio_rustls::TlsAcceptor {
        tokio_rustls::TlsAcceptor::from(self.config.clone())
    }
}

impl ResolvesServerCert for Resolver {
    fn resolve(&self, _: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        Some(self.0.read().expect("gRPC certificate lock is healthy").clone())
    }
}

/* === Public Functions === */

/* === Private Functions === */

fn certified_key(certificate: &[u8], private_key: &[u8], provider: &CryptoProvider) -> Result<CertifiedKey> {
    let chain = CertificateDer::pem_slice_iter(certificate).collect::<std::result::Result<Vec<_>, _>>().map_err(failure)?;
    if chain.is_empty() {
        return Err(Error::InvalidInput {
            field: "gRPC certificate",
            reason: "certificate chain is empty",
        });
    }
    let key = PrivateKeyDer::from_pem_slice(private_key).map_err(failure)?;
    CertifiedKey::from_der(chain, key, provider).map_err(failure)
}

fn failure(source: impl std::error::Error + Send + Sync + 'static) -> Error {
    Error::TaskFailed {
        domain: "gRPC certificate",
        source: Arc::new(source),
    }
}

/* === Tests === */
