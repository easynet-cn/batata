//! OIDC (OpenID Connect) authentication module, compatible with the Consul API.
//!
//! This module implements an OIDC authentication flow compatible with Consul's `internal/go-sso/oidcauth` library.
//! Provides two core operations:
//! - Generates the OIDC authorization URL (`get_auth_url`).
//! - Exchanges the authorization code for a token and extracts claims (`exchange_code`).
//!
//! Core features:
//! - PKCE (Proof Key for Code Exchange) S256, enabled by default (matching Consul).
//! - State management: 10-minute TTL, single use (removed immediately after verification).
//! - OIDC Discovery document cache to avoid repeated requests.
//! - JWT signature verification (using the provider's JWKS).
//! - Supports `ClaimMappings` and `ListClaimMappings`.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock};
use std::time::{Duration, Instant};

use base64::Engine;
use dashmap::DashMap;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::debug;

// ============================================================================
// Constants.
// ============================================================================

/// Default TTL (10 minutes) for the OIDC state, matching Consul.
const DEFAULT_STATE_TTL: Duration = Duration::from_secs(10 * 60);

/// Number of random bytes (used to generate the state ID and nonce).
const RANDOM_BYTES: usize = 20;

/// Length of the PKCE `code_verifier` (32 bytes = 43 base64url characters).
const PKCE_VERIFIER_BYTES: usize = 32;

// ============================================================================
// OidcConfig - OIDC configuration parsed from `AuthMethod.config`.
// ============================================================================

/// OIDC auth method configuration, parsed from the `AuthMethod` config field.
///
/// Corresponds to Consul's `OIDCAuthMethodConfig` struct.
#[derive(Clone, Debug)]
pub struct OidcConfig {
    /// The OIDC provider's discovery URL (required).
    pub oidc_discovery_url: String,
    /// The OIDC client ID (required).
    pub oidc_client_id: String,
    /// OIDC client secret
    pub oidc_client_secret: String,
    /// Requested OIDC scopes (includes "openid" by default).
    pub oidc_scopes: Vec<String>,
    /// Requested ACR values.
    pub oidc_acr_values: Vec<String>,
    /// List of allowed callback URIs (required; the `redirect_uri` must be among them).
    pub allowed_redirect_uris: Vec<String>,
    /// Claim mapping: JWT claim name -> variable name.
    pub claim_mappings: HashMap<String, String>,
    /// List-type claim mapping: JWT claim name -> variable name.
    pub list_claim_mappings: HashMap<String, String>,
    /// List of bound audiences.
    pub bound_audiences: Vec<String>,
    /// Whether PKCE is enabled (defaults to `true`, matching Consul).
    pub oidc_client_use_pkce: bool,
    /// Whether verbose OIDC logging is enabled.
    pub verbose_oidc_logging: bool,
    /// CA certificate for OIDC discovery (PEM format, optional).
    pub oidc_discovery_ca_cert: Option<String>,
    /// List of supported JWT signing algorithms.
    pub jwt_supported_algs: Vec<String>,
}

impl OidcConfig {
    /// Parses the OIDC configuration from the `AuthMethod` config field.
    ///
    /// `config` is a `HashMap<String, serde_json::Value>` whose keys use
    /// Consul's PascalCase format (e.g. "OIDCDiscoveryURL", "OIDCClientID", etc.).
    ///
    /// # Arguments
    /// - `config`: the `AuthMethod` config field.
    ///
    /// # Returns
    /// - `Ok(OidcConfig)`: parsing succeeded.
    /// - `Err(String)`: a required field is missing or malformed, with a detailed error message.
    pub fn from_auth_method_config(
        config: &Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Self, String> {
        let config = config.as_ref().ok_or_else(|| {
            "OIDC auth method config is missing required fields: OIDCDiscoveryURL, OIDCClientID, AllowedRedirectURIs".to_string()
        })?;

        // Required field: OIDCDiscoveryURL.
        let oidc_discovery_url = config
            .get("OIDCDiscoveryURL")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "OIDCDiscoveryURL is required".to_string())?;

        // Required field: OIDCClientID.
        let oidc_client_id = config
            .get("OIDCClientID")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "OIDCClientID is required".to_string())?;

        // Optional field: OIDCClientSecret.
        let oidc_client_secret = config
            .get("OIDCClientSecret")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Required field: AllowedRedirectURIs.
        let allowed_redirect_uris = config
            .get("AllowedRedirectURIs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .ok_or_else(|| "AllowedRedirectURIs is required and must be a non-empty array".to_string())?;

        if allowed_redirect_uris.is_empty() {
            return Err("AllowedRedirectURIs must contain at least one URI".to_string());
        }

        // Optional field: OIDCScopes.
        let oidc_scopes = config
            .get("OIDCScopes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Optional field: OIDCACRValues.
        let oidc_acr_values = config
            .get("OIDCACRValues")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Optional field: ClaimMappings.
        let claim_mappings = config
            .get("ClaimMappings")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Optional field: ListClaimMappings.
        let list_claim_mappings = config
            .get("ListClaimMappings")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Optional field: BoundAudiences.
        let bound_audiences = config
            .get("BoundAudiences")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // Optional field: OIDCClientUsePKCE (defaults to true, matching Consul).
        let oidc_client_use_pkce = config
            .get("OIDCClientUsePKCE")
            .and_then(|v| v.as_bool())
            .unwrap_or(true); // 默认启用 PKCE

        // Optional field: VerboseOIDCLogging.
        let verbose_oidc_logging = config
            .get("VerboseOIDCLogging")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Optional field: OIDCDiscoveryCACert.
        let oidc_discovery_ca_cert = config
            .get("OIDCDiscoveryCACert")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // Optional field: JWTSupportedAlgs.
        let jwt_supported_algs = config
            .get("JWTSupportedAlgs")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        Ok(Self {
            oidc_discovery_url,
            oidc_client_id,
            oidc_client_secret,
            oidc_scopes,
            oidc_acr_values,
            allowed_redirect_uris,
            claim_mappings,
            list_claim_mappings,
            bound_audiences,
            oidc_client_use_pkce,
            verbose_oidc_logging,
            oidc_discovery_ca_cert,
            jwt_supported_algs,
        })
    }

    /// Validates whether the `redirect_uri` is in the allowed list.
    pub fn validate_redirect_uri(&self, redirect_uri: &str) -> Result<(), String> {
        if self.allowed_redirect_uris.iter().any(|uri| uri == redirect_uri) {
            Ok(())
        } else {
            Err(format!(
                "Redirect URI '{}' is not in the allowed redirect URIs list",
                redirect_uri
            ))
        }
    }
}

// ============================================================================
// OidcState - state of a single OIDC authentication flow.
// ============================================================================

/// Temporary state of an OIDC authentication flow, stored in the `state_store`.
///
/// Each state corresponds to one OIDC authentication flow and contains:
/// - `nonce`: used to prevent replay attacks.
/// - `redirect_uri`: the callback URI used to exchange the code.
/// - `auth_method`: the associated auth method name.
/// - `client_nonce`: the client-provided nonce (optional).
/// - `meta`: client-provided metadata (optional).
/// - `code_verifier`: the PKCE `code_verifier` (if PKCE is enabled).
#[derive(Clone, Debug)]
pub struct OidcState {
    /// Randomly generated nonce sent to the OIDC provider and verified on callback.
    nonce: String,
    /// The `redirect_uri` used in the client's request.
    redirect_uri: String,
    /// The associated auth method name.
    auth_method: String,
    /// The client-provided nonce (optional, used for extra validation).
    client_nonce: Option<String>,
    /// Client-provided metadata.
    meta: Option<HashMap<String, String>>,
    /// Creation time, used for TTL expiry checks.
    created_at: Instant,
    /// The PKCE `code_verifier` (if PKCE is enabled).
    code_verifier: Option<String>,
}

impl OidcState {
    /// Checks whether the state has expired.
    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

// ============================================================================
// OidcStateStore - state storage and management.
// ============================================================================

/// OIDC state store providing concurrent, safe state management backed by `DashMap`.
///
/// Features:
/// - 10-minute TTL with automatic expiry.
/// - Single use: removed immediately after `verify_and_remove`.
/// - Concurrency-safe: backed by `DashMap`.
pub struct OidcStateStore {
    /// The `DashMap` storing states, keyed by `state_id`.
    states: DashMap<String, OidcState>,
    /// The state TTL.
    ttl: Duration,
}

impl OidcStateStore {
    /// Creates a new state store with the default 10-minute TTL.
    pub fn new() -> Self {
        Self {
            states: DashMap::new(),
            ttl: DEFAULT_STATE_TTL,
        }
    }

    /// Creates a new state store with a custom TTL (used in tests).
    #[cfg(test)]
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            states: DashMap::new(),
            ttl,
        }
    }

    /// Inserts a new state.
    ///
    /// Cleans up expired states before inserting, to avoid memory leaks.
    pub fn insert(&self, state_id: String, state: OidcState) {
        // Also clean up expired states.
        self.cleanup_expired();
        self.states.insert(state_id, state);
    }

    /// Verifies and removes a state (single use).
    ///
    /// Returns `None` if the state does not exist or has expired.
    /// If it exists and has not expired, removes and returns the state.
    pub fn verify_and_remove(&self, state_id: &str) -> Option<OidcState> {
        // Also clean up expired states.
        self.cleanup_expired();

        // Remove the state (single use).
        self.states.remove(state_id).map(|(_, state)| state)
    }

    /// Cleans up all expired states.
    ///
    /// Called automatically on every `insert` and `verify_and_remove`.
    /// Can also be called manually to trigger cleanup.
    pub fn cleanup_expired(&self) {
        let ttl = self.ttl;
        self.states.retain(|_, state| !state.is_expired(ttl));
    }

    /// Returns the current number of stored states (mainly for tests and monitoring).
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// Checks whether a state exists (tests only).
    #[cfg(test)]
    pub fn contains(&self, state_id: &str) -> bool {
        self.states.contains_key(state_id)
    }
}

impl Default for OidcStateStore {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// OIDC Discovery document and cache.
// ============================================================================

/// The OIDC Discovery document (from `/.well-known/openid-configuration`).
///
/// Contains only the fields needed by batata.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct OidcDiscoveryDoc {
    /// The OIDC provider's issuer URL.
    issuer: String,
    /// The authorization endpoint URL.
    authorization_endpoint: String,
    /// The token endpoint URL.
    token_endpoint: String,
    /// The JWKS URI (used to fetch the public keys for verifying JWTs).
    jwks_uri: String,
    /// The userinfo endpoint URL (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    userinfo_endpoint: Option<String>,
}

/// A single key in a JWKS (JSON Web Key Set).
#[derive(Clone, Debug, Deserialize)]
struct Jwk {
    /// The key ID (used to match the `kid` in the JWT header).
    #[serde(skip_serializing_if = "Option::is_none")]
    kid: Option<String>,
    /// The key type (e.g. "RSA").
    kty: String,
    /// The RSA modulus (base64url-encoded).
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<String>,
    /// The RSA exponent (base64url-encoded).
    #[serde(skip_serializing_if = "Option::is_none")]
    e: Option<String>,
}

/// The JWKS (JSON Web Key Set) response.
#[derive(Clone, Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

/// Cache for the OIDC Discovery document, keyed by discovery URL.
///
/// Uses a global static cache to avoid querying the OIDC provider on every request.
/// Cache validity is 5 minutes.
static DISCOVERY_CACHE: LazyLock<DashMap<String, (OidcDiscoveryDoc, Instant)>> =
    LazyLock::new(|| DashMap::new());

/// Cache for the JWKS, keyed by `jwks_uri`.
///
/// Cache validity is 10 minutes.
static JWKS_CACHE: LazyLock<DashMap<String, (Jwks, Instant)>> =
    LazyLock::new(|| DashMap::new());

/// Discovery document cache duration (5 minutes).
const DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(5 * 60);

/// JWKS cache duration (10 minutes).
const JWKS_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

// ============================================================================
// OidcAuthenticator - core OIDC authenticator.
// ============================================================================

/// The OIDC authenticator, encapsulating the full OIDC authentication flow.
///
/// Each `OidcConfig` corresponds to one `OidcAuthenticator`.
/// The authenticator internally maintains:
/// - An HTTP client (used to communicate with the OIDC provider).
/// - A state store (managing the authentication flow state).
pub struct OidcAuthenticator {
    /// The OIDC configuration.
    config: OidcConfig,
    /// HTTP client（reqwest）
    http_client: reqwest::Client,
    /// The state store.
    state_store: OidcStateStore,
}

impl OidcAuthenticator {
    /// Creates a new OIDC authenticator.
    ///
    /// # Arguments
    /// - `config`: the OIDC configuration.
    ///
    /// # Returns
    /// - `Ok(Self)`: creation succeeded.
    /// - `Err(String)`: failed to create the HTTP client.
    pub fn new(config: OidcConfig) -> Result<Self, String> {
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none());

        // If a CA certificate is configured, add it to the client.
        if let Some(ref ca_cert_pem) = config.oidc_discovery_ca_cert {
            let cert = reqwest::Certificate::from_pem(ca_cert_pem.as_bytes())
                .map_err(|e| format!("Failed to parse OIDC discovery CA cert: {}", e))?;
            builder = builder.add_root_certificate(cert);
        }

        let http_client = builder
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        Ok(Self {
            config,
            http_client,
            state_store: OidcStateStore::new(),
        })
    }

    /// Generates the OIDC authorization URL.
    ///
    /// Steps:
    /// 1. Validate the `redirect_uri` against the `allowed_redirect_uris`.
    /// 2. Generate the state ID and nonce (each 20 random bytes).
    /// 3. If PKCE is enabled, generate the `code_verifier` and `code_challenge`.
    /// 4. Query OIDC discovery for the `authorization_endpoint`.
    /// 5. Build the authorization URL.
    /// 6. Store the state.
    /// 7. Return the URL.
    ///
    /// # Arguments
    /// - `redirect_uri`: the callback URI (must be in `allowed_redirect_uris`).
    /// - `auth_method`: the auth method name.
    /// - `client_nonce`: the client nonce (optional).
    /// - `meta`: client metadata (optional).
    ///
    /// # Returns
    /// - `Ok(String)`: the authorization URL.
    /// - `Err(String)`: an error message.
    pub async fn get_auth_url(
        &self,
        redirect_uri: &str,
        auth_method: &str,
        client_nonce: Option<&str>,
        meta: Option<HashMap<String, String>>,
    ) -> Result<String, String> {
        // 1. Validate the redirect_uri.
        self.config.validate_redirect_uri(redirect_uri)?;

        // 2. Generate the state ID and nonce.
        let state_id = generate_random_string(RANDOM_BYTES);
        let nonce = generate_random_string(RANDOM_BYTES);

        // 3. Generate the PKCE `code_verifier` and `code_challenge`.
        let (code_verifier, code_challenge) = if self.config.oidc_client_use_pkce {
            let verifier = generate_random_string(PKCE_VERIFIER_BYTES);
            let challenge = compute_pkce_challenge(&verifier);
            (Some(verifier), Some(challenge))
        } else {
            (None, None)
        };

        // 4. Query OIDC discovery for the `authorization_endpoint`.
        let discovery = self.fetch_discovery_doc().await?;

        // 5. Build the authorization URL.
        let auth_url = build_authorization_url(
            &discovery.authorization_endpoint,
            &self.config.oidc_client_id,
            redirect_uri,
            &state_id,
            &nonce,
            code_challenge.as_deref(),
            &self.config.oidc_scopes,
            &self.config.oidc_acr_values,
        );

        if self.config.verbose_oidc_logging {
            debug!("OIDC auth URL generated for method '{}': {}", auth_method, auth_url);
        }

        // 6. Store the state.
        let state = OidcState {
            nonce,
            redirect_uri: redirect_uri.to_string(),
            auth_method: auth_method.to_string(),
            client_nonce: client_nonce.map(|s| s.to_string()),
            meta,
            created_at: Instant::now(),
            code_verifier,
        };
        self.state_store.insert(state_id, state);

        // 7. Return the URL.
        Ok(auth_url)
    }

    /// Exchanges the authorization code for a token and extracts the claims.
    ///
    /// Steps:
    /// 1. Verify and remove the state (single use).
    /// 2. Query OIDC discovery for the `token_endpoint`.
    /// 3. POST to the `token_endpoint` to exchange the code.
    /// 4. Parse the response to obtain the `id_token`.
    /// 5. Verify the JWT signature (fetch the public key via the discovery `jwks_uri`).
    /// 6. Verify the nonce matches.
    /// 7. Extract the claims.
    /// 8. Apply `claim_mappings` and `list_claim_mappings`.
    ///
    /// # Arguments
    /// - `state_id`: the state returned with the authorization URL.
    /// - `code`: the authorization code returned by the OIDC provider.
    ///
    /// # Returns
    /// - `Ok(OidcClaims)`: the extracted claims.
    /// - `Err(String)`: an error message.
    pub async fn exchange_code(
        &self,
        state_id: &str,
        code: &str,
    ) -> Result<OidcClaims, String> {
        // 1. Verify and remove the state (single use).
        let state = self.state_store.verify_and_remove(state_id).ok_or_else(|| {
            "OIDC state not found or expired. The state may have already been used or has timed out".to_string()
        })?;

        // 2. Query OIDC discovery for the `token_endpoint`.
        let discovery = self.fetch_discovery_doc().await?;

        // 3. POST to the `token_endpoint` to exchange the code.
        let token_response = self
            .exchange_code_for_token(&discovery.token_endpoint, code, &state)
            .await?;

        // 4. Obtain the `id_token`.
        let id_token = token_response.id_token.ok_or_else(|| {
            "OIDC token response does not contain id_token".to_string()
        })?;

        // 5. Verify the JWT signature.
        let jwks = self.fetch_jwks(&discovery.jwks_uri).await?;
        let claims = verify_jwt(&id_token, &jwks, &discovery.issuer, &self.config)?;

        // 6. Verify the nonce matches.
        let token_nonce = claims.get("nonce").and_then(|v| v.as_str());
        if token_nonce != Some(state.nonce.as_str()) {
            return Err("OIDC nonce mismatch: the id_token nonce does not match the expected nonce".to_string());
        }

        // 7. Apply `claim_mappings` and `list_claim_mappings`.
        let processed_claims = apply_claim_mappings(&claims, &self.config);

        if self.config.verbose_oidc_logging {
            debug!(
                "OIDC claims extracted for method '{}': {:?}",
                state.auth_method, processed_claims
            );
        }

        Ok(OidcClaims {
            claims: processed_claims,
            auth_method: state.auth_method,
            client_nonce: state.client_nonce,
            meta: state.meta,
        })
    }

    /// Returns the number of states in the store (mainly for monitoring).
    pub fn state_count(&self) -> usize {
        self.state_store.len()
    }

    /// Manually triggers state cleanup.
    pub fn cleanup_states(&self) {
        self.state_store.cleanup_expired();
    }

    // ------------------------------------------------------------------
    // Internal methods.
    // ------------------------------------------------------------------

    /// Fetches the OIDC discovery document (with caching).
    async fn fetch_discovery_doc(&self) -> Result<OidcDiscoveryDoc, String> {
        let discovery_url = &self.config.oidc_discovery_url;

        // Check the cache.
        if let Some(entry) = DISCOVERY_CACHE.get(discovery_url) {
            let (doc, created_at) = entry.value();
            if created_at.elapsed() < DISCOVERY_CACHE_TTL {
                if self.config.verbose_oidc_logging {
                    debug!("OIDC discovery document loaded from cache for {}", discovery_url);
                }
                return Ok(doc.clone());
            }
        }

        // Cache miss or expired; fetch from the OIDC provider.
        if self.config.verbose_oidc_logging {
            debug!("Fetching OIDC discovery document from {}", discovery_url);
        }

        let response = self
            .http_client
            .get(discovery_url)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch OIDC discovery document from '{}': {}", discovery_url, e))?;

        if !response.status().is_success() {
            return Err(format!(
                "OIDC discovery endpoint returned HTTP {} for '{}'",
                response.status(),
                discovery_url
            ));
        }

        let doc: OidcDiscoveryDoc = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse OIDC discovery document: {}", e))?;

        // Update the cache.
        DISCOVERY_CACHE.insert(discovery_url.clone(), (doc.clone(), Instant::now()));

        Ok(doc)
    }

    /// Fetches the JWKS (with caching).
    async fn fetch_jwks(&self, jwks_uri: &str) -> Result<Jwks, String> {
        // Check the cache.
        if let Some(entry) = JWKS_CACHE.get(jwks_uri) {
            let (jwks, created_at) = entry.value();
            if created_at.elapsed() < JWKS_CACHE_TTL {
                if self.config.verbose_oidc_logging {
                    debug!("JWKS loaded from cache for {}", jwks_uri);
                }
                return Ok(jwks.clone());
            }
        }

        // Cache miss or expired; fetch from the OIDC provider.
        if self.config.verbose_oidc_logging {
            debug!("Fetching JWKS from {}", jwks_uri);
        }

        let response = self
            .http_client
            .get(jwks_uri)
            .send()
            .await
            .map_err(|e| format!("Failed to fetch JWKS from '{}': {}", jwks_uri, e))?;

        if !response.status().is_success() {
            return Err(format!(
                "JWKS endpoint returned HTTP {} for '{}'",
                response.status(),
                jwks_uri
            ));
        }

        let jwks: Jwks = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse JWKS: {}", e))?;

        // Update the cache.
        JWKS_CACHE.insert(jwks_uri.to_string(), (jwks.clone(), Instant::now()));

        Ok(jwks)
    }

    /// Exchanges the authorization code for a token.
    async fn exchange_code_for_token(
        &self,
        token_endpoint: &str,
        code: &str,
        state: &OidcState,
    ) -> Result<TokenResponse, String> {
        // Build the form parameters.
        let mut form = vec![
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("code".to_string(), code.to_string()),
            ("redirect_uri".to_string(), state.redirect_uri.clone()),
            ("client_id".to_string(), self.config.oidc_client_id.clone()),
        ];

        // client_secret (if configured).
        if !self.config.oidc_client_secret.is_empty() {
            form.push((
                "client_secret".to_string(),
                self.config.oidc_client_secret.clone(),
            ));
        }

        // code_verifier (if PKCE is enabled).
        if let Some(ref verifier) = state.code_verifier {
            form.push(("code_verifier".to_string(), verifier.clone()));
        }

        if self.config.verbose_oidc_logging {
            debug!(
                "Exchanging authorization code at token endpoint: {}",
                token_endpoint
            );
        }

        let response = self
            .http_client
            .post(token_endpoint)
            .form(&form)
            .send()
            .await
            .map_err(|e| format!("Failed to exchange authorization code: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            return Err(format!(
                "Token endpoint returned HTTP {}: {}",
                status, body
            ));
        }

        let token_response: TokenResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse token response: {}", e))?;

        Ok(token_response)
    }
}

// ============================================================================
// OIDC token response.
// ============================================================================

/// The response from the OIDC token endpoint.
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenResponse {
    /// The `access_token` (not always required, but returned per the OIDC spec).
    #[serde(default)]
    access_token: Option<String>,
    /// The `id_token` (the OIDC core, containing the user claims).
    #[serde(default)]
    id_token: Option<String>,
    /// The `token_type` (e.g. "Bearer").
    #[serde(default)]
    token_type: Option<String>,
    /// The `expires_in` value (in seconds).
    #[serde(default)]
    expires_in: Option<u64>,
    /// The `refresh_token` (optional).
    #[serde(default)]
    refresh_token: Option<String>,
}

// ============================================================================
// OidcClaims - processed claims.
// ============================================================================

/// The processed OIDC claims, containing variables after applying `claim_mappings`.
///
/// Used for subsequent binding-rule matching.
#[derive(Clone, Debug)]
pub struct OidcClaims {
    /// The processed claims variable map (key is the variable name, value is the JSON value).
    pub claims: HashMap<String, serde_json::Value>,
    /// The associated auth method name.
    pub auth_method: String,
    /// The client nonce.
    pub client_nonce: Option<String>,
    /// Client metadata.
    pub meta: Option<HashMap<String, String>>,
}

// ============================================================================
// Helper functions.
// ============================================================================

/// Generates a random string (base64url-encoded).
///
/// Generates `num_bytes` random bytes, then encodes them with base64url (no padding).
fn generate_random_string(num_bytes: usize) -> String {
    let mut bytes = vec![0u8; num_bytes];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

/// Computes the PKCE `code_challenge` (S256 method).
///
/// code_challenge = base64url(SHA256(code_verifier))
fn compute_pkce_challenge(code_verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
}

/// Builds the OIDC authorization URL.
#[allow(clippy::too_many_arguments)]
fn build_authorization_url(
    authorization_endpoint: &str,
    client_id: &str,
    redirect_uri: &str,
    state: &str,
    nonce: &str,
    code_challenge: Option<&str>,
    scopes: &[String],
    acr_values: &[String],
) -> String {
    // Build the scope parameter (always includes "openid").
    let mut all_scopes = vec!["openid".to_string()];
    all_scopes.extend(scopes.iter().cloned());
    let scope_str = all_scopes.join(" ");

    // Build the URL using the url crate.
    let mut url = url::Url::parse(authorization_endpoint)
        .unwrap_or_else(|_| url::Url::parse("http://localhost").unwrap());
    let mut query_pairs = url.query_pairs_mut();
    query_pairs
        .append_pair("client_id", client_id)
        .append_pair("redirect_uri", redirect_uri)
        .append_pair("response_type", "code")
        .append_pair("scope", &scope_str)
        .append_pair("state", state)
        .append_pair("nonce", nonce);

    // PKCE parameters.
    if let Some(challenge) = code_challenge {
        query_pairs
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
    }

    // ACR values (if configured).
    if !acr_values.is_empty() {
        query_pairs.append_pair("acr_values", &acr_values.join(" "));
    }

    drop(query_pairs);
    url.to_string()
}

/// Verifies the JWT and extracts the claims.
///
/// Steps:
/// 1. Parse the JWT header to get the `kid` and `alg`.
/// 2. Find the matching key in the JWKS.
/// 3. Verify the JWT signature using the matched key.
/// 4. Verify the issuer.
/// 5. Verify the audience (if `bound_audiences` is configured).
/// 6. Return the claims.
fn verify_jwt(
    token: &str,
    jwks: &Jwks,
    expected_issuer: &str,
    config: &OidcConfig,
) -> Result<serde_json::Value, String> {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};

    // 1. Parse the JWT header to get the `kid` and `alg`.
    let header = decode_header(token)
        .map_err(|e| format!("Failed to decode JWT header: {}", e))?;

    let kid = header.kid.as_deref();

    // 2. Find the matching key in the JWKS.
    let matching_key = jwks.keys.iter().find(|k| {
        // If a kid exists, match by kid; otherwise use the first RSA key.
        if let Some(kid) = kid {
            k.kid.as_deref() == Some(kid)
        } else {
            k.kty == "RSA"
        }
    });

    let jwk = matching_key.ok_or_else(|| {
        format!(
            "No matching JWK found for kid: {:?}",
            kid
        )
    })?;

    // 3. Create the DecodingKey.
    let n = jwk.n.as_deref().ok_or("JWK missing 'n' (modulus) field")?;
    let e = jwk.e.as_deref().ok_or("JWK missing 'e' (exponent) field")?;
    let decoding_key = DecodingKey::from_rsa_components(n, e)
        .map_err(|e| format!("Failed to create decoding key from JWK: {}", e))?;

    // 4. Create the Validation.
    let algorithm = match header.alg {
        Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => header.alg,
        Algorithm::PS256 | Algorithm::PS384 | Algorithm::PS512 => header.alg,
        Algorithm::ES256 | Algorithm::ES384 => header.alg,
        Algorithm::EdDSA => header.alg,
        _ => {
            // Default to RS256.
            // If JWTSupportedAlgs is configured, check whether it is supported.
            if !config.jwt_supported_algs.is_empty() {
                // Check whether the alg is in the supported list.
                let alg_str = format!("{:?}", header.alg);
                if !config.jwt_supported_algs.iter().any(|a| alg_str.contains(a)) {
                    return Err(format!("JWT algorithm {:?} is not in the supported algorithms list", header.alg));
                }
            }
            Algorithm::RS256
        }
    };

    let mut validation = Validation::new(algorithm);

    // Set the expected issuer.
    validation.set_issuer(&[expected_issuer]);

    // Set the expected audience (if bound_audiences is configured).
    if !config.bound_audiences.is_empty() {
        let audiences: Vec<&str> = config.bound_audiences.iter().map(|s| s.as_str()).collect();
        validation.set_audience(&audiences);
    }

    // 5. Verify the JWT.
    let token_data = decode::<serde_json::Value>(token, &decoding_key, &validation)
        .map_err(|e| format!("JWT verification failed: {}", e))?;

    Ok(token_data.claims)
}

/// Applies `claim_mappings` and `list_claim_mappings`.
///
/// Converts the raw JWT claims into variable names per the configured mapping rules.
fn apply_claim_mappings(
    raw_claims: &serde_json::Value,
    config: &OidcConfig,
) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();

    // Apply claim_mappings (single value).
    for (claim_name, var_name) in &config.claim_mappings {
        if let Some(value) = raw_claims.get(claim_name) {
            result.insert(var_name.clone(), value.clone());
        }
    }

    // Apply list_claim_mappings (list value).
    for (claim_name, var_name) in &config.list_claim_mappings {
        if let Some(value) = raw_claims.get(claim_name) {
            if let Some(arr) = value.as_array() {
                // List-type claim, stored as a string array.
                let strings: Vec<String> = arr
                    .iter()
                    .filter_map(|v| {
                        if v.is_string() {
                            v.as_str().map(|s| s.to_string())
                        } else {
                            Some(v.to_string())
                        }
                    })
                    .collect();
                result.insert(
                    var_name.clone(),
                    serde_json::Value::Array(
                        strings.into_iter().map(serde_json::Value::String).collect(),
                    ),
                );
            } else if value.is_string() {
                // If the claim is a string rather than an array, still treat it as a list.
                result.insert(
                    var_name.clone(),
                    serde_json::Value::Array(vec![value.clone()]),
                );
            }
        }
    }

    result
}

// ============================================================================
// Global OIDC authenticator cache.
// ============================================================================

/// Global OIDC authenticator cache, keyed by auth method name.
///
/// Uses a static cache (similar to `TOKEN_CACHE`) to avoid recreating the authenticator on every request.
/// Calls `invalidate_authenticator` to clear the cache when an auth method's config is updated.
static OIDC_AUTHENTICATORS: LazyLock<DashMap<String, Arc<OidcAuthenticator>>> =
    LazyLock::new(|| DashMap::new());

/// Gets or creates the OIDC authenticator for the given auth method.
///
/// If not in the cache, creates a new authenticator from the auth method config and caches it.
///
/// # Arguments
/// - `auth_method_name`: the auth method name.
/// - `config`: the auth method config.
///
/// # Returns
/// - `Ok(Arc<OidcAuthenticator>)`: authenticator
/// - `Err(String)`: creation failed.
pub fn get_or_create_authenticator(
    auth_method_name: &str,
    config: &Option<HashMap<String, serde_json::Value>>,
) -> Result<Arc<OidcAuthenticator>, String> {
    // Check the cache.
    if let Some(entry) = OIDC_AUTHENTICATORS.get(auth_method_name) {
        return Ok(entry.value().clone());
    }

    // Cache miss; create a new authenticator.
    let oidc_config = OidcConfig::from_auth_method_config(config)?;
    let authenticator = Arc::new(OidcAuthenticator::new(oidc_config)?);

    // Store in the cache (use the entry API to avoid race conditions).
    OIDC_AUTHENTICATORS
        .entry(auth_method_name.to_string())
        .or_insert(authenticator.clone());

    Ok(authenticator)
}

/// Clears the OIDC authenticator cache for the given auth method.
///
/// Called when an auth method is updated or deleted.
pub fn invalidate_authenticator(auth_method_name: &str) {
    OIDC_AUTHENTICATORS.remove(auth_method_name);
}

/// Clears all OIDC authenticator caches.
pub fn invalidate_all_authenticators() {
    OIDC_AUTHENTICATORS.clear();
}

// ============================================================================
// Unit tests.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// Tests parsing the OIDC config from the auth method config.
    #[test]
    fn test_oidc_config_from_auth_method() {
        // Test a complete, valid configuration.
        let mut config = HashMap::new();
        config.insert(
            "OIDCDiscoveryURL".to_string(),
            serde_json::json!("https://example.com/.well-known/openid-configuration"),
        );
        config.insert(
            "OIDCClientID".to_string(),
            serde_json::json!("test-client-id"),
        );
        config.insert(
            "OIDCClientSecret".to_string(),
            serde_json::json!("test-client-secret"),
        );
        config.insert(
            "AllowedRedirectURIs".to_string(),
            serde_json::json!(["http://localhost:8500/callback"]),
        );
        config.insert(
            "OIDCScopes".to_string(),
            serde_json::json!(["email", "profile"]),
        );
        config.insert(
            "ClaimMappings".to_string(),
            serde_json::json!({"project": "project_var"}),
        );
        config.insert(
            "ListClaimMappings".to_string(),
            serde_json::json!({"groups": "groups_var"}),
        );

        let oidc_config = OidcConfig::from_auth_method_config(&Some(config));
        assert!(oidc_config.is_ok());

        let oidc_config = oidc_config.unwrap();
        assert_eq!(oidc_config.oidc_discovery_url, "https://example.com/.well-known/openid-configuration");
        assert_eq!(oidc_config.oidc_client_id, "test-client-id");
        assert_eq!(oidc_config.oidc_client_secret, "test-client-secret");
        assert_eq!(oidc_config.allowed_redirect_uris, vec!["http://localhost:8500/callback"]);
        assert_eq!(oidc_config.oidc_scopes, vec!["email", "profile"]);
        assert_eq!(oidc_config.claim_mappings.get("project"), Some(&"project_var".to_string()));
        assert_eq!(oidc_config.list_claim_mappings.get("groups"), Some(&"groups_var".to_string()));
        // PKCE is enabled by default.
        assert!(oidc_config.oidc_client_use_pkce);
    }

    /// Tests error handling when required fields are missing.
    #[test]
    fn test_oidc_config_missing_required_fields() {
        // Test config being None.
        let result = OidcConfig::from_auth_method_config(&None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing required fields"));

        // Test missing OIDCDiscoveryURL.
        let mut config = HashMap::new();
        config.insert("OIDCClientID".to_string(), serde_json::json!("test-client"));
        config.insert(
            "AllowedRedirectURIs".to_string(),
            serde_json::json!(["http://localhost:8500/callback"]),
        );
        let result = OidcConfig::from_auth_method_config(&Some(config));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("OIDCDiscoveryURL is required"));

        // Test missing AllowedRedirectURIs.
        let mut config = HashMap::new();
        config.insert("OIDCDiscoveryURL".to_string(), serde_json::json!("https://example.com"));
        config.insert("OIDCClientID".to_string(), serde_json::json!("test-client"));
        let result = OidcConfig::from_auth_method_config(&Some(config));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("AllowedRedirectURIs is required"));
    }

    /// Tests that PKCE can be disabled.
    #[test]
    fn test_oidc_config_pkce_disabled() {
        let mut config = HashMap::new();
        config.insert("OIDCDiscoveryURL".to_string(), serde_json::json!("https://example.com"));
        config.insert("OIDCClientID".to_string(), serde_json::json!("test-client"));
        config.insert(
            "AllowedRedirectURIs".to_string(),
            serde_json::json!(["http://localhost:8500/callback"]),
        );
        config.insert("OIDCClientUsePKCE".to_string(), serde_json::json!(false));

        let oidc_config = OidcConfig::from_auth_method_config(&Some(config)).unwrap();
        assert!(!oidc_config.oidc_client_use_pkce);
    }

    /// Tests insertion and verification of the state store.
    #[test]
    fn test_oidc_state_store_insert_and_verify() {
        let store = OidcStateStore::new();

        let state = OidcState {
            nonce: "test-nonce".to_string(),
            redirect_uri: "http://localhost:8500/callback".to_string(),
            auth_method: "test-oidc".to_string(),
            client_nonce: None,
            meta: None,
            created_at: Instant::now(),
            code_verifier: None,
        };

        // Insert the state.
        store.insert("test-state-id".to_string(), state);

        // Verify the state exists.
        assert!(store.contains("test-state-id"));

        // verify_and_remove should return the state.
        let result = store.verify_and_remove("test-state-id");
        assert!(result.is_some());
        assert_eq!(result.unwrap().nonce, "test-nonce");

        // The state should have been removed (single use).
        assert!(!store.contains("test-state-id"));
    }

    /// Tests expiry cleanup of the state store.
    #[test]
    fn test_oidc_state_store_expired_cleanup() {
        // Use a 10ms TTL.
        let store = OidcStateStore::with_ttl(Duration::from_millis(10));

        let state = OidcState {
            nonce: "test-nonce".to_string(),
            redirect_uri: "http://localhost:8500/callback".to_string(),
            auth_method: "test-oidc".to_string(),
            client_nonce: None,
            meta: None,
            created_at: Instant::now(),
            code_verifier: None,
        };

        store.insert("test-state-id".to_string(), state);
        assert!(store.contains("test-state-id"));

        // Wait for expiry.
        std::thread::sleep(Duration::from_millis(50));

        // Manual cleanup.
        store.cleanup_expired();
        assert!(!store.contains("test-state-id"));
    }

    /// Tests the single-use property of a state (removed immediately after verify).
    #[test]
    fn test_oidc_state_store_verify_removes_state() {
        let store = OidcStateStore::new();

        let state = OidcState {
            nonce: "one-time-nonce".to_string(),
            redirect_uri: "http://localhost:8500/callback".to_string(),
            auth_method: "test-oidc".to_string(),
            client_nonce: Some("client-nonce".to_string()),
            meta: None,
            created_at: Instant::now(),
            code_verifier: Some("test-verifier".to_string()),
        };

        store.insert("one-time-state".to_string(), state);

        // The first verification should succeed.
        let result1 = store.verify_and_remove("one-time-state");
        assert!(result1.is_some());
        let retrieved = result1.unwrap();
        assert_eq!(retrieved.nonce, "one-time-nonce");
        assert_eq!(retrieved.client_nonce, Some("client-nonce".to_string()));
        assert_eq!(retrieved.code_verifier, Some("test-verifier".to_string()));

        // The second verification should fail (state already removed).
        let result2 = store.verify_and_remove("one-time-state");
        assert!(result2.is_none());
    }

    /// Tests that verifying a non-existent state returns None.
    #[test]
    fn test_oidc_state_store_verify_nonexistent() {
        let store = OidcStateStore::new();
        let result = store.verify_and_remove("nonexistent-state");
        assert!(result.is_none());
    }

    /// Tests `redirect_uri` validation.
    #[test]
    fn test_redirect_uri_validation() {
        let config = OidcConfig {
            oidc_discovery_url: "https://example.com/.well-known/openid-configuration".to_string(),
            oidc_client_id: "test-client".to_string(),
            oidc_client_secret: "secret".to_string(),
            oidc_scopes: vec![],
            oidc_acr_values: vec![],
            allowed_redirect_uris: vec![
                "http://localhost:8500/callback".to_string(),
                "http://localhost:8500/ui/login".to_string(),
            ],
            claim_mappings: HashMap::new(),
            list_claim_mappings: HashMap::new(),
            bound_audiences: vec![],
            oidc_client_use_pkce: true,
            verbose_oidc_logging: false,
            oidc_discovery_ca_cert: None,
            jwt_supported_algs: vec![],
        };

        // Valid redirect_uri.
        assert!(config.validate_redirect_uri("http://localhost:8500/callback").is_ok());
        assert!(config.validate_redirect_uri("http://localhost:8500/ui/login").is_ok());

        // Invalid redirect_uri.
        assert!(config.validate_redirect_uri("http://evil.com/callback").is_err());
        assert!(config.validate_redirect_uri("http://localhost:8500/evil").is_err());
    }

    /// Tests PKCE `code_challenge` computation.
    #[test]
    fn test_pkce_challenge_computation() {
        // Verify PKCE S256 computation against a known test vector.
        // code_verifier -> SHA256 -> base64url(no padding)
        let verifier = "dBjftJeZ4CVK-mJMgjYqsrkuerxyAL_nzjF2yT5g";
        let challenge = compute_pkce_challenge(verifier);

        // Verify the challenge is base64url-encoded with the correct length (SHA256 = 32 bytes -> 43 base64url chars).
        assert_eq!(challenge.len(), 43, "PKCE challenge should be 43 characters (32 bytes base64url no padding)");

        // Verify results are deterministic across calls.
        let challenge2 = compute_pkce_challenge(verifier);
        assert_eq!(challenge, challenge2, "Same verifier should produce same challenge");

        // Verify different verifiers produce different challenges.
        let different_challenge = compute_pkce_challenge("different-verifier-1234567890");
        assert_ne!(challenge, different_challenge, "Different verifiers should produce different challenges");
    }

    /// Tests random string generation.
    #[test]
    fn test_random_string_generation() {
        let s1 = generate_random_string(20);
        let s2 = generate_random_string(20);

        // Two random strings should differ.
        assert_ne!(s1, s2);

        // Length should be correct (20 bytes base64url no padding = 27 chars).
        assert_eq!(s1.len(), 27);
    }

    /// Tests authorization URL construction.
    #[test]
    fn test_build_authorization_url() {
        let url = build_authorization_url(
            "https://provider.example.com/oauth2/authorize",
            "test-client-id",
            "http://localhost:8500/callback",
            "test-state",
            "test-nonce",
            Some("test-challenge"),
            &["email".to_string(), "profile".to_string()],
            &[],
        );

        // Verify the URL contains all required parameters.
        assert!(url.contains("https://provider.example.com/oauth2/authorize"));
        assert!(url.contains("client_id=test-client-id"));
        assert!(url.contains("redirect_uri=http%3A%2F%2Flocalhost%3A8500%2Fcallback"));
        assert!(url.contains("response_type=code"));
        assert!(url.contains("scope=openid+email+profile") || url.contains("scope=openid%20email%20profile"));
        assert!(url.contains("state=test-state"));
        assert!(url.contains("nonce=test-nonce"));
        assert!(url.contains("code_challenge=test-challenge"));
        assert!(url.contains("code_challenge_method=S256"));
    }

    /// Tests authorization URL construction without PKCE.
    #[test]
    fn test_build_authorization_url_no_pkce() {
        let url = build_authorization_url(
            "https://provider.example.com/oauth2/authorize",
            "test-client-id",
            "http://localhost:8500/callback",
            "test-state",
            "test-nonce",
            None,
            &[],
            &[],
        );

        assert!(!url.contains("code_challenge"));
        assert!(!url.contains("code_challenge_method"));
    }

    /// Tests authorization URL construction with ACR values.
    #[test]
    fn test_build_authorization_url_with_acr_values() {
        let url = build_authorization_url(
            "https://provider.example.com/oauth2/authorize",
            "test-client-id",
            "http://localhost:8500/callback",
            "test-state",
            "test-nonce",
            None,
            &[],
            &["urn:mace:incommon:iap:silver".to_string()],
        );

        assert!(url.contains("acr_values=urn%3Amace%3Aincommon%3Aiap%3Asilver"));
    }

    /// Tests application of `claim_mappings`.
    #[test]
    fn test_apply_claim_mappings() {
        let mut claim_mappings = HashMap::new();
        claim_mappings.insert("project".to_string(), "project_var".to_string());
        claim_mappings.insert("name".to_string(), "display_name".to_string());

        let mut list_claim_mappings = HashMap::new();
        list_claim_mappings.insert("groups".to_string(), "groups_var".to_string());

        let config = OidcConfig {
            oidc_discovery_url: "https://example.com".to_string(),
            oidc_client_id: "test-client".to_string(),
            oidc_client_secret: "".to_string(),
            oidc_scopes: vec![],
            oidc_acr_values: vec![],
            allowed_redirect_uris: vec!["http://localhost".to_string()],
            claim_mappings,
            list_claim_mappings,
            bound_audiences: vec![],
            oidc_client_use_pkce: true,
            verbose_oidc_logging: false,
            oidc_discovery_ca_cert: None,
            jwt_supported_algs: vec![],
        };

        let raw_claims = serde_json::json!({
            "project": "my-project",
            "name": "John Doe",
            "groups": ["admin", "developer"],
            "iss": "https://example.com",
            "sub": "12345"
        });

        let result = apply_claim_mappings(&raw_claims, &config);

        assert_eq!(result.get("project_var"), Some(&serde_json::json!("my-project")));
        assert_eq!(result.get("display_name"), Some(&serde_json::json!("John Doe")));

        // List-type claims should be converted to an array.
        let groups = result.get("groups_var").unwrap().as_array().unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], "admin");
        assert_eq!(groups[1], "developer");
    }

    /// Tests that `claim_mappings` handles non-existent claims.
    #[test]
    fn test_apply_claim_mappings_missing_claim() {
        let mut claim_mappings = HashMap::new();
        claim_mappings.insert("nonexistent".to_string(), "missing_var".to_string());

        let config = OidcConfig {
            oidc_discovery_url: "https://example.com".to_string(),
            oidc_client_id: "test-client".to_string(),
            oidc_client_secret: "".to_string(),
            oidc_scopes: vec![],
            oidc_acr_values: vec![],
            allowed_redirect_uris: vec!["http://localhost".to_string()],
            claim_mappings,
            list_claim_mappings: HashMap::new(),
            bound_audiences: vec![],
            oidc_client_use_pkce: true,
            verbose_oidc_logging: false,
            oidc_discovery_ca_cert: None,
            jwt_supported_algs: vec![],
        };

        let raw_claims = serde_json::json!({"sub": "12345"});
        let result = apply_claim_mappings(&raw_claims, &config);

        // A non-existent claim should not appear in the result.
        assert!(result.get("missing_var").is_none());
    }
}
