//! Coverage for `F-NAC-CSDK-006`: every outgoing client HTTP request must
//! carry a fresh v4 UUID under the `Nacos-Request-Id` header, mirroring Nacos
//! 3.3.0 GA `RequestIdHttpClientRequestInterceptor`.
//!
//! Wire assertions use a `MockServer` and inspect the recorded request headers.
//! Identity auth is used where possible so the client never touches the login
//! endpoint (which would mask the request-id assertions); the JWT login path is
//! covered explicitly by a dedicated test against a stubbed auth endpoint.

use batata_client::{BatataHttpClient, HttpClientConfig};
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Build a client that uses server-identity auth, keeping it off the login
/// endpoint so request-id assertions aren't masked by authentication traffic.
async fn identity_client(server: &MockServer) -> BatataHttpClient {
    let config = HttpClientConfig {
        server_addrs: vec![server.uri()],
        server_identity_key: "serverIdentity".to_string(),
        server_identity_value: "test-identity".to_string(),
        ..Default::default()
    };
    BatataHttpClient::new_without_auth(config).unwrap()
}

/// Pull the (case-insensitive) `Nacos-Request-Id` value from a recorded request.
fn request_id_value(req: &wiremock::Request) -> Option<String> {
    req.headers
        .iter()
        .find(|(k, _)| k.as_str().eq_ignore_ascii_case("Nacos-Request-Id"))
        .and_then(|(_, v)| v.to_str().ok().map(|s| s.to_string()))
}

/// Assert the recorded request carries a valid, non-nil v4 UUID under
/// `Nacos-Request-Id`.
fn assert_valid_request_id(req: &wiremock::Request) {
    let value = request_id_value(req)
        .unwrap_or_else(|| panic!("missing Nacos-Request-Id on {}", req.url.path()));
    let parsed = Uuid::parse_str(&value)
        .unwrap_or_else(|e| panic!("Nacos-Request-Id {value:?} is not a UUID: {e}"));
    assert_eq!(
        parsed.get_version_num(),
        4,
        "Nacos-Request-Id {value:?} must be a v4 UUID"
    );
    assert_ne!(
        parsed,
        Uuid::nil(),
        "Nacos-Request-Id must not be the nil UUID"
    );
}

fn empty_ok() -> ResponseTemplate {
    ResponseTemplate::new(200).set_body_string("{}")
}

/// GET requests must carry the request-id header.
#[tokio::test]
async fn get_carries_nacos_request_id() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/configs"))
        .respond_with(empty_ok())
        .mount(&server)
        .await;

    let client = identity_client(&server).await;
    client
        .get::<serde_json::Value>("/v1/configs")
        .await
        .unwrap();

    let reqs = server.received_requests().await.unwrap();
    let get = reqs
        .iter()
        .find(|r| r.url.path() == "/v1/configs")
        .expect("GET was not received");
    assert_valid_request_id(get);
}

/// POST requests must carry the request-id header.
#[tokio::test]
async fn post_json_carries_nacos_request_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/configs"))
        .respond_with(empty_ok())
        .mount(&server)
        .await;

    let client = identity_client(&server).await;
    client
        .post_json::<serde_json::Value, _>("/v1/configs", &serde_json::json!({}))
        .await
        .unwrap();

    let reqs = server.received_requests().await.unwrap();
    let post = reqs
        .iter()
        .find(|r| r.url.path() == "/v1/configs")
        .expect("POST was not received");
    assert_valid_request_id(post);
}

/// The JWT login request issued by `authenticate()` must also carry the
/// request-id header (it bypasses `request_with_retry`).
#[tokio::test]
async fn authenticate_carries_nacos_request_id() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/v1/auth/login"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({ "accessToken": "tok", "tokenTtl": 18000 })),
        )
        .mount(&server)
        .await;

    let config = HttpClientConfig {
        server_addrs: vec![server.uri()],
        ..Default::default()
    };
    let client = BatataHttpClient::new_without_auth(config).unwrap();
    client.authenticate().await.unwrap();

    let reqs = server.received_requests().await.unwrap();
    let login = reqs
        .iter()
        .find(|r| r.url.path() == "/v1/auth/login")
        .expect("login POST was not received");
    assert_valid_request_id(login);
}
