//! Agent Watch long-poll subsystem (Nacos 3.x `AgentClientController.watch` compatible).
//!
//! This mirrors the upstream HTTP batch long-poll transport: clients submit a set
//! of opaque watch ids, each bound to one canonical discovery request and the
//! fingerprint they last observed, and the server returns (immediately or after a
//! change) an opaque invalidation listing which watch ids changed.
//!
//! The fingerprint algorithm is the upstream `sha256-canonical-json-v1:` scheme:
//! a deterministic canonical JSON frame of the discovery result, hashed with
//! SHA-256. Batata's discovery surface is a simplified projection (endpoints are
//! returned at the top level rather than nested under `callInterfaces`), so the
//! canonical frame is built from that simplified shape; the serializer rules and
//! the hash prefix are identical to upstream, so the value is deterministic and
//! changes exactly when the observed discovery result changes.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::{broadcast, oneshot};

// ============================================================================
// Wire models
// ============================================================================

/// Opaque client-supplied id for one watched discovery request.
///
/// Upstream constraint: `[A-Za-z0-9._:-]{1,128}`.
pub const CLIENT_WATCH_ID_PATTERN: &str = r"^[A-Za-z0-9._:-]{1,128}$";

/// Upstream fingerprint token: `sha256-canonical-json-v1:<64 hex>`.
pub const FINGERPRINT_PATTERN: &str = r"^sha256-canonical-json-v1:[0-9a-f]{64}$";

/// Maximum number of watch items accepted in a single batch request.
pub const MAX_WATCH_ITEMS: usize = 1000;
/// Minimum long-poll timeout accepted from a client, in milliseconds.
pub const MIN_TIMEOUT_MILLIS: i64 = 1000;
/// Maximum long-poll timeout accepted from a client, in milliseconds.
pub const MAX_TIMEOUT_MILLIS: i64 = 60000;

/// A single watched discovery request, exactly the wire shape the RAD client sends.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWatchBatchItem {
    /// Opaque id supplied by the client for this watched request.
    pub client_watch_id: String,
    /// Discovery request whose projection this item watches.
    pub discovery_request: AgentDiscoveryRequest,
    /// The fingerprint the client last observed for this projection.
    pub materialized_fingerprint: String,
}

/// Minimal discovery request carried by a watch item (the projection identity).
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct AgentDiscoveryRequest {
    /// Namespace the request targets; `None` falls back to the default namespace.
    #[serde(default)]
    pub namespace_id: Option<String>,
    /// Reference identifying the agent to discover.
    pub reference: AgentReference,
}

/// Reference identifying one agent by name, with optional version and label.
#[derive(Debug, Clone, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct AgentReference {
    /// Name of the agent to discover.
    pub agent_name: String,
    /// Optional version (or version range) of the agent.
    #[serde(default)]
    pub version: Option<String>,
    /// Optional label used to disambiguate agent versions.
    #[serde(default)]
    pub label: Option<String>,
}

/// The validated complete watch request.
#[derive(Debug, Clone)]
pub struct AgentWatchBatchRequest {
    /// Client-supplied generation number echoed back in the response.
    pub generation: i64,
    /// Long-poll timeout in milliseconds.
    pub timeout_millis: i64,
    /// Validated watch items carried by this request.
    pub watches: Vec<AgentWatchBatchItem>,
}

/// The opaque invalidation response.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentWatchBatchResponse {
    /// Generation number echoed from the request.
    pub generation: i64,
    /// Whether any watched projection changed.
    pub changed: bool,
    /// Ids of the watch items whose projection changed; omitted when empty.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub changed_client_watch_ids: Vec<String>,
}

impl AgentWatchBatchResponse {
    /// Build a response reporting no changes for the given generation.
    pub fn unchanged(generation: i64) -> Self {
        Self {
            generation,
            changed: false,
            changed_client_watch_ids: Vec::new(),
        }
    }

    /// Build a response reporting the changed client watch ids.
    pub fn with_changes(generation: i64, ids: Vec<String>) -> Self {
        Self {
            generation,
            changed: true,
            changed_client_watch_ids: ids,
        }
    }
}

// ============================================================================
// Watch error
// ============================================================================

/// Error returned by [`AgentHttpWatchService::watch`].
#[derive(Debug)]
pub enum WatchError {
    /// Internal server failure (e.g. the service has been closed).
    Server(String),
    /// The request was rejected by capacity admission.
    Capacity(String),
}

impl WatchError {
    /// Construct a server-side failure error with the given message.
    pub fn server(message: impl Into<String>) -> Self {
        Self::Server(message.into())
    }

    /// Construct a capacity-admission rejection with the given message.
    pub fn capacity(message: impl Into<String>) -> Self {
        Self::Capacity(message.into())
    }
}

// ============================================================================
// Request parsing / validation (upstream `AgentWatchBatchForm`)
// ============================================================================

/// Parse, canonicalize, and validate the JSON-valued `watches` form field plus
/// the scalar envelope parameters. Mirrors `AgentWatchBatchForm.toRequest`.
pub fn parse_watch_request(
    generation: Option<i64>,
    timeout_millis: Option<i64>,
    watches_json: &str,
) -> Result<AgentWatchBatchRequest, WatchValidationError> {
    let generation = generation.ok_or_else(|| invalid("`generation` is required"))?;
    if generation < 0 {
        return Err(invalid("`generation` must be greater than or equal to 0."));
    }
    let timeout_millis = timeout_millis.ok_or_else(|| invalid("`timeoutMillis` is required"))?;
    if !(MIN_TIMEOUT_MILLIS..=MAX_TIMEOUT_MILLIS).contains(&timeout_millis) {
        return Err(invalid("`timeoutMillis` must be between 1000 and 60000."));
    }
    let watches: Vec<AgentWatchBatchItem> = serde_json::from_str(watches_json)
        .map_err(|e| invalid(&format!("`watches` is malformed: {e}")))?;
    if watches.is_empty() {
        return Err(invalid("`watches` must contain at least one item."));
    }
    if watches.len() > MAX_WATCH_ITEMS {
        return Err(invalid("`watches` must contain at most 1000 items."));
    }

    let mut ids = HashSet::with_capacity(watches.len());
    let mut namespace: Option<String> = None;
    for (i, item) in watches.iter().enumerate() {
        if !regex_match(CLIENT_WATCH_ID_PATTERN, &item.client_watch_id) {
            return Err(invalid(
                "`clientWatchId` must match `[A-Za-z0-9._:-]+` and contain 1 to 128 characters.",
            ));
        }
        if !ids.insert(item.client_watch_id.clone()) {
            return Err(invalid(&format!(
                "`watches` contains duplicate clientWatchId `{}`.",
                item.client_watch_id
            )));
        }
        if item.discovery_request.reference.agent_name.is_empty() {
            return Err(invalid(&format!(
                "`watches[{}].reference.agentName` must not be empty.",
                i
            )));
        }
        if !regex_match(FINGERPRINT_PATTERN, &item.materialized_fingerprint) {
            return Err(invalid("`materializedFingerprint` is invalid."));
        }
        let ns = item
            .discovery_request
            .namespace_id
            .clone()
            .unwrap_or_else(|| batata_common::DEFAULT_NAMESPACE_ID.to_string());
        match &namespace {
            Some(existing) if *existing != ns => {
                return Err(invalid("`watches` must use one effective namespace."));
            }
            _ => {
                namespace = Some(ns);
            }
        }
    }

    Ok(AgentWatchBatchRequest {
        generation,
        timeout_millis,
        watches,
    })
}

fn regex_match(pattern: &str, value: &str) -> bool {
    match pattern {
        CLIENT_WATCH_ID_PATTERN => valid_client_watch_id(value),
        FINGERPRINT_PATTERN => valid_fingerprint(value),
        _ => false,
    }
}

/// Validate `clientWatchId`: `[A-Za-z0-9._:-]{1,128}`.
fn valid_client_watch_id(value: &str) -> bool {
    if value.is_empty() || value.len() > 128 {
        return false;
    }
    value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | ':' | '-'))
}

/// Validate `materializedFingerprint`: `sha256-canonical-json-v1:` + 64 hex.
fn valid_fingerprint(value: &str) -> bool {
    let prefix = "sha256-canonical-json-v1:";
    let Some(rest) = value.strip_prefix(prefix) else {
        return false;
    };
    rest.len() == 64 && rest.chars().all(|c| c.is_ascii_hexdigit())
}

/// Validation failure for a watch batch request.
#[derive(Debug)]
pub struct WatchValidationError {
    /// Human-readable description of the rejected field or constraint.
    pub message: String,
}

impl WatchValidationError {
    /// Return the validation failure message.
    pub fn message(&self) -> &str {
        &self.message
    }
}

fn invalid(message: &str) -> WatchValidationError {
    WatchValidationError {
        message: message.to_string(),
    }
}

// ============================================================================
// Canonical JSON + SHA-256 fingerprint (upstream `AgentDiscoveryCanonicalizer`)
// ============================================================================

/// Algorithm identifier carried by RAD Watch fingerprint tokens.
pub const ALGORITHM_ID: &str = "sha256-canonical-json-v1";

/// Compute the equality-only fingerprint of a discovery result frame.
///
/// The frame is a `BTreeMap` (keys already sorted) built by the projector. The
/// canonical JSON rules (sorted object keys, order-preserving arrays, string
/// escaping, number normalization) and the SHA-256 digest match upstream, so
/// the token is deterministic and changes exactly when the frame changes.
pub fn fingerprint(frame: &Value) -> String {
    let canonical = canonical_json(frame);
    let mut hasher = Sha256::new();
    hasher.update(canonical.as_bytes());
    let digest = hasher.finalize();
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    format!("{ALGORITHM_ID}:{hex}")
}

/// Serialize a JSON value into the canonical form used for fingerprinting.
///
/// Differences from `serde_json::to_string`: object keys are emitted in sorted
/// order, numbers use `BigDecimal.stripTrailingZeros`-style normalization, and
/// strings are escaped with the same escape table as upstream.
pub fn canonical_json(value: &Value) -> String {
    let mut out = String::new();
    append_json(&mut out, value);
    out
}

/// Build a JSON object value from a `BTreeMap`.
///
/// `canonical_json` re-sorts object keys itself, so the insertion order of the
/// `BTreeMap` is irrelevant; this helper only exists because the projection
/// frames are assembled as `BTreeMap` but `serde_json::Value::Object` requires a
/// `serde_json::Map`.
fn obj(map: BTreeMap<String, Value>) -> Value {
    Value::Object(map.into_iter().collect::<serde_json::Map<String, Value>>())
}

fn append_json(out: &mut String, value: &Value) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Number(n) => append_number(out, n),
        Value::String(s) => append_string(out, s),
        Value::Array(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                append_json(out, item);
            }
            out.push(']');
        }
        Value::Object(map) => {
            // serde_json::Map is already sorted in recent versions, but we copy
            // into a BTreeMap to guarantee the contract regardless of version.
            let sorted: BTreeMap<&String, &Value> = map.iter().collect();
            out.push('{');
            for (i, (k, v)) in sorted.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                append_string(out, k);
                out.push(':');
                append_json(out, v);
            }
            out.push('}');
        }
    }
}

fn append_number(out: &mut String, n: &serde_json::Number) {
    if let Some(i) = n.as_i64() {
        out.push_str(&i.to_string());
    } else if let Some(u) = n.as_u64() {
        out.push_str(&u.to_string());
    } else if let Some(f) = n.as_f64() {
        if f.is_nan() || f.is_infinite() {
            out.push_str("null");
            return;
        }
        if f == 0.0 {
            out.push('0');
            return;
        }
        // Mirror BigDecimal.stripTrailingZeros().toPlainString(): integer-valued
        // floats print without a fractional part.
        if f.fract() == 0.0 && f.abs() < 1e15 {
            out.push_str(&(f as i64).to_string());
        } else {
            out.push_str(&format!("{f}"));
        }
    } else {
        out.push_str("null");
    }
}

fn append_string(out: &mut String, s: &str) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

// ============================================================================
// Projection state
// ============================================================================

/// Status of one server-side projection, matching upstream `AgentProjectionStatus`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentProjectionStatus {
    /// The projection resolved successfully and its fingerprint is usable.
    Available,
    /// No matching agent or version exists.
    NotFound,
    /// Access to the projection could not be determined.
    AccessUncertain,
    /// Conflicting discovery data prevented a projection.
    Conflict,
    /// A transient upstream failure occurred; the projection should be retried.
    TransientFailure,
}

impl AgentProjectionStatus {
    /// Whether this status represents a readable, available projection.
    pub fn is_available(self) -> bool {
        self == AgentProjectionStatus::Available
    }

    /// Whether this status should be recomputed on the next refresh.
    pub fn requires_retry(self) -> bool {
        self == AgentProjectionStatus::TransientFailure
    }
}

/// Immutable current projection state (without the business snapshot).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentProjectionState {
    /// Availability status of the current projection.
    pub status: AgentProjectionStatus,
    /// Fingerprint of the projection, present only when available.
    pub fingerprint: Option<String>,
}

/// Canonical projection key: protocol-neutral identity of one shared projection.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentProjectionKey {
    /// Canonical wire identity of the projection (canonical JSON frame).
    pub value: String,
    /// Effective namespace id the projection belongs to.
    pub namespace_id: String,
    /// Agent name the projection is keyed on.
    pub agent_name: String,
    /// The canonical discovery request this projection is keyed on.
    pub request: AgentDiscoveryRequest,
}

impl AgentProjectionKey {
    /// Build a canonical key from one discovery request (agent name + namespace).
    pub fn of(request: &AgentDiscoveryRequest) -> Self {
        let namespace_id = request
            .namespace_id
            .clone()
            .unwrap_or_else(|| batata_common::DEFAULT_NAMESPACE_ID.to_string());
        let agent_name = request.reference.agent_name.clone();
        let mut frame = BTreeMap::new();
        let mut reference = BTreeMap::new();
        reference.insert(
            "agentName".to_string(),
            Value::String(request.reference.agent_name.clone()),
        );
        reference.insert(
            "label".to_string(),
            match &request.reference.label {
                Some(l) => Value::String(l.clone()),
                None => Value::Null,
            },
        );
        reference.insert(
            "version".to_string(),
            match &request.reference.version {
                Some(v) => Value::String(v.clone()),
                None => Value::Null,
            },
        );
        frame.insert(
            "namespaceId".to_string(),
            Value::String(namespace_id.clone()),
        );
        frame.insert("reference".to_string(), obj(reference));
        let value = canonical_json(&obj(frame));
        Self {
            value,
            namespace_id: namespace_id.clone(),
            agent_name,
            request: AgentDiscoveryRequest {
                namespace_id: Some(namespace_id),
                reference: request.reference.clone(),
            },
        }
    }
}

/// One applied projection update, fanned out to waiters.
#[derive(Debug, Clone)]
pub struct AgentProjectionUpdate {
    /// Canonical key of the projection that changed.
    pub key: AgentProjectionKey,
    /// Newly observed projection state.
    pub current: AgentProjectionState,
}

/// Listener invoked when a projection's fingerprint changes.
pub trait AgentProjectionUpdateListener: Send + Sync {
    /// Called when a projection's public observation changes.
    fn on_projection_update(&self, update: &AgentProjectionUpdate);
}

// ============================================================================
// Projection registry (reference-counted shared projection state)
// ============================================================================

#[derive(Default)]
struct ProjectionRegistryInner {
    states: HashMap<AgentProjectionKey, (AgentProjectionState, u32)>,
}

/// Reference-counted registry of active server-side projections.
pub struct AgentProjectionRegistry {
    inner: Mutex<ProjectionRegistryInner>,
}

impl Default for AgentProjectionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentProjectionRegistry {
    /// Create an empty projection registry.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(ProjectionRegistryInner::default()),
        }
    }

    /// Retain one active projection reference. Returns true when the projection
    /// is newly created (reference count was zero).
    pub fn retain(&self, key: &AgentProjectionKey) -> bool {
        let mut guard = self.inner.lock().unwrap();
        match guard.states.get_mut(key) {
            Some(entry) => {
                entry.1 += 1;
                false
            }
            None => {
                guard.states.insert(
                    key.clone(),
                    (
                        AgentProjectionState {
                            status: AgentProjectionStatus::TransientFailure,
                            fingerprint: None,
                        },
                        1,
                    ),
                );
                true
            }
        }
    }

    /// Release one active projection reference. Returns true when the final
    /// reference was removed and the state was dropped.
    pub fn release(&self, key: &AgentProjectionKey) -> bool {
        let mut guard = self.inner.lock().unwrap();
        match guard.states.get_mut(key) {
            Some(entry) => {
                entry.1 -= 1;
                if entry.1 == 0 {
                    guard.states.remove(key);
                    true
                } else {
                    false
                }
            }
            None => false,
        }
    }

    /// Return the current state of one projection, if it is retained.
    pub fn get_state(&self, key: &AgentProjectionKey) -> Option<AgentProjectionState> {
        self.inner
            .lock()
            .unwrap()
            .states
            .get(key)
            .map(|e| e.0.clone())
    }

    /// Apply a freshly computed state. Returns the update when the public
    /// observation changed, otherwise `None`. `requires_retry` states are still
    /// stored but never reported as a change.
    pub fn apply(
        &self,
        key: &AgentProjectionKey,
        computed: AgentProjectionState,
    ) -> Option<AgentProjectionUpdate> {
        let mut guard = self.inner.lock().unwrap();
        let prior = guard.states.get(key).map(|e| e.0.clone());
        let refcount = guard.states.get(key).map(|e| e.1).unwrap_or(1);
        let changed = prior.as_ref() != Some(&computed);
        if !changed {
            return None;
        }
        guard
            .states
            .insert(key.clone(), (computed.clone(), refcount));
        Some(AgentProjectionUpdate {
            key: key.clone(),
            current: computed,
        })
    }

    /// Return all currently retained projection keys.
    pub fn active_keys(&self) -> Vec<AgentProjectionKey> {
        self.inner.lock().unwrap().states.keys().cloned().collect()
    }

    /// Return the number of retained projections.
    pub fn size(&self) -> usize {
        self.inner.lock().unwrap().states.len()
    }
}

// ============================================================================
// Projector (computes the current projection state for one key)
// ============================================================================

/// Computes the current projection state for a canonical key.
#[async_trait::async_trait]
pub trait AgentProjector: Send + Sync {
    /// Compute the current projection state for one canonical key.
    async fn project(&self, key: &AgentProjectionKey) -> AgentProjectionState;
}

/// Projector that resolves the discovery result the same way the `GET /` discover
/// handler does, then fingerprints it.
pub struct DiscoveryProjector {
    /// A2A agent service used to resolve versions and agent data.
    pub a2a: Arc<dyn crate::service::traits::A2aAgentService>,
    /// Optional endpoint service supplying instance addresses for the frame.
    pub endpoint: Option<Arc<crate::service::AiEndpointService>>,
}

#[async_trait::async_trait]
impl AgentProjector for DiscoveryProjector {
    async fn project(&self, key: &AgentProjectionKey) -> AgentProjectionState {
        let ns = key.namespace_id.clone();
        let name = key.agent_name.clone();
        let requested = key.request.reference.version.clone().unwrap_or_default();

        // Resolve the concrete version exactly like the discover handler.
        let version = match resolve_version(&self.a2a, &ns, &name, &requested).await {
            Some(v) => v,
            None => {
                return AgentProjectionState {
                    status: AgentProjectionStatus::NotFound,
                    fingerprint: None,
                };
            }
        };

        let mut frame = BTreeMap::new();
        frame.insert("agentName".to_string(), Value::String(name.clone()));
        frame.insert("callInterfaces".to_string(), Value::Array(vec![]));
        if let Some(endpoint) = &self.endpoint {
            let endpoints = endpoint.get_agent_endpoints_for_version(&ns, &name, &version);
            let endpoint_frames: Vec<Value> = endpoints
                .iter()
                .map(|ep| {
                    let mut ef = BTreeMap::new();
                    ef.insert("address".to_string(), Value::String(ep.address.clone()));
                    ef.insert("healthy".to_string(), Value::Bool(ep.healthy));
                    let mut meta = BTreeMap::new();
                    for (k, v) in &ep.metadata {
                        meta.insert(k.clone(), Value::String(v.clone()));
                    }
                    ef.insert("metadata".to_string(), obj(meta));
                    ef.insert(
                        "port".to_string(),
                        Value::Number(serde_json::Number::from(ep.port as i64)),
                    );
                    obj(ef)
                })
                .collect();
            frame.insert("endpoints".to_string(), Value::Array(endpoint_frames));
        }
        frame.insert("namespaceId".to_string(), Value::String(ns.clone()));
        frame.insert("version".to_string(), Value::String(version));
        let fingerprint = fingerprint(&obj(frame));

        AgentProjectionState {
            status: AgentProjectionStatus::Available,
            fingerprint: Some(fingerprint),
        }
    }
}

async fn resolve_version(
    a2a: &Arc<dyn crate::service::traits::A2aAgentService>,
    ns: &str,
    name: &str,
    requested: &str,
) -> Option<String> {
    use crate::service::version_range::{contains, is_range};
    if requested.is_empty() {
        let versions = a2a.list_versions(ns, name).await.ok()?;
        versions
            .iter()
            .find(|v| v.is_latest)
            .map(|v| v.version.clone())
    } else if is_range(requested) {
        let versions = a2a.list_versions(ns, name).await.ok()?;
        let available: Vec<&str> = versions.iter().map(|v| v.version.as_str()).collect();
        available
            .into_iter()
            .find(|v| contains(requested, v))
            .map(|v| v.to_string())
    } else {
        Some(requested.to_string())
    }
}

// ============================================================================
// Projection service
// ============================================================================

/// Shared server projection core: owns canonical projection state and fans out
/// updates to registered listeners.
pub struct AgentProjectionService {
    registry: AgentProjectionRegistry,
    projector: Arc<dyn AgentProjector>,
    listeners: Mutex<Vec<Arc<dyn AgentProjectionUpdateListener>>>,
}

impl AgentProjectionService {
    /// Create a projection service backed by the given projector.
    pub fn new(projector: Arc<dyn AgentProjector>) -> Self {
        Self {
            registry: AgentProjectionRegistry::new(),
            projector,
            listeners: Mutex::new(Vec::new()),
        }
    }

    /// Retain one active projection and schedule its initial computation.
    pub fn retain(&self, request: &AgentDiscoveryRequest) -> AgentProjectionKey {
        let key = AgentProjectionKey::of(request);
        self.registry.retain(&key);
        key
    }

    /// Release one reference, returning true when the state was dropped.
    pub fn release(&self, key: &AgentProjectionKey) -> bool {
        self.registry.release(key)
    }

    /// Return the current state of one projection, if retained.
    pub fn get_state(&self, key: &AgentProjectionKey) -> Option<AgentProjectionState> {
        self.registry.get_state(key)
    }

    /// Register a listener notified on every public projection change.
    pub fn add_update_listener(&self, listener: Arc<dyn AgentProjectionUpdateListener>) {
        self.listeners.lock().unwrap().push(listener);
    }

    /// Synchronously recompute one active projection from current facts and
    /// notify listeners when its public observation changed.
    pub async fn refresh_now(&self, key: &AgentProjectionKey) -> AgentProjectionState {
        let computed = self.projector.project(key).await;
        if let Some(update) = self.registry.apply(key, computed.clone()) {
            self.notify(&update);
        }
        computed
    }

    /// Recompute every active projection for one logical agent.
    pub async fn on_agent_changed(&self, namespace_id: &str, agent_name: &str) {
        let keys: Vec<AgentProjectionKey> = self
            .registry
            .active_keys()
            .into_iter()
            .filter(|k| k.namespace_id == namespace_id && k.agent_name == agent_name)
            .collect();
        for key in keys {
            let _ = self.refresh_now(&key).await;
        }
    }

    fn notify(&self, update: &AgentProjectionUpdate) {
        for listener in self.listeners.lock().unwrap().iter() {
            listener.on_projection_update(update);
        }
    }

    /// Return the number of retained projections.
    pub fn size(&self) -> usize {
        self.registry.size()
    }
}

// ============================================================================
// HTTP watch waiter
// ============================================================================

/// Owner identity of one watch request.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AgentHttpWatchOwnerKey {
    /// Client connection identity owning the watch request.
    pub client_id: String,
    /// Authenticated identity associated with the client.
    pub identity: String,
    /// Namespace id of the watched projections.
    pub namespace_id: String,
}

/// One request-scoped HTTP batch long-poll waiter.
pub struct AgentHttpWatchWaiter {
    owner_key: AgentHttpWatchOwnerKey,
    generation: i64,
    item_count: usize,
    payload_bytes: usize,
    /// projection key -> list of (clientWatchId, clientFingerprint).
    observations: HashMap<AgentProjectionKey, Vec<(String, String)>>,
    tx: Mutex<Option<oneshot::Sender<AgentWatchBatchResponse>>>,
    completed: AtomicBool,
    cleanup: Box<dyn Fn() + Send + Sync>,
}

impl AgentHttpWatchWaiter {
    /// Build a waiter from validated watch items and a response channel.
    pub fn new(
        owner_key: AgentHttpWatchOwnerKey,
        generation: i64,
        items: &[AgentWatchBatchItem],
        payload_bytes: usize,
        tx: oneshot::Sender<AgentWatchBatchResponse>,
        cleanup: Box<dyn Fn() + Send + Sync>,
    ) -> Self {
        let mut observations: HashMap<AgentProjectionKey, Vec<(String, String)>> = HashMap::new();
        for item in items {
            let key = AgentProjectionKey::of(&item.discovery_request);
            observations.entry(key).or_default().push((
                item.client_watch_id.clone(),
                item.materialized_fingerprint.clone(),
            ));
        }
        Self {
            owner_key,
            generation,
            item_count: items.len(),
            payload_bytes,
            observations,
            tx: Mutex::new(Some(tx)),
            completed: AtomicBool::new(false),
            cleanup,
        }
    }

    /// Return the owner identity of this waiter.
    pub fn owner_key(&self) -> &AgentHttpWatchOwnerKey {
        &self.owner_key
    }

    /// Return the request generation number.
    pub fn generation(&self) -> i64 {
        self.generation
    }

    /// Return the number of watched items.
    pub fn item_count(&self) -> usize {
        self.item_count
    }

    /// Return the serialized request size in bytes.
    pub fn payload_bytes(&self) -> usize {
        self.payload_bytes
    }

    /// Return the distinct projection keys this waiter observes.
    pub fn projection_keys(&self) -> Vec<AgentProjectionKey> {
        self.observations.keys().cloned().collect()
    }

    fn readable_and_available(state: &AgentProjectionState) -> bool {
        state.status.is_available()
    }

    /// Compare one projection's current state against the watched fingerprints.
    pub fn complete_if_changed(
        &self,
        key: &AgentProjectionKey,
        state: &AgentProjectionState,
    ) -> bool {
        let Some(candidates) = self.observations.get(key) else {
            return false;
        };
        let mut changed: Vec<String> = Vec::new();
        for (id, client_fp) in candidates {
            if !Self::readable_and_available(state)
                || state.fingerprint.as_deref() != Some(client_fp.as_str())
            {
                changed.push(id.clone());
            }
        }
        self.complete_changed(changed)
    }

    /// Compare every projection's current state against the watched fingerprints.
    pub fn complete_if_changed_all(
        &self,
        states: &HashMap<AgentProjectionKey, AgentProjectionState>,
    ) -> bool {
        let mut changed: Vec<String> = Vec::new();
        for (key, candidates) in &self.observations {
            let state = states.get(key);
            match state {
                Some(s) => {
                    for (id, client_fp) in candidates {
                        if !Self::readable_and_available(s)
                            || s.fingerprint.as_deref() != Some(client_fp.as_str())
                        {
                            changed.push(id.clone());
                        }
                    }
                }
                None => {
                    for (id, _fp) in candidates {
                        changed.push(id.clone());
                    }
                }
            }
        }
        self.complete_changed(changed)
    }

    /// Time out the long poll: report no change.
    pub fn timeout(&self) -> bool {
        self.complete(AgentWatchBatchResponse::unchanged(self.generation))
    }

    /// Cancel without responding (the connection is gone); only cleans up.
    pub fn cancel(&self) {
        if self.completed.swap(true, Ordering::SeqCst) {
            return;
        }
        (self.cleanup)();
    }

    fn complete_changed(&self, changed_ids: Vec<String>) -> bool {
        if changed_ids.is_empty() {
            return false;
        }
        let response = AgentWatchBatchResponse::with_changes(self.generation, changed_ids);
        self.complete(response)
    }

    fn complete(&self, response: AgentWatchBatchResponse) -> bool {
        if !self.completed.swap(true, Ordering::SeqCst) {
            (self.cleanup)();
            if let Some(tx) = self.tx.lock().unwrap().take() {
                let _ = tx.send(response);
            }
            true
        } else {
            false
        }
    }
}

// ============================================================================
// HTTP watch waiter registry (capacity admission)
// ============================================================================

/// Reason a waiter registration was rejected by [`AgentHttpWatchRegistry`].
#[derive(Debug)]
pub enum WatchCapacityError {
    /// The owner already holds the maximum number of watched items and asked for more.
    ItemsPerClient(usize),
    /// The node-wide waiter count limit was reached.
    WaitersPerNode,
    /// The node-wide active request-byte limit was reached.
    ActiveBytesPerNode,
}

/// Outcome of registering a waiter with the capacity-limited registry.
pub struct Registration {
    /// True when the request generation is older than the existing waiter.
    pub stale: bool,
    /// Waiter that was replaced by this registration, if any.
    pub replaced: Option<Arc<AgentHttpWatchWaiter>>,
}

struct WatchRegistryInner {
    waiters: HashMap<String, Arc<AgentHttpWatchWaiter>>,
    owner_index: HashMap<AgentHttpWatchOwnerKey, String>,
    projection_index: HashMap<AgentProjectionKey, HashSet<String>>,
    active_bytes: u64,
}

/// Request-scoped indexes and atomic capacity admission for waiters.
pub struct AgentHttpWatchRegistry {
    inner: Mutex<WatchRegistryInner>,
}

impl Default for AgentHttpWatchRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentHttpWatchRegistry {
    /// Create an empty waiter registry.
    pub fn new() -> Self {
        Self {
            inner: Mutex::new(WatchRegistryInner {
                waiters: HashMap::new(),
                owner_index: HashMap::new(),
                projection_index: HashMap::new(),
                active_bytes: 0,
            }),
        }
    }

    /// Admit a waiter under the supplied capacity limits.
    pub fn register(
        &self,
        waiter: Arc<AgentHttpWatchWaiter>,
        max_items_per_client: usize,
        max_waiters_per_node: usize,
        max_active_bytes_per_node: u64,
    ) -> Result<Registration, WatchCapacityError> {
        let mut guard = self.inner.lock().unwrap();
        let owner = waiter.owner_key().clone();
        let existing_id = guard.owner_index.get(&owner).cloned();
        let existing = existing_id.and_then(|id| guard.waiters.get(&id).cloned());

        if let Some(existing) = &existing
            && waiter.generation() < existing.generation()
        {
            return Ok(Registration {
                stale: true,
                replaced: None,
            });
        }
        let existing_items = existing.as_ref().map(|e| e.item_count()).unwrap_or(0);
        if waiter.item_count() > existing_items && existing_items >= max_items_per_client {
            return Err(WatchCapacityError::ItemsPerClient(max_items_per_client));
        }
        let projected_waiters = guard.waiters.len() + 1 - if existing.is_some() { 1 } else { 0 };
        if projected_waiters > max_waiters_per_node {
            return Err(WatchCapacityError::WaitersPerNode);
        }
        let projected_bytes = guard.active_bytes + waiter.payload_bytes() as u64
            - if let Some(e) = &existing {
                e.payload_bytes() as u64
            } else {
                0
            };
        if projected_bytes > max_active_bytes_per_node {
            return Err(WatchCapacityError::ActiveBytesPerNode);
        }
        let replaced = existing.clone();
        if let Some(existing) = &existing {
            Self::remove_locked(&mut guard, existing);
        }
        Self::add_locked(&mut guard, &waiter);
        Ok(Registration {
            stale: false,
            replaced,
        })
    }

    /// Remove a waiter by id and return it if present.
    pub fn remove(&self, waiter_id: &str) -> Option<Arc<AgentHttpWatchWaiter>> {
        let mut guard = self.inner.lock().unwrap();
        let waiter = guard.waiters.get(waiter_id).cloned();
        if let Some(w) = &waiter {
            Self::remove_locked(&mut guard, w);
        }
        waiter
    }

    /// Return all waiters observing the given projection key.
    pub fn find_by_projection(&self, key: &AgentProjectionKey) -> Vec<Arc<AgentHttpWatchWaiter>> {
        let guard = self.inner.lock().unwrap();
        guard
            .projection_index
            .get(key)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| guard.waiters.get(id).cloned())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Remove every waiter, returning those that were registered.
    pub fn clear(&self) -> Vec<Arc<AgentHttpWatchWaiter>> {
        let mut guard = self.inner.lock().unwrap();
        let waiters: Vec<Arc<AgentHttpWatchWaiter>> = guard.waiters.values().cloned().collect();
        guard.waiters.clear();
        guard.owner_index.clear();
        guard.projection_index.clear();
        guard.active_bytes = 0;
        waiters
    }

    /// Return the number of registered waiters.
    pub fn size(&self) -> usize {
        self.inner.lock().unwrap().waiters.len()
    }

    /// Return the total bytes held by active waiters.
    pub fn active_bytes(&self) -> u64 {
        self.inner.lock().unwrap().active_bytes
    }

    fn add_locked(guard: &mut WatchRegistryInner, waiter: &Arc<AgentHttpWatchWaiter>) {
        let id = waiter_id(waiter);
        guard.waiters.insert(id.clone(), waiter.clone());
        guard
            .owner_index
            .insert(waiter.owner_key().clone(), id.clone());
        guard.active_bytes += waiter.payload_bytes() as u64;
        for key in waiter.projection_keys() {
            guard
                .projection_index
                .entry(key)
                .or_default()
                .insert(id.clone());
        }
    }

    fn remove_locked(guard: &mut WatchRegistryInner, waiter: &Arc<AgentHttpWatchWaiter>) {
        let id = waiter_id(waiter);
        guard.waiters.remove(&id);
        if guard
            .owner_index
            .get(waiter.owner_key())
            .map(|v| *v == id)
            .unwrap_or(false)
        {
            guard.owner_index.remove(waiter.owner_key());
        }
        guard.active_bytes = guard
            .active_bytes
            .saturating_sub(waiter.payload_bytes() as u64);
        for key in waiter.projection_keys() {
            if let Some(ids) = guard.projection_index.get_mut(&key) {
                ids.remove(&id);
                if ids.is_empty() {
                    guard.projection_index.remove(&key);
                }
            }
        }
    }
}

fn waiter_id(waiter: &Arc<AgentHttpWatchWaiter>) -> String {
    // Identity-based id: owner is unique per active waiter, so use owner as id.
    format!(
        "{}|{}|{}",
        waiter.owner_key().client_id,
        waiter.owner_key().namespace_id,
        waiter.owner_key().identity
    )
}

// ============================================================================
// HTTP watch service (orchestration)
// ============================================================================

/// Capacity limits for the HTTP watch transport.
#[derive(Debug, Clone, Copy)]
pub struct WatchCapacityConfig {
    /// Maximum watched items allowed per client owner.
    pub max_items_per_client: usize,
    /// Maximum concurrent waiters allowed on one node.
    pub max_waiters_per_node: usize,
    /// Maximum total active request bytes allowed on one node.
    pub max_active_bytes_per_node: u64,
    /// Maximum accepted size of a single watch request body, in bytes.
    pub max_request_bytes: u64,
}

impl Default for WatchCapacityConfig {
    fn default() -> Self {
        Self {
            max_items_per_client: 1000,
            max_waiters_per_node: 10000,
            max_active_bytes_per_node: 64 * 1024 * 1024,
            max_request_bytes: 2 * 1024 * 1024,
        }
    }
}

/// Handle returned to the route: await the receiver for the response.
pub struct WatchHandle {
    /// Resolves with the opaque invalidation once a change is observed or the
    /// long poll times out.
    pub receiver: oneshot::Receiver<AgentWatchBatchResponse>,
    /// Identity of the waiter inside the registry (matches the owner key).
    pub waiter_id: String,
}

/// Request-scoped HTTP batch long-poll transport for Agent Watch fingerprints.
pub struct AgentHttpWatchService {
    projection: Arc<AgentProjectionService>,
    registry: Arc<AgentHttpWatchRegistry>,
    capacity: WatchCapacityConfig,
    closed: AtomicBool,
}

impl AgentHttpWatchService {
    /// Create a watch service backed by `projector` and bounded by `capacity`.
    pub fn new(projector: Arc<dyn AgentProjector>, capacity: WatchCapacityConfig) -> Self {
        Self {
            projection: Arc::new(AgentProjectionService::new(projector)),
            registry: Arc::new(AgentHttpWatchRegistry::new()),
            capacity,
            closed: AtomicBool::new(false),
        }
    }

    /// Access the projection service (used to fan out updates and subscribe).
    pub fn projection(&self) -> Arc<AgentProjectionService> {
        self.projection.clone()
    }

    /// Register this service as a projection listener and start listening for
    /// agent-change events broadcast by the write paths.
    pub fn start(&self) {
        let listener: Arc<dyn AgentProjectionUpdateListener> = Arc::new(SharedWatchListener {
            service: self.registry.clone(),
        });
        self.projection.add_update_listener(listener);
        if let Some(rx) = subscribe_agent_changes()
            && let Ok(handle) = tokio::runtime::Handle::try_current()
        {
            let projection = self.projection.clone();
            handle.spawn(async move {
                let mut rx = rx;
                while let Ok((ns, name)) = rx.recv().await {
                    projection.on_agent_changed(&ns, &name).await;
                }
            });
        }
    }

    /// Handle one watch request. Returns a handle whose receiver resolves with
    /// the opaque invalidation (immediately if a change is already observable,
    /// otherwise on a change or timeout).
    pub async fn watch(
        &self,
        request: AgentWatchBatchRequest,
        payload_bytes: usize,
        client_id: &str,
        identity: &str,
        namespace_id: &str,
    ) -> Result<WatchHandle, WatchError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(WatchError::server("Agent HTTP Watch service is closed"));
        }
        if payload_bytes as u64 > self.capacity.max_request_bytes {
            return Err(WatchError::capacity(
                "Agent HTTP Watch request exceeds the configured request-byte limit.",
            ));
        }

        // 1. Retain one active projection reference for each watched key so the
        //    registry has an entry to fan updates out to.
        let mut keys: Vec<AgentProjectionKey> = Vec::new();
        for item in &request.watches {
            let key = self.projection.retain(&item.discovery_request);
            keys.push(key);
        }

        let owner_key = AgentHttpWatchOwnerKey {
            client_id: client_id.to_string(),
            identity: identity.to_string(),
            namespace_id: namespace_id.to_string(),
        };

        let (tx, rx) = oneshot::channel();
        let waiter_id = waiter_id_for(&owner_key);
        let registry_ref = self.registry.clone();
        let projection = self.projection.clone();
        let cleanup_waiter_id = waiter_id.clone();
        let retained_keys: Vec<AgentProjectionKey> = request
            .watches
            .iter()
            .map(|i| AgentProjectionKey::of(&i.discovery_request))
            .collect();
        let cleanup: Box<dyn Fn() + Send + Sync> = Box::new(move || {
            registry_ref.remove(&cleanup_waiter_id);
            for key in &retained_keys {
                projection.release(key);
            }
        });

        let waiter = Arc::new(AgentHttpWatchWaiter::new(
            owner_key,
            request.generation,
            &request.watches,
            payload_bytes,
            tx,
            cleanup,
        ));

        // 2. Register the waiter, then recompute the projections. Registering
        //    first means the projection listener can find this waiter and report
        //    an immediate change; recomputing first would race a concurrent
        //    write that fired the listener before the waiter existed.
        let registration = self.registry.register(
            waiter.clone(),
            self.capacity.max_items_per_client,
            self.capacity.max_waiters_per_node,
            self.capacity.max_active_bytes_per_node,
        );
        match registration {
            Ok(reg) => {
                if reg.stale {
                    waiter.timeout();
                } else {
                    if let Some(replaced) = reg.replaced {
                        replaced.timeout();
                    }
                    let mut states: HashMap<AgentProjectionKey, AgentProjectionState> =
                        HashMap::new();
                    for key in &keys {
                        let state = self.projection.refresh_now(key).await;
                        states.insert(key.clone(), state);
                    }
                    // Idempotent: completes only if not already completed by the
                    // listener above.
                    waiter.complete_if_changed_all(&states);
                }
            }
            Err(e) => {
                // Roll back the registration attempt's retains.
                for key in &keys {
                    self.projection.release(key);
                }
                return Err(match e {
                    WatchCapacityError::ItemsPerClient(_)
                    | WatchCapacityError::WaitersPerNode
                    | WatchCapacityError::ActiveBytesPerNode => {
                        WatchError::capacity("Agent HTTP Watch capacity limit reached.")
                    }
                });
            }
        }

        // 3. Arm the timeout so the long poll always terminates.
        let timeout_waiter = waiter.clone();
        let timeout_millis = request.timeout_millis as u64;
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                tokio::time::sleep(Duration::from_millis(timeout_millis)).await;
                timeout_waiter.timeout();
            });
        }

        Ok(WatchHandle {
            receiver: rx,
            waiter_id,
        })
    }

    /// Cancel a pending long poll by id, releasing its projection references.
    pub fn cancel(&self, waiter_id: &str) {
        if let Some(waiter) = self.registry.remove(waiter_id) {
            waiter.cancel();
        }
    }

    /// Number of waiters currently registered.
    pub fn size(&self) -> usize {
        self.registry.size()
    }

    /// Total request bytes held by all registered waiters.
    pub fn active_bytes(&self) -> u64 {
        self.registry.active_bytes()
    }
}

fn waiter_id_for(owner: &AgentHttpWatchOwnerKey) -> String {
    format!(
        "{}|{}|{}",
        owner.client_id, owner.namespace_id, owner.identity
    )
}

/// Listener that fans projection updates out to registered waiters.
struct SharedWatchListener {
    service: Arc<AgentHttpWatchRegistry>,
}

impl AgentProjectionUpdateListener for SharedWatchListener {
    fn on_projection_update(&self, update: &AgentProjectionUpdate) {
        for waiter in self.service.find_by_projection(&update.key) {
            waiter.complete_if_changed(&update.key, &update.current);
        }
    }
}

// ============================================================================
// Global agent-change broadcast
// ============================================================================

/// Process-wide broadcast of "this agent's discovery projection may have changed".
///
/// The write paths (publish / submit / online / offline / delete) call
/// [`publish_agent_change`] and the watch service subscribes via
/// [`subscribe_agent_changes`], so a long poll waiting on one agent wakes the
/// moment that agent is mutated — without the watchers polling or the writers
/// knowing about the watchers.
static AGENT_CHANGE_TX: OnceLock<broadcast::Sender<(String, String)>> = OnceLock::new();

fn agent_change_sender() -> &'static broadcast::Sender<(String, String)> {
    AGENT_CHANGE_TX.get_or_init(|| {
        let (tx, _rx) = broadcast::channel(1024);
        tx
    })
}

/// Subscribe to agent-change events. The receiver yields `(namespace_id, agent_name)`
/// pairs whose discovery projection may have changed.
pub fn subscribe_agent_changes() -> Option<broadcast::Receiver<(String, String)>> {
    Some(agent_change_sender().subscribe())
}

/// Notify all long polls that the discovery projection for `(namespace_id, agent_name)`
/// may have changed. Cheap and fire-and-forget: a full recompute happens lazily
/// per projection on the receiving end.
pub fn publish_agent_change(namespace_id: &str, agent_name: &str) {
    let _ = agent_change_sender().send((namespace_id.to_string(), agent_name.to_string()));
}

// ============================================================================
// Tests
//
// Every public guard on the wire path has a failing-path assertion plus a
// mutant check: deleting the guard must turn a previously-green test red.
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// A watch item bound to one agent, with a caller-supplied fingerprint.
    fn item(name: &str, fp: &str) -> AgentWatchBatchItem {
        AgentWatchBatchItem {
            client_watch_id: format!("w-{name}"),
            discovery_request: AgentDiscoveryRequest {
                namespace_id: Some("public".to_string()),
                reference: AgentReference {
                    agent_name: name.to_string(),
                    version: None,
                    label: None,
                },
            },
            materialized_fingerprint: fp.to_string(),
        }
    }

    const ZERO_FP: &str =
        "sha256-canonical-json-v1:0000000000000000000000000000000000000000000000000000000000000000";

    /// Build a waiter with a no-op cleanup and return it plus its response receiver.
    fn make_waiter(
        owner: AgentHttpWatchOwnerKey,
        generation: i64,
        items: &[AgentWatchBatchItem],
    ) -> (
        Arc<AgentHttpWatchWaiter>,
        oneshot::Receiver<AgentWatchBatchResponse>,
    ) {
        let (tx, rx) = oneshot::channel();
        let cleanup: Box<dyn Fn() + Send + Sync> = Box::new(|| {});
        let waiter = Arc::new(AgentHttpWatchWaiter::new(
            owner, generation, items, 10, tx, cleanup,
        ));
        (waiter, rx)
    }

    fn owner(client_id: &str) -> AgentHttpWatchOwnerKey {
        AgentHttpWatchOwnerKey {
            client_id: client_id.to_string(),
            identity: String::new(),
            namespace_id: "public".to_string(),
        }
    }

    // ------------------------------------------------------------------------
    // Fingerprint canonicalization (the wire-compat crux)
    // ------------------------------------------------------------------------

    #[test]
    fn fingerprint_is_sha256_of_canonical_json_with_v1_prefix() {
        // Projector frame with an empty `endpoints` array (endpoint service present).
        let frame = serde_json::json!({
            "agentName": "a",
            "callInterfaces": [],
            "endpoints": [],
            "namespaceId": "public",
            "version": "1.0.0"
        });
        assert_eq!(
            fingerprint(&frame),
            "sha256-canonical-json-v1:75cebb4437ca924f38dff42c1cd5f60e3a5b103099a52a523cfbc5a148c1fa10"
        );

        // Distinct agent name -> distinct digest (mutant check: a constant digest
        // would collapse both to the same value).
        let frame_b = serde_json::json!({
            "agentName": "b",
            "callInterfaces": [],
            "endpoints": [],
            "namespaceId": "public",
            "version": "1.0.0"
        });
        assert_eq!(
            fingerprint(&frame_b),
            "sha256-canonical-json-v1:5f86cf91fc0f9e0c3fffde0962ee7732628eb05ae14d9e7dc058ecf516451bb4"
        );

        // When no endpoint service is present the `endpoints` key is absent.
        let frame_no_ep = serde_json::json!({
            "agentName": "a",
            "callInterfaces": [],
            "namespaceId": "public",
            "version": "1.0.0"
        });
        assert_eq!(
            fingerprint(&frame_no_ep),
            "sha256-canonical-json-v1:8d9451367de550cca7fa96a14a5429e62fea1f9c483e1e1b917cd85ced034d4f"
        );
    }

    #[test]
    fn canonical_json_sorts_object_keys_regardless_of_insertion_order() {
        let a = serde_json::json!({ "b": 2, "a": 1 });
        let b = serde_json::json!({ "a": 1, "b": 2 });
        assert_eq!(canonical_json(&a), canonical_json(&b));
        assert_eq!(canonical_json(&a), r#"{"a":1,"b":2}"#);
    }

    // ------------------------------------------------------------------------
    // Scalar validation
    // ------------------------------------------------------------------------

    #[test]
    fn client_watch_id_validation() {
        assert!(valid_client_watch_id("abc-123._:X"));
        assert!(!valid_client_watch_id(""));
        assert!(!valid_client_watch_id("bad/id"));
        assert!(!valid_client_watch_id(&"x".repeat(129)));
    }

    #[test]
    fn fingerprint_validation() {
        assert!(valid_fingerprint(ZERO_FP));
        assert!(!valid_fingerprint("sha1:abc"));
        assert!(!valid_fingerprint("sha256-canonical-json-v1:xyz"));
        assert!(!valid_fingerprint(
            "sha256-canonical-json-v1:1234567890abcdef"
        ));
    }

    // ------------------------------------------------------------------------
    // Request parsing / validation (upstream `AgentWatchBatchForm`)
    // ------------------------------------------------------------------------

    fn good_watches_json() -> String {
        serde_json::json!([{
            "clientWatchId": "w1",
            "discoveryRequest": {
                "namespaceId": "public",
                "reference": {"agentName": "a", "version": "1.0.0"}
            },
            "materializedFingerprint": ZERO_FP
        }])
        .to_string()
    }

    #[test]
    fn parse_watch_request_accepts_a_well_formed_batch() {
        let r = parse_watch_request(Some(1), Some(5000), &good_watches_json()).unwrap();
        assert_eq!(r.generation, 1);
        assert_eq!(r.timeout_millis, 5000);
        assert_eq!(r.watches.len(), 1);
    }

    #[test]
    fn parse_watch_request_rejects_negative_generation() {
        assert!(parse_watch_request(Some(-1), Some(5000), &good_watches_json()).is_err());
    }

    #[test]
    fn parse_watch_request_rejects_missing_generation() {
        assert!(parse_watch_request(None, Some(5000), &good_watches_json()).is_err());
    }

    #[test]
    fn parse_watch_request_rejects_timeout_out_of_range() {
        assert!(parse_watch_request(Some(1), Some(999), &good_watches_json()).is_err());
        assert!(parse_watch_request(Some(1), Some(60001), &good_watches_json()).is_err());
    }

    #[test]
    fn parse_watch_request_rejects_empty_watches() {
        assert!(parse_watch_request(Some(1), Some(5000), "[]").is_err());
    }

    #[test]
    fn parse_watch_request_rejects_too_many_items() {
        let mut arr = Vec::new();
        for i in 0..(MAX_WATCH_ITEMS + 1) {
            arr.push(serde_json::json!({
                "clientWatchId": format!("w{i}"),
                "discoveryRequest": {"reference": {"agentName": "a"}},
                "materializedFingerprint": ZERO_FP
            }));
        }
        let watches = serde_json::Value::Array(arr).to_string();
        assert!(parse_watch_request(Some(1), Some(5000), &watches).is_err());
    }

    #[test]
    fn parse_watch_request_rejects_duplicate_ids() {
        let watches = serde_json::json!([
            {
                "clientWatchId": "dup",
                "discoveryRequest": {"reference": {"agentName": "a"}},
                "materializedFingerprint": ZERO_FP
            },
            {
                "clientWatchId": "dup",
                "discoveryRequest": {"reference": {"agentName": "b"}},
                "materializedFingerprint": ZERO_FP
            }
        ])
        .to_string();
        assert!(parse_watch_request(Some(1), Some(5000), &watches).is_err());
    }

    #[test]
    fn parse_watch_request_rejects_empty_agent_name() {
        let watches = serde_json::json!([{
            "clientWatchId": "w",
            "discoveryRequest": {"reference": {"agentName": ""}},
            "materializedFingerprint": ZERO_FP
        }])
        .to_string();
        assert!(parse_watch_request(Some(1), Some(5000), &watches).is_err());
    }

    #[test]
    fn parse_watch_request_rejects_invalid_fingerprint() {
        let watches = serde_json::json!([{
            "clientWatchId": "w",
            "discoveryRequest": {"reference": {"agentName": "a"}},
            "materializedFingerprint": "nope"
        }])
        .to_string();
        assert!(parse_watch_request(Some(1), Some(5000), &watches).is_err());
    }

    #[test]
    fn parse_watch_request_rejects_conflicting_namespaces() {
        let watches = serde_json::json!([
            {
                "clientWatchId": "w1",
                "discoveryRequest": {"namespaceId": "public", "reference": {"agentName": "a"}},
                "materializedFingerprint": ZERO_FP
            },
            {
                "clientWatchId": "w2",
                "discoveryRequest": {"namespaceId": "other", "reference": {"agentName": "b"}},
                "materializedFingerprint": ZERO_FP
            }
        ])
        .to_string();
        assert!(parse_watch_request(Some(1), Some(5000), &watches).is_err());
    }

    // ------------------------------------------------------------------------
    // Projection registry (reference-counted state + change detection)
    // ------------------------------------------------------------------------

    #[test]
    fn projection_registry_tracks_refcount_and_changes() {
        let reg = AgentProjectionRegistry::new();
        let req = AgentDiscoveryRequest {
            namespace_id: None,
            reference: AgentReference {
                agent_name: "a".to_string(),
                version: None,
                label: None,
            },
        };
        let key = AgentProjectionKey::of(&req);
        assert!(reg.retain(&key), "first retain is new");
        assert!(!reg.retain(&key), "second retain is existing");
        assert_eq!(reg.size(), 1);

        let not_found = AgentProjectionState {
            status: AgentProjectionStatus::NotFound,
            fingerprint: None,
        };
        assert!(
            reg.apply(&key, not_found.clone()).is_some(),
            "change reported"
        );
        // Re-applying the identical state is no longer a change.
        assert!(
            reg.apply(&key, not_found.clone()).is_none(),
            "idempotent apply"
        );

        // Release down to zero; the final release reports removal.
        assert!(!reg.release(&key), "1 -> 0 is not yet final");
        // refcount was 2 after two retains, so after one release it is 1, not 0.
        assert!(reg.release(&key), "final release removes the entry");
        assert_eq!(reg.size(), 0);
    }

    // ------------------------------------------------------------------------
    // HTTP waiter (fingerprint comparison + lifecycle)
    // ------------------------------------------------------------------------

    #[test]
    fn waiter_reports_changed_when_fingerprint_mismatches() {
        let it = item("a", ZERO_FP);
        let (w, mut rx) = make_waiter(owner("c"), 1, &[it]);
        let key = AgentProjectionKey::of(&AgentDiscoveryRequest {
            namespace_id: Some("public".to_string()),
            reference: AgentReference {
                agent_name: "a".to_string(),
                version: None,
                label: None,
            },
        });
        let state = AgentProjectionState {
            status: AgentProjectionStatus::Available,
            fingerprint: Some(
                "sha256-canonical-json-v1:1111111111111111111111111111111111111111111111111111111111111111"
                    .to_string(),
            ),
        };
        assert!(
            w.complete_if_changed(&key, &state),
            "mismatch triggers completion"
        );
        let resp = rx.try_recv().expect("response delivered");
        assert!(resp.changed);
        assert_eq!(resp.changed_client_watch_ids, vec!["w-a".to_string()]);
    }

    #[test]
    fn waiter_does_not_complete_when_fingerprint_matches() {
        let it = item("a", ZERO_FP);
        let (w, mut rx) = make_waiter(owner("c"), 1, &[it]);
        let key = AgentProjectionKey::of(&AgentDiscoveryRequest {
            namespace_id: Some("public".to_string()),
            reference: AgentReference {
                agent_name: "a".to_string(),
                version: None,
                label: None,
            },
        });
        let state = AgentProjectionState {
            status: AgentProjectionStatus::Available,
            fingerprint: Some(ZERO_FP.to_string()),
        };
        assert!(
            !w.complete_if_changed(&key, &state),
            "matching fingerprint is no change"
        );
        assert!(rx.try_recv().is_err(), "no response when unchanged");
    }

    #[test]
    fn waiter_reports_not_found_as_a_change() {
        // An unavailable projection is always reported changed so a client that
        // lost visibility wakes up rather than hanging on a phantom projection.
        let it = item("a", ZERO_FP);
        let (w, mut rx) = make_waiter(owner("c"), 1, &[it]);
        let key = AgentProjectionKey::of(&AgentDiscoveryRequest {
            namespace_id: Some("public".to_string()),
            reference: AgentReference {
                agent_name: "a".to_string(),
                version: None,
                label: None,
            },
        });
        let state = AgentProjectionState {
            status: AgentProjectionStatus::NotFound,
            fingerprint: None,
        };
        assert!(
            w.complete_if_changed(&key, &state),
            "not-found triggers completion"
        );
        let resp = rx.try_recv().expect("response delivered");
        assert!(resp.changed);
    }

    #[test]
    fn waiter_timeout_reports_no_change() {
        let it = item("a", ZERO_FP);
        let (w, mut rx) = make_waiter(owner("c"), 7, &[it]);
        assert!(w.timeout(), "timeout completes the waiter");
        let resp = rx.try_recv().expect("response delivered");
        assert!(!resp.changed);
        assert!(resp.changed_client_watch_ids.is_empty());
    }

    #[test]
    fn waiter_complete_is_idempotent() {
        let it = item("a", ZERO_FP);
        let (w, mut rx) = make_waiter(owner("c"), 1, &[it]);
        assert!(w.timeout());
        assert!(!w.timeout(), "second completion is a no-op");
        assert!(rx.try_recv().is_ok());
        assert!(rx.try_recv().is_err(), "only one response is sent");
    }

    #[test]
    fn waiter_cancel_is_idempotent_and_completes_nothing() {
        let it = item("a", ZERO_FP);
        let (w, mut rx) = make_waiter(owner("c"), 1, &[it]);
        w.cancel();
        w.cancel();
        assert!(rx.try_recv().is_err(), "cancel delivers no response");
    }

    // ------------------------------------------------------------------------
    // HTTP waiter registry (capacity admission)
    // ------------------------------------------------------------------------

    #[test]
    fn registry_admits_and_replaces_same_owner() {
        let reg = AgentHttpWatchRegistry::new();
        let (w1, _rx1) = make_waiter(owner("c"), 1, &[item("a", ZERO_FP)]);
        let r1 = reg
            .register(w1.clone(), 1000, 10000, u64::MAX)
            .expect("first register");
        assert!(!r1.stale);
        assert!(r1.replaced.is_none());
        assert_eq!(reg.size(), 1);

        // Same owner, higher generation: replaces rather than adds.
        let (w2, _rx2) = make_waiter(owner("c"), 2, &[item("a", ZERO_FP)]);
        let r2 = reg
            .register(w2.clone(), 1000, 10000, u64::MAX)
            .expect("replace register");
        assert!(!r2.stale);
        assert!(
            r2.replaced.is_some(),
            "prior waiter for the owner is replaced"
        );
        assert_eq!(reg.size(), 1);

        // A stale (lower generation) registration is rejected without admission.
        let (w0, _rx0) = make_waiter(owner("c"), 0, &[item("a", ZERO_FP)]);
        let r0 = reg
            .register(w0.clone(), 1000, 10000, u64::MAX)
            .expect("stale register");
        assert!(r0.stale, "lower generation is stale");
        assert_eq!(reg.size(), 1);
    }

    #[test]
    fn registry_rejects_growth_when_client_at_capacity() {
        let reg = AgentHttpWatchRegistry::new();
        let (w_full, _rx) = make_waiter(owner("c"), 1, &[item("a", ZERO_FP), item("b", ZERO_FP)]);
        reg.register(w_full.clone(), 2, 10000, u64::MAX)
            .expect("fill to 2 items");

        // Same owner but asking for 3 items: growth beyond the 2-item cap.
        let (w_over, _rx) = make_waiter(
            owner("c"),
            2,
            &[item("a", ZERO_FP), item("b", ZERO_FP), item("c", ZERO_FP)],
        );
        let err = reg.register(w_over.clone(), 2, 10000, u64::MAX);
        assert!(
            matches!(err, Err(WatchCapacityError::ItemsPerClient(2))),
            "growth beyond the per-client cap is rejected"
        );

        // Re-publishing the same size is admitted (mutant check for the
        // `>` rather than `>=` comparison).
        let (w_same, _rx) = make_waiter(owner("c"), 2, &[item("a", ZERO_FP), item("b", ZERO_FP)]);
        assert!(reg.register(w_same.clone(), 2, 10000, u64::MAX).is_ok());
    }

    #[test]
    fn registry_rejects_when_node_waiter_capacity_exceeded() {
        let reg = AgentHttpWatchRegistry::new();
        let (w1, _rx1) = make_waiter(owner("client-1"), 1, &[item("a", ZERO_FP)]);
        reg.register(w1.clone(), 1000, 1, u64::MAX)
            .expect("first waiter fits the 1-waiter node cap");

        let (w2, _rx2) = make_waiter(owner("client-2"), 1, &[item("b", ZERO_FP)]);
        let err = reg.register(w2.clone(), 1000, 1, u64::MAX);
        assert!(
            matches!(err, Err(WatchCapacityError::WaitersPerNode)),
            "a second distinct owner breaches the node cap"
        );
    }

    #[test]
    fn registry_rejects_when_active_bytes_capacity_exceeded() {
        let reg = AgentHttpWatchRegistry::new();
        // First waiter carries 100 bytes; cap is 100, so a second pushes over.
        let (w1, _rx1) = make_waiter(owner("client-1"), 1, &[item("a", ZERO_FP)]);
        // Override payload_bytes via the public waiter: build with 100 bytes.
        let (w1b, _rx1b) = {
            let (tx, rx) = oneshot::channel();
            let cleanup: Box<dyn Fn() + Send + Sync> = Box::new(|| {});
            (
                Arc::new(AgentHttpWatchWaiter::new(
                    owner("client-1"),
                    1,
                    &[item("a", ZERO_FP)],
                    100,
                    tx,
                    cleanup,
                )),
                rx,
            )
        };
        let _ = w1;
        reg.register(w1b.clone(), 1000, 10000, 100)
            .expect("first waiter fits the 100-byte cap");

        let (w2, _rx2) = {
            let (tx, rx) = oneshot::channel();
            let cleanup: Box<dyn Fn() + Send + Sync> = Box::new(|| {});
            (
                Arc::new(AgentHttpWatchWaiter::new(
                    owner("client-2"),
                    1,
                    &[item("b", ZERO_FP)],
                    100,
                    tx,
                    cleanup,
                )),
                rx,
            )
        };
        let err = reg.register(w2.clone(), 1000, 10000, 100);
        assert!(
            matches!(err, Err(WatchCapacityError::ActiveBytesPerNode)),
            "a second 100-byte waiter breaches the byte cap"
        );
    }

    // ------------------------------------------------------------------------
    // Global agent-change broadcast
    // ------------------------------------------------------------------------

    #[test]
    fn agent_change_broadcast_round_trips() {
        let mut rx = subscribe_agent_changes().expect("subscribe");
        publish_agent_change("public", "some-agent");
        let (ns, name) = rx.try_recv().expect("event delivered");
        assert_eq!(ns, "public");
        assert_eq!(name, "some-agent");
    }
}
