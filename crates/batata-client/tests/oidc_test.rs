//! Coverage for `F-NAC-CSDK-005`: GA OIDC Client Credentials authentication.
//!
//! Wire assertions use a `MockServer` for OIDC Discovery, the token endpoint
//! and a real `BatataHttpClient` request, so both the provider contract and
//! its production call sites are exercised.

use std::collections::HashMap;

use batata_client::auth::ClientAuthService;
use batata_client::oidc::{OIDC_ACCESS_TOKEN_PARAM, OIDC_AUTHORIZATION_HEADER, OIDC_BEARER_PREFIX};
use batata_client::{
    BatataHttpClient, ClientConfig, OidcAuthProvider, OidcConfig, RequestResource,
};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn token_response(token: &str, expires_in: Option<u64>) -> ResponseTemplate {
    let mut body = serde_json::json!({ "access_token": token, "token_type": "Bearer" });
    if let Some(ttl) = expires_in {
        body["expires_in"] = serde_json::json!(ttl);
    }
    ResponseTemplate::new(200).set_body_json(body)
}

fn header_value(req: &wiremock::Request, name: &str) -> Option<String> {
    req.headers
        .iter()
        .find(|(key, _)| key.as_str().eq_ignore_ascii_case(name))
        .and_then(|(_, value)| value.to_str().ok().map(str::to_string))
}

fn form_body(req: &wiremock::Request) -> HashMap<String, String> {
    serde_urlencoded::from_bytes(&req.body).expect("token request must be urlencoded form")
}

async fn mount_discovery(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "issuer": server.uri(),
            "token_endpoint": format!("{}/token", server.uri()),
        })))
        .mount(server)
        .await;
}

/// Discovery → token POST → GA dual identity headers; the cached token is
/// reused on the next login cycle.
#[tokio::test]
async fn discovery_then_client_credentials_flow() {
    let server = MockServer::start().await;
    mount_discovery(&server).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(token_response("tok-ga", Some(3600)))
        .mount(&server)
        .await;

    let provider =
        OidcAuthProvider::new(OidcConfig::new("batata-sdk", "s3cret").with_issuer(&server.uri()))
            .unwrap();

    assert!(provider.is_enabled());
    assert_eq!(provider.name(), "oidc");
    assert!(provider.login(&[]).await);
    assert_eq!(provider.token_ttl_secs(), 3600);
    assert_eq!(provider.access_token().as_deref(), Some("tok-ga"));

    let identity = provider.get_identity_context(&RequestResource::default());
    assert_eq!(
        identity.headers.get(OIDC_AUTHORIZATION_HEADER).unwrap(),
        &format!("{OIDC_BEARER_PREFIX}tok-ga")
    );
    assert_eq!(
        identity.headers.get(OIDC_ACCESS_TOKEN_PARAM).unwrap(),
        "tok-ga"
    );

    // A second login cycle must not re-fetch a still-valid token.
    assert!(provider.login(&[]).await);

    let requests = server.received_requests().await.unwrap();
    let discovery = requests
        .iter()
        .find(|r| r.url.path() == "/.well-known/openid-configuration")
        .expect("Discovery GET was not received");
    assert_eq!(
        header_value(discovery, "accept").as_deref(),
        Some("application/json")
    );
    assert_eq!(
        requests
            .iter()
            .filter(|r| r.url.path() == "/.well-known/openid-configuration")
            .count(),
        1
    );

    let token_calls: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path() == "/token")
        .collect();
    assert_eq!(token_calls.len(), 1, "token must be cached until refresh");
    let form = form_body(token_calls[0]);
    assert_eq!(form.get("grant_type").unwrap(), "client_credentials");
    assert_eq!(form.get("client_id").unwrap(), "batata-sdk");
    assert_eq!(form.get("client_secret").unwrap(), "s3cret");
    assert_eq!(form.get("scope").unwrap(), "openid");
    assert_eq!(
        header_value(token_calls[0], "content-type").as_deref(),
        Some("application/x-www-form-urlencoded")
    );
}

/// A configured token endpoint skips OIDC Discovery entirely.
#[tokio::test]
async fn direct_token_endpoint_skips_discovery() {
    let server = MockServer::start().await;
    mount_discovery(&server).await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(token_response("direct-tok", Some(300)))
        .mount(&server)
        .await;

    let provider = OidcAuthProvider::new(
        OidcConfig::new("id", "secret").with_token_endpoint(&format!("{}/token", server.uri())),
    )
    .unwrap();
    assert!(provider.login(&[]).await);
    assert_eq!(provider.access_token().as_deref(), Some("direct-tok"));

    let requests = server.received_requests().await.unwrap();
    assert!(
        !requests
            .iter()
            .any(|r| r.url.path() == "/.well-known/openid-configuration"),
        "a direct token endpoint must not trigger Discovery"
    );
}

/// Discovery failures are fail-closed: no token request, no identity headers.
#[tokio::test]
async fn discovery_failure_fails_closed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/.well-known/openid-configuration"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(token_response("unused", None))
        .mount(&server)
        .await;

    let provider =
        OidcAuthProvider::new(OidcConfig::new("id", "secret").with_issuer(&server.uri())).unwrap();
    assert!(!provider.login(&[]).await);
    assert!(
        provider
            .get_identity_context(&RequestResource::default())
            .headers
            .is_empty()
    );

    let requests = server.received_requests().await.unwrap();
    assert!(!requests.iter().any(|r| r.url.path() == "/token"));
}

/// A token response without `access_token` must not be treated as a login.
#[tokio::test]
async fn missing_access_token_fails_closed() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!({ "expires_in": 3600 })),
        )
        .mount(&server)
        .await;

    let provider = OidcAuthProvider::new(
        OidcConfig::new("id", "secret").with_token_endpoint(&format!("{}/token", server.uri())),
    )
    .unwrap();
    assert!(!provider.login(&[]).await);
    assert!(provider.access_token().is_none());
}

/// GA defaults `expires_in` to 300s when the IdP omits it.
#[tokio::test]
async fn missing_expires_in_uses_ga_default() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(token_response("no-ttl", None))
        .mount(&server)
        .await;

    let provider = OidcAuthProvider::new(
        OidcConfig::new("id", "secret").with_token_endpoint(&format!("{}/token", server.uri())),
    )
    .unwrap();
    assert!(provider.login(&[]).await);
    // Literal on purpose: comparing against the crate constant would make a
    // mutant that changes the GA default invisible.
    assert_eq!(provider.token_ttl_secs(), 300);
}

/// A zero-length TTL forces the next login cycle to refresh the token.
#[tokio::test]
async fn expired_token_is_refreshed_on_next_login() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(token_response("short", Some(0)))
        .mount(&server)
        .await;

    let provider = OidcAuthProvider::new(
        OidcConfig::new("id", "secret").with_token_endpoint(&format!("{}/token", server.uri())),
    )
    .unwrap();
    assert!(provider.login(&[]).await);
    assert!(provider.login(&[]).await);

    let requests = server.received_requests().await.unwrap();
    assert_eq!(
        requests.iter().filter(|r| r.url.path() == "/token").count(),
        2,
        "an expired OIDC token must be refetched"
    );
}

/// An explicit refresh replaces the token even while the old one is cached.
#[tokio::test]
async fn explicit_refresh_replaces_token() {
    let server = MockServer::start().await;
    let counter = std::sync::atomic::AtomicUsize::new(0);
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(move |_: &wiremock::Request| {
            let call = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            let token = if call == 0 { "first" } else { "second" };
            token_response(token, Some(3600))
        })
        .mount(&server)
        .await;

    let provider = OidcAuthProvider::new(
        OidcConfig::new("id", "secret").with_token_endpoint(&format!("{}/token", server.uri())),
    )
    .unwrap();
    assert!(provider.login(&[]).await);
    assert_eq!(provider.access_token().as_deref(), Some("first"));
    assert!(provider.refresh().await);
    assert_eq!(provider.access_token().as_deref(), Some("second"));
}

/// The real HTTP transport must attach both GA identity headers on every
/// request (declaration != call site).
#[tokio::test]
async fn http_transport_sends_both_oidc_identity_headers() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(token_response("http-tok", Some(3600)))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/nacos/v3/client/config"))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .mount(&server)
        .await;

    let config = ClientConfig::new(&server.uri()).with_oidc(
        OidcConfig::new("id", "secret").with_token_endpoint(&format!("{}/token", server.uri())),
    );
    assert!(config.has_oidc_auth());
    let client = BatataHttpClient::from_config(&config).await.unwrap();
    client
        .get::<serde_json::Value>("/v3/client/config")
        .await
        .unwrap();

    let requests = server.received_requests().await.unwrap();
    let api = requests
        .iter()
        .find(|r| r.url.path() == "/nacos/v3/client/config")
        .expect("API GET was not received");
    assert_eq!(
        header_value(api, OIDC_ACCESS_TOKEN_PARAM).as_deref(),
        Some("http-tok")
    );
    assert_eq!(
        header_value(api, OIDC_AUTHORIZATION_HEADER).as_deref(),
        Some("Bearer http-tok")
    );
    assert_eq!(
        requests.iter().filter(|r| r.url.path() == "/token").count(),
        1,
        "the OIDC token must be cached across API calls"
    );
}
