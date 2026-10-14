//! Cross-node auth cache invalidation.
//!
//! Every node keeps its own in-process RBAC caches (`batata_auth`), so a role
//! or permission change applied by one node would otherwise stay invisible to
//! its peers until the cached entries expire. Nacos accepts that trade-off and
//! simply waits for the TTL; Batata already ships the transport to do better:
//! [`AuthCacheInvalidateRequest`] plus a handler that evicts the local
//! entries, registered in `batata-server`.
//!
//! [`ClusterAuthCacheBroadcaster`] is the missing sender side: it turns a local
//! invalidation into a fire-and-forget broadcast to all healthy peers.
//!
//! The broadcast is asynchronous and best-effort — a peer that is down simply
//! converges when its entries expire, which is the previous behaviour.

use std::sync::Arc;

use tracing::warn;

use batata_api::model::Member;

use crate::service::ServerMemberManager;
use crate::service::cluster_client::ClusterRequestSender;

/// Sender side of the auth cache invalidation protocol.
///
/// Constructed once at startup when the cluster client is available; in
/// standalone mode it is never built and local invalidation stays local.
pub struct ClusterAuthCacheBroadcaster {
    sender: Arc<ClusterRequestSender>,
    member_manager: Arc<ServerMemberManager>,
}

impl ClusterAuthCacheBroadcaster {
    /// Create a broadcaster over an existing cluster request sender.
    pub fn new(
        sender: Arc<ClusterRequestSender>,
        member_manager: Arc<ServerMemberManager>,
    ) -> Self {
        Self {
            sender,
            member_manager,
        }
    }

    /// Main port of this node, used to skip ourselves in the member list.
    ///
    /// `ClusterClientManager::broadcast` already drops the member whose address
    /// equals its `local_address`, but that address comes from the *resolved*
    /// local IP while a development cluster lists its members as
    /// `127.0.0.1:<port>` — the two never match, so the node would send the
    /// eviction to itself. Ports are unique per node on one host, which is the
    /// case that matters here.
    fn self_port(&self) -> Option<u16> {
        self.member_manager
            .local_address()
            .rsplit(':')
            .next()
            .and_then(|port| port.parse().ok())
    }

    /// Ask every healthy peer to drop the entries described by `kind`/`target`.
    ///
    /// `kind` is one of `"user"`, `"role"`, `"permission"`, `"token"`, `"all"`,
    /// matching the vocabulary of `AuthCacheInvalidateRequest`.
    pub async fn broadcast(&self, kind: &str, target: &str) {
        let members: Vec<Member> = self
            .member_manager
            .healthy_members()
            .into_iter()
            .filter(|member| Some(member.port) != self.self_port())
            .collect();

        if members.is_empty() {
            return;
        }

        let results = self
            .sender
            .broadcast_auth_cache_invalidation(&members, kind, target)
            .await;

        for (peer, result) in results {
            if let Err(error) = result {
                warn!(
                    peer = %peer,
                    invalidate_type = %kind,
                    target = %target,
                    "Failed to broadcast auth cache invalidation: {}", error
                );
            }
        }
    }
}
