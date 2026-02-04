//! TLS configuration for gRPC servers
//!
//! Provides configuration options for enabling TLS on SDK and cluster gRPC servers.

use std::path::PathBuf;

/// TLS configuration for gRPC servers
#[derive(Debug, Clone, Default)]
pub struct GrpcTlsConfig {
    /// Enable TLS for SDK gRPC server
    pub sdk_enabled: bool,
    /// Enable TLS for cluster gRPC server
    pub cluster_enabled: bool,
    /// Path to the server certificate file (PEM format)
    pub cert_path: Option<PathBuf>,
    /// Path to the server private key file (PEM format)
    pub key_path: Option<PathBuf>,
    /// Path to the CA certificate for client verification (mTLS)
    pub ca_cert_path: Option<PathBuf>,
    /// Enable mutual TLS (require client certificates)
    pub mtls_enabled: bool,
    /// Protocols to support (e.g., ["h2", "http/1.1"])
    pub alpn_protocols: Vec<String>,
}

impl GrpcTlsConfig {
    /// Check if TLS is properly configured
    pub fn is_configured(&self) -> bool {
        self.cert_path.is_some() && self.key_path.is_some()
    }

    /// Check if SDK TLS should be used
    pub fn should_use_sdk_tls(&self) -> bool {
        self.sdk_enabled && self.is_configured()
    }

    /// Check if cluster TLS should be used
    pub fn should_use_cluster_tls(&self) -> bool {
        self.cluster_enabled && self.is_configured()
    }

    /// Check if mTLS is properly configured
    pub fn is_mtls_configured(&self) -> bool {
        self.mtls_enabled && self.is_configured() && self.ca_cert_path.is_some()
    }

    /// Load certificate from file
    pub async fn load_cert(&self) -> anyhow::Result<Vec<u8>> {
        match &self.cert_path {
            Some(path) => {
                let contents = tokio::fs::read(path).await?;
                Ok(contents)
            }
            None => anyhow::bail!("Certificate path not configured"),
        }
    }

    /// Load private key from file
    pub async fn load_key(&self) -> anyhow::Result<Vec<u8>> {
        match &self.key_path {
            Some(path) => {
                let contents = tokio::fs::read(path).await?;
                Ok(contents)
            }
            None => anyhow::bail!("Private key path not configured"),
        }
    }

    /// Load CA certificate from file
    pub async fn load_ca_cert(&self) -> anyhow::Result<Vec<u8>> {
        match &self.ca_cert_path {
            Some(path) => {
                let contents = tokio::fs::read(path).await?;
                Ok(contents)
            }
            None => anyhow::bail!("CA certificate path not configured"),
        }
    }

    /// Create tonic ServerTlsConfig
    pub async fn create_server_tls_config(
        &self,
    ) -> anyhow::Result<tonic::transport::ServerTlsConfig> {
        let cert = self.load_cert().await?;
        let key = self.load_key().await?;

        let identity = tonic::transport::Identity::from_pem(cert, key);
        let mut tls_config = tonic::transport::ServerTlsConfig::new().identity(identity);

        // Configure mTLS if enabled
        if self.is_mtls_configured() {
            let ca_cert = self.load_ca_cert().await?;
            let client_ca = tonic::transport::Certificate::from_pem(ca_cert);
            tls_config = tls_config.client_ca_root(client_ca);
        }

        Ok(tls_config)
    }
}

/// Result of TLS configuration validation
#[derive(Debug)]
pub struct TlsValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

impl TlsValidationResult {
    pub fn new() -> Self {
        Self {
            valid: true,
            errors: Vec::new(),
            warnings: Vec::new(),
        }
    }

    pub fn add_error(&mut self, error: &str) {
        self.valid = false;
        self.errors.push(error.to_string());
    }

    pub fn add_warning(&mut self, warning: &str) {
        self.warnings.push(warning.to_string());
    }
}

impl Default for TlsValidationResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate TLS configuration
pub fn validate_tls_config(config: &GrpcTlsConfig) -> TlsValidationResult {
    let mut result = TlsValidationResult::new();

    // If TLS is enabled, certificate and key must be configured
    if config.sdk_enabled || config.cluster_enabled {
        if config.cert_path.is_none() {
            result.add_error("TLS is enabled but certificate path is not configured");
        }
        if config.key_path.is_none() {
            result.add_error("TLS is enabled but private key path is not configured");
        }

        // Check if files exist
        if let Some(ref path) = config.cert_path {
            if !path.exists() {
                result.add_error(&format!("Certificate file not found: {:?}", path));
            }
        }
        if let Some(ref path) = config.key_path {
            if !path.exists() {
                result.add_error(&format!("Private key file not found: {:?}", path));
            }
        }
    }

    // If mTLS is enabled, CA cert must be configured
    if config.mtls_enabled {
        if !config.sdk_enabled && !config.cluster_enabled {
            result.add_warning("mTLS is enabled but TLS is not enabled for any server");
        }
        if config.ca_cert_path.is_none() {
            result.add_error("mTLS is enabled but CA certificate path is not configured");
        }
        if let Some(ref path) = config.ca_cert_path {
            if !path.exists() {
                result.add_error(&format!("CA certificate file not found: {:?}", path));
            }
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GrpcTlsConfig::default();
        assert!(!config.sdk_enabled);
        assert!(!config.cluster_enabled);
        assert!(!config.is_configured());
    }

    #[test]
    fn test_is_configured() {
        let mut config = GrpcTlsConfig::default();
        assert!(!config.is_configured());

        config.cert_path = Some(PathBuf::from("/path/to/cert.pem"));
        assert!(!config.is_configured());

        config.key_path = Some(PathBuf::from("/path/to/key.pem"));
        assert!(config.is_configured());
    }

    #[test]
    fn test_should_use_tls() {
        let mut config = GrpcTlsConfig::default();
        config.cert_path = Some(PathBuf::from("/path/to/cert.pem"));
        config.key_path = Some(PathBuf::from("/path/to/key.pem"));

        assert!(!config.should_use_sdk_tls());
        assert!(!config.should_use_cluster_tls());

        config.sdk_enabled = true;
        assert!(config.should_use_sdk_tls());
        assert!(!config.should_use_cluster_tls());

        config.cluster_enabled = true;
        assert!(config.should_use_sdk_tls());
        assert!(config.should_use_cluster_tls());
    }

    #[test]
    fn test_validation_disabled() {
        let config = GrpcTlsConfig::default();
        let result = validate_tls_config(&config);
        assert!(result.valid);
        assert!(result.errors.is_empty());
    }

    #[test]
    fn test_validation_enabled_without_paths() {
        let mut config = GrpcTlsConfig::default();
        config.sdk_enabled = true;

        let result = validate_tls_config(&config);
        assert!(!result.valid);
        assert!(result.errors.len() >= 2);
    }
}
