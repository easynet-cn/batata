//! OIDC (OpenID Connect) 认证模块，兼容 Consul API。
//!
//! 本模块实现了与 Consul `internal/go-sso/oidcauth` 库兼容的 OIDC 认证流程，
//! 提供两个核心操作：
//! - 生成 OIDC 授权 URL（`get_auth_url`）
//! - 交换授权码获取 token 并提取 claims（`exchange_code`）
//!
//! 核心特性：
//! - PKCE (Proof Key for Code Exchange) S256，默认启用（与 Consul 一致）
//! - State 管理：10 分钟 TTL，一次性使用（验证后立即删除）
//! - OIDC Discovery 文档缓存，避免重复请求
//! - JWT 签名验证（使用 provider 的 JWKS）
//! - 支持 ClaimMappings 和 ListClaimMappings

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
// 常量定义
// ============================================================================

/// OIDC state 的默认 TTL（10 分钟），与 Consul 一致
const DEFAULT_STATE_TTL: Duration = Duration::from_secs(10 * 60);

/// 随机字节数（用于生成 state ID 和 nonce）
const RANDOM_BYTES: usize = 20;

/// PKCE code_verifier 长度（32 字节 = 43 字符 base64url）
const PKCE_VERIFIER_BYTES: usize = 32;

// ============================================================================
// OidcConfig - 从 AuthMethod.config 解析的 OIDC 配置
// ============================================================================

/// OIDC 认证方法配置，从 AuthMethod 的 config 字段解析而来。
///
/// 对应 Consul 的 `OIDCAuthMethodConfig` 结构体。
#[derive(Clone, Debug)]
pub struct OidcConfig {
    /// OIDC provider 的 discovery URL（必填）
    pub oidc_discovery_url: String,
    /// OIDC client ID（必填）
    pub oidc_client_id: String,
    /// OIDC client secret
    pub oidc_client_secret: String,
    /// 请求的 OIDC scopes（默认包含 "openid"）
    pub oidc_scopes: Vec<String>,
    /// 请求的 ACR values
    pub oidc_acr_values: Vec<String>,
    /// 允许的回调 URI 列表（必填，redirect_uri 必须在其中）
    pub allowed_redirect_uris: Vec<String>,
    /// claim 映射：JWT claim 名 -> 变量名
    pub claim_mappings: HashMap<String, String>,
    /// 列表型 claim 映射：JWT claim 名 -> 变量名
    pub list_claim_mappings: HashMap<String, String>,
    /// 绑定的 audience 列表
    pub bound_audiences: Vec<String>,
    /// 是否启用 PKCE（默认 true，与 Consul 一致）
    pub oidc_client_use_pkce: bool,
    /// 是否启用详细 OIDC 日志
    pub verbose_oidc_logging: bool,
    /// OIDC discovery 的 CA 证书（PEM 格式，可选）
    pub oidc_discovery_ca_cert: Option<String>,
    /// 支持的 JWT 签名算法列表
    pub jwt_supported_algs: Vec<String>,
}

impl OidcConfig {
    /// 从 AuthMethod 的 config 字段解析 OIDC 配置。
    ///
    /// config 是一个 `HashMap<String, serde_json::Value>`，其中键名使用
    /// Consul 的 PascalCase 格式（如 "OIDCDiscoveryURL"、"OIDCClientID" 等）。
    ///
    /// # 参数
    /// - `config`: AuthMethod 的 config 字段
    ///
    /// # 返回
    /// - `Ok(OidcConfig)`: 解析成功
    /// - `Err(String)`: 配置缺失或格式错误，包含详细的错误信息
    pub fn from_auth_method_config(
        config: &Option<HashMap<String, serde_json::Value>>,
    ) -> Result<Self, String> {
        let config = config.as_ref().ok_or_else(|| {
            "OIDC auth method config is missing required fields: OIDCDiscoveryURL, OIDCClientID, AllowedRedirectURIs".to_string()
        })?;

        // 必填字段：OIDCDiscoveryURL
        let oidc_discovery_url = config
            .get("OIDCDiscoveryURL")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "OIDCDiscoveryURL is required".to_string())?;

        // 必填字段：OIDCClientID
        let oidc_client_id = config
            .get("OIDCClientID")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .ok_or_else(|| "OIDCClientID is required".to_string())?;

        // 可选字段：OIDCClientSecret
        let oidc_client_secret = config
            .get("OIDCClientSecret")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // 必填字段：AllowedRedirectURIs
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

        // 可选字段：OIDCScopes
        let oidc_scopes = config
            .get("OIDCScopes")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // 可选字段：OIDCACRValues
        let oidc_acr_values = config
            .get("OIDCACRValues")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // 可选字段：ClaimMappings
        let claim_mappings = config
            .get("ClaimMappings")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // 可选字段：ListClaimMappings
        let list_claim_mappings = config
            .get("ListClaimMappings")
            .and_then(|v| v.as_object())
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // 可选字段：BoundAudiences
        let bound_audiences = config
            .get("BoundAudiences")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        // 可选字段：OIDCClientUsePKCE（默认 true，与 Consul 一致）
        let oidc_client_use_pkce = config
            .get("OIDCClientUsePKCE")
            .and_then(|v| v.as_bool())
            .unwrap_or(true); // 默认启用 PKCE

        // 可选字段：VerboseOIDCLogging
        let verbose_oidc_logging = config
            .get("VerboseOIDCLogging")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // 可选字段：OIDCDiscoveryCACert
        let oidc_discovery_ca_cert = config
            .get("OIDCDiscoveryCACert")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        // 可选字段：JWTSupportedAlgs
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

    /// 验证 redirect_uri 是否在允许的列表中。
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
// OidcState - 单次 OIDC 认证流程的状态
// ============================================================================

/// OIDC 认证流程的临时状态，存储在 state_store 中。
///
/// 每个 state 对应一次 OIDC 认证流程，包含：
/// - nonce：用于防止重放攻击
/// - redirect_uri：回调时用于交换 code 的 redirect_uri
/// - auth_method：关联的认证方法名
/// - client_nonce：客户端提供的 nonce（可选）
/// - meta：客户端提供的元数据（可选）
/// - code_verifier：PKCE code_verifier（如果启用 PKCE）
#[derive(Clone, Debug)]
pub struct OidcState {
    /// 随机生成的 nonce，发送给 OIDC provider 并在回调时验证
    nonce: String,
    /// 客户端请求时使用的 redirect_uri
    redirect_uri: String,
    /// 关联的认证方法名
    auth_method: String,
    /// 客户端提供的 nonce（可选，用于额外验证）
    client_nonce: Option<String>,
    /// 客户端提供的元数据
    meta: Option<HashMap<String, String>>,
    /// 创建时间，用于 TTL 过期检查
    created_at: Instant,
    /// PKCE code_verifier（如果启用 PKCE）
    code_verifier: Option<String>,
}

impl OidcState {
    /// 检查 state 是否已过期
    fn is_expired(&self, ttl: Duration) -> bool {
        self.created_at.elapsed() > ttl
    }
}

// ============================================================================
// OidcStateStore - State 存储与管理
// ============================================================================

/// OIDC state 存储器，使用 DashMap 实现并发安全的 state 管理。
///
/// 特性：
/// - 10 分钟 TTL，自动过期
/// - 一次性使用：verify_and_remove 后立即删除
/// - 并发安全：使用 DashMap
pub struct OidcStateStore {
    /// 存储 state 的 DashMap，key 为 state_id
    states: DashMap<String, OidcState>,
    /// state 的 TTL
    ttl: Duration,
}

impl OidcStateStore {
    /// 创建新的 state 存储器，使用默认 10 分钟 TTL。
    pub fn new() -> Self {
        Self {
            states: DashMap::new(),
            ttl: DEFAULT_STATE_TTL,
        }
    }

    /// 创建新的 state 存储器，使用自定义 TTL（用于测试）。
    #[cfg(test)]
    pub fn with_ttl(ttl: Duration) -> Self {
        Self {
            states: DashMap::new(),
            ttl,
        }
    }

    /// 插入一个新的 state。
    ///
    /// 插入前会清理过期的 state，避免内存泄漏。
    pub fn insert(&self, state_id: String, state: OidcState) {
        // 顺便清理过期 state
        self.cleanup_expired();
        self.states.insert(state_id, state);
    }

    /// 验证并移除 state（一次性使用）。
    ///
    /// 如果 state 不存在或已过期，返回 None。
    /// 如果存在且未过期，移除并返回 state。
    pub fn verify_and_remove(&self, state_id: &str) -> Option<OidcState> {
        // 顺便清理过期 state
        self.cleanup_expired();

        // 移除 state（一次性使用）
        self.states.remove(state_id).map(|(_, state)| state)
    }

    /// 清理所有过期的 state。
    ///
    /// 在每次 insert 和 verify_and_remove 时自动调用。
    /// 也可以手动调用以主动清理。
    pub fn cleanup_expired(&self) {
        let ttl = self.ttl;
        self.states.retain(|_, state| !state.is_expired(ttl));
    }

    /// 返回当前存储的 state 数量（主要用于测试和监控）。
    pub fn len(&self) -> usize {
        self.states.len()
    }

    /// 检查 state 是否存在（仅用于测试）。
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
// OIDC Discovery 文档与缓存
// ============================================================================

/// OIDC Discovery 文档（来自 `/.well-known/openid-configuration`）。
///
/// 只包含 batata 需要的字段。
#[derive(Clone, Debug, Serialize, Deserialize)]
struct OidcDiscoveryDoc {
    /// OIDC provider 的 issuer URL
    issuer: String,
    /// 授权端点 URL
    authorization_endpoint: String,
    /// token 端点 URL
    token_endpoint: String,
    /// JWKS URI（用于获取验证 JWT 的公钥）
    jwks_uri: String,
    /// userinfo 端点 URL（可选）
    #[serde(skip_serializing_if = "Option::is_none")]
    userinfo_endpoint: Option<String>,
}

/// JWKS (JSON Web Key Set) 中的单个 key。
#[derive(Clone, Debug, Deserialize)]
struct Jwk {
    /// key ID（用于匹配 JWT header 中的 kid）
    #[serde(skip_serializing_if = "Option::is_none")]
    kid: Option<String>,
    /// key type（如 "RSA"）
    kty: String,
    /// RSA modulus（base64url 编码）
    #[serde(skip_serializing_if = "Option::is_none")]
    n: Option<String>,
    /// RSA exponent（base64url 编码）
    #[serde(skip_serializing_if = "Option::is_none")]
    e: Option<String>,
}

/// JWKS (JSON Web Key Set) 响应。
#[derive(Clone, Debug, Deserialize)]
struct Jwks {
    keys: Vec<Jwk>,
}

/// OIDC Discovery 文档缓存，按 discovery URL 缓存。
///
/// 使用全局静态缓存，避免每个请求都查询 OIDC provider。
/// 缓存有效期为 5 分钟。
static DISCOVERY_CACHE: LazyLock<DashMap<String, (OidcDiscoveryDoc, Instant)>> =
    LazyLock::new(|| DashMap::new());

/// JWKS 缓存，按 jwks_uri 缓存。
///
/// 缓存有效期为 10 分钟。
static JWKS_CACHE: LazyLock<DashMap<String, (Jwks, Instant)>> =
    LazyLock::new(|| DashMap::new());

/// Discovery 文档缓存时间（5 分钟）
const DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(5 * 60);

/// JWKS 缓存时间（10 分钟）
const JWKS_CACHE_TTL: Duration = Duration::from_secs(10 * 60);

// ============================================================================
// OidcAuthenticator - 核心 OIDC 认证器
// ============================================================================

/// OIDC 认证器，封装了完整的 OIDC 认证流程。
///
/// 每个 OidcConfig 对应一个 OidcAuthenticator。
/// Authenticator 内部维护：
/// - HTTP client（用于与 OIDC provider 通信）
/// - state store（管理认证流程状态）
pub struct OidcAuthenticator {
    /// OIDC 配置
    config: OidcConfig,
    /// HTTP client（reqwest）
    http_client: reqwest::Client,
    /// state 存储器
    state_store: OidcStateStore,
}

impl OidcAuthenticator {
    /// 创建新的 OIDC 认证器。
    ///
    /// # 参数
    /// - `config`: OIDC 配置
    ///
    /// # 返回
    /// - `Ok(Self)`: 创建成功
    /// - `Err(String)`: HTTP client 创建失败
    pub fn new(config: OidcConfig) -> Result<Self, String> {
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .redirect(reqwest::redirect::Policy::none());

        // 如果配置了 CA 证书，添加到 client
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

    /// 生成 OIDC 授权 URL。
    ///
    /// 流程：
    /// 1. 验证 redirect_uri 在 allowed_redirect_uris 中
    /// 2. 生成 state ID 和 nonce（各 20 字节随机数）
    /// 3. 如果 PKCE 启用，生成 code_verifier 和 code_challenge
    /// 4. 查询 OIDC discovery 获取 authorization_endpoint
    /// 5. 构建授权 URL
    /// 6. 存储 state
    /// 7. 返回 URL
    ///
    /// # 参数
    /// - `redirect_uri`: 回调 URI（必须在 allowed_redirect_uris 中）
    /// - `auth_method`: 认证方法名
    /// - `client_nonce`: 客户端 nonce（可选）
    /// - `meta`: 客户端元数据（可选）
    ///
    /// # 返回
    /// - `Ok(String)`: 授权 URL
    /// - `Err(String)`: 错误信息
    pub async fn get_auth_url(
        &self,
        redirect_uri: &str,
        auth_method: &str,
        client_nonce: Option<&str>,
        meta: Option<HashMap<String, String>>,
    ) -> Result<String, String> {
        // 1. 验证 redirect_uri
        self.config.validate_redirect_uri(redirect_uri)?;

        // 2. 生成 state ID 和 nonce
        let state_id = generate_random_string(RANDOM_BYTES);
        let nonce = generate_random_string(RANDOM_BYTES);

        // 3. 生成 PKCE code_verifier 和 code_challenge
        let (code_verifier, code_challenge) = if self.config.oidc_client_use_pkce {
            let verifier = generate_random_string(PKCE_VERIFIER_BYTES);
            let challenge = compute_pkce_challenge(&verifier);
            (Some(verifier), Some(challenge))
        } else {
            (None, None)
        };

        // 4. 查询 OIDC discovery 获取 authorization_endpoint
        let discovery = self.fetch_discovery_doc().await?;

        // 5. 构建授权 URL
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

        // 6. 存储 state
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

        // 7. 返回 URL
        Ok(auth_url)
    }

    /// 交换授权码获取 token 并提取 claims。
    ///
    /// 流程：
    /// 1. 验证并移除 state（一次性使用）
    /// 2. 查询 OIDC discovery 获取 token_endpoint
    /// 3. POST 到 token_endpoint 交换 code
    /// 4. 解析响应获取 id_token
    /// 5. 验证 JWT 签名（使用 discovery 的 jwks_uri 获取公钥）
    /// 6. 验证 nonce 匹配
    /// 7. 提取 claims
    /// 8. 应用 claim_mappings 和 list_claim_mappings
    ///
    /// # 参数
    /// - `state_id`: 授权 URL 返回时携带的 state
    /// - `code`: OIDC provider 返回的授权码
    ///
    /// # 返回
    /// - `Ok(OidcClaims)`: 提取的 claims
    /// - `Err(String)`: 错误信息
    pub async fn exchange_code(
        &self,
        state_id: &str,
        code: &str,
    ) -> Result<OidcClaims, String> {
        // 1. 验证并移除 state（一次性使用）
        let state = self.state_store.verify_and_remove(state_id).ok_or_else(|| {
            "OIDC state not found or expired. The state may have already been used or has timed out".to_string()
        })?;

        // 2. 查询 OIDC discovery 获取 token_endpoint
        let discovery = self.fetch_discovery_doc().await?;

        // 3. POST 到 token_endpoint 交换 code
        let token_response = self
            .exchange_code_for_token(&discovery.token_endpoint, code, &state)
            .await?;

        // 4. 获取 id_token
        let id_token = token_response.id_token.ok_or_else(|| {
            "OIDC token response does not contain id_token".to_string()
        })?;

        // 5. 验证 JWT 签名
        let jwks = self.fetch_jwks(&discovery.jwks_uri).await?;
        let claims = verify_jwt(&id_token, &jwks, &discovery.issuer, &self.config)?;

        // 6. 验证 nonce 匹配
        let token_nonce = claims.get("nonce").and_then(|v| v.as_str());
        if token_nonce != Some(state.nonce.as_str()) {
            return Err("OIDC nonce mismatch: the id_token nonce does not match the expected nonce".to_string());
        }

        // 7. 应用 claim_mappings 和 list_claim_mappings
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

    /// 获取 state store 中的 state 数量（主要用于监控）。
    pub fn state_count(&self) -> usize {
        self.state_store.len()
    }

    /// 手动触发 state 清理。
    pub fn cleanup_states(&self) {
        self.state_store.cleanup_expired();
    }

    // ------------------------------------------------------------------
    // 内部方法
    // ------------------------------------------------------------------

    /// 获取 OIDC discovery 文档（带缓存）。
    async fn fetch_discovery_doc(&self) -> Result<OidcDiscoveryDoc, String> {
        let discovery_url = &self.config.oidc_discovery_url;

        // 检查缓存
        if let Some(entry) = DISCOVERY_CACHE.get(discovery_url) {
            let (doc, created_at) = entry.value();
            if created_at.elapsed() < DISCOVERY_CACHE_TTL {
                if self.config.verbose_oidc_logging {
                    debug!("OIDC discovery document loaded from cache for {}", discovery_url);
                }
                return Ok(doc.clone());
            }
        }

        // 缓存未命中或已过期，从 OIDC provider 获取
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

        // 更新缓存
        DISCOVERY_CACHE.insert(discovery_url.clone(), (doc.clone(), Instant::now()));

        Ok(doc)
    }

    /// 获取 JWKS（带缓存）。
    async fn fetch_jwks(&self, jwks_uri: &str) -> Result<Jwks, String> {
        // 检查缓存
        if let Some(entry) = JWKS_CACHE.get(jwks_uri) {
            let (jwks, created_at) = entry.value();
            if created_at.elapsed() < JWKS_CACHE_TTL {
                if self.config.verbose_oidc_logging {
                    debug!("JWKS loaded from cache for {}", jwks_uri);
                }
                return Ok(jwks.clone());
            }
        }

        // 缓存未命中或已过期，从 OIDC provider 获取
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

        // 更新缓存
        JWKS_CACHE.insert(jwks_uri.to_string(), (jwks.clone(), Instant::now()));

        Ok(jwks)
    }

    /// 使用授权码交换 token。
    async fn exchange_code_for_token(
        &self,
        token_endpoint: &str,
        code: &str,
        state: &OidcState,
    ) -> Result<TokenResponse, String> {
        // 构建 form 表单参数
        let mut form = vec![
            ("grant_type".to_string(), "authorization_code".to_string()),
            ("code".to_string(), code.to_string()),
            ("redirect_uri".to_string(), state.redirect_uri.clone()),
            ("client_id".to_string(), self.config.oidc_client_id.clone()),
        ];

        // client_secret（如果配置了）
        if !self.config.oidc_client_secret.is_empty() {
            form.push((
                "client_secret".to_string(),
                self.config.oidc_client_secret.clone(),
            ));
        }

        // code_verifier（如果启用了 PKCE）
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
// OIDC Token 响应
// ============================================================================

/// OIDC token 端点的响应。
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenResponse {
    /// access_token（不一定需要，但 OIDC 标准要求返回）
    #[serde(default)]
    access_token: Option<String>,
    /// id_token（OIDC 核心，包含用户 claims）
    #[serde(default)]
    id_token: Option<String>,
    /// token_type（如 "Bearer"）
    #[serde(default)]
    token_type: Option<String>,
    /// expires_in（秒）
    #[serde(default)]
    expires_in: Option<u64>,
    /// refresh_token（可选）
    #[serde(default)]
    refresh_token: Option<String>,
}

// ============================================================================
// OidcClaims - 处理后的 claims
// ============================================================================

/// 处理后的 OIDC claims，包含应用了 claim_mappings 的变量。
///
/// 用于后续的 binding rules 匹配。
#[derive(Clone, Debug)]
pub struct OidcClaims {
    /// 处理后的 claims 变量映射（key 为变量名，value 为 JSON 值）
    pub claims: HashMap<String, serde_json::Value>,
    /// 关联的认证方法名
    pub auth_method: String,
    /// 客户端 nonce
    pub client_nonce: Option<String>,
    /// 客户端元数据
    pub meta: Option<HashMap<String, String>>,
}

// ============================================================================
// 辅助函数
// ============================================================================

/// 生成随机字符串（base64url 编码）。
///
/// 生成 `num_bytes` 个随机字节，然后使用 base64url（无 padding）编码。
fn generate_random_string(num_bytes: usize) -> String {
    let mut bytes = vec![0u8; num_bytes];
    rand::rng().fill_bytes(&mut bytes);
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes)
}

/// 计算 PKCE code_challenge（S256 方法）。
///
/// code_challenge = base64url(SHA256(code_verifier))
fn compute_pkce_challenge(code_verifier: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(code_verifier.as_bytes());
    let hash = hasher.finalize();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash)
}

/// 构建 OIDC 授权 URL。
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
    // 构建 scope 参数（始终包含 "openid"）
    let mut all_scopes = vec!["openid".to_string()];
    all_scopes.extend(scopes.iter().cloned());
    let scope_str = all_scopes.join(" ");

    // 使用 url 库构建 URL
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

    // PKCE 参数
    if let Some(challenge) = code_challenge {
        query_pairs
            .append_pair("code_challenge", challenge)
            .append_pair("code_challenge_method", "S256");
    }

    // ACR values（如果配置了）
    if !acr_values.is_empty() {
        query_pairs.append_pair("acr_values", &acr_values.join(" "));
    }

    drop(query_pairs);
    url.to_string()
}

/// 验证 JWT 并提取 claims。
///
/// 流程：
/// 1. 解析 JWT header 获取 kid 和 alg
/// 2. 在 JWKS 中查找匹配的 key
/// 3. 使用匹配的 key 验证 JWT 签名
/// 4. 验证 issuer
/// 5. 验证 audience（如果配置了 bound_audiences）
/// 6. 返回 claims
fn verify_jwt(
    token: &str,
    jwks: &Jwks,
    expected_issuer: &str,
    config: &OidcConfig,
) -> Result<serde_json::Value, String> {
    use jsonwebtoken::{Algorithm, DecodingKey, Validation, decode, decode_header};

    // 1. 解析 JWT header 获取 kid 和 alg
    let header = decode_header(token)
        .map_err(|e| format!("Failed to decode JWT header: {}", e))?;

    let kid = header.kid.as_deref();

    // 2. 在 JWKS 中查找匹配的 key
    let matching_key = jwks.keys.iter().find(|k| {
        // 如果 kid 存在，按 kid 匹配；否则用第一个 RSA key
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

    // 3. 创建 DecodingKey
    let n = jwk.n.as_deref().ok_or("JWK missing 'n' (modulus) field")?;
    let e = jwk.e.as_deref().ok_or("JWK missing 'e' (exponent) field")?;
    let decoding_key = DecodingKey::from_rsa_components(n, e)
        .map_err(|e| format!("Failed to create decoding key from JWK: {}", e))?;

    // 4. 创建 Validation
    let algorithm = match header.alg {
        Algorithm::RS256 | Algorithm::RS384 | Algorithm::RS512 => header.alg,
        Algorithm::PS256 | Algorithm::PS384 | Algorithm::PS512 => header.alg,
        Algorithm::ES256 | Algorithm::ES384 => header.alg,
        Algorithm::EdDSA => header.alg,
        _ => {
            // 默认使用 RS256
            // 如果配置了 JWTSupportedAlgs，检查是否支持
            if !config.jwt_supported_algs.is_empty() {
                // 检查 alg 是否在支持的列表中
                let alg_str = format!("{:?}", header.alg);
                if !config.jwt_supported_algs.iter().any(|a| alg_str.contains(a)) {
                    return Err(format!("JWT algorithm {:?} is not in the supported algorithms list", header.alg));
                }
            }
            Algorithm::RS256
        }
    };

    let mut validation = Validation::new(algorithm);

    // 设置预期的 issuer
    validation.set_issuer(&[expected_issuer]);

    // 设置预期的 audience（如果配置了 bound_audiences）
    if !config.bound_audiences.is_empty() {
        let audiences: Vec<&str> = config.bound_audiences.iter().map(|s| s.as_str()).collect();
        validation.set_audience(&audiences);
    }

    // 5. 验证 JWT
    let token_data = decode::<serde_json::Value>(token, &decoding_key, &validation)
        .map_err(|e| format!("JWT verification failed: {}", e))?;

    Ok(token_data.claims)
}

/// 应用 claim_mappings 和 list_claim_mappings。
///
/// 将原始 JWT claims 按照配置的映射规则转换为变量名。
fn apply_claim_mappings(
    raw_claims: &serde_json::Value,
    config: &OidcConfig,
) -> HashMap<String, serde_json::Value> {
    let mut result = HashMap::new();

    // 应用 claim_mappings（单个值）
    for (claim_name, var_name) in &config.claim_mappings {
        if let Some(value) = raw_claims.get(claim_name) {
            result.insert(var_name.clone(), value.clone());
        }
    }

    // 应用 list_claim_mappings（列表值）
    for (claim_name, var_name) in &config.list_claim_mappings {
        if let Some(value) = raw_claims.get(claim_name) {
            if let Some(arr) = value.as_array() {
                // 列表型 claim，存储为字符串数组
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
                // 如果 claim 不是数组而是字符串，也按列表处理
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
// 全局 OIDC Authenticator 缓存
// ============================================================================

/// 全局 OIDC authenticator 缓存，按 auth method name 缓存。
///
/// 使用静态缓存（类似 TOKEN_CACHE），避免每次请求都重新创建 authenticator。
/// 当 auth method 配置更新时，调用 `invalidate_authenticator` 清除缓存。
static OIDC_AUTHENTICATORS: LazyLock<DashMap<String, Arc<OidcAuthenticator>>> =
    LazyLock::new(|| DashMap::new());

/// 获取或创建指定 auth method 的 OIDC authenticator。
///
/// 如果缓存中不存在，则从 auth method 配置创建新的 authenticator 并缓存。
///
/// # 参数
/// - `auth_method_name`: 认证方法名
/// - `config`: auth method 的 config
///
/// # 返回
/// - `Ok(Arc<OidcAuthenticator>)`: authenticator
/// - `Err(String)`: 创建失败
pub fn get_or_create_authenticator(
    auth_method_name: &str,
    config: &Option<HashMap<String, serde_json::Value>>,
) -> Result<Arc<OidcAuthenticator>, String> {
    // 检查缓存
    if let Some(entry) = OIDC_AUTHENTICATORS.get(auth_method_name) {
        return Ok(entry.value().clone());
    }

    // 缓存未命中，创建新的 authenticator
    let oidc_config = OidcConfig::from_auth_method_config(config)?;
    let authenticator = Arc::new(OidcAuthenticator::new(oidc_config)?);

    // 存入缓存（使用 entry API 避免竞争条件）
    OIDC_AUTHENTICATORS
        .entry(auth_method_name.to_string())
        .or_insert(authenticator.clone());

    Ok(authenticator)
}

/// 清除指定 auth method 的 OIDC authenticator 缓存。
///
/// 在 auth method 更新或删除时调用。
pub fn invalidate_authenticator(auth_method_name: &str) {
    OIDC_AUTHENTICATORS.remove(auth_method_name);
}

/// 清除所有 OIDC authenticator 缓存。
pub fn invalidate_all_authenticators() {
    OIDC_AUTHENTICATORS.clear();
}

// ============================================================================
// 单元测试
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 测试从 auth method config 解析 OIDC 配置
    #[test]
    fn test_oidc_config_from_auth_method() {
        // 测试完整的有效配置
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
        // PKCE 默认启用
        assert!(oidc_config.oidc_client_use_pkce);
    }

    /// 测试缺少必填字段时的错误处理
    #[test]
    fn test_oidc_config_missing_required_fields() {
        // 测试 config 为 None
        let result = OidcConfig::from_auth_method_config(&None);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("missing required fields"));

        // 测试缺少 OIDCDiscoveryURL
        let mut config = HashMap::new();
        config.insert("OIDCClientID".to_string(), serde_json::json!("test-client"));
        config.insert(
            "AllowedRedirectURIs".to_string(),
            serde_json::json!(["http://localhost:8500/callback"]),
        );
        let result = OidcConfig::from_auth_method_config(&Some(config));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("OIDCDiscoveryURL is required"));

        // 测试缺少 AllowedRedirectURIs
        let mut config = HashMap::new();
        config.insert("OIDCDiscoveryURL".to_string(), serde_json::json!("https://example.com"));
        config.insert("OIDCClientID".to_string(), serde_json::json!("test-client"));
        let result = OidcConfig::from_auth_method_config(&Some(config));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("AllowedRedirectURIs is required"));
    }

    /// 测试 PKCE 可被禁用
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

    /// 测试 state store 的插入和验证
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

        // 插入 state
        store.insert("test-state-id".to_string(), state);

        // 验证 state 存在
        assert!(store.contains("test-state-id"));

        // verify_and_remove 应该返回 state
        let result = store.verify_and_remove("test-state-id");
        assert!(result.is_some());
        assert_eq!(result.unwrap().nonce, "test-nonce");

        // state 应该已被移除（一次性使用）
        assert!(!store.contains("test-state-id"));
    }

    /// 测试 state store 的过期清理
    #[test]
    fn test_oidc_state_store_expired_cleanup() {
        // 使用 10ms TTL
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

        // 等待过期
        std::thread::sleep(Duration::from_millis(50));

        // 手动清理
        store.cleanup_expired();
        assert!(!store.contains("test-state-id"));
    }

    /// 测试 state 的一次性使用特性（verify 后立即删除）
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

        // 第一次验证应该成功
        let result1 = store.verify_and_remove("one-time-state");
        assert!(result1.is_some());
        let retrieved = result1.unwrap();
        assert_eq!(retrieved.nonce, "one-time-nonce");
        assert_eq!(retrieved.client_nonce, Some("client-nonce".to_string()));
        assert_eq!(retrieved.code_verifier, Some("test-verifier".to_string()));

        // 第二次验证应该失败（state 已被删除）
        let result2 = store.verify_and_remove("one-time-state");
        assert!(result2.is_none());
    }

    /// 测试不存在的 state 验证返回 None
    #[test]
    fn test_oidc_state_store_verify_nonexistent() {
        let store = OidcStateStore::new();
        let result = store.verify_and_remove("nonexistent-state");
        assert!(result.is_none());
    }

    /// 测试 redirect_uri 验证
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

        // 有效的 redirect_uri
        assert!(config.validate_redirect_uri("http://localhost:8500/callback").is_ok());
        assert!(config.validate_redirect_uri("http://localhost:8500/ui/login").is_ok());

        // 无效的 redirect_uri
        assert!(config.validate_redirect_uri("http://evil.com/callback").is_err());
        assert!(config.validate_redirect_uri("http://localhost:8500/evil").is_err());
    }

    /// 测试 PKCE code_challenge 计算
    #[test]
    fn test_pkce_challenge_computation() {
        // 使用已知测试向量验证 PKCE S256 计算
        // code_verifier -> SHA256 -> base64url(no padding)
        let verifier = "dBjftJeZ4CVK-mJMgjYqsrkuerxyAL_nzjF2yT5g";
        let challenge = compute_pkce_challenge(verifier);

        // 验证 challenge 是 base64url 编码且长度正确（SHA256 = 32 bytes -> 43 chars base64url）
        assert_eq!(challenge.len(), 43, "PKCE challenge should be 43 characters (32 bytes base64url no padding)");

        // 验证每次计算结果一致（确定性）
        let challenge2 = compute_pkce_challenge(verifier);
        assert_eq!(challenge, challenge2, "Same verifier should produce same challenge");

        // 验证不同 verifier 产生不同 challenge
        let different_challenge = compute_pkce_challenge("different-verifier-1234567890");
        assert_ne!(challenge, different_challenge, "Different verifiers should produce different challenges");
    }

    /// 测试随机字符串生成
    #[test]
    fn test_random_string_generation() {
        let s1 = generate_random_string(20);
        let s2 = generate_random_string(20);

        // 两个随机字符串应该不同
        assert_ne!(s1, s2);

        // 长度应该正确（20 字节 base64url 无 padding = 27 字符）
        assert_eq!(s1.len(), 27);
    }

    /// 测试授权 URL 构建
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

        // 验证 URL 包含所有必需参数
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

    /// 测试不带 PKCE 的授权 URL 构建
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

    /// 测试带 ACR values 的授权 URL 构建
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

    /// 测试 claim_mappings 应用
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

        // 列表型 claim 应该被转换为数组
        let groups = result.get("groups_var").unwrap().as_array().unwrap();
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0], "admin");
        assert_eq!(groups[1], "developer");
    }

    /// 测试 claim_mappings 处理不存在的 claim
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

        // 不存在的 claim 不应该出现在结果中
        assert!(result.get("missing_var").is_none());
    }
}
