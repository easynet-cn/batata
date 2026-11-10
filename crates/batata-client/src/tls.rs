//! Shared TLS settings for the SDK's gRPC and HTTP transports.
//!
//! `TlsOptions` is the normalized form of the TLS fields on [`ClientConfig`](crate::ClientConfig),
//! [`GrpcClientConfig`](crate::GrpcClientConfig) and [`HttpClientConfig`](crate::HttpClientConfig).
//! The same options are applied to every transport so a private CA works for
//! both the gRPC channel and the HTTP login/admin calls.
//!
//! `enabled` controls the gRPC channel: when true, the SDK gRPC port is
//! reached over `https://` with a rustls client configuration. HTTP URLs keep
//! whatever scheme the caller put in `server_addrs` (`https://...` for a
//! TLS-terminating proxy), but custom CA / client identity PEMs are added to
//! the reqwest client whenever they are configured.

use crate::error::{ClientError, Result};

/// Normalized TLS settings shared by the gRPC and HTTP transports.
///
/// Paths are PEM files. `domain` overrides the gRPC SNI / certificate name,
/// which is needed when the server address is an IP literal but the server
/// certificate is issued for a DNS name.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TlsOptions {
    /// Whether the gRPC channel uses TLS (`https://`).
    pub enabled: bool,
    /// Path to a PEM CA certificate used to verify the server.
    /// If omitted, the platform's native root certificates are used.
    pub ca_cert_path: Option<String>,
    /// Path to the client certificate PEM (mTLS).
    pub client_cert_path: Option<String>,
    /// Path to the client private key PEM (mTLS).
    pub client_key_path: Option<String>,
    /// Domain name used for gRPC SNI and certificate verification.
    pub domain: Option<String>,
}

impl TlsOptions {
    /// Enable TLS with an optional custom CA certificate path.
    ///
    /// Passing `None` uses the platform's native root certificates.
    pub fn enabled(ca_cert_path: Option<&str>) -> Self {
        Self {
            enabled: true,
            ca_cert_path: ca_cert_path.map(str::to_string),
            ..Self::default()
        }
    }

    /// Enable mutual TLS with a CA certificate and a client identity.
    pub fn mtls(ca_cert_path: Option<&str>, client_cert_path: &str, client_key_path: &str) -> Self {
        Self {
            enabled: true,
            ca_cert_path: ca_cert_path.map(str::to_string),
            client_cert_path: Some(client_cert_path.to_string()),
            client_key_path: Some(client_key_path.to_string()),
            ..Self::default()
        }
    }

    /// Override the domain name used for gRPC SNI / certificate verification.
    #[must_use]
    pub fn with_domain(mut self, domain: &str) -> Self {
        self.domain = Some(domain.to_string());
        self
    }

    /// URI scheme used for the gRPC endpoint.
    pub fn scheme(&self) -> &'static str {
        if self.enabled { "https" } else { "http" }
    }

    /// Whether a client certificate/key pair is configured.
    pub fn is_mtls(&self) -> bool {
        self.client_cert_path.is_some() && self.client_key_path.is_some()
    }

    /// Reject incomplete or inconsistent TLS settings before any connection
    /// is attempted, so a half-configured mTLS setup never silently degrades
    /// to server-only TLS or plaintext.
    pub fn validate(&self) -> Result<()> {
        match (&self.client_cert_path, &self.client_key_path) {
            (Some(_), None) => {
                return Err(ClientError::Other(anyhow::anyhow!(
                    "TLS client certificate configured without a client key"
                )));
            }
            (None, Some(_)) => {
                return Err(ClientError::Other(anyhow::anyhow!(
                    "TLS client key configured without a client certificate"
                )));
            }
            _ => {}
        }
        if let Some(domain) = &self.domain
            && domain.trim().is_empty()
        {
            return Err(ClientError::Other(anyhow::anyhow!(
                "TLS domain override must not be empty"
            )));
        }
        Ok(())
    }

    /// Build the tonic client TLS configuration, or `None` for plaintext.
    ///
    /// File I/O errors are returned instead of silently falling back to an
    /// unauthenticated channel.
    pub async fn to_tonic(&self) -> Result<Option<tonic::transport::ClientTlsConfig>> {
        if !self.enabled {
            return Ok(None);
        }
        self.validate()?;

        let mut tls = tonic::transport::ClientTlsConfig::new();
        tls = match &self.ca_cert_path {
            Some(path) => {
                let pem = read_pem(path, "TLS CA certificate").await?;
                tls.ca_certificate(tonic::transport::Certificate::from_pem(pem))
            }
            None => tls.with_enabled_roots(),
        };

        if let (Some(cert_path), Some(key_path)) = (&self.client_cert_path, &self.client_key_path) {
            let cert = read_pem(cert_path, "TLS client certificate").await?;
            let key = read_pem(key_path, "TLS client key").await?;
            tls = tls.identity(tonic::transport::Identity::from_pem(cert, key));
        }

        if let Some(domain) = &self.domain {
            tls = tls.domain_name(domain.clone());
        }

        Ok(Some(tls))
    }

    /// Add the configured CA certificate and client identity to a reqwest
    /// builder. Called for login, admin API and address-server requests.
    pub fn apply_to_reqwest(
        &self,
        mut builder: reqwest::ClientBuilder,
    ) -> anyhow::Result<reqwest::ClientBuilder> {
        self.validate()?;

        if let Some(path) = &self.ca_cert_path {
            let pem = std::fs::read(path)
                .map_err(|e| anyhow::anyhow!("failed to read TLS CA certificate {path}: {e}"))?;
            // `Certificate::from_pem` only stores the bytes on the rustls
            // backend; `from_pem_bundle` actually parses them (and supports
            // CA bundles), so a malformed file fails here instead of at the
            // first request.
            let certs = reqwest::Certificate::from_pem_bundle(&pem)?;
            if certs.is_empty() {
                anyhow::bail!("TLS CA certificate {path} contains no PEM certificates");
            }
            for cert in certs {
                builder = builder.add_root_certificate(cert);
            }
        }

        if let (Some(cert_path), Some(key_path)) = (&self.client_cert_path, &self.client_key_path) {
            let mut cert = std::fs::read(cert_path).map_err(|e| {
                anyhow::anyhow!("failed to read TLS client certificate {cert_path}: {e}")
            })?;
            let key = std::fs::read(key_path)
                .map_err(|e| anyhow::anyhow!("failed to read TLS client key {key_path}: {e}"))?;
            cert.extend_from_slice(&key);
            builder = builder.identity(reqwest::Identity::from_pem(&cert)?);
        }

        Ok(builder)
    }
}

async fn read_pem(path: &str, what: &str) -> Result<Vec<u8>> {
    tokio::fs::read(path)
        .await
        .map_err(|e| ClientError::Other(anyhow::anyhow!("failed to read {what} {path}: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn self_signed_pem() -> String {
        let certified = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
            .expect("test certificate must be generated");
        certified.cert.pem()
    }

    #[test]
    fn test_default_is_plaintext() {
        let tls = TlsOptions::default();
        assert_eq!(tls.scheme(), "http");
        assert!(!tls.is_mtls());
        tls.validate().unwrap();
    }

    #[test]
    fn test_enabled_uses_https() {
        let tls = TlsOptions::enabled(None);
        assert!(tls.enabled);
        assert_eq!(tls.scheme(), "https");
        assert!(tls.ca_cert_path.is_none());
    }

    #[test]
    fn test_mtls_builder_sets_all_paths() {
        let tls = TlsOptions::mtls(Some("ca.pem"), "client.pem", "client.key")
            .with_domain("batata.example.com");
        assert_eq!(tls.ca_cert_path.as_deref(), Some("ca.pem"));
        assert_eq!(tls.client_cert_path.as_deref(), Some("client.pem"));
        assert_eq!(tls.client_key_path.as_deref(), Some("client.key"));
        assert_eq!(tls.domain.as_deref(), Some("batata.example.com"));
        assert!(tls.is_mtls());
        tls.validate().unwrap();
    }

    #[test]
    fn test_validate_rejects_partial_identity() {
        let cert_only = TlsOptions {
            enabled: true,
            client_cert_path: Some("client.pem".into()),
            ..TlsOptions::default()
        };
        let key_only = TlsOptions {
            enabled: true,
            client_key_path: Some("client.key".into()),
            ..TlsOptions::default()
        };
        assert!(cert_only.validate().is_err());
        assert!(key_only.validate().is_err());
    }

    #[test]
    fn test_validate_rejects_empty_domain() {
        let tls = TlsOptions::enabled(None).with_domain("  ");
        assert!(tls.validate().is_err());
    }

    #[tokio::test]
    async fn test_to_tonic_none_when_disabled() {
        let tls = TlsOptions {
            ca_cert_path: Some("unused.pem".into()),
            ..TlsOptions::default()
        };
        assert!(tls.to_tonic().await.unwrap().is_none());
    }

    #[tokio::test]
    async fn test_to_tonic_with_custom_ca() {
        let dir = tempfile::tempdir().unwrap();
        let ca_path = dir.path().join("ca.pem");
        std::fs::write(&ca_path, self_signed_pem()).unwrap();

        let tls = TlsOptions::enabled(Some(ca_path.to_str().unwrap()));
        assert!(tls.to_tonic().await.unwrap().is_some());
    }

    #[tokio::test]
    async fn test_to_tonic_missing_ca_is_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.pem");
        let tls = TlsOptions::enabled(Some(missing.to_str().unwrap()));
        let err = tls.to_tonic().await.unwrap_err();
        assert!(err.to_string().contains("TLS CA certificate"));
    }

    #[tokio::test]
    async fn test_to_tonic_native_roots_branch() {
        // `ClientTlsConfig` has no getters, but its Debug representation
        // exposes which branch was taken. This kills a mutant that silently
        // drops native roots when no custom CA is configured.
        let cfg = TlsOptions::enabled(None).to_tonic().await.unwrap().unwrap();
        let debug = format!("{cfg:?}");
        assert!(debug.contains("with_native_roots: true"));
        assert!(debug.contains("certs: []"));
    }

    #[tokio::test]
    async fn test_to_tonic_custom_ca_domain_and_identity_branches() {
        let dir = tempfile::tempdir().unwrap();
        let ca_path = dir.path().join("ca.pem");
        std::fs::write(&ca_path, self_signed_pem()).unwrap();
        let cert_path = dir.path().join("client.pem");
        std::fs::write(&cert_path, self_signed_pem()).unwrap();
        let key_path = dir.path().join("client.key");
        std::fs::write(
            &key_path,
            "-----BEGIN PRIVATE KEY-----\nZm9v\n-----END PRIVATE KEY-----\n",
        )
        .unwrap();

        let tls = TlsOptions::mtls(
            Some(ca_path.to_str().unwrap()),
            cert_path.to_str().unwrap(),
            key_path.to_str().unwrap(),
        )
        .with_domain("batata.example.com");
        let cfg = tls.to_tonic().await.unwrap().unwrap();
        let debug = format!("{cfg:?}");
        assert!(debug.contains("certs: ["), "CA must be attached: {debug}");
        assert!(
            debug.contains("with_native_roots: false"),
            "custom CA must not be mixed with native roots: {debug}"
        );
        assert!(
            debug.contains("identity: Some("),
            "mTLS identity must be attached: {debug}"
        );
        assert!(
            debug.contains("domain: Some(\"batata.example.com\")"),
            "SNI override must be attached: {debug}"
        );
    }

    #[tokio::test]
    async fn test_to_tonic_rejects_partial_identity() {
        let tls = TlsOptions {
            enabled: true,
            client_cert_path: Some("client.pem".into()),
            ..TlsOptions::default()
        };
        assert!(tls.to_tonic().await.is_err());
    }

    #[test]
    fn test_apply_to_reqwest_with_valid_ca() {
        let dir = tempfile::tempdir().unwrap();
        let ca_path = dir.path().join("ca.pem");
        std::fs::write(&ca_path, self_signed_pem()).unwrap();

        let tls = TlsOptions {
            ca_cert_path: Some(ca_path.to_str().unwrap().to_string()),
            ..TlsOptions::default()
        };
        let builder = tls.apply_to_reqwest(reqwest::Client::builder()).unwrap();
        assert!(builder.build().is_ok());
    }

    #[test]
    fn test_apply_to_reqwest_rejects_invalid_ca_pem() {
        let dir = tempfile::tempdir().unwrap();
        let ca_path = dir.path().join("bad.pem");
        std::fs::write(&ca_path, "not a pem certificate").unwrap();

        let tls = TlsOptions {
            ca_cert_path: Some(ca_path.to_str().unwrap().to_string()),
            ..TlsOptions::default()
        };
        assert!(tls.apply_to_reqwest(reqwest::Client::builder()).is_err());
    }

    #[test]
    fn test_apply_to_reqwest_rejects_empty_ca_bundle() {
        let dir = tempfile::tempdir().unwrap();
        let ca_path = dir.path().join("empty.pem");
        std::fs::write(&ca_path, "   \n").unwrap();

        let tls = TlsOptions {
            ca_cert_path: Some(ca_path.to_str().unwrap().to_string()),
            ..TlsOptions::default()
        };
        assert!(tls.apply_to_reqwest(reqwest::Client::builder()).is_err());
    }

    #[test]
    fn test_apply_to_reqwest_rejects_partial_identity() {
        let cert_only = TlsOptions {
            client_cert_path: Some("client.pem".into()),
            ..TlsOptions::default()
        };
        let key_only = TlsOptions {
            client_key_path: Some("client.key".into()),
            ..TlsOptions::default()
        };
        assert!(
            cert_only
                .apply_to_reqwest(reqwest::Client::builder())
                .is_err()
        );
        assert!(
            key_only
                .apply_to_reqwest(reqwest::Client::builder())
                .is_err()
        );
    }

    #[test]
    fn test_apply_to_reqwest_rejects_invalid_identity_pem() {
        let dir = tempfile::tempdir().unwrap();
        let cert_path = dir.path().join("client.pem");
        let key_path = dir.path().join("client.key");
        std::fs::write(&cert_path, "not a certificate").unwrap();
        std::fs::write(&key_path, "not a key").unwrap();

        let tls = TlsOptions {
            client_cert_path: Some(cert_path.to_str().unwrap().to_string()),
            client_key_path: Some(key_path.to_str().unwrap().to_string()),
            ..TlsOptions::default()
        };
        assert!(tls.apply_to_reqwest(reqwest::Client::builder()).is_err());
    }

    #[test]
    fn test_apply_to_reqwest_rejects_missing_ca_file() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("missing.pem");
        let tls = TlsOptions {
            ca_cert_path: Some(missing.to_str().unwrap().to_string()),
            ..TlsOptions::default()
        };
        let err = tls
            .apply_to_reqwest(reqwest::Client::builder())
            .unwrap_err();
        assert!(err.to_string().contains("TLS CA certificate"));
    }
}
