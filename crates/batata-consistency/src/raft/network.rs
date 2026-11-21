//! Raft network layer for gRPC-based inter-node communication.
//!
//! Implements openraft's network traits using tonic gRPC.

use crate::bincode;
use std::future::Future;
use std::time::Duration;

use openraft::errors::{NetworkError, RPCError, ReplicationClosed, StreamingError, Unreachable};
use openraft::network::{RPCOption, RaftNetworkV2};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, AppendResult, SnapshotResponse, VoteRequest,
    VoteResponse,
};
use openraft::{BasicNode, OptionalSend};
use tonic::transport::{Channel, Endpoint};
use tracing::{debug, error, warn};

use batata_server_api::raft::{
    AppendEntriesRequest as ProtoAppendEntriesRequest, Entry as ProtoEntry, LogId as ProtoLogId,
    Vote as ProtoVote, VoteRequest as ProtoVoteRequest, raft_service_client::RaftServiceClient,
};

use super::types::{CommittedLeaderId, Entry, LogId, NodeId, Snapshot, TypeConfig, Vote};

/// Network timeout configuration for Raft gRPC connections
#[derive(Clone, Debug)]
pub struct RaftNetworkConfig {
    /// Timeout for regular RPCs (AppendEntries, Vote)
    ///
    /// Channel-level ceiling, NOT the per-request deadline: openraft 0.10.1
    /// supplies `RPCOption` for each RPC and the transport must enforce
    /// `soft_ttl()`. This value is also the fallback when openraft supplies a
    /// zero TTL. See `rpc_deadline` and `docs/raft-timing.md`.
    pub rpc_timeout: Duration,
    /// Timeout for snapshot transfer (larger due to data volume)
    pub snapshot_timeout: Duration,
    /// Connection establishment timeout
    pub connect_timeout: Duration,
    /// TCP keep-alive interval
    pub tcp_keepalive: Duration,
    /// Enable TCP_NODELAY
    pub tcp_nodelay: bool,
    /// HTTP/2 keep-alive interval
    pub http2_keepalive_interval: Duration,
    /// HTTP/2 keep-alive timeout
    pub http2_keepalive_timeout: Duration,
    /// Size of one snapshot chunk pushed to a follower
    pub snapshot_chunk_size: usize,
    /// Cap for a single Raft gRPC message, in both directions
    pub max_message_size: usize,
    /// HTTP/2 flow-control window. Must exceed `snapshot_chunk_size`, otherwise
    /// every chunk stalls waiting for WINDOW_UPDATE.
    pub http2_initial_stream_window_size: u32,
    /// HTTP/2 frame size
    pub http2_max_frame_size: u32,
}

impl Default for RaftNetworkConfig {
    fn default() -> Self {
        Self {
            rpc_timeout: Duration::from_secs(10),
            snapshot_timeout: Duration::from_secs(30),
            connect_timeout: Duration::from_secs(5),
            tcp_keepalive: Duration::from_secs(10),
            tcp_nodelay: true,
            http2_keepalive_interval: Duration::from_secs(10),
            http2_keepalive_timeout: Duration::from_secs(5),
            snapshot_chunk_size: super::config::DEFAULT_SNAPSHOT_CHUNK_SIZE,
            max_message_size: 4 * 1024 * 1024,
            http2_initial_stream_window_size: super::config::DEFAULT_RAFT_STREAM_WINDOW_SIZE,
            http2_max_frame_size: super::config::DEFAULT_RAFT_MAX_FRAME_SIZE,
        }
    }
}

impl RaftNetworkConfig {
    /// Create from RaftConfig
    pub fn from_raft_config(config: &super::config::RaftConfig) -> Self {
        Self {
            rpc_timeout: config.rpc_timeout(),
            snapshot_timeout: config.snapshot_transfer_timeout(),
            connect_timeout: Duration::from_secs(5),
            tcp_keepalive: Duration::from_secs(config.grpc_tcp_keepalive_secs),
            tcp_nodelay: config.grpc_tcp_nodelay,
            http2_keepalive_interval: Duration::from_secs(
                config.grpc_http2_keepalive_interval_secs,
            ),
            http2_keepalive_timeout: Duration::from_secs(config.grpc_http2_keepalive_timeout_secs),
            snapshot_chunk_size: config.effective_snapshot_chunk_size(),
            max_message_size: config.snapshot_max_message_size(),
            http2_initial_stream_window_size: super::config::DEFAULT_RAFT_STREAM_WINDOW_SIZE,
            http2_max_frame_size: super::config::DEFAULT_RAFT_MAX_FRAME_SIZE,
        }
    }
}

/// Factory for creating Raft network connections
pub struct BatataRaftNetworkFactory {
    network_config: RaftNetworkConfig,
}

impl BatataRaftNetworkFactory {
    /// Create a factory using default network timeout configuration.
    pub fn new() -> Self {
        Self {
            network_config: RaftNetworkConfig::default(),
        }
    }

    /// Create a factory with custom network timeouts
    pub fn with_config(network_config: RaftNetworkConfig) -> Self {
        Self { network_config }
    }
}

impl Default for BatataRaftNetworkFactory {
    fn default() -> Self {
        Self::new()
    }
}

impl openraft::network::RaftNetworkFactory<TypeConfig> for BatataRaftNetworkFactory {
    type Network = RaftNetworkConnection;

    async fn new_client(&mut self, _target: NodeId, node: &BasicNode) -> Self::Network {
        RaftNetworkConnection::with_config(node.addr.clone(), self.network_config.clone())
    }
}

/// A network connection to a remote Raft node
pub struct RaftNetworkConnection {
    addr: String,
    client: Option<RaftServiceClient<Channel>>,
    /// Dedicated client for snapshot transfer: it needs a far longer per
    /// request timeout than the regular RPCs share.
    snapshot_client: Option<RaftServiceClient<Channel>>,
    network_config: RaftNetworkConfig,
}

impl RaftNetworkConnection {
    /// Create a new connection handle for the given remote address using default network timeouts.
    pub fn new(addr: String) -> Self {
        Self {
            addr,
            client: None,
            snapshot_client: None,
            network_config: RaftNetworkConfig::default(),
        }
    }

    /// Create with custom network configuration
    pub fn with_config(addr: String, network_config: RaftNetworkConfig) -> Self {
        Self {
            addr,
            client: None,
            snapshot_client: None,
            network_config,
        }
    }

    /// Build an endpoint with the transport settings every Raft RPC wants.
    ///
    /// The caller picks the timeout, because a snapshot transfer needs a much
    /// longer one than an ordinary AppendEntries.
    fn endpoint(&self, timeout: Duration) -> Result<Endpoint, NetworkError<TypeConfig>> {
        let endpoint = format!("http://{}", self.addr);
        debug!("Connecting to Raft node at {}", endpoint);

        Ok(Channel::from_shared(endpoint)
            .map_err(|e| NetworkError::new(&e))?
            .connect_timeout(self.network_config.connect_timeout)
            .timeout(timeout)
            .tcp_keepalive(Some(self.network_config.tcp_keepalive))
            .tcp_nodelay(self.network_config.tcp_nodelay)
            .http2_keep_alive_interval(self.network_config.http2_keepalive_interval)
            .keep_alive_timeout(self.network_config.http2_keepalive_timeout)
            // Without an explicit window the HTTP/2 default (64 KiB) applies,
            // which is smaller than one snapshot chunk and stalls the transfer.
            .initial_stream_window_size(self.network_config.http2_initial_stream_window_size)
            .initial_connection_window_size(
                self.network_config.http2_initial_stream_window_size * 2,
            )
            .max_frame_size(self.network_config.http2_max_frame_size))
    }

    /// Wrap a channel so a whole snapshot chunk fits in a single gRPC message.
    fn wrap_client(channel: Channel, max_message_size: usize) -> RaftServiceClient<Channel> {
        RaftServiceClient::new(channel)
            .max_decoding_message_size(max_message_size)
            .max_encoding_message_size(max_message_size)
    }

    /// Ensure we have a connected client for ordinary RPCs
    async fn ensure_client(
        &mut self,
    ) -> Result<&mut RaftServiceClient<Channel>, NetworkError<TypeConfig>> {
        if self.client.is_none() {
            let channel = self
                .endpoint(self.network_config.rpc_timeout)?
                .connect()
                .await
                .map_err(|e| {
                    warn!("Failed to connect to {}: {}", self.addr, e);
                    NetworkError::new(&e)
                })?;

            self.client = Some(Self::wrap_client(
                channel,
                self.network_config.max_message_size,
            ));
        }

        // Safe: client is guaranteed to be Some after the above block
        self.client.as_mut().ok_or_else(|| {
            NetworkError::new(&std::io::Error::other(
                "Client connection failed unexpectedly",
            ))
        })
    }

    /// Ensure we have a client whose timeout covers a full snapshot transfer.
    ///
    /// Reusing the ordinary client would cap the whole stream at the regular
    /// RPC timeout, which any sizeable snapshot exceeds.
    async fn ensure_snapshot_client(
        &mut self,
    ) -> Result<&mut RaftServiceClient<Channel>, NetworkError<TypeConfig>> {
        if self.snapshot_client.is_none() {
            let channel = self
                .endpoint(self.network_config.snapshot_timeout)?
                .connect()
                .await
                .map_err(|e| {
                    warn!("Failed to connect to Raft node {}: {}", self.addr, e);
                    NetworkError::new(&e)
                })?;

            self.snapshot_client = Some(Self::wrap_client(
                channel,
                self.network_config.max_message_size,
            ));
        }

        self.snapshot_client.as_mut().ok_or_else(|| {
            NetworkError::new(&std::io::Error::other(
                "Snapshot client connection failed unexpectedly",
            ))
        })
    }

    /// Convert openraft LogId to proto LogId.
    ///
    /// Keep in sync with `proto_convert::to_proto_log_id`: the proposing
    /// leader's node id must cross the wire so advanced leader ids keep their
    /// ordering after a round trip.
    fn to_proto_log_id(log_id: Option<LogId>) -> Option<ProtoLogId> {
        log_id.map(|l| ProtoLogId {
            term: l.leader_id.term,
            index: l.index,
            leader_id: l.leader_id.node_id,
        })
    }

    /// Convert proto LogId to openraft LogId.
    fn from_proto_log_id(log_id: Option<ProtoLogId>) -> Option<LogId> {
        log_id.map(|l| {
            LogId::new(
                CommittedLeaderId {
                    term: l.term,
                    node_id: l.leader_id,
                },
                l.index,
            )
        })
    }

    /// Convert openraft Vote to proto Vote
    fn to_proto_vote(vote: &Vote) -> ProtoVote {
        ProtoVote {
            leader_id: vote.leader_id().node_id,
            term: vote.leader_id().term,
            committed: vote.is_committed(),
        }
    }

    /// Convert proto Vote to openraft Vote
    fn from_proto_vote(vote: Option<ProtoVote>) -> Option<Vote> {
        vote.map(|v| {
            let leader_id = CommittedLeaderId {
                term: v.term,
                node_id: v.leader_id,
            };
            if v.committed {
                Vote::new_committed(leader_id.term, leader_id.node_id)
            } else {
                Vote::new(leader_id.term, leader_id.node_id)
            }
        })
    }

    /// Convert openraft Entry to proto Entry
    ///
    /// Serializes just the inner payload data using bincode (not the EntryPayload enum wrapper),
    /// matching the deserialization in grpc_service.rs which deserializes by payload_type.
    /// A serialization failure aborts the RPC: emitting an empty payload with
    /// `payload_type=1/2` would make the follower reject the whole batch while
    /// the leader has already committed it locally, stalling replication.
    fn to_proto_entry(entry: &Entry) -> Result<ProtoEntry, String> {
        let (payload_type, payload) = match &entry.payload {
            openraft::EntryPayload::Blank => (0u32, Vec::new()),
            openraft::EntryPayload::Normal(req) => serialize_entry_payload(1, req, &entry.log_id)?,
            openraft::EntryPayload::Membership(membership) => {
                serialize_entry_payload(2, membership, &entry.log_id)?
            }
        };

        Ok(ProtoEntry {
            log_id: Self::to_proto_log_id(Some(entry.log_id)),
            payload_type,
            payload,
        })
    }
}

/// Serialize a non-blank Raft entry payload, propagating failures to the caller.
fn serialize_entry_payload<T: serde::Serialize>(
    payload_type: u32,
    value: &T,
    log_id: &LogId,
) -> Result<(u32, Vec<u8>), String> {
    bincode::serialize(value)
        .map(|data| (payload_type, data))
        .map_err(|e| {
            error!(
                error = %e,
                log_id = ?log_id,
                "Failed to serialize Raft entry payload"
            );
            format!("failed to serialize Raft entry payload {log_id:?}: {e}")
        })
}

/// Convert the 0.9-shaped wire reply into the 0.10 `AppendResult`.
///
/// `AppendEntriesResponse::into_append_result()` unwraps `prev_log_id` for a
/// `Conflict`. A request replicating from the very beginning of the log has no
/// `prev_log_id`, so a conflict reply in that position (only reachable from a
/// malformed or mixed-version peer) must be reported as a transport failure
/// instead of panicking the replication task. A homogeneous 0.10 follower
/// cannot produce this combination: openraft's `ensure_log_consecutive()`
/// accepts any request whose `prev_log_id` is `None`.
fn append_result_from_wire(
    wire_response: AppendEntriesResponse<TypeConfig>,
    prev_log_id: Option<LogId>,
    last_log_id: Option<LogId>,
) -> Result<AppendResult<TypeConfig>, RPCError<TypeConfig>> {
    if matches!(wire_response, AppendEntriesResponse::Conflict) && prev_log_id.is_none() {
        warn!("Rejecting conflict reply without prev_log_id: inconsistent peer response");
        return Err(RPCError::Unreachable(Unreachable::new(
            &std::io::Error::other("conflict reply without prev_log_id"),
        )));
    }
    Ok(wire_response.into_append_result(prev_log_id, last_log_id))
}

/// Derive the transport deadline for one request-response Raft RPC.
///
/// openraft 0.10.1 requires the network implementation to enforce
/// `RPCOption::soft_ttl()`, not `hard_ttl()`: the framework may cancel an
/// in-flight RPC at the hard limit, so the transport must return a timeout
/// error before that. For heartbeat and streaming replication the hard TTL is
/// `heartbeat_interval` (`core/heartbeat/worker.rs:106`,
/// `replication/mod.rs:334`), therefore `rpc_request_timeout_ms` does NOT bound
/// those RPCs. It remains the channel-level ceiling and the fallback used only
/// when openraft supplies a zero TTL.
fn rpc_deadline(option: &RPCOption, fallback: Duration) -> Duration {
    let hard = option.hard_ttl();
    if hard.is_zero() {
        return fallback;
    }

    // `soft_ttl()` is currently `hard * 3 / 4`, but that ratio is explicitly
    // not part of the API contract. Clamp to `hard` so a future upstream
    // change can never make the transport wait past openraft's cancellation
    // point, and keep the deadline positive for sub-millisecond hard TTLs
    // where integer division rounds `soft_ttl()` down to zero.
    option
        .soft_ttl()
        .min(hard)
        .max(Duration::from_nanos(1))
        .min(hard)
}

impl RaftNetworkConnection {
    /// Effective per-RPC deadline. The configured `rpc_timeout` is only a
    /// fallback/ceiling; the authority for an individual RPC is `RPCOption`.
    fn effective_timeout(&self, option: &RPCOption) -> Duration {
        rpc_deadline(option, self.network_config.rpc_timeout)
    }
}

impl RaftNetworkV2<TypeConfig> for RaftNetworkConnection {
    type SnapshotData = super::snapshot_file::SnapshotFile;

    async fn append_entries(
        &mut self,
        req: AppendEntriesRequest<TypeConfig>,
        option: RPCOption,
    ) -> Result<AppendResult<TypeConfig>, RPCError<TypeConfig>> {
        // The wire enum carries neither the acknowledged log id nor a conflict
        // hint, so remember the ids of this request for the conversion back.
        let prev_log_id = req.prev_log_id;
        let last_log_id = req.last_log_id();
        let timeout = self.effective_timeout(&option);
        let client = self
            .ensure_client()
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;

        let entries = req
            .entries
            .iter()
            .map(Self::to_proto_entry)
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                RPCError::Network(NetworkError::new(&std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                )))
            })?;

        let proto_req = ProtoAppendEntriesRequest {
            term: req.vote.leader_id().term,
            leader_id: req.vote.leader_id().node_id,
            prev_log_id: Self::to_proto_log_id(req.prev_log_id),
            entries,
            leader_commit: Self::to_proto_log_id(req.leader_commit),
            vote: Some(Self::to_proto_vote(&req.vote)),
        };

        let response = tokio::time::timeout(timeout, client.append_entries(proto_req))
            .await
            .map_err(|_| {
                warn!("AppendEntries RPC timed out after {:?}", timeout);
                RPCError::Unreachable(Unreachable::new(&std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "AppendEntries RPC timed out",
                )))
            })?
            .map_err(|e| {
                error!("AppendEntries RPC failed: {}", e);
                RPCError::Unreachable(Unreachable::new(&e))
            })?
            .into_inner();

        // Keep the 0.9-shaped wire enum, then convert to the AppendResult the
        // openraft 0.10 leader handles.
        let wire_response = if response.success {
            AppendEntriesResponse::Success
        } else if let Some(vote_proto) = response.higher_vote {
            // Follower has seen a higher vote — leader must step down
            let vote = Self::from_proto_vote(Some(vote_proto))
                .unwrap_or_else(|| Vote::new(response.term, 0));
            AppendEntriesResponse::HigherVote(vote)
        } else {
            // Conflict or unknown failure: probing again is the safe fallback.
            AppendEntriesResponse::Conflict
        };
        append_result_from_wire(wire_response, prev_log_id, last_log_id)
    }

    async fn full_snapshot(
        &mut self,
        vote: Vote,
        snapshot: Snapshot,
        cancel: impl Future<Output = ReplicationClosed> + OptionalSend + 'static,
        _option: RPCOption,
    ) -> Result<SnapshotResponse<TypeConfig>, StreamingError<TypeConfig>> {
        use futures::stream;
        use tokio::io::{AsyncReadExt, BufReader};
        use tokio::select;

        // Read it up front: `client` below holds a mutable borrow of `self`.
        let chunk_size = self.network_config.snapshot_chunk_size.max(1);

        // The membership must ride along: the receiving side calls
        // `save_membership(meta.last_membership)` after installing, so a
        // missing membership would wipe the config on the follower.
        let meta = Some(super::proto_convert::to_proto_snapshot_meta(&snapshot.meta));
        let vote_proto = Self::to_proto_vote(&vote);
        let term = vote.leader_id().term;
        let leader_id = vote.leader_id().node_id;

        let client = match self.ensure_snapshot_client().await {
            Ok(c) => c,
            Err(e) => return Err(StreamingError::Unreachable(Unreachable::new(&e))),
        };

        // Total size is known up front because the body is a file, so the
        // stream can report `done` on the last chunk without buffering.
        let total_bytes = match snapshot.snapshot.len().await {
            Ok(len) => len,
            Err(e) => return Err(StreamingError::Unreachable(Unreachable::new(&e))),
        };

        // Stream the body chunk by chunk. Every chunk reuses one buffer, so
        // sending a snapshot costs chunk_size of memory instead of two copies
        // of the whole body (the old code read it into a Vec and then copied
        // each chunk out of it).
        let body = BufReader::with_capacity(chunk_size, snapshot.snapshot);
        let requests = stream::unfold(
            (body, vec![0u8; chunk_size], 0u64, true),
            move |(mut body, mut buf, mut offset, mut first)| {
                let meta = meta.clone();
                async move {
                    if total_bytes == 0 {
                        if first {
                            first = false;
                            return Some((
                                batata_server_api::raft::InstallSnapshotRequest {
                                    term,
                                    leader_id,
                                    meta,
                                    offset: 0,
                                    data: Vec::new(),
                                    done: true,
                                    vote: Some(vote_proto),
                                },
                                (body, buf, offset, first),
                            ));
                        }
                        return None;
                    }

                    let mut filled = 0usize;
                    while filled < buf.len() {
                        match body.read(&mut buf[filled..]).await {
                            Ok(0) => break,
                            Ok(n) => filled += n,
                            Err(e) => {
                                error!("Failed to read snapshot body: {}", e);
                                return None;
                            }
                        }
                    }
                    if filled == 0 {
                        return None;
                    }

                    let done = offset + filled as u64 >= total_bytes;
                    let request = batata_server_api::raft::InstallSnapshotRequest {
                        term,
                        leader_id,
                        meta: if first { meta.clone() } else { None },
                        offset,
                        data: buf[..filled].to_vec(),
                        done,
                        vote: Some(vote_proto),
                    };
                    first = false;
                    offset += filled as u64;

                    Some((request, (body, buf, offset, first)))
                }
            },
        );

        // Send snapshot stream with cancellation support
        let response = select! {
            result = client.install_snapshot(requests) => {
                match result {
                    Ok(resp) => resp.into_inner(),
                    Err(e) => {
                        error!("Full snapshot transfer failed: {}", e);
                        return Err(StreamingError::Unreachable(Unreachable::new(&e)));
                    }
                }
            }
            _ = cancel => {
                warn!("Snapshot transfer cancelled");
                return Err(StreamingError::Unreachable(Unreachable::new(
                    &std::io::Error::new(std::io::ErrorKind::Interrupted, "cancelled")
                )));
            }
        };

        // The follower always echoes an explicit vote. Falling back to our own
        // vote here would hide a malformed/older response and could make the
        // leader miss a step-down.
        let response_vote = Self::from_proto_vote(response.vote).ok_or_else(|| {
            StreamingError::Unreachable(Unreachable::new(&std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "InstallSnapshot response is missing vote",
            )))
        })?;

        Ok(SnapshotResponse {
            vote: response_vote,
        })
    }

    async fn vote(
        &mut self,
        req: VoteRequest<TypeConfig>,
        option: RPCOption,
    ) -> Result<VoteResponse<TypeConfig>, RPCError<TypeConfig>> {
        let timeout = self.effective_timeout(&option);
        let client = self
            .ensure_client()
            .await
            .map_err(|e| RPCError::Unreachable(Unreachable::new(&e)))?;

        let proto_req = ProtoVoteRequest {
            term: req.vote.leader_id().term,
            candidate_id: req.vote.leader_id().node_id,
            last_log_id: Self::to_proto_log_id(req.last_log_id),
            vote: Some(Self::to_proto_vote(&req.vote)),
        };

        let response = tokio::time::timeout(timeout, client.vote(proto_req))
            .await
            .map_err(|_| {
                warn!("Vote RPC timed out after {:?}", timeout);
                RPCError::Unreachable(Unreachable::new(&std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "Vote RPC timed out",
                )))
            })?
            .map_err(|e| {
                error!("Vote RPC failed: {}", e);
                RPCError::Unreachable(Unreachable::new(&e))
            })?
            .into_inner();

        Ok(VoteResponse {
            vote: Self::from_proto_vote(response.vote).unwrap_or(req.vote),
            vote_granted: response.vote_granted,
            last_log_id: Self::from_proto_log_id(response.last_log_id),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_proto_log_id_conversion() {
        let log_id = LogId::new(
            CommittedLeaderId {
                term: 5,
                node_id: 100,
            },
            42,
        );
        let proto = RaftNetworkConnection::to_proto_log_id(Some(log_id));
        assert!(proto.is_some());
        let proto = proto.unwrap();
        assert_eq!(proto.term, 5);
        assert_eq!(proto.index, 42);
        assert_eq!(proto.leader_id, 100);

        // The network connection has its own conversion helpers; verify the
        // full round trip keeps the proposing leader id, not just the send side.
        let restored = RaftNetworkConnection::from_proto_log_id(Some(proto)).unwrap();
        assert_eq!(restored, log_id);
    }

    #[test]
    fn test_proto_vote_conversion() {
        let vote = Vote::new(3, 100);
        let proto = RaftNetworkConnection::to_proto_vote(&vote);
        assert_eq!(proto.term, 3);
        assert_eq!(proto.leader_id, 100);
        assert!(!proto.committed);
    }

    /// A serializer that always fails, to prove the error is propagated rather
    /// than replaced by an empty payload.
    struct FailingPayload;

    impl serde::Serialize for FailingPayload {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("serialization failed"))
        }
    }

    #[test]
    fn test_entry_payload_serialization_failure_is_propagated() {
        let log_id = LogId::new(
            CommittedLeaderId {
                term: 5,
                node_id: 1,
            },
            42,
        );
        let err = serialize_entry_payload(1, &FailingPayload, &log_id).unwrap_err();
        assert!(
            err.contains("failed to serialize Raft entry payload"),
            "{err}"
        );
    }

    #[test]
    fn test_entry_payload_serializes_to_non_empty_bytes() {
        let log_id = LogId::new(
            CommittedLeaderId {
                term: 5,
                node_id: 1,
            },
            42,
        );
        let (payload_type, payload) =
            serialize_entry_payload(1, &"config-publish", &log_id).unwrap();
        assert_eq!(payload_type, 1);
        assert!(!payload.is_empty());
        assert_eq!(
            bincode::deserialize::<String>(&payload).unwrap(),
            "config-publish"
        );
    }

    fn log_id(term: u64, index: u64) -> LogId {
        LogId::new(CommittedLeaderId { term, node_id: 1 }, index)
    }

    /// Guard test: `into_append_result()` unwraps `prev_log_id` for a conflict.
    /// Removing the guard in `append_result_from_wire()` must make this panic.
    #[test]
    fn conflict_without_prev_log_id_is_a_transport_error() {
        let err = append_result_from_wire(AppendEntriesResponse::Conflict, None, None).unwrap_err();
        assert!(
            matches!(err, RPCError::Unreachable(_)),
            "expected a retriable transport error, got {err:?}"
        );
    }

    #[test]
    fn conflict_with_prev_log_id_returns_the_conflict() {
        let prev = log_id(2, 7);
        let result = append_result_from_wire(
            AppendEntriesResponse::Conflict,
            Some(prev),
            Some(log_id(2, 9)),
        )
        .unwrap();
        match result {
            Err(openraft::raft::AppendError::Conflict { log_id, hint }) => {
                assert_eq!(log_id, prev);
                assert!(hint.is_none(), "the wire carries no conflict hint");
            }
            other => panic!("expected Conflict, got {other:?}"),
        }
    }

    #[test]
    fn success_acknowledges_the_last_log_id() {
        let last = log_id(2, 9);
        let result = append_result_from_wire(
            AppendEntriesResponse::Success,
            Some(log_id(2, 7)),
            Some(last),
        )
        .unwrap();
        assert_eq!(result.unwrap(), Some(last));
    }

    #[test]
    fn partial_success_acknowledges_the_matching_log_id() {
        let matched = log_id(2, 8);
        let result = append_result_from_wire(
            AppendEntriesResponse::PartialSuccess(Some(matched)),
            Some(log_id(2, 7)),
            Some(log_id(2, 9)),
        )
        .unwrap();
        assert_eq!(result.unwrap(), Some(matched));
    }

    #[test]
    fn higher_vote_is_preserved() {
        let vote = Vote::new(3, 42);
        let result =
            append_result_from_wire(AppendEntriesResponse::HigherVote(vote), None, None).unwrap();
        match result {
            Err(openraft::raft::AppendError::HigherVote(v)) => assert_eq!(v, vote),
            other => panic!("expected HigherVote, got {other:?}"),
        }
    }

    /// openraft 0.10.1 requires the transport to enforce `soft_ttl()`. Using
    /// `hard_ttl()` used to make every heartbeat/replication RPC wait until
    /// openraft's own cancellation point, hiding the real timeout error.
    #[test]
    fn rpc_deadline_uses_soft_ttl_not_hard_ttl() {
        let fallback = Duration::from_secs(5);
        let option = RPCOption::new(Duration::from_millis(200));
        let deadline = rpc_deadline(&option, fallback);
        assert_eq!(option.hard_ttl(), Duration::from_millis(200));
        assert_eq!(deadline, Duration::from_millis(150));
        assert!(deadline < option.hard_ttl());
    }

    /// The config-level RPC timeout is only used when openraft gives no TTL.
    /// A non-zero TTL must win, otherwise `rpc_request_timeout_ms` would
    /// silently override the framework's per-RPC budget.
    #[test]
    fn rpc_deadline_falls_back_only_for_zero_hard_ttl() {
        let fallback = Duration::from_secs(5);
        assert_eq!(
            rpc_deadline(&RPCOption::new(Duration::ZERO), fallback),
            fallback
        );
        let deadline = rpc_deadline(&RPCOption::new(Duration::from_millis(200)), fallback);
        assert!(deadline < fallback);
    }

    /// The deadline may never outlive the hard TTL, including the sub-ms case
    /// where `soft_ttl()` rounds down to zero.
    #[test]
    fn rpc_deadline_never_outlives_hard_ttl() {
        let fallback = Duration::from_secs(5);
        let hard_values = [
            Duration::from_nanos(1),
            Duration::from_millis(1),
            Duration::from_millis(200),
            Duration::from_secs(30),
        ];
        for hard in hard_values {
            let option = RPCOption::new(hard);
            let deadline = rpc_deadline(&option, fallback);
            assert!(
                !deadline.is_zero(),
                "hard={hard:?} produced a zero deadline"
            );
            assert!(deadline <= hard, "hard={hard:?}, deadline={deadline:?}");
        }
    }
}
