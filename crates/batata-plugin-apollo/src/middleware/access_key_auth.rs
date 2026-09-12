//! Apollo client authentication (AccessKey signature verification).
//!
//! Port of upstream `configservice/filter/ClientAuthenticationFilter`:
//!
//! ```text
//! Authorization: Apollo <appId>:<signature>
//! Timestamp: <millis>
//!
//! signature = Base64( HMAC-SHA1( secret, <timestamp> + "\n" + <pathWithQuery> ) )
//! ```
//!
//! Upstream enforcement semantics (this module mirrors them):
//! 1. appId is extracted from the URL path (`/configs/{appId}/...`,
//!    `/configfiles[/json|/raw]/{appId}/...`) or the `appId` query param for
//!    `/notifications*`. A blank appId → **400 InvalidAppId**.
//! 2. If the app has any **enabled** AccessKey, every request MUST carry a
//!    valid signature — missing/invalid → **401 Unauthorized**. ALL enabled
//!    secrets are tried (upstream `checkAuthorization` loops secrets).
//! 3. Timestamp skew tolerance defaults to 60 s
//!    (`apollo.access-key.auth-time-diff-tolerance`, override via env).
//! 4. Requests to unprotected paths pass through untouched.

use std::sync::Arc;

use actix_web::HttpRequest;
use base64::Engine;
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use subtle::ConstantTimeEq;

use crate::api::dto::ErrorResponse;
use crate::persistence::traits::ApolloPersistenceService;
use crate::service::AccessKeyService;

type HmacSha1 = Hmac<Sha1>;

/// Maximum allowed clock skew between client and server, in milliseconds.
/// Upstream default: `apollo.access-key.auth-time-diff-tolerance` = 60s.
fn max_skew_ms() -> i64 {
    const DEFAULT_SECS: i64 = 60;
    std::env::var("APOLLO_ACCESS_KEY_AUTH_TIME_DIFF_TOLERANCE_SECS")
        .ok()
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(DEFAULT_SECS)
        * 1000
}

/// Verify the Apollo access-key signature for a request.
///
/// Returns `Ok(())` when the request may proceed, otherwise `Err(response)`
/// with a 400/401 body per upstream semantics.
pub async fn authenticate(
    req: &HttpRequest,
    persistence: &Arc<dyn ApolloPersistenceService>,
) -> Result<(), actix_web::HttpResponse> {
    let path = req.path().to_string();
    let protected = path.starts_with("/configs")
        || path.starts_with("/configfiles")
        || path.starts_with("/notifications");

    if !protected {
        return Ok(());
    }

    // Upstream AccessKeyUtil.extractAppIdFromRequest.
    let Some(app_id) = extract_app_id(&path, req) else {
        return Err(bad_request("InvalidAppId"));
    };

    let keys = AccessKeyService::new(persistence.clone())
        .list_by_app(&app_id)
        .await
        .unwrap_or_default();

    let enabled_secrets: Vec<String> = keys
        .iter()
        .filter(|k| k.is_enabled)
        .map(|k| k.secret.clone())
        .collect();
    if enabled_secrets.is_empty() {
        // No enabled access key for this app → open access (upstream behavior).
        return Ok(());
    }

    let timestamp = req
        .headers()
        .get("Timestamp")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.parse::<i64>().ok());

    let ts = match timestamp {
        Some(t) => t,
        None => return Err(unauthorized("Unauthorized")),
    };

    let now = chrono::Utc::now().timestamp_millis();
    if (now - ts).abs() >= max_skew_ms() {
        return Err(unauthorized("Unauthorized"));
    }

    let auth_header = req
        .headers()
        .get("Authorization")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let Some(signature) = auth_header.as_deref().and_then(parse_signature) else {
        return Err(unauthorized("Unauthorized"));
    };

    let path_with_query = req
        .uri()
        .path_and_query()
        .map(|pq| pq.as_str().to_string())
        .unwrap_or_else(|| path.clone());

    // Try EVERY enabled secret (upstream loops availableSecrets).
    let sig_bytes = signature.as_bytes();
    for secret in &enabled_secrets {
        if let Ok(expected) = compute_signature(secret, ts, &path_with_query) {
            if bool::from(expected.as_bytes().ct_eq(sig_bytes)) {
                return Ok(());
            }
        }
    }
    Err(unauthorized("Unauthorized"))
}

fn bad_request(message: &str) -> actix_web::HttpResponse {
    actix_web::HttpResponse::BadRequest().json(ErrorResponse {
        status: 400,
        message: message.to_string(),
    })
}

fn unauthorized(message: &str) -> actix_web::HttpResponse {
    actix_web::HttpResponse::Unauthorized().json(ErrorResponse {
        status: 401,
        message: message.to_string(),
    })
}

/// Extract `{appId}` from `/configs/{appId}/...`, `/configfiles[...]{appId}/...`
/// or from the `appId` query param on notification paths; None when blank.
fn extract_app_id(path: &str, req: &HttpRequest) -> Option<String> {
    const PREFIXES: [&str; 4] = ["/configs/", "/configfiles/json/", "/configfiles/raw/", "/configfiles/"];
    let mut app_id: Option<String> = None;
    for prefix in PREFIXES {
        if let Some(rest) = path.strip_prefix(prefix) {
            app_id = rest.split('/').next().map(|s| s.to_string());
            break;
        }
    }
    if app_id.is_none() && path.starts_with("/notifications") {
        let from_path = req
            .match_info()
            .get("app_id")
            .or_else(|| req.match_info().get("appId"))
            .unwrap_or("");
        app_id = if from_path.is_empty() {
            query_param(req, "appId")
        } else {
            Some(from_path.to_string())
        };
    }
    match app_id {
        Some(id) if !id.trim().is_empty() => Some(id.trim().to_string()),
        _ => None,
    }
}

fn query_param(req: &HttpRequest, name: &str) -> Option<String> {
    let raw = req
        .uri()
        .query()?
        .split('&')
        .find_map(|pair| {
            let (k, v) = pair.split_once('=')?;
            (k == name).then_some(v.to_string())
        })?;
    Some(percent_decode(&raw))
}

fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'%' if i + 2 < bytes.len() => {
                let hex_str = String::from_utf8_lossy(&bytes[i + 1..i + 3]).to_string();
                match u8::from_str_radix(&hex_str, 16) {
                    Ok(v) => {
                        out.push(v);
                        i += 3;
                    }
                    Err(_) => {
                        out.push(bytes[i]);
                        i += 1;
                    }
                }
            }
            b => {
                out.push(b);
                i += 1;
            }
        }
    }
    String::from_utf8(out).unwrap_or_else(|_| s.to_string())
}

/// `Authorization: Apollo {appId}:{signature}` → signature part.
fn parse_signature(header: &str) -> Option<&str> {
    let trimmed = header.strip_prefix("Apollo ").unwrap_or(header);
    let (_app_id, signature) = trimmed.split_once(':')?;
    if signature.is_empty() {
        return None;
    }
    Some(signature)
}

/// Performs the `compute_signature` operation.
pub fn compute_signature(
    secret: &str,
    timestamp: i64,
    path_with_query: &str,
) -> anyhow::Result<String> {
    let mut mac = HmacSha1::new_from_slice(secret.as_bytes())
        .map_err(|e| anyhow::anyhow!("HMAC init failed: {}", e))?;
    let data = format!("{}\n{}", timestamp, path_with_query);
    mac.update(data.as_bytes());
    let result = mac.finalize().into_bytes();
    Ok(base64::engine::general_purpose::STANDARD.encode(result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signature_matches_apollo_vector() {
        // Test vector from the original apollo AccessKeyUtilTest.
        let secret = "someSecret";
        let timestamp = 1575018989200i64;
        let path_with_query = "/configs/someAppId/default/application?ip=10.0.0.1";
        let sig = compute_signature(secret, timestamp, path_with_query).unwrap();
        assert_eq!(sig, "WYjjyJFei6DYiaMlwZjew2O/Yqk=");
    }

    #[test]
    fn test_extract_app_id_from_paths() {
        assert_eq!(
            extract_app_id("/configs/myApp/default/ns", &test_req("/configs/myApp/default/ns", "")),
            Some("myApp".to_string())
        );
        assert_eq!(
            extract_app_id("/configfiles/json/a/c/n", &test_req("/configfiles/json/a/c/n", "")),
            Some("a".to_string())
        );
    }

    #[test]
    fn test_extract_app_id_blank_rejected() {
        // notifications without an appId anywhere → None (→ 400 InvalidAppId)
        assert_eq!(extract_app_id("/notifications/v2", &test_req("/notifications/v2", "cluster=default")), None);
    }

    #[test]
    fn test_extract_app_id_from_notifications_query() {
        assert_eq!(
            extract_app_id("/notifications/v2", &test_req("/notifications/v2", "appId=abc&cluster=x")),
            Some("abc".to_string())
        );
    }

    fn test_req(path: &str, query: &str) -> HttpRequest {
        use actix_web::test::TestRequest;
        let uri = if query.is_empty() {
            path.to_string()
        } else {
            format!("{}?{}", path, query)
        };
        TestRequest::get().uri(&uri).to_http_request()
    }
}
