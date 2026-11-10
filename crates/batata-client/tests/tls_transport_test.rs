//! End-to-end TLS tests for the SDK gRPC and HTTP transports.
//!
//! These stand up a real tonic gRPC server and a rustls HTTPS server with a
//! self-signed CA. A plaintext channel, a channel that ignores the custom CA,
//! a missing SNI domain, or a missing client identity must all fail; the
//! correctly configured connections must succeed on both transports.

use std::collections::HashMap;
use std::sync::Once;
use std::time::Duration;

use batata_api::grpc::{
    Payload,
    bi_request_stream_server::{BiRequestStream, BiRequestStreamServer},
    request_server::{Request, RequestServer},
};
use batata_client::grpc::GrpcClientConfig;
use batata_client::grpc::auth::AuthProvider;
use batata_client::grpc::connection::GrpcConnection;
use batata_client::{BatataHttpClient, ClientConfig, GrpcClient, HttpClientConfig, TlsOptions};
use rcgen::{BasicConstraints, CertificateParams, IsCa, Issuer, KeyPair};
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::{mpsc, oneshot};
use tokio_stream::wrappers::{ReceiverStream, TcpListenerStream};
use tonic::transport::{Certificate as TonicCertificate, Identity, Server, ServerTlsConfig};

static CRYPTO_PROVIDER: Once = Once::new();

/// Workspace builds enable both rustls providers (`ring` via ldap3 and
/// `aws_lc_rs` via reqwest). Install one explicitly so tonic's server-side
/// `ServerConfig::builder()` cannot panic on ambiguity.
fn ensure_crypto_provider() {
    CRYPTO_PROVIDER.call_once(|| {
        if rustls::crypto::CryptoProvider::get_default().is_none() {
            let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();
        }
    });
}

struct Certs {
    // Kept alive so the PEM files are not deleted while the test is running.
    _dir: tempfile::TempDir,
    ca_path: String,
    server_cert_path: String,
    server_key_path: String,
    client_cert_path: String,
    client_key_path: String,
    ca_cert_der: CertificateDer<'static>,
    server_cert_der: CertificateDer<'static>,
    server_key_der: Vec<u8>,
}

impl Certs {
    fn generate() -> Self {
        let ca_key = KeyPair::generate().expect("CA key");
        let mut ca_params =
            CertificateParams::new(vec!["localhost".to_string()]).expect("CA params");
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        let issuer = Issuer::from_params(&ca_params, &ca_key);
        let ca_cert = ca_params.self_signed(&ca_key).expect("CA cert");

        let server_key = KeyPair::generate().expect("server key");
        let server_params =
            CertificateParams::new(vec!["localhost".to_string()]).expect("server params");
        let server_cert = server_params
            .signed_by(&server_key, &issuer)
            .expect("server cert");

        let client_key = KeyPair::generate().expect("client key");
        let client_params =
            CertificateParams::new(vec!["client.example.com".to_string()]).expect("client params");
        let client_cert = client_params
            .signed_by(&client_key, &issuer)
            .expect("client cert");

        let dir = tempfile::tempdir().expect("temp dir");
        let write = |name: &str, body: String| {
            let path = dir.path().join(name);
            std::fs::write(&path, body).expect("write pem");
            path.to_str().expect("utf8 path").to_string()
        };
        let ca_cert_der = ca_cert.der().clone();
        let server_cert_der = server_cert.der().clone();
        let server_key_der = server_key.serialize_der();
        let ca_path = write("ca.pem", ca_cert.pem());
        let server_cert_path = write("server.pem", server_cert.pem());
        let server_key_path = write("server.key", server_key.serialize_pem());
        let client_cert_path = write("client.pem", client_cert.pem());
        let client_key_path = write("client.key", client_key.serialize_pem());

        Self {
            _dir: dir,
            ca_path,
            server_cert_path,
            server_key_path,
            client_cert_path,
            client_key_path,
            ca_cert_der,
            server_cert_der,
            server_key_der,
        }
    }

    /// `server_addr` is an HTTP-looking address: the client adds 1000 to the
    /// port, so subtract it here to reach the TLS gRPC listener.
    fn server_addr(&self, port: u16) -> String {
        format!("127.0.0.1:{}", port - 1000)
    }
}

#[derive(Default)]
struct EchoService;

#[tonic::async_trait]
impl Request for EchoService {
    async fn request(
        &self,
        request: tonic::Request<Payload>,
    ) -> Result<tonic::Response<Payload>, tonic::Status> {
        Ok(tonic::Response::new(request.into_inner()))
    }
}

#[derive(Default)]
struct BiService;

#[tonic::async_trait]
#[allow(non_camel_case_types)]
impl BiRequestStream for BiService {
    type requestBiStreamStream = ReceiverStream<Result<Payload, tonic::Status>>;

    async fn request_bi_stream(
        &self,
        _request: tonic::Request<tonic::Streaming<Payload>>,
    ) -> Result<tonic::Response<Self::requestBiStreamStream>, tonic::Status> {
        let (tx, rx) = mpsc::channel::<Result<Payload, tonic::Status>>(1);
        // Keep the server stream open long enough for the client to send its
        // ConnectionSetupRequest. Ending it immediately closes the whole
        // bi-directional RPC on the client side.
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_secs(5)).await;
            drop(tx);
        });
        Ok(tonic::Response::new(ReceiverStream::new(rx)))
    }
}

/// Spawn a TLS gRPC server. `require_client_cert` turns on mTLS.
/// Returns the listening port and a shutdown sender.
async fn spawn_tls_server(certs: &Certs, require_client_cert: bool) -> (u16, oneshot::Sender<()>) {
    ensure_crypto_provider();

    let server_pem = std::fs::read(&certs.server_cert_path).expect("server cert");
    let server_key = std::fs::read(&certs.server_key_path).expect("server key");
    let mut tls = ServerTlsConfig::new().identity(Identity::from_pem(server_pem, server_key));
    if require_client_cert {
        let ca_pem = std::fs::read(&certs.ca_path).expect("ca cert");
        tls = tls.client_ca_root(TonicCertificate::from_pem(ca_pem));
    }

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind TLS test listener");
    let port = listener.local_addr().expect("local addr").port();
    let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        Server::builder()
            .tls_config(tls)
            .expect("valid server TLS config")
            .add_service(RequestServer::new(EchoService))
            .add_service(BiRequestStreamServer::new(BiService))
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = shutdown_rx.await;
            })
            .await
            .expect("TLS test server exited cleanly");
    });

    (port, shutdown_tx)
}

async fn try_connect(server_addr: &str, tls: &TlsOptions) -> Result<(), String> {
    match GrpcConnection::connect_with_tls(server_addr, "config", HashMap::new(), "", None, 8, tls)
        .await
    {
        Ok(_) => Ok(()),
        Err(e) => Err(e.to_string()),
    }
}

/// A TLS connection with the custom CA and the SNI override must succeed.
/// This is the success path that kills the mutants which drop `tls_config`,
/// the CA certificate or `domain_name`.
#[tokio::test]
async fn tls_connection_succeeds_with_custom_ca_and_domain() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_tls_server(&certs, false).await;
    let addr = certs.server_addr(port);
    let tls = TlsOptions::enabled(Some(&certs.ca_path)).with_domain("localhost");

    let result = tokio::time::timeout(Duration::from_secs(10), try_connect(&addr, &tls)).await;
    let _ = shutdown.send(());
    assert_eq!(result.expect("TLS connect must not hang"), Ok(()));
}

/// Native roots must not silently accept a private CA.
#[tokio::test]
async fn tls_connection_without_ca_is_rejected() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_tls_server(&certs, false).await;
    let addr = certs.server_addr(port);
    let tls = TlsOptions::enabled(None).with_domain("localhost");

    let result = tokio::time::timeout(Duration::from_secs(10), try_connect(&addr, &tls)).await;
    let _ = shutdown.send(());
    assert!(result.expect("TLS connect must not hang").is_err());
}

/// Without the domain override the certificate is checked against the IP
/// literal and must be rejected.
#[tokio::test]
async fn tls_connection_without_domain_override_is_rejected() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_tls_server(&certs, false).await;
    let addr = certs.server_addr(port);
    let tls = TlsOptions::enabled(Some(&certs.ca_path));

    let result = tokio::time::timeout(Duration::from_secs(10), try_connect(&addr, &tls)).await;
    let _ = shutdown.send(());
    assert!(result.expect("TLS connect must not hang").is_err());
}

/// A plaintext channel must not be able to talk to a TLS-only server.
#[tokio::test]
async fn plaintext_connection_to_tls_server_is_rejected() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_tls_server(&certs, false).await;
    let addr = certs.server_addr(port);

    let result = tokio::time::timeout(
        Duration::from_secs(10),
        try_connect(&addr, &TlsOptions::default()),
    )
    .await;
    let _ = shutdown.send(());
    assert!(result.expect("connect must not hang").is_err());
}

/// mTLS: with a client identity the handshake succeeds; without it the server
/// rejects the connection.
#[tokio::test]
async fn mtls_connection_requires_client_identity() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_tls_server(&certs, true).await;
    let addr = certs.server_addr(port);

    let with_identity = TlsOptions::mtls(
        Some(&certs.ca_path),
        &certs.client_cert_path,
        &certs.client_key_path,
    )
    .with_domain("localhost");
    let ok =
        tokio::time::timeout(Duration::from_secs(10), try_connect(&addr, &with_identity)).await;

    let without_identity = TlsOptions::enabled(Some(&certs.ca_path)).with_domain("localhost");
    let rejected = tokio::time::timeout(
        Duration::from_secs(10),
        try_connect(&addr, &without_identity),
    )
    .await;

    let _ = shutdown.send(());
    assert_eq!(ok.expect("mTLS connect must not hang"), Ok(()));
    assert!(rejected.expect("connect must not hang").is_err());
}

// ---------------------------------------------------------------------------
// HTTP transport: verify that ClientConfig's CA / mTLS settings really reach
// the reqwest client (a builder-level test cannot detect a dropped root CA).
// ---------------------------------------------------------------------------

/// Build a rustls server config from the test CA, optionally requiring a
/// client certificate.
fn https_server_config(
    certs: &Certs,
    require_client_cert: bool,
) -> std::sync::Arc<rustls::ServerConfig> {
    ensure_crypto_provider();
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(certs.server_key_der.clone()));
    let builder = rustls::ServerConfig::builder();
    let builder = if require_client_cert {
        let mut roots = rustls::RootCertStore::empty();
        roots.add(certs.ca_cert_der.clone()).expect("CA DER");
        let verifier = rustls::server::WebPkiClientVerifier::builder(std::sync::Arc::new(roots))
            .build()
            .expect("client cert verifier");
        builder.with_client_cert_verifier(verifier)
    } else {
        builder.with_no_client_auth()
    };
    std::sync::Arc::new(
        builder
            .with_single_cert(vec![certs.server_cert_der.clone()], key)
            .expect("server TLS config"),
    )
}

/// Body returned by the HTTPS test server: a valid V3 login response, so the
/// same server can be used to test `AuthProvider` over TLS.
const LOGIN_BODY: &str = r#"{"data":{"accessToken":"test-token","tokenTtl":3600}}"#;

/// Minimal HTTP/1.1 server over TLS. Answers every request with `LOGIN_BODY`.
async fn spawn_https_server(
    config: std::sync::Arc<rustls::ServerConfig>,
) -> (u16, oneshot::Sender<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind HTTPS test listener");
    let port = listener.local_addr().expect("local addr").port();
    let acceptor = tokio_rustls::TlsAcceptor::from(config);
    let (shutdown_tx, mut shutdown_rx) = oneshot::channel::<()>();

    tokio::spawn(async move {
        loop {
            tokio::select! {
                _ = &mut shutdown_rx => break,
                accepted = listener.accept() => {
                    let (stream, _) = accepted.expect("accept");
                    let acceptor = acceptor.clone();
                    tokio::spawn(async move {
                        let Ok(mut tls) = acceptor.accept(stream).await else {
                            return;
                        };
                        let mut buf = [0u8; 2048];
                        let _ = tls.read(&mut buf).await;
                        let head = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                            LOGIN_BODY.len()
                        );
                        let _ = tls.write_all(head.as_bytes()).await;
                        let _ = tls.write_all(LOGIN_BODY.as_bytes()).await;
                        let _ = tls.shutdown().await;
                    });
                }
            }
        }
    });

    (port, shutdown_tx)
}

async fn https_get(port: u16, config: ClientConfig) -> Result<String, String> {
    let client = config.build_reqwest_client().map_err(|e| e.to_string())?;
    let response = client
        .get(format!("https://localhost:{port}/"))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    response.text().await.map_err(|e| e.to_string())
}

/// A private CA configured on `ClientConfig` must be trusted by the HTTP
/// client; without it the same request must fail certificate verification.
#[tokio::test]
async fn http_client_trusts_configured_ca() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_https_server(https_server_config(&certs, false)).await;

    let with_ca = ClientConfig::new("localhost:8848").with_tls(Some(&certs.ca_path));
    let ok = tokio::time::timeout(Duration::from_secs(10), https_get(port, with_ca)).await;

    let without_ca = ClientConfig::new("localhost:8848");
    let rejected = tokio::time::timeout(Duration::from_secs(10), https_get(port, without_ca)).await;

    let _ = shutdown.send(());
    assert_eq!(
        ok.expect("HTTPS request must not hang"),
        Ok(LOGIN_BODY.to_string())
    );
    assert!(rejected.expect("HTTPS request must not hang").is_err());
}

/// The client identity must be presented when mTLS is configured, and the
/// HTTPS server must reject a client that has no certificate.
#[tokio::test]
async fn http_client_presents_mtls_identity() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_https_server(https_server_config(&certs, true)).await;

    let with_identity = ClientConfig::new("localhost:8848").with_mtls(
        Some(&certs.ca_path),
        &certs.client_cert_path,
        &certs.client_key_path,
    );
    let ok = tokio::time::timeout(Duration::from_secs(10), https_get(port, with_identity)).await;

    let without_identity = ClientConfig::new("localhost:8848").with_tls(Some(&certs.ca_path));
    let rejected =
        tokio::time::timeout(Duration::from_secs(10), https_get(port, without_identity)).await;

    let _ = shutdown.send(());
    assert_eq!(
        ok.expect("mTLS request must not hang"),
        Ok(LOGIN_BODY.to_string())
    );
    assert!(rejected.expect("HTTPS request must not hang").is_err());
}

/// JWT login performed for the gRPC connection must use the same CA as the
/// gRPC channel, otherwise an `https://` login endpoint would fail.
#[tokio::test]
async fn auth_provider_login_trusts_configured_ca() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_https_server(https_server_config(&certs, false)).await;
    let base = format!("https://localhost:{port}");

    let with_ca = TlsOptions {
        ca_cert_path: Some(certs.ca_path.clone()),
        ..TlsOptions::default()
    };
    let provider =
        AuthProvider::with_context_path_and_tls(&base, "/nacos", "user", "pass", &with_ca)
            .expect("auth provider with custom CA");
    let ok = tokio::time::timeout(Duration::from_secs(10), provider.get_token()).await;

    let provider_without_ca = AuthProvider::with_context_path_and_tls(
        &base,
        "/nacos",
        "user",
        "pass",
        &TlsOptions::default(),
    )
    .expect("auth provider without CA");
    let rejected =
        tokio::time::timeout(Duration::from_secs(10), provider_without_ca.get_token()).await;

    let _ = shutdown.send(());
    assert_eq!(
        ok.expect("login must not hang").expect("login with CA"),
        Some("test-token".to_string())
    );
    assert!(rejected.expect("login must not hang").is_err());
}

fn tls_grpc_config(certs: &Certs, addr: &str) -> GrpcClientConfig {
    GrpcClientConfig {
        server_addrs: vec![addr.to_string()],
        tls_enabled: true,
        tls_ca_cert: Some(certs.ca_path.clone()),
        tls_domain: Some("localhost".to_string()),
        ..GrpcClientConfig::default()
    }
}

/// `BatataHttpClient` (used by the maintainer client and admin API) must
/// build its reqwest client from `HttpClientConfig::tls` as well.
#[tokio::test]
async fn batata_http_client_trusts_configured_ca() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_https_server(https_server_config(&certs, false)).await;

    let http_config = HttpClientConfig::new(&format!("https://localhost:{port}"))
        .with_server_identity("serverIdentity", "test-identity")
        .with_tls(TlsOptions {
            ca_cert_path: Some(certs.ca_path.clone()),
            ..TlsOptions::default()
        });
    let http_client = BatataHttpClient::new_without_auth(http_config).expect("http client");
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        http_client.get::<serde_json::Value>("/"),
    )
    .await;

    let _ = shutdown.send(());
    let body = result
        .expect("HTTP request must not hang")
        .expect("request with custom CA");
    assert_eq!(body["data"]["accessToken"], "test-token");
}

/// `GrpcClient::connect` must forward its TLS settings through the failover
/// loop (this is the path applications actually use).
#[tokio::test]
async fn grpc_client_connect_forwards_tls() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_tls_server(&certs, false).await;
    let addr = certs.server_addr(port);

    let client = GrpcClient::new(tls_grpc_config(&certs, &addr)).expect("client");
    let ok = tokio::time::timeout(Duration::from_secs(10), client.connect()).await;

    let mut plain = tls_grpc_config(&certs, &addr);
    plain.tls_enabled = false;
    let plain_client = GrpcClient::new(plain).expect("plaintext client");
    let rejected = tokio::time::timeout(Duration::from_secs(10), plain_client.connect()).await;

    let _ = shutdown.send(());
    assert!(ok.expect("connect must not hang").is_ok());
    assert!(rejected.expect("connect must not hang").is_err());
}

/// The reconnect-to path must keep TLS enabled too.
#[tokio::test]
async fn grpc_client_reconnect_to_keeps_tls() {
    let certs = Certs::generate();
    let (port, shutdown) = spawn_tls_server(&certs, false).await;
    // `reconnect_to` builds `ip:port` and the connection adds 1000 again.
    let http_port = port - 1000;

    let client = GrpcClient::new(tls_grpc_config(&certs, "127.0.0.1:8848")).expect("client");
    let result = tokio::time::timeout(
        Duration::from_secs(10),
        client.reconnect_to("127.0.0.1", &http_port.to_string()),
    )
    .await;

    let _ = shutdown.send(());
    assert!(result.expect("reconnect must not hang").is_ok());
}
