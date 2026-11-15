//! Same-origin gateway for the packaged console UI.
//!
//! The UI uses relative paths (`/consul-api/...`, `/apollo-api/...`) so the
//! browser only ever talks to the console port. This module forwards those two
//! prefixes to the matching protocol-adapter HTTP servers, leaving every API
//! listening port and its existing routes untouched.
//!
//! Request bodies are buffered (bounded by the console `PayloadConfig`) while
//! responses are streamed, so Consul blocking queries and SSE endpoints are
//! neither buffered nor cut short by a total request timeout.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use actix_web::{
    HttpRequest, HttpResponse, HttpResponseBuilder,
    http::{
        StatusCode,
        header::{CONTENT_ENCODING, HeaderName, HeaderValue},
    },
    web,
};
use reqwest::{Client, Method, redirect::Policy};
use tracing::{info, warn};

use crate::model::common::Configuration;

/// Browser-facing prefix for the Consul adapter.
pub const CONSUL_PREFIX: &str = "/consul-api";
/// Browser-facing prefix for the Apollo adapter.
pub const APOLLO_PREFIX: &str = "/apollo-api";

/// Header returned when a sticky target was abandoned after a transport error.
/// The UI resets its Consul blocking-query index when it sees this header.
const FAILOVER_HEADER: &str = "x-batata-gateway-failover";

/// Headers that must not be copied across a proxy hop.
///
/// `host` is rebuilt by reqwest for the chosen upstream. `content-length` is
/// dropped because the response is re-chunked by `HttpResponse::streaming()`.
const HOP_BY_HOP: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "host",
    "content-length",
];

fn is_hop_by_hop(name: &str) -> bool {
    HOP_BY_HOP.iter().any(|h| name.eq_ignore_ascii_case(h))
}

/// One gateway route with an ordered target list and a sticky cursor.
struct GatewayTarget {
    targets: Vec<String>,
    current: AtomicUsize,
}

impl GatewayTarget {
    fn current_url(&self) -> Option<&str> {
        let len = self.targets.len();
        if len == 0 {
            return None;
        }
        self.targets
            .get(self.current.load(Ordering::SeqCst) % len)
            .map(String::as_str)
    }

    /// Move to the next target. Called only on transport errors, matching the
    /// sticky failover in `batata-client::http::HttpClient`. Successful calls
    /// never rotate, which keeps blocking-query subscriptions on one node.
    fn switch(&self) {
        self.current.fetch_add(1, Ordering::SeqCst);
    }
}

/// Shared gateway state injected into the console `App`.
pub struct ConsoleGatewayState {
    client: Client,
    consul: Option<GatewayTarget>,
    apollo: Option<GatewayTarget>,
}

impl ConsoleGatewayState {
    /// Whether the Consul route should be registered.
    pub fn consul_enabled(&self) -> bool {
        self.consul.is_some()
    }

    /// Whether the Apollo route should be registered.
    pub fn apollo_enabled(&self) -> bool {
        self.apollo.is_some()
    }
}

/// Resolve one route's upstreams at startup.
///
/// Explicit targets are required for separated-console deployments, where this
/// process never starts a plugin server and its local `enabled` is therefore
/// false. With no explicit target the route is only registered when this
/// process actually serves the plugin.
fn build_target(configuration: &Configuration, name: &str, prefix: &str) -> Option<GatewayTarget> {
    let explicit = configuration.console_gateway_targets(name);
    let targets = if explicit.is_empty() {
        if !configuration.plugin_enabled(name) {
            info!(
                "Console gateway {prefix} disabled: batata.plugin.{name}.enabled is false and no remote target is configured"
            );
            return None;
        }
        vec![format!(
            "http://127.0.0.1:{}",
            configuration.plugin_port(name)
        )]
    } else {
        info!(
            "Console gateway {prefix} uses {} explicit target(s)",
            explicit.len()
        );
        explicit
    };
    Some(GatewayTarget {
        targets,
        current: AtomicUsize::new(0),
    })
}

/// Build gateway state from the effective configuration, once at startup.
///
/// Returns `None` when the gateway is switched off or no route is usable; the
/// console then behaves exactly as it did before this module existed.
pub fn build_state(configuration: &Configuration) -> Option<web::Data<ConsoleGatewayState>> {
    if !configuration.console_gateway_enabled() {
        info!("Console same-origin gateway disabled by batata.console.gateway.enabled");
        return None;
    }

    let client = match Client::builder()
        .no_proxy()
        .redirect(Policy::none())
        .connect_timeout(Duration::from_secs(
            configuration.console_gateway_connect_timeout_secs(),
        ))
        .build()
    {
        Ok(client) => client,
        Err(err) => {
            warn!("Console gateway disabled: failed to build HTTP client: {err}");
            return None;
        }
    };

    let consul = build_target(configuration, "consul", CONSUL_PREFIX);
    let apollo = build_target(configuration, "apollo", APOLLO_PREFIX);
    if consul.is_none() && apollo.is_none() {
        return None;
    }

    Some(web::Data::new(ConsoleGatewayState {
        client,
        consul,
        apollo,
    }))
}

/// Convert actix (`http 0.2`) request headers to reqwest (`http 1`) headers.
fn outbound_headers(src: &actix_web::http::header::HeaderMap) -> reqwest::header::HeaderMap {
    let mut dst = reqwest::header::HeaderMap::new();
    for (name, value) in src.iter() {
        if is_hop_by_hop(name.as_str()) {
            continue;
        }
        if let (Ok(n), Ok(v)) = (
            reqwest::header::HeaderName::from_bytes(name.as_str().as_bytes()),
            reqwest::header::HeaderValue::from_bytes(value.as_bytes()),
        ) {
            dst.append(n, v);
        }
    }
    dst
}

/// Copy upstream response headers into the actix response builder.
///
/// Returns whether the upstream already declared a `Content-Encoding`.
fn copy_response_headers(
    src: &reqwest::header::HeaderMap,
    builder: &mut HttpResponseBuilder,
) -> bool {
    let mut encoded = false;
    for (name, value) in src.iter() {
        let name_str = name.as_str();
        if is_hop_by_hop(name_str) {
            continue;
        }
        if name_str.eq_ignore_ascii_case("content-encoding") {
            encoded = true;
        }
        if let (Ok(n), Ok(v)) = (
            HeaderName::from_bytes(name_str.as_bytes()),
            HeaderValue::from_bytes(value.as_bytes()),
        ) {
            builder.append_header((n, v));
        }
    }
    encoded
}

/// Build a streaming response that preserves upstream status and headers.
fn build_response(upstream: reqwest::Response) -> HttpResponse {
    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut builder = HttpResponse::build(status);
    let encoded = copy_response_headers(upstream.headers(), &mut builder);
    if !encoded {
        // `Compress` only compresses when no Content-Encoding is present
        // (actix-web middleware/compress.rs). Declaring identity avoids double
        // compression and keeps blocking-query/SSE streams unbuffered.
        builder.insert_header((CONTENT_ENCODING, HeaderValue::from_static("identity")));
    }
    builder.streaming(upstream.bytes_stream())
}

async fn forward(
    req: &HttpRequest,
    body: web::Bytes,
    state: &ConsoleGatewayState,
    prefix: &str,
    target: Option<&GatewayTarget>,
) -> HttpResponse {
    let Some(target) = target else {
        return HttpResponse::NotFound().finish();
    };
    let Some(base) = target.current_url() else {
        return HttpResponse::BadGateway().body("gateway has no upstream target");
    };

    let stripped = req.path().strip_prefix(prefix).unwrap_or(req.path());
    let stripped = if stripped.is_empty() { "/" } else { stripped };
    let mut url = format!("{}{}", base.trim_end_matches('/'), stripped);
    if !req.query_string().is_empty() {
        url.push('?');
        url.push_str(req.query_string());
    }

    let method = match Method::from_bytes(req.method().as_str().as_bytes()) {
        Ok(method) => method,
        Err(_) => return HttpResponse::BadRequest().finish(),
    };

    let result = state
        .client
        .request(method, &url)
        .headers(outbound_headers(req.headers()))
        .body(body)
        .send()
        .await;

    match result {
        Ok(upstream) => build_response(upstream),
        Err(err) => {
            target.switch();
            warn!("Console gateway {prefix} upstream error, switching target: {err}");
            HttpResponse::BadGateway()
                .insert_header((
                    HeaderName::from_static(FAILOVER_HEADER),
                    HeaderValue::from_static("1"),
                ))
                .body(format!("gateway upstream unavailable: {err}"))
        }
    }
}

/// Handler for `/consul-api/**`.
pub async fn forward_consul(
    req: HttpRequest,
    body: web::Bytes,
    state: web::Data<ConsoleGatewayState>,
) -> HttpResponse {
    let state = state.get_ref();
    forward(&req, body, state, CONSUL_PREFIX, state.consul.as_ref()).await
}

/// Handler for `/apollo-api/**`.
pub async fn forward_apollo(
    req: HttpRequest,
    body: web::Bytes,
    state: web::Data<ConsoleGatewayState>,
) -> HttpResponse {
    let state = state.get_ref();
    forward(&req, body, state, APOLLO_PREFIX, state.apollo.as_ref()).await
}

/// Whether a SPA-fallback path belongs to a gateway route.
///
/// Used to fail closed with 404 when the gateway is disabled, instead of
/// answering an API request with the SPA shell (HTML 200). Matching on a
/// path-segment boundary keeps unrelated paths such as `consul-api-extra`
/// on their original code path.
pub fn is_gateway_path(path: &str) -> bool {
    ["consul-api", "apollo-api"].iter().any(|prefix| {
        path == *prefix
            || path
                .strip_prefix(*prefix)
                .is_some_and(|rest| rest.starts_with('/'))
    })
}

#[cfg(test)]
mod tests {
    use std::io::{BufRead, BufReader, Read, Write};
    use std::net::TcpListener;

    use super::*;
    use actix_web::{
        App,
        http::header::{HeaderMap as ActixHeaderMap, HeaderValue as ActixHeaderValue},
    };
    // Import under an alias: bringing `actix_web::test` into scope under its
    // own name shadows the built-in `#[test]` attribute in this module.
    use actix_web::test as actix_test;
    use config::Config;

    fn configuration(overrides: &[(&str, config::Value)]) -> Configuration {
        let mut builder = Config::builder();
        for (key, value) in overrides {
            builder = builder.set_override(*key, value.clone()).unwrap();
        }
        let mut cfg = Configuration::default();
        cfg.config = builder.build().unwrap();
        cfg
    }

    fn bool_value(v: bool) -> config::Value {
        v.into()
    }

    fn int_value(v: i64) -> config::Value {
        v.into()
    }

    fn str_value(v: &str) -> config::Value {
        v.to_string().into()
    }

    #[test]
    fn hop_by_hop_headers_are_case_insensitive() {
        assert!(is_hop_by_hop("Connection"));
        assert!(is_hop_by_hop("HOST"));
        assert!(is_hop_by_hop("Content-Length"));
        assert!(is_hop_by_hop("Transfer-Encoding"));
        assert!(!is_hop_by_hop("X-Consul-Token"));
        assert!(!is_hop_by_hop("X-Consul-Index"));
        assert!(!is_hop_by_hop("Content-Type"));
    }

    #[test]
    fn outbound_headers_drop_hop_by_hop_but_keep_plugin_tokens() {
        let mut src = ActixHeaderMap::new();
        src.append(
            actix_web::http::header::HOST,
            ActixHeaderValue::from_static("localhost:8081"),
        );
        src.append(
            actix_web::http::header::CONTENT_LENGTH,
            ActixHeaderValue::from_static("12"),
        );
        src.append(
            actix_web::http::header::CONNECTION,
            ActixHeaderValue::from_static("keep-alive"),
        );
        src.append(
            HeaderName::from_static("x-consul-token"),
            ActixHeaderValue::from_static("secret"),
        );
        src.append(
            actix_web::http::header::CONTENT_TYPE,
            ActixHeaderValue::from_static("application/json"),
        );

        let dst = outbound_headers(&src);
        assert!(!dst.contains_key("host"));
        assert!(!dst.contains_key("content-length"));
        assert!(!dst.contains_key("connection"));
        assert_eq!(dst.get("x-consul-token").unwrap(), "secret");
        assert_eq!(dst.get("content-type").unwrap(), "application/json");
    }

    #[test]
    fn sticky_target_rotates_only_on_switch() {
        let target = GatewayTarget {
            targets: vec!["http://a:8500".into(), "http://b:8510".into()],
            current: AtomicUsize::new(0),
        };
        assert_eq!(target.current_url(), Some("http://a:8500"));
        assert_eq!(target.current_url(), Some("http://a:8500"));
        target.switch();
        assert_eq!(target.current_url(), Some("http://b:8510"));
        target.switch();
        assert_eq!(target.current_url(), Some("http://a:8500"));
    }

    #[test]
    fn empty_target_list_yields_no_url() {
        let target = GatewayTarget {
            targets: Vec::new(),
            current: AtomicUsize::new(0),
        };
        assert_eq!(target.current_url(), None);
    }

    #[test]
    fn disabled_plugin_without_remote_target_disables_route() {
        let cfg = configuration(&[("batata.plugin.consul.enabled", bool_value(false))]);
        assert!(build_target(&cfg, "consul", CONSUL_PREFIX).is_none());
    }

    #[test]
    fn enabled_plugin_without_remote_target_uses_own_port() {
        let cfg = configuration(&[
            ("batata.plugin.consul.enabled", bool_value(true)),
            ("batata.plugin.consul.port", int_value(8510)),
        ]);
        let target = build_target(&cfg, "consul", CONSUL_PREFIX).unwrap();
        assert_eq!(target.current_url(), Some("http://127.0.0.1:8510"));
    }

    #[test]
    fn explicit_remote_target_overrides_disabled_local_plugin() {
        let cfg = configuration(&[
            ("batata.plugin.apollo.enabled", bool_value(false)),
            (
                "batata.console.gateway.apollo.target",
                str_value("http://10.0.0.1:8080, http://10.0.0.2:18090"),
            ),
        ]);
        let target = build_target(&cfg, "apollo", APOLLO_PREFIX).unwrap();
        assert_eq!(target.targets.len(), 2);
        assert_eq!(target.current_url(), Some("http://10.0.0.1:8080"));
        target.switch();
        assert_eq!(target.current_url(), Some("http://10.0.0.2:18090"));
    }

    #[test]
    fn gateway_switch_off_returns_no_state() {
        let cfg = configuration(&[
            ("batata.console.gateway.enabled", bool_value(false)),
            ("batata.plugin.consul.enabled", bool_value(true)),
        ]);
        assert!(build_state(&cfg).is_none());
    }

    #[test]
    fn gateway_state_is_none_when_both_plugins_are_off() {
        let cfg = configuration(&[
            ("batata.plugin.consul.enabled", bool_value(false)),
            ("batata.plugin.apollo.enabled", bool_value(false)),
        ]);
        assert!(build_state(&cfg).is_none());
    }

    #[test]
    fn gateway_state_registers_only_enabled_route() {
        let cfg = configuration(&[("batata.plugin.consul.enabled", bool_value(true))]);
        let state = build_state(&cfg).unwrap();
        assert!(state.consul_enabled());
        assert!(!state.apollo_enabled());
    }

    #[test]
    fn response_headers_drop_hop_by_hop_and_report_encoding() {
        let mut src = reqwest::header::HeaderMap::new();
        src.append(
            reqwest::header::CONTENT_LENGTH,
            reqwest::header::HeaderValue::from_static("7"),
        );
        src.append(
            reqwest::header::CONTENT_ENCODING,
            reqwest::header::HeaderValue::from_static("gzip"),
        );
        src.append(
            reqwest::header::HeaderName::from_static("x-consul-index"),
            reqwest::header::HeaderValue::from_static("42"),
        );
        let mut builder = HttpResponse::build(StatusCode::OK);
        assert!(copy_response_headers(&src, &mut builder));
        let response = builder.finish();
        assert!(!response.headers().contains_key("content-length"));
        assert_eq!(response.headers().get("content-encoding").unwrap(), "gzip");
        assert_eq!(response.headers().get("x-consul-index").unwrap(), "42");
    }

    #[test]
    fn response_headers_report_missing_encoding() {
        let src = reqwest::header::HeaderMap::new();
        let mut builder = HttpResponse::build(StatusCode::OK);
        assert!(!copy_response_headers(&src, &mut builder));
    }

    #[test]
    fn gateway_path_matches_only_segment_boundaries() {
        assert!(is_gateway_path("consul-api"));
        assert!(is_gateway_path("consul-api/v1/kv"));
        assert!(is_gateway_path("apollo-api"));
        assert!(is_gateway_path("apollo-api/openapi/v1/apps"));
        // Unrelated paths must keep their original SPA-fallback behaviour.
        assert!(!is_gateway_path("consul-api-extra"));
        assert!(!is_gateway_path("apollo-api-docs"));
        assert!(!is_gateway_path("v3/console/configs"));
        assert!(!is_gateway_path(""));
    }

    // --- Real HTTP forwarding -------------------------------------------------
    //
    // The tests above exercise pure helpers. These put a real TCP listening
    // socket behind the gateway, so a green result proves an HTTP request
    // actually reached an upstream with the expected method, path, query,
    // headers and body.

    async fn not_found() -> HttpResponse {
        HttpResponse::NotFound().finish()
    }

    async fn pong() -> HttpResponse {
        HttpResponse::Ok().body("pong")
    }

    /// Minimal HTTP/1.1 upstream that echoes the received request as JSON.
    fn spawn_echo_upstream(status: u16, extra_headers: &[(&str, &str)]) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let extras: Vec<(String, String)> = extra_headers
            .iter()
            .map(|(name, value)| (name.to_string(), value.to_string()))
            .collect();
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { continue };
                let extras = extras.clone();
                std::thread::spawn(move || {
                    let mut reader = BufReader::new(stream.try_clone().unwrap());
                    let mut request_line = String::new();
                    reader.read_line(&mut request_line).unwrap();
                    let mut headers = Vec::<(String, String)>::new();
                    loop {
                        let mut line = String::new();
                        if reader.read_line(&mut line).unwrap() == 0 || line == "\r\n" {
                            break;
                        }
                        if let Some((name, value)) = line.split_once(':') {
                            headers
                                .push((name.trim().to_ascii_lowercase(), value.trim().to_string()));
                        }
                    }
                    let len = headers
                        .iter()
                        .find(|(name, _)| name == "content-length")
                        .and_then(|(_, value)| value.parse::<usize>().ok())
                        .unwrap_or(0);
                    let mut body = vec![0u8; len];
                    if len > 0 {
                        reader.read_exact(&mut body).unwrap();
                    }
                    let parts: Vec<&str> = request_line.split_whitespace().collect();
                    let request_target = parts.get(1).copied().unwrap_or("");
                    let (path, query) = request_target
                        .split_once('?')
                        .unwrap_or((request_target, ""));
                    let payload = serde_json::json!({
                        "method": parts.first().copied().unwrap_or(""),
                        "path": path,
                        "query": query,
                        "headers": headers
                            .iter()
                            .map(|(name, value)| {
                                serde_json::json!({"name": name, "value": value})
                            })
                            .collect::<Vec<_>>(),
                        "body": String::from_utf8_lossy(&body),
                    })
                    .to_string();
                    let mut response = format!(
                        "HTTP/1.1 {status} OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n",
                        payload.len()
                    );
                    for (name, value) in &extras {
                        response.push_str(&format!("{name}: {value}\r\n"));
                    }
                    response.push_str("\r\n");
                    response.push_str(&payload);
                    stream.write_all(response.as_bytes()).unwrap();
                });
            }
        });
        format!("http://{addr}")
    }

    fn upstream_header<'a>(json: &'a serde_json::Value, name: &str) -> Option<&'a str> {
        json["headers"]
            .as_array()?
            .iter()
            .find(|entry| entry["name"] == name)
            .and_then(|entry| entry["value"].as_str())
    }

    fn target(urls: Vec<String>) -> GatewayTarget {
        GatewayTarget {
            targets: urls,
            current: AtomicUsize::new(0),
        }
    }

    fn gateway_state(consul: Option<Vec<String>>) -> web::Data<ConsoleGatewayState> {
        let client = Client::builder()
            .no_proxy()
            .redirect(Policy::none())
            .build()
            .unwrap();
        web::Data::new(ConsoleGatewayState {
            client,
            consul: consul.map(target),
            apollo: None,
        })
    }

    #[actix_web::test]
    async fn forwards_get_with_stripped_prefix_query_and_plugin_token() {
        let upstream = spawn_echo_upstream(200, &[]);
        let state = gateway_state(Some(vec![upstream]));
        let app = actix_test::init_service(
            App::new()
                .app_data(state)
                .service(web::scope(CONSUL_PREFIX).default_service(web::to(forward_consul)))
                .default_service(web::to(not_found)),
        )
        .await;

        let req = actix_test::TestRequest::get()
            .uri("/consul-api/v1/kv/config?wait=5m&index=42")
            .insert_header(("x-consul-token", "secret-token"))
            .insert_header(("host", "localhost:8081"))
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let json: serde_json::Value =
            serde_json::from_slice(&actix_test::read_body(resp).await).unwrap();
        assert_eq!(json["method"], "GET");
        assert_eq!(json["path"], "/v1/kv/config");
        assert_eq!(json["query"], "wait=5m&index=42");
        assert_eq!(
            upstream_header(&json, "x-consul-token"),
            Some("secret-token")
        );
        // Host is rebuilt for the upstream, never copied from the browser.
        assert_ne!(upstream_header(&json, "host"), Some("localhost:8081"));
        assert!(upstream_header(&json, "host").is_some());
    }

    #[actix_web::test]
    async fn forwards_post_method_body_and_content_type() {
        let upstream = spawn_echo_upstream(200, &[]);
        let state = gateway_state(Some(vec![upstream]));
        let app = actix_test::init_service(
            App::new()
                .app_data(state)
                .service(web::scope(CONSUL_PREFIX).default_service(web::to(forward_consul)))
                .default_service(web::to(not_found)),
        )
        .await;

        let req = actix_test::TestRequest::post()
            .uri("/consul-api/v1/kv/foo?dc=dc1")
            .insert_header(("content-type", "application/x-www-form-urlencoded"))
            .set_payload("a=1&b=2")
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        let json: serde_json::Value =
            serde_json::from_slice(&actix_test::read_body(resp).await).unwrap();
        assert_eq!(json["method"], "POST");
        assert_eq!(json["path"], "/v1/kv/foo");
        assert_eq!(json["query"], "dc=dc1");
        assert_eq!(json["body"], "a=1&b=2");
        assert_eq!(
            upstream_header(&json, "content-type"),
            Some("application/x-www-form-urlencoded")
        );
    }

    #[actix_web::test]
    async fn preserves_upstream_status_headers_and_skips_double_compression() {
        let upstream = spawn_echo_upstream(201, &[("x-consul-index", "42")]);
        let state = gateway_state(Some(vec![upstream]));
        let app = actix_test::init_service(
            App::new()
                .app_data(state)
                .service(web::scope(CONSUL_PREFIX).default_service(web::to(forward_consul)))
                .default_service(web::to(not_found)),
        )
        .await;

        let req = actix_test::TestRequest::get()
            .uri("/consul-api/v1/health/stream")
            .to_request();
        let resp = actix_test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::CREATED);
        assert_eq!(resp.headers().get("x-consul-index").unwrap(), "42");
        assert_eq!(resp.headers().get(CONTENT_ENCODING).unwrap(), "identity");
        let json: serde_json::Value =
            serde_json::from_slice(&actix_test::read_body(resp).await).unwrap();
        assert_eq!(json["path"], "/v1/health/stream");
    }

    #[actix_web::test]
    async fn transport_failure_switches_target_and_stays_sticky() {
        // Port 1 is not listening, so the first target always fails at connect.
        let dead = "http://127.0.0.1:1".to_string();
        let live = spawn_echo_upstream(200, &[]);
        let state = gateway_state(Some(vec![dead, live]));
        let app = actix_test::init_service(
            App::new()
                .app_data(state)
                .service(web::scope(CONSUL_PREFIX).default_service(web::to(forward_consul)))
                .default_service(web::to(not_found)),
        )
        .await;

        let first = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/consul-api/v1/status/leader")
                .to_request(),
        )
        .await;
        assert_eq!(first.status(), StatusCode::BAD_GATEWAY);
        assert_eq!(first.headers().get(FAILOVER_HEADER).unwrap(), "1");

        for _ in 0..2 {
            let resp = actix_test::call_service(
                &app,
                actix_test::TestRequest::get()
                    .uri("/consul-api/v1/status/leader")
                    .to_request(),
            )
            .await;
            assert_eq!(resp.status(), StatusCode::OK);
            let json: serde_json::Value =
                serde_json::from_slice(&actix_test::read_body(resp).await).unwrap();
            assert_eq!(json["path"], "/v1/status/leader");
        }
    }

    #[actix_web::test]
    async fn gateway_scope_must_precede_the_empty_console_scope() {
        // `startup/http.rs` mounts console routes under `web::scope("")` when
        // `batata.console.context_path` is empty (the default). That scope is
        // a catch-all and actix never falls back to a sibling service once it
        // matches, so registering the gateway afterwards silently turns every
        // `/consul-api/**` request into a 404 even when the plugin is enabled.
        let upstream = spawn_echo_upstream(200, &[]);
        let state = gateway_state(Some(vec![upstream]));

        let correct = actix_test::init_service(
            App::new()
                .app_data(state.clone())
                .service(web::scope(CONSUL_PREFIX).default_service(web::to(forward_consul)))
                .service(web::scope("").service(web::resource("/v3/console/ping").to(pong)))
                .default_service(web::to(not_found)),
        )
        .await;
        let ok = actix_test::call_service(
            &correct,
            actix_test::TestRequest::get()
                .uri("/consul-api/v1/status/leader")
                .to_request(),
        )
        .await;
        assert_eq!(ok.status(), StatusCode::OK);

        let wrong_order = actix_test::init_service(
            App::new()
                .app_data(state)
                .service(web::scope("").service(web::resource("/v3/console/ping").to(pong)))
                .service(web::scope(CONSUL_PREFIX).default_service(web::to(forward_consul)))
                .default_service(web::to(not_found)),
        )
        .await;
        let swallowed = actix_test::call_service(
            &wrong_order,
            actix_test::TestRequest::get()
                .uri("/consul-api/v1/status/leader")
                .to_request(),
        )
        .await;
        assert_eq!(swallowed.status(), StatusCode::NOT_FOUND);
    }

    #[actix_web::test]
    async fn gateway_scope_does_not_shadow_unrelated_console_routes() {
        let upstream = spawn_echo_upstream(200, &[]);
        let state = gateway_state(Some(vec![upstream]));
        let app = actix_test::init_service(
            App::new()
                .app_data(state)
                .service(web::scope(CONSUL_PREFIX).default_service(web::to(forward_consul)))
                .service(web::resource("/v3/console/ping").to(pong))
                .default_service(web::to(not_found)),
        )
        .await;

        let resp = actix_test::call_service(
            &app,
            actix_test::TestRequest::get()
                .uri("/v3/console/ping")
                .to_request(),
        )
        .await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            actix_test::read_body(resp).await,
            actix_web::web::Bytes::from_static(b"pong")
        );
    }
}
