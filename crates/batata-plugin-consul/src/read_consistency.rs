//! Consul read-consistency routing for the HTTP API.
//!
//! Upstream Consul separates two concerns:
//!
//! * a non-`?stale` read is forwarded through the server RPC layer to the
//!   Raft leader (`agent/consul/rpc.go::forwardRPC`);
//! * `?consistent` additionally verifies leadership before reading
//!   (`agent/blockingquery/blockingquery.go::Query` → `ConsistentRead`).
//!
//! Batata has no HTTP-level leader forwarding in the core cluster RPC path, so
//! this module provides it at the Consul `/v1` scope. Followers proxy non-stale
//! GET/HEAD requests **for Raft-replicated state** (KV and sessions) to the
//! leader's Consul HTTP endpoint; the leader runs a Raft ReadIndex barrier only
//! for `?consistent`. `?stale` stays local.
//!
//! Upstream can proxy every read because its whole FSM is replicated to every
//! server. Batata only replicates a subset: ACL entities, namespaces,
//! partitions, autopilot config, config entries, catalog/health views and agent
//! state still live on the contacted node (catalog/health additionally resolve
//! the local node identity). Proxying those to another server returns 404 or
//! stale data, so they are served locally. This is a deliberate parity gap;
//! forwarding is restricted to the prefixes below rather than all `/v1`.

use std::future::{Ready, ready};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use actix_web::body::EitherBody;
use actix_web::dev::{Service, ServiceRequest, ServiceResponse, Transform};
use actix_web::http::{Method, StatusCode};
use actix_web::{Error, HttpResponse, web};
use async_trait::async_trait;
use bytes::Bytes;

use crate::consul_meta::LinearizableReadError;
use crate::raft::ConsulRaftWriter;

/// Header used to mark a request that has already been proxied once.
pub const FORWARDED_HEADER: &str = "x-batata-consul-forwarded";
/// Header reporting the node that proxied the request.
pub const FORWARDED_BY_HEADER: &str = "x-batata-consul-forwarded-by";

const FORWARD_TIMEOUT: Duration = Duration::from_secs(610);
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Only these surfaces are backed by the Raft-replicated Consul FSM on every
/// node. A different server can answer their reads correctly; all other `/v1`
/// reads must stay on the contacted node (see the module docs).
const LEADER_READ_PREFIXES: [&str; 3] = ["/v1/kv/", "/v1/session/", "/v1/txn"];

fn is_leader_read_path(path: &str) -> bool {
    LEADER_READ_PREFIXES.iter().any(|p| path.starts_with(p))
}

const LOCAL_ONLY_PREFIXES: [&str; 11] = [
    "/v1/agent/",
    "/v1/status/",
    "/v1/event/",
    "/v1/snapshot",
    "/v1/coordinate/",
    // These keep per-node in-memory state (or a memory-backed ACL store when
    // ACL is disabled). Catalog/health views additionally resolve the
    // *contacted* node's identity, so a different server cannot answer them.
    "/v1/acl/",
    "/v1/namespace",
    "/v1/partition",
    "/v1/operator/autopilot/",
    "/v1/catalog/",
    "/v1/health/",
];

/// Request headers that must not be replayed verbatim when proxying.
///
/// The caller's `Accept-Encoding: gzip` would make the leader compress the
/// response. The proxy strips `Content-Encoding` on the way back (the body is
/// already decoded by reqwest), so the SDK would then receive gzip bytes with
/// no encoding header and fail to parse them as JSON. Only `identity` is sent.
const SUPPRESSED_REQUEST_HEADERS: [&str; 1] = ["accept-encoding"];

fn should_copy_forward_header(name: &str) -> bool {
    !HOP_BY_HOP_HEADERS.contains(&name) && !SUPPRESSED_REQUEST_HEADERS.contains(&name)
}

const HOP_BY_HOP_HEADERS: [&str; 9] = [
    "connection",
    "content-length",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
];

/// What the read-consistency middleware should do for one request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ConsistencyDecision {
    forward: bool,
    verify: bool,
    consistency: &'static str,
}

/// Decide whether a request is local, leader-forwarded, and ReadIndex-guarded.
fn consistency_decision(query: Option<&str>) -> Result<ConsistencyDecision, &'static str> {
    let mut stale = false;
    let mut consistent = false;
    let mut cached = false;
    let mut max_stale = false;

    if let Some(query) = query {
        for part in query.split('&') {
            if part.is_empty() {
                continue;
            }
            let (key, value) = match part.find('=') {
                Some(pos) => (&part[..pos], &part[pos + 1..]),
                None => (part, ""),
            };
            match key {
                "stale" => stale = true,
                "consistent" => consistent = true,
                "cached" => cached = true,
                "max_stale" => {
                    max_stale = !value.is_empty() && value.chars().any(|c| c.is_ascii_digit());
                }
                // `?leader` forces the default mode; no extra state needed.
                "leader" => {}
                _ => {}
            }
        }
    }

    if stale && consistent {
        return Err("Cannot specify ?stale with ?consistent, conflicting semantics.");
    }
    if cached && consistent {
        return Err("Cannot specify ?cached with ?consistent, conflicting semantics.");
    }

    if stale || max_stale {
        Ok(ConsistencyDecision {
            forward: false,
            verify: false,
            consistency: "stale",
        })
    } else if consistent {
        Ok(ConsistencyDecision {
            forward: true,
            verify: true,
            consistency: "consistent",
        })
    } else {
        Ok(ConsistencyDecision {
            forward: true,
            verify: false,
            consistency: "leader",
        })
    }
}

/// Whether this path is served by the contacted agent and must not be proxied.
fn is_local_only_path(path: &str) -> bool {
    LOCAL_ONLY_PREFIXES.iter().any(|p| path.starts_with(p))
}

/// Whether a request is eligible for leader forwarding.
fn should_forward(method: &Method, path: &str, decision: ConsistencyDecision) -> bool {
    decision.forward
        && matches!(*method, Method::GET | Method::HEAD)
        && path.starts_with("/v1/")
        && is_leader_read_path(path)
        && !is_local_only_path(path)
}

/// Split a Raft `host:port` address, normalising IPv6 brackets away.
fn split_host_port(addr: &str) -> Option<(String, u16)> {
    if addr.is_empty() {
        return None;
    }
    if let Ok(socket) = addr.parse::<std::net::SocketAddr>() {
        return Some((socket.ip().to_string(), socket.port()));
    }
    let (host, port) = addr.rsplit_once(':')?;
    let port: u16 = port.parse().ok()?;
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port))
}

/// Re-attach a port to a host, restoring IPv6 brackets when needed.
fn join_host_port(host: &str, port: u16) -> String {
    if host.contains(':') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    }
}

/// Convert a Raft listener address to the paired Batata **main** HTTP address.
///
/// Raft ports are derived as `main_port - 1000`, so the inverse is
/// `raft_port + 1000`. This yields the Nacos-facing listener (8848 by default),
/// NOT the Consul compatibility listener (8500 by default). Read forwarding
/// must use [`raft_addr_to_consul_addr`] instead — forwarding a Consul `/v1/**`
/// request here returns 404 because the main listener has no such routes.
pub fn raft_addr_to_http_addr(raft_addr: &str) -> Option<String> {
    let (host, port) = split_host_port(raft_addr)?;
    Some(join_host_port(&host, port.checked_add(1000)?))
}

/// Convert the leader's Raft address to its **Consul** HTTP address.
///
/// Cluster nodes step every listener by the same delta (e.g. node 1 uses
/// 7848/8500, node 2 7858/8510), so the leader's Consul port is the local
/// Consul port plus the Raft port delta. Using the real local Raft address
/// keeps this correct for any node layout without a hard-coded offset.
pub fn raft_addr_to_consul_addr(
    leader_raft_addr: &str,
    local_raft_addr: &str,
    local_consul_port: u16,
) -> Option<String> {
    let (host, leader_raft_port) = split_host_port(leader_raft_addr)?;
    let (_, local_raft_port) = split_host_port(local_raft_addr)?;
    let delta = i32::from(leader_raft_port) - i32::from(local_raft_port);
    let consul_port = i32::from(local_consul_port).checked_add(delta)?;
    let consul_port = u16::try_from(consul_port).ok()?;
    Some(join_host_port(&host, consul_port))
}

/// Abstraction over Raft leader state used by the forwarding middleware.
#[async_trait]
pub trait ConsulReadPolicy: Send + Sync {
    /// Whether this node may serve the read itself.
    fn is_leader(&self) -> bool;
    /// Stable local node id, reported through the forwarding header.
    fn local_node_id(&self) -> &str;
    /// Consul HTTP address of the current leader, if known.
    fn leader_http_addr(&self) -> Option<String>;
    /// Run the leader-side ReadIndex barrier for `?consistent`.
    async fn verify_leader(&self) -> Result<(), LinearizableReadError>;
}

/// Production [`ConsulReadPolicy`] backed by the Consul Raft writer.
pub struct RaftReadPolicy {
    raft: Option<Arc<ConsulRaftWriter>>,
    node_id: String,
    /// This node's Consul compatibility HTTP port (e.g. 8500).
    consul_port: u16,
}

impl RaftReadPolicy {
    /// Build a policy. Standalone nodes pass `None` and never forward.
    ///
    /// `consul_port` is required (no default) so a caller cannot silently
    /// forward to the wrong listener on a non-default cluster node.
    pub fn new(raft: Option<Arc<ConsulRaftWriter>>, node_id: String, consul_port: u16) -> Self {
        Self {
            raft,
            node_id,
            consul_port,
        }
    }
}

#[async_trait]
impl ConsulReadPolicy for RaftReadPolicy {
    fn is_leader(&self) -> bool {
        self.raft
            .as_ref()
            .map(|raft| raft.is_leader())
            .unwrap_or(true)
    }

    fn local_node_id(&self) -> &str {
        &self.node_id
    }

    fn leader_http_addr(&self) -> Option<String> {
        let raft = self.raft.as_ref()?;
        raft_addr_to_consul_addr(&raft.leader_addr()?, raft.local_addr(), self.consul_port)
    }

    async fn verify_leader(&self) -> Result<(), LinearizableReadError> {
        let Some(ref raft) = self.raft else {
            return Ok(());
        };
        if !raft.is_leader() {
            return Err(LinearizableReadError::NotLeader {
                leader_addr: raft.leader_addr(),
            });
        }
        raft.linearizable_read()
            .await
            .map_err(|e| LinearizableReadError::Other(format!("linearizable_read failed: {}", e)))
    }
}

/// Actix transform installed on the Consul `/v1` scope.
pub struct ConsulReadConsistency;

impl ConsulReadConsistency {
    /// Create the middleware factory.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConsulReadConsistency {
    fn default() -> Self {
        Self::new()
    }
}

impl<S, B> Transform<S, ServiceRequest> for ConsulReadConsistency
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = ConsulReadConsistencyMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        let client = reqwest::Client::builder()
            .connect_timeout(CONNECT_TIMEOUT)
            .timeout(FORWARD_TIMEOUT)
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());
        ready(Ok(ConsulReadConsistencyMiddleware {
            service: Arc::new(service),
            client,
        }))
    }
}

/// Middleware that forwards non-stale reads and verifies `?consistent` reads.
pub struct ConsulReadConsistencyMiddleware<S> {
    service: Arc<S>,
    client: reqwest::Client,
}

fn leader_unavailable(addr: Option<&str>, reason: &str, consistency: &str) -> HttpResponse {
    let mut builder = HttpResponse::ServiceUnavailable();
    if let Some(addr) = addr {
        builder.insert_header(("X-Consul-Leader-Address", addr));
    }
    builder.insert_header(("X-Consul-Effective-Consistency", consistency));
    builder.body(format!("non-stale Consul read requires leader: {}", reason))
}

fn copy_forward_headers(req: &ServiceRequest) -> (Vec<(String, String)>, bool) {
    let mut headers = Vec::new();
    let mut already_forwarded = false;
    for (name, value) in req.headers().iter() {
        let name = name.as_str();
        if name == FORWARDED_HEADER {
            already_forwarded = true;
            continue;
        }
        if !should_copy_forward_header(name) {
            continue;
        }
        if let Ok(value) = value.to_str() {
            headers.push((name.to_string(), value.to_string()));
        }
    }
    (headers, already_forwarded)
}

async fn forward_to_leader(
    client: &reqwest::Client,
    method: Method,
    url: String,
    headers: Vec<(String, String)>,
    body: Bytes,
    node_id: String,
    consistency: &'static str,
    leader_addr: &str,
) -> HttpResponse {
    let reqwest_method = match reqwest::Method::from_bytes(method.as_str().as_bytes()) {
        Ok(method) => method,
        Err(e) => return leader_unavailable(Some(leader_addr), &e.to_string(), consistency),
    };

    let mut builder = client.request(reqwest_method, &url);
    for (name, value) in headers {
        builder = builder.header(name, value);
    }
    builder = builder
        .header(FORWARDED_HEADER, "1")
        .header(FORWARDED_BY_HEADER, node_id.clone())
        .header("accept-encoding", "identity");

    let response = match builder.body(body.to_vec()).send().await {
        Ok(response) => response,
        Err(e) => {
            tracing::warn!(%leader_addr, error = %e, "Consul leader read forwarding failed");
            return leader_unavailable(Some(leader_addr), "leader forwarding failed", consistency);
        }
    };

    let status =
        StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut out = HttpResponse::build(status);
    for (name, value) in response.headers() {
        let name = name.as_str();
        if HOP_BY_HOP_HEADERS.contains(&name) || name == "content-encoding" {
            continue;
        }
        if let Ok(value) = value.to_str() {
            out.insert_header((name, value));
        }
    }
    out.insert_header((FORWARDED_BY_HEADER, node_id));

    match response.bytes().await {
        Ok(bytes) => out.body(bytes),
        Err(e) => {
            tracing::warn!(%leader_addr, error = %e, "Reading forwarded Consul response failed");
            HttpResponse::BadGateway().body("failed to read leader response")
        }
    }
}

impl<S, B> Service<ServiceRequest> for ConsulReadConsistencyMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    S::Future: 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>>>>;

    fn poll_ready(
        &self,
        ctx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(ctx)
    }

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let service = self.service.clone();
        // Tests and standalone servers may register the routes without a
        // policy; in that case this middleware is intentionally a no-op.
        let Some(policy) = req
            .app_data::<web::Data<Arc<dyn ConsulReadPolicy>>>()
            .cloned()
        else {
            let fut = service.call(req);
            return Box::pin(async move { fut.await.map(|res| res.map_into_left_body()) });
        };

        let decision = match consistency_decision(req.uri().query()) {
            Ok(decision) => decision,
            Err(message) => {
                let response = HttpResponse::BadRequest().body(message);
                return Box::pin(
                    async move { Ok(req.into_response(response).map_into_right_body()) },
                );
            }
        };

        let method = req.method().clone();
        let path = req.path().to_string();
        let path_and_query = req
            .uri()
            .path_and_query()
            .map(|v| v.as_str().to_string())
            .unwrap_or_else(|| path.clone());
        let (headers, already_forwarded) = copy_forward_headers(&req);

        if should_forward(&method, &path, decision) && !policy.is_leader() {
            let leader_addr = policy.leader_http_addr();
            let Some(leader_addr) = leader_addr.filter(|addr| !addr.is_empty()) else {
                let response = leader_unavailable(None, "leader unknown", decision.consistency);
                return Box::pin(
                    async move { Ok(req.into_response(response).map_into_right_body()) },
                );
            };
            if already_forwarded {
                let response = leader_unavailable(
                    Some(&leader_addr),
                    "leader changed while forwarding",
                    decision.consistency,
                );
                return Box::pin(
                    async move { Ok(req.into_response(response).map_into_right_body()) },
                );
            }

            let url = if leader_addr.contains("://") {
                format!("{}{}", leader_addr.trim_end_matches('/'), path_and_query)
            } else {
                format!("http://{}{}", leader_addr, path_and_query)
            };
            let client = self.client.clone();
            let node_id = policy.local_node_id().to_string();
            let extract = req.extract::<Bytes>();

            return Box::pin(async move {
                let body = extract.await?;
                let response = forward_to_leader(
                    &client,
                    method,
                    url,
                    headers,
                    body,
                    node_id,
                    decision.consistency,
                    &leader_addr,
                )
                .await;
                Ok(req.into_response(response).map_into_right_body())
            });
        }

        // `?consistent` only has meaning for Raft-replicated state. For
        // node-local surfaces the ReadIndex barrier would reject the follower
        // outright, even though the contacted node can answer correctly.
        if decision.verify && is_leader_read_path(&path) {
            return Box::pin(async move {
                if let Err(e) = policy.verify_leader().await {
                    return Ok(req
                        .into_response(e.into_http_response())
                        .map_into_right_body());
                }
                let fut = service.call(req);
                fut.await.map(|res| res.map_into_left_body())
            });
        }

        let fut = service.call(req);
        Box::pin(async move { fut.await.map(|res| res.map_into_left_body()) })
    }
}

/// Ensure the skip list only covers agent-local surfaces.
pub fn is_agent_local_path(path: &str) -> bool {
    path.starts_with("/v1/agent/")
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    #[test]
    fn converts_raft_addr_to_http_addr() {
        assert_eq!(
            raft_addr_to_http_addr("127.0.0.1:7848").as_deref(),
            Some("127.0.0.1:8848")
        );
        assert_eq!(
            raft_addr_to_http_addr("[::1]:7848").as_deref(),
            Some("[::1]:8848")
        );
        assert_eq!(
            raft_addr_to_http_addr("node-a:7848").as_deref(),
            Some("node-a:8848")
        );
        assert_eq!(raft_addr_to_http_addr("node-a"), None);
        assert_eq!(raft_addr_to_http_addr("node-a:65500"), None);
    }

    #[test]
    fn maps_leader_raft_addr_to_consul_port() {
        // Leader is this node: stay on the local Consul listener.
        assert_eq!(
            raft_addr_to_consul_addr("127.0.0.1:7848", "127.0.0.1:7848", 8500).as_deref(),
            Some("127.0.0.1:8500")
        );
        // Node 1 forwards to a leader on node 2 (both listeners step by 10).
        assert_eq!(
            raft_addr_to_consul_addr("127.0.0.1:7858", "127.0.0.1:7848", 8500).as_deref(),
            Some("127.0.0.1:8510")
        );
        // Node 2 forwards back to a leader on node 1 (negative delta).
        assert_eq!(
            raft_addr_to_consul_addr("127.0.0.1:7848", "127.0.0.1:7858", 8510).as_deref(),
            Some("127.0.0.1:8500")
        );
        // IPv6 and hostname forms keep their syntax.
        assert_eq!(
            raft_addr_to_consul_addr("[::1]:7858", "[::1]:7848", 8500).as_deref(),
            Some("[::1]:8510")
        );
        assert_eq!(
            raft_addr_to_consul_addr("node-b:7858", "node-a:7848", 8500).as_deref(),
            Some("node-b:8510")
        );
        // Malformed addresses and out-of-range ports fail closed.
        assert_eq!(
            raft_addr_to_consul_addr("node-b", "node-a:7848", 8500),
            None
        );
        assert_eq!(
            raft_addr_to_consul_addr("127.0.0.1:7858", "127.0.0.1:7848", 65530),
            None
        );
    }

    #[test]
    fn consul_forwarding_never_targets_main_http_port() {
        // Forwarding a Consul /v1/** read to the main listener (raft + 1000)
        // yields 404: that listener has no Consul routes. The two mappings
        // must stay distinct for every cluster node.
        for (leader_raft, main_http, consul_http) in [
            ("127.0.0.1:7848", "127.0.0.1:8848", "127.0.0.1:8500"),
            ("127.0.0.1:7858", "127.0.0.1:8858", "127.0.0.1:8510"),
            ("127.0.0.1:7868", "127.0.0.1:8868", "127.0.0.1:8520"),
        ] {
            assert_eq!(
                raft_addr_to_http_addr(leader_raft).as_deref(),
                Some(main_http)
            );
            assert_eq!(
                raft_addr_to_consul_addr(leader_raft, "127.0.0.1:7848", 8500).as_deref(),
                Some(consul_http)
            );
            assert_ne!(
                raft_addr_to_consul_addr(leader_raft, "127.0.0.1:7848", 8500),
                raft_addr_to_http_addr(leader_raft)
            );
        }
    }

    #[test]
    fn classifies_consistency_modes() {
        assert_eq!(
            consistency_decision(None),
            Ok(ConsistencyDecision {
                forward: true,
                verify: false,
                consistency: "leader"
            })
        );
        assert_eq!(
            consistency_decision(Some("stale=1")),
            Ok(ConsistencyDecision {
                forward: false,
                verify: false,
                consistency: "stale"
            })
        );
        assert_eq!(
            consistency_decision(Some("consistent")),
            Ok(ConsistencyDecision {
                forward: true,
                verify: true,
                consistency: "consistent"
            })
        );
        assert_eq!(
            consistency_decision(Some("max_stale=10s")),
            Ok(ConsistencyDecision {
                forward: false,
                verify: false,
                consistency: "stale"
            })
        );
        assert_eq!(
            consistency_decision(Some("leader")),
            Ok(ConsistencyDecision {
                forward: true,
                verify: false,
                consistency: "leader"
            })
        );
    }

    #[test]
    fn rejects_conflicting_consistency_modes() {
        assert!(consistency_decision(Some("stale&consistent")).is_err());
        assert!(consistency_decision(Some("cached&consistent")).is_err());
    }

    #[test]
    fn skips_local_only_paths() {
        let decision = ConsistencyDecision {
            forward: true,
            verify: false,
            consistency: "leader",
        };
        assert!(!should_forward(
            &Method::GET,
            "/v1/agent/services",
            decision
        ));
        assert!(!should_forward(&Method::GET, "/v1/status/leader", decision));
        assert!(!should_forward(&Method::POST, "/v1/kv/key", decision));
        assert!(should_forward(&Method::GET, "/v1/kv/key", decision));
        assert!(should_forward(&Method::HEAD, "/v1/session/list", decision));
        assert!(!should_forward(
            &Method::HEAD,
            "/v1/catalog/services",
            decision
        ));
        assert!(is_agent_local_path("/v1/agent/self"));
    }

    #[test]
    fn node_local_state_paths_are_not_proxied() {
        let decision = ConsistencyDecision {
            forward: true,
            verify: false,
            consistency: "leader",
        };
        // Writes to these endpoints land on the contacted node only. Proxying
        // the subsequent read to a different node returns 404 / stale data.
        for path in [
            "/v1/acl/token/abc",
            "/v1/acl/tokens",
            "/v1/acl/policies",
            "/v1/namespace/team-a",
            "/v1/namespaces",
            "/v1/partition/team-a",
            "/v1/partitions",
            "/v1/operator/autopilot/configuration",
            "/v1/catalog/node/c1",
            "/v1/catalog/services",
            "/v1/health/service/web",
            "/v1/health/state/critical",
            "/v1/config/service-defaults/web",
            "/v1/config",
            "/v1/query/abc",
            "/v1/operator/raft/configuration",
            "/v1/connect/ca/roots",
        ] {
            assert!(
                !should_forward(&Method::GET, path, decision),
                "{path} must be served by the contacted node"
            );
        }
        // Only Raft-replicated KV/session state goes to the leader.
        for path in [
            "/v1/kv/app",
            "/v1/kv/app?recurse",
            "/v1/session/info/abc",
            "/v1/session/list",
            "/v1/txn",
        ] {
            assert!(
                should_forward(&Method::GET, path, decision),
                "{path} must still be forwarded"
            );
        }
    }

    #[test]
    fn proxy_never_replays_client_accept_encoding() {
        assert!(!should_copy_forward_header("accept-encoding"));
        assert!(!should_copy_forward_header("connection"));
        assert!(should_copy_forward_header("x-consul-token"));
        assert!(should_copy_forward_header("x-consul-index"));
    }

    struct FakePolicy {
        leader: bool,
        leader_addr: Option<String>,
        verify_ok: bool,
        verify_calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl ConsulReadPolicy for FakePolicy {
        fn is_leader(&self) -> bool {
            self.leader
        }

        fn local_node_id(&self) -> &str {
            "follower-1"
        }

        fn leader_http_addr(&self) -> Option<String> {
            self.leader_addr.clone()
        }

        async fn verify_leader(&self) -> Result<(), LinearizableReadError> {
            self.verify_calls.fetch_add(1, Ordering::SeqCst);
            if self.verify_ok {
                Ok(())
            } else {
                Err(LinearizableReadError::Other("not ready".to_string()))
            }
        }
    }

    async fn app_with_policy(
        policy: FakePolicy,
        local_calls: Arc<AtomicUsize>,
    ) -> impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse<EitherBody<actix_web::body::BoxBody>>,
        Error = actix_web::Error,
    > {
        let policy: Arc<dyn ConsulReadPolicy> = Arc::new(policy);
        actix_web::test::init_service(
            actix_web::App::new()
                .app_data(web::Data::new(policy))
                .wrap(ConsulReadConsistency::new())
                .route(
                    "/v1/kv/{key}",
                    web::get().to(move |_: web::Path<String>| {
                        let calls = local_calls.clone();
                        async move {
                            calls.fetch_add(1, Ordering::SeqCst);
                            HttpResponse::Ok().body("local")
                        }
                    }),
                ),
        )
        .await
    }

    #[actix_web::test]
    async fn forwards_non_stale_reads_to_leader() {
        let server = wiremock::MockServer::start().await;
        wiremock::Mock::given(wiremock::matchers::method("GET"))
            .and(wiremock::matchers::path("/v1/kv/app"))
            .and(wiremock::matchers::header("x-token-test", "abc"))
            .respond_with(
                wiremock::ResponseTemplate::new(202)
                    .set_body_string("from-leader")
                    .insert_header("x-leader-answer", "yes"),
            )
            .mount(&server)
            .await;

        let local_calls = Arc::new(AtomicUsize::new(0));
        let app = app_with_policy(
            FakePolicy {
                leader: false,
                leader_addr: Some(server.address().to_string()),
                verify_ok: true,
                verify_calls: Arc::new(AtomicUsize::new(0)),
            },
            local_calls.clone(),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/v1/kv/app?consistent")
            .insert_header(("x-token-test", "abc"))
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;

        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        assert_eq!(actix_web::test::read_body(resp).await, "from-leader");
        assert_eq!(local_calls.load(Ordering::SeqCst), 0);
    }

    #[actix_web::test]
    async fn stale_reads_stay_local() {
        let server = wiremock::MockServer::start().await;
        let local_calls = Arc::new(AtomicUsize::new(0));
        let app = app_with_policy(
            FakePolicy {
                leader: false,
                leader_addr: Some(server.address().to_string()),
                verify_ok: true,
                verify_calls: Arc::new(AtomicUsize::new(0)),
            },
            local_calls.clone(),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/v1/kv/app?stale")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(actix_web::test::read_body(resp).await, "local");
        assert_eq!(local_calls.load(Ordering::SeqCst), 1);
        assert!(server.received_requests().await.unwrap().is_empty());
    }

    #[actix_web::test]
    async fn leader_verifies_consistent_reads() {
        let verify_calls = Arc::new(AtomicUsize::new(0));
        let local_calls = Arc::new(AtomicUsize::new(0));
        let app = app_with_policy(
            FakePolicy {
                leader: true,
                leader_addr: None,
                verify_ok: true,
                verify_calls: verify_calls.clone(),
            },
            local_calls.clone(),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/v1/kv/app?consistent")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(verify_calls.load(Ordering::SeqCst), 1);
        assert_eq!(local_calls.load(Ordering::SeqCst), 1);
    }

    #[actix_web::test]
    async fn failed_leader_verification_blocks_local_read() {
        let verify_calls = Arc::new(AtomicUsize::new(0));
        let local_calls = Arc::new(AtomicUsize::new(0));
        let app = app_with_policy(
            FakePolicy {
                leader: true,
                leader_addr: None,
                verify_ok: false,
                verify_calls: verify_calls.clone(),
            },
            local_calls.clone(),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/v1/kv/app?consistent")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(verify_calls.load(Ordering::SeqCst), 1);
        assert_eq!(local_calls.load(Ordering::SeqCst), 0);
    }

    #[actix_web::test]
    async fn forwarded_request_is_not_forwarded_again() {
        let local_calls = Arc::new(AtomicUsize::new(0));
        let app = app_with_policy(
            FakePolicy {
                leader: false,
                leader_addr: Some("127.0.0.1:18500".to_string()),
                verify_ok: true,
                verify_calls: Arc::new(AtomicUsize::new(0)),
            },
            local_calls.clone(),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/v1/kv/app")
            .insert_header((FORWARDED_HEADER, "1"))
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            resp.headers()
                .get("x-consul-leader-address")
                .unwrap()
                .to_str()
                .unwrap(),
            "127.0.0.1:18500"
        );
        let body = actix_web::test::read_body(resp).await;
        assert!(String::from_utf8_lossy(&body).contains("leader changed while forwarding"));
        assert_eq!(local_calls.load(Ordering::SeqCst), 0);
    }

    #[actix_web::test]
    async fn agent_local_paths_are_not_forwarded() {
        let server = wiremock::MockServer::start().await;
        let local_calls = Arc::new(AtomicUsize::new(0));
        let app = app_with_policy(
            FakePolicy {
                leader: false,
                leader_addr: Some(server.address().to_string()),
                verify_ok: true,
                verify_calls: Arc::new(AtomicUsize::new(0)),
            },
            local_calls.clone(),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/v1/agent/self")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        assert!(server.received_requests().await.unwrap().is_empty());
        assert_eq!(local_calls.load(Ordering::SeqCst), 0);
    }

    #[actix_web::test]
    async fn conflicting_modes_return_bad_request() {
        let local_calls = Arc::new(AtomicUsize::new(0));
        let app = app_with_policy(
            FakePolicy {
                leader: true,
                leader_addr: None,
                verify_ok: true,
                verify_calls: Arc::new(AtomicUsize::new(0)),
            },
            local_calls.clone(),
        )
        .await;

        let req = actix_web::test::TestRequest::get()
            .uri("/v1/kv/app?stale&consistent")
            .to_request();
        let resp = actix_web::test::call_service(&app, req).await;
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        assert_eq!(local_calls.load(Ordering::SeqCst), 0);
    }
}
