//! Shared parameter resolution for AI ADMIN write handlers.
//!
//! Nacos Spring controllers bind `@RequestParam` / form objects from **both**
//! the URL query string and an `application/x-www-form-urlencoded` body. The
//! Nacos 3.3.0 GA maintainer client sends every POST/PUT parameter in the
//! form body: `ClientHttpProxy.executeSync` calls `postForm` / `putForm`, and
//! `AgentMaintainerServiceImpl` / `McpMaintainerServiceImpl` fill a
//! `setParamValue` map. Batata's Rust SDK, the console and the existing route
//! tests use the query string (sometimes with a JSON object body).
//!
//! Historically the AI admin handlers extracted `web::Query<T>` only, so a GA
//! Java request arrived with an empty form object and returned 400. This
//! module collects all three encodings into one flat string map and
//! deserializes it into any `DeserializeOwned` DTO, so either client generation
//! works without changing the wire contract.
//!
//! Deliberately not handled here:
//! - `multipart/form-data` (ZIP uploads use `actix_multipart::Multipart` and
//!   already keep identity fields in the query string);
//! - nested `AgentCard` JSON bodies, which `api::agent::resolve_agent_write`
//!   decodes separately from the flat parameter map (both wires are accepted;
//!   the remaining ARD model divergence is tracked as `F-NAC-AISRV-001`).

use std::collections::{BTreeMap, HashMap};
use std::future::Future;
use std::ops::Deref;
use std::pin::Pin;

use actix_web::dev::Payload;
use actix_web::{FromRequest, HttpRequest, web};
use serde::de::DeserializeOwned;
use serde_json::Value;

/// Actix extractor equivalent of [`AdminParams`].
///
/// Handlers keep using `query.into_inner()` and `query.some_field` exactly as
/// with `web::Query<T>`; the only change is the extractor type. It reads the
/// request body once, so it must not be combined with another body-consuming
/// extractor (`web::Bytes`, `web::Json`, `Multipart`) in the same handler.
pub(crate) struct AdminForm<T>(T);

impl<T> AdminForm<T> {
    pub(crate) fn into_inner(self) -> T {
        self.0
    }
}

impl<T> Deref for AdminForm<T> {
    type Target = T;

    fn deref(&self) -> &T {
        &self.0
    }
}

impl<T: DeserializeOwned + 'static> FromRequest for AdminForm<T> {
    type Error = actix_web::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self, Self::Error>>>>;

    fn from_request(req: &HttpRequest, payload: &mut Payload) -> Self::Future {
        let req = req.clone();
        let body = web::Bytes::from_request(&req, payload);
        Box::pin(async move {
            let body = body.await?;
            let value = AdminParams::from_request(&req, &body)
                .decode::<T>()
                .map_err(actix_web::error::ErrorBadRequest)?;
            Ok(AdminForm(value))
        })
    }
}

/// Flat string parameters collected from one AI ADMIN request.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AdminParams(BTreeMap<String, String>);

impl AdminParams {
    /// Collect parameters from the request body first, then let the URL query
    /// string win on duplicate keys (same precedence as
    /// `batata-naming`'s `merge_form`).
    ///
    /// Malformed or unrecognized bodies are ignored rather than rejected: some
    /// callers legitimately send an empty body with all parameters in the
    /// query, and the handler's own validation produces the structured 400.
    pub(crate) fn from_request(req: &HttpRequest, body: &[u8]) -> Self {
        let mut values = BTreeMap::new();
        let content_type = content_type(req);
        let body = body.trim_ascii();

        if !content_type.contains("multipart/form-data")
            && !body.is_empty()
            && let Some(pairs) = body_pairs(&content_type, body)
        {
            for (key, value) in pairs {
                values.insert(key, value);
            }
        }

        if let Ok(pairs) = serde_urlencoded::from_str::<Vec<(String, String)>>(req.query_string()) {
            for (key, value) in pairs {
                values.insert(key, value);
            }
        }

        Self(values)
    }

    /// Look up one raw parameter, e.g. a JSON-encoded `labels` value.
    pub(crate) fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    /// Deserialize the merged parameters into a DTO.
    ///
    /// Encoding the map back through `serde_urlencoded` keeps `Option<bool>`
    /// (`enabled=false`), `Option<u64>` (`pageNo=2`) and string-valued JSON
    /// fields working exactly as they do with `web::Query<T>`.
    pub(crate) fn decode<T: DeserializeOwned>(&self) -> Result<T, String> {
        let pairs: Vec<(&str, &str)> = self
            .0
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect();
        let encoded = serde_urlencoded::to_string(&pairs)
            .map_err(|e| format!("Invalid AI admin parameters: {e}"))?;
        serde_urlencoded::from_str::<T>(&encoded)
            .map_err(|e| format!("Invalid AI admin parameters: {e}"))
    }

    /// Decode the `labels` map regardless of how it arrived.
    ///
    /// The GA Java client serializes the map into a single `labels` form
    /// parameter (`putJsonIfNotNull`), while the console posts either
    /// `{"labels": {...}}` or a bare JSON map.
    pub(crate) fn labels(&self, body: &[u8]) -> Result<HashMap<String, String>, String> {
        if let Some(raw) = self.get("labels").map(str::trim).filter(|s| !s.is_empty()) {
            return serde_json::from_str(raw).map_err(|e| format!("Invalid labels: {e}"));
        }
        let body = String::from_utf8_lossy(body);
        let body = body.trim();
        if body.is_empty() {
            return Ok(HashMap::new());
        }
        if let Ok(request) = serde_json::from_str::<LabelsRequest>(body) {
            return Ok(request.labels);
        }
        serde_json::from_str(body).map_err(|e| format!("Invalid labels: {e}"))
    }
}

/// `{"labels": {...}}` JSON body shape.
#[derive(serde::Deserialize)]
struct LabelsRequest {
    labels: HashMap<String, String>,
}

/// Lower-cased `Content-Type` header.
fn content_type(req: &HttpRequest) -> String {
    use actix_web::http::header::CONTENT_TYPE;
    req.headers()
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase()
}

/// Flatten a JSON object or an urlencoded form into `(key, value)` pairs.
fn body_pairs(content_type: &str, body: &[u8]) -> Option<Vec<(String, String)>> {
    if content_type.contains("json") || body.first() == Some(&b'{') {
        let value: Value = serde_json::from_slice(body).ok()?;
        let Value::Object(object) = value else {
            return None;
        };
        return Some(
            object
                .into_iter()
                .filter_map(|(key, value)| scalar(&value).map(|scalar| (key, scalar)))
                .collect(),
        );
    }
    serde_urlencoded::from_bytes::<Vec<(String, String)>>(body).ok()
}

/// A JSON value usable as a flat form parameter. Objects and arrays become
/// their JSON text, which is how the GA client sends `labels` and how the
/// console sends nested label maps.
fn scalar(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        Value::String(text) => Some(text.clone()),
        Value::Bool(flag) => Some(flag.to_string()),
        Value::Number(number) => Some(number.to_string()),
        _ => Some(value.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::test::TestRequest;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct Sample {
        namespace_id: Option<String>,
        agent_name: Option<String>,
        enabled: Option<bool>,
        page_no: Option<u64>,
        labels: Option<String>,
    }

    fn request(uri: &str, content_type: Option<&str>) -> HttpRequest {
        let mut builder = TestRequest::with_uri(uri);
        if let Some(content_type) = content_type {
            builder = builder.insert_header(("content-type", content_type));
        }
        builder.to_http_request()
    }

    fn resolve<T: DeserializeOwned>(req: &HttpRequest, body: &[u8]) -> T {
        AdminParams::from_request(req, body)
            .decode::<T>()
            .expect("parameters must decode")
    }

    #[actix_web::test]
    async fn query_only_request_is_accepted() {
        let req = request(
            "/v3/admin/ai/agents/submit?namespaceId=ns-1&agentName=my-agent&enabled=true&pageNo=3",
            None,
        );
        assert_eq!(
            resolve::<Sample>(&req, b""),
            Sample {
                namespace_id: Some("ns-1".to_string()),
                agent_name: Some("my-agent".to_string()),
                enabled: Some(true),
                page_no: Some(3),
                labels: None,
            }
        );
    }

    #[actix_web::test]
    async fn ga_urlencoded_form_body_is_accepted() {
        // This is exactly `ClientHttpProxy.postForm` + `setParamValue`.
        let req = request(
            "/v3/admin/ai/agents/submit",
            Some("application/x-www-form-urlencoded"),
        );
        assert_eq!(
            resolve::<Sample>(
                &req,
                b"namespaceId=ns-1&agentName=my-agent&enabled=false&pageNo=2"
            ),
            Sample {
                namespace_id: Some("ns-1".to_string()),
                agent_name: Some("my-agent".to_string()),
                enabled: Some(false),
                page_no: Some(2),
                labels: None,
            }
        );
    }

    #[actix_web::test]
    async fn json_object_body_is_accepted_and_nested_values_flatten() {
        let req = request("/v3/admin/ai/agents/labels", Some("application/json"));
        let params = AdminParams::from_request(
            &req,
            br#"{"agentName":"my-agent","enabled":true,"labels":{"stable":"1.0"}}"#,
        );
        assert_eq!(params.get("labels"), Some(r#"{"stable":"1.0"}"#));
        assert_eq!(
            params.decode::<Sample>().unwrap(),
            Sample {
                namespace_id: None,
                agent_name: Some("my-agent".to_string()),
                enabled: Some(true),
                page_no: None,
                labels: Some(r#"{"stable":"1.0"}"#.to_string()),
            }
        );
    }

    #[actix_web::test]
    async fn query_wins_and_body_fills_the_gaps() {
        let req = request(
            "/v3/admin/ai/agents/submit?agentName=from-query",
            Some("application/x-www-form-urlencoded"),
        );
        // `agentName` is present in BOTH encodings: this is the precedence
        // assertion. The other body-only keys prove the merge still happens.
        assert_eq!(
            resolve::<Sample>(&req, b"namespaceId=from-body&pageNo=4&agentName=from-body"),
            Sample {
                namespace_id: Some("from-body".to_string()),
                agent_name: Some("from-query".to_string()),
                enabled: None,
                page_no: Some(4),
                labels: None,
            }
        );
    }

    #[actix_web::test]
    async fn empty_request_yields_an_empty_form() {
        let req = request("/v3/admin/ai/agents/submit", None);
        let params = AdminParams::from_request(&req, b"");
        assert_eq!(params, AdminParams::default());
        assert_eq!(
            params.decode::<Sample>().unwrap(),
            Sample {
                namespace_id: None,
                agent_name: None,
                enabled: None,
                page_no: None,
                labels: None,
            }
        );
    }

    #[actix_web::test]
    async fn labels_arrive_as_a_form_parameter_or_a_json_body() {
        let form_req = request(
            "/v3/admin/ai/agents/labels",
            Some("application/x-www-form-urlencoded"),
        );
        let form_body = br#"agentName=my-agent&labels=%7B%22stable%22%3A%221.0%22%7D"#;
        let form = AdminParams::from_request(&form_req, form_body);
        assert_eq!(form.get("agentName"), Some("my-agent"));
        assert_eq!(
            form.labels(form_body)
                .unwrap()
                .get("stable")
                .map(String::as_str),
            Some("1.0")
        );

        let json_req = request("/v3/admin/ai/agents/labels", Some("application/json"));
        let json_body = br#"{"agentName":"my-agent","labels":{"stable":"1.0"}}"#;
        let json = AdminParams::from_request(&json_req, json_body);
        assert_eq!(
            json.labels(json_body)
                .unwrap()
                .get("stable")
                .map(String::as_str),
            Some("1.0")
        );

        let bare = AdminParams::from_request(&form_req, b"");
        assert_eq!(
            bare.labels(br#"{"stable":"1.0"}"#)
                .unwrap()
                .get("stable")
                .map(String::as_str),
            Some("1.0")
        );
    }

    #[actix_web::test]
    async fn multipart_body_is_not_mistaken_for_urlencoded_params() {
        let req = request(
            "/v3/admin/ai/skills/upload?namespaceId=ns-1",
            Some("multipart/form-data; boundary=X"),
        );
        let params = AdminParams::from_request(&req, b"PK\x03\x04-binary-zip");
        assert_eq!(params.get("namespaceId"), Some("ns-1"));
        assert_eq!(params.0.len(), 1);

        // A multipart payload that looks like an urlencoded pair must not be
        // able to inject or override admin parameters.
        let zip_part = b"--X\r\nContent-Disposition: form-data; name=\"file\"\r\n\r\nnamespaceId=attacker\r\n--X--\r\n";
        let params = AdminParams::from_request(&req, zip_part);
        assert_eq!(params.get("namespaceId"), Some("ns-1"));
        assert_eq!(params.0.len(), 1);
    }

    #[actix_web::test]
    async fn malformed_body_does_not_mask_the_query() {
        let req = request(
            "/v3/admin/ai/agents/submit?agentName=my-agent",
            Some("application/x-www-form-urlencoded"),
        );
        assert_eq!(
            resolve::<Sample>(&req, b"not a valid payload \xff"),
            Sample {
                namespace_id: None,
                agent_name: Some("my-agent".to_string()),
                enabled: None,
                page_no: None,
                labels: None,
            }
        );
    }
}
