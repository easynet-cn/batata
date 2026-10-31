//! Runtime Endpoint publication capacity gate.
//!
//! Upstream anchor:
//! `com.alibaba.nacos.ai.service.agent.runtime.AgentRuntimePublicationCapacityGate`.
//! It caps how many Runtime Endpoint entries one publisher Client may own, so
//! a single client cannot grow without bound.
//!
//! Upstream maintains two counters per client:
//! - the *physical* entries the Naming `Client` owns (`client.getAllPublishedService()`
//!   filtered on the `agent-endpoints` group), and
//! - a *logical* reservation map (`ConcurrentHashMap<String, Map<String, Integer>>`)
//!   for publications materialized under child publishers.
//!
//! Batata's Naming registration path (`NamingServiceProvider::register_instance`)
//! carries no publisher identity, so the physical half has nothing to read —
//! an HTTP `POST /v3/client/ai/agents/endpoints` never binds a Naming client.
//! What Batata can do faithfully is the logical half: a reservation per
//! `(client, publication)`, counted per client and released on deregistration.
//! That is upstream's `registerLogical` / `deregisterLogical` pair, and it is
//! the pair the HTTP publish path ends up in after `AgentHttpClientLifecycleService`
//! binds the external `clientId` header to an internal publisher.
//!
//! Consequences worth stating plainly:
//! - The reservation is in-process, so it resets on restart. Upstream's
//!   `logicalPublications` map is in-process too; its physical half would
//!   rebuild from Naming clients as they reconnect.
//! - A request without a `clientId` header is admitted without accounting,
//!   mirroring upstream's `client == null` branch (`registration.run()` with no
//!   capacity check).

use std::collections::HashMap;
use std::sync::Mutex;

/// Default cap on Runtime Endpoint entries owned by one publisher Client.
///
/// Upstream: `Constants.Agent.DEFAULT_MAX_PUBLICATIONS_PER_CLIENT`.
pub const DEFAULT_MAX_PUBLICATIONS_PER_CLIENT: usize = 100;

/// Rejection returned when a Client is at capacity and the new batch would grow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublicationOverLimit {
    /// The configured cap that was hit.
    pub limit: usize,
}

impl std::fmt::Display for PublicationOverLimit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Agent Endpoint publication limit of {} reached for this Client.",
            self.limit
        )
    }
}

/// Serializes Runtime Endpoint publication admission and replacement for one
/// publisher Client.
pub struct AgentRuntimePublicationCapacityGate {
    max_publications_per_client: usize,
    /// client id -> (publication key -> reserved endpoint count)
    logical_publications: Mutex<HashMap<String, HashMap<String, usize>>>,
}

impl AgentRuntimePublicationCapacityGate {
    /// Creates a gate admitting at most `max_publications_per_client` entries
    /// per Client.
    ///
    /// # Panics
    ///
    /// Panics when `max_publications_per_client` is 0 — the same contract as
    /// upstream, which rejects a non-positive limit in the constructor.
    pub fn new(max_publications_per_client: usize) -> Self {
        assert!(
            max_publications_per_client >= 1,
            "max_publications_per_client must be greater than 0"
        );
        Self {
            max_publications_per_client,
            logical_publications: Mutex::new(HashMap::new()),
        }
    }

    /// The configured cap.
    pub fn limit(&self) -> usize {
        self.max_publications_per_client
    }

    /// Admit one logical Runtime publication, replacing any reservation the
    /// same key already holds.
    ///
    /// Runs `registration` and records `requested_publication_count` unless the
    /// Client is at capacity *and* the batch would grow. A replacement that
    /// shrinks or keeps the size is always admitted — the same rule as
    /// upstream's `ensureCapacity`, so a client that is full can still refresh
    /// what it already published.
    ///
    /// A blank `client_id` admits unconditionally: there is no Client to
    /// account against.
    pub fn register_logical<F>(
        &self,
        client_id: &str,
        publication_key: &str,
        requested_publication_count: usize,
        registration: F,
    ) -> Result<(), PublicationOverLimit>
    where
        F: FnOnce(),
    {
        if client_id.is_empty() || publication_key.is_empty() {
            registration();
            return Ok(());
        }

        let mut all = self
            .logical_publications
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let reservations = all.entry(client_id.to_string()).or_default();
        let existing = reservations.get(publication_key).copied().unwrap_or(0);
        let held: usize = reservations.values().sum();

        if held >= self.max_publications_per_client && requested_publication_count > existing {
            return Err(PublicationOverLimit {
                limit: self.max_publications_per_client,
            });
        }

        registration();
        reservations.insert(publication_key.to_string(), requested_publication_count);
        Ok(())
    }

    /// Release one logical reservation after its primary publication is removed.
    ///
    /// Runs `deregistration` first, then drops the reservation — upstream's
    /// `deregisterLogical` order, so a failed removal keeps the reservation and
    /// the Client stays charged for what it still owns.
    pub fn deregister_logical<F>(&self, client_id: &str, publication_key: &str, deregistration: F)
    where
        F: FnOnce(),
    {
        if client_id.is_empty() || publication_key.is_empty() {
            deregistration();
            return;
        }

        deregistration();

        let mut all = self
            .logical_publications
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut empty = false;
        if let Some(reservations) = all.get_mut(client_id) {
            reservations.remove(publication_key);
            empty = reservations.is_empty();
        }
        if empty {
            all.remove(client_id);
        }
    }

    /// Release every reservation owned by a disconnected Client.
    pub fn clear_logical_publications(&self, client_id: &str) {
        let mut all = self
            .logical_publications
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        all.remove(client_id);
    }

    /// Total reserved entries for one Client. Test/introspection helper.
    pub fn count_logical_publications(&self, client_id: &str) -> usize {
        let all = self
            .logical_publications
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        all.get(client_id).map(|m| m.values().sum()).unwrap_or(0)
    }

    /// The Agent versions one Client published, read back from its
    /// reservations.
    ///
    /// Upstream answers "what did this Client publish?" from the Naming
    /// `Client`, which owns the instances it registered. Batata's registration
    /// path carries no publisher identity, so the reservation map is the only
    /// record of it — which is also why deregistration has to consult it: a
    /// runtime version that was never published through the catalog is not
    /// discoverable any other way.
    ///
    /// Returns an empty vec for a blank `client_id`: nothing is accounted for
    /// an unattributable request, so there is nothing to read back.
    pub fn owned_versions(
        &self,
        client_id: &str,
        namespace: &str,
        agent_name: &str,
    ) -> Vec<String> {
        if client_id.is_empty() {
            return Vec::new();
        }
        let prefix = format!("{namespace}::{agent_name}::");
        let all = self
            .logical_publications
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        all.get(client_id)
            .map(|reservations| {
                reservations
                    .keys()
                    .filter_map(|key| key.strip_prefix(&prefix))
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Build the logical publication key for one Runtime publication.
    ///
    /// Upstream keys a logical publication by `(namespaceId, agentName, version)`;
    /// Batata flattens the same triple so one agent version reserves one slot
    /// per client regardless of how many endpoints the batch carries.
    pub fn publication_key(namespace: &str, agent_name: &str, version: &str) -> String {
        format!("{namespace}::{agent_name}::{version}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn admits_a_batch_while_under_capacity() {
        let gate = AgentRuntimePublicationCapacityGate::new(3);
        let mut ran = false;
        gate.register_logical("c1", "k1", 2, || ran = true)
            .expect("under capacity");
        assert!(ran);
        assert_eq!(gate.count_logical_publications("c1"), 2);
    }

    #[test]
    fn replaces_a_publication_without_charging_twice() {
        let gate = AgentRuntimePublicationCapacityGate::new(3);
        gate.register_logical("c1", "k1", 2, || {}).expect("first");
        // Same key: the reservation is replaced, not added.
        gate.register_logical("c1", "k1", 3, || {})
            .expect("replacement of the same key");
        assert_eq!(gate.count_logical_publications("c1"), 3);
    }

    #[test]
    fn rejects_growth_at_capacity() {
        let gate = AgentRuntimePublicationCapacityGate::new(3);
        gate.register_logical("c1", "k1", 3, || {})
            .expect("fills up");
        // A different key at full capacity is growth -> rejected.
        let err = gate
            .register_logical("c1", "k2", 1, || unreachable!("must not run"))
            .expect_err("at capacity");
        assert_eq!(err.limit, 3);
        assert_eq!(gate.count_logical_publications("c1"), 3);
    }

    #[test]
    fn allows_shrinking_replacement_at_capacity() {
        let gate = AgentRuntimePublicationCapacityGate::new(3);
        gate.register_logical("c1", "k1", 3, || {})
            .expect("fills up");
        // requested (1) <= existing (3): a refresh is admitted even when full.
        let mut ran = false;
        gate.register_logical("c1", "k1", 1, || ran = true)
            .expect("shrinking replacement is allowed");
        assert!(ran);
        assert_eq!(gate.count_logical_publications("c1"), 1);
    }

    #[test]
    fn counts_are_per_client() {
        let gate = AgentRuntimePublicationCapacityGate::new(2);
        gate.register_logical("c1", "k1", 2, || {}).expect("c1");
        gate.register_logical("c2", "k1", 2, || {})
            .expect("c2 has its own budget");
        assert_eq!(gate.count_logical_publications("c1"), 2);
        assert_eq!(gate.count_logical_publications("c2"), 2);
    }

    #[test]
    fn blank_client_id_is_admitted_without_accounting() {
        let gate = AgentRuntimePublicationCapacityGate::new(1);
        for i in 0..5 {
            gate.register_logical("", &format!("k{i}"), 10, || {})
                .expect("no client to charge");
        }
        assert_eq!(gate.count_logical_publications(""), 0);
    }

    #[test]
    fn deregistration_releases_the_reservation() {
        let gate = AgentRuntimePublicationCapacityGate::new(2);
        gate.register_logical("c1", "k1", 2, || {})
            .expect("fills up");
        gate.deregister_logical("c1", "k1", || {});
        assert_eq!(gate.count_logical_publications("c1"), 0);

        // The slot is free again.
        gate.register_logical("c1", "k2", 2, || {})
            .expect("freed by deregistration");
    }

    #[test]
    fn deregistration_runs_the_operation_even_without_a_client() {
        let gate = AgentRuntimePublicationCapacityGate::new(1);
        let mut ran = false;
        gate.deregister_logical("", "k1", || ran = true);
        assert!(ran);
    }

    #[test]
    fn clear_drops_every_reservation_for_a_client() {
        let gate = AgentRuntimePublicationCapacityGate::new(10);
        gate.register_logical("c1", "k1", 1, || {}).expect("k1");
        gate.register_logical("c1", "k2", 1, || {}).expect("k2");
        gate.clear_logical_publications("c1");
        assert_eq!(gate.count_logical_publications("c1"), 0);
    }

    #[test]
    fn emptying_a_client_removes_its_entry() {
        let gate = AgentRuntimePublicationCapacityGate::new(10);
        gate.register_logical("c1", "k1", 1, || {}).expect("k1");
        gate.deregister_logical("c1", "k1", || {});
        // Distinguishes "no client" from "client with no reservations" so the
        // map does not grow without bound.
        assert!(!gate.logical_publications.lock().unwrap().contains_key("c1"));
    }

    /// Deregistring reads back what the Client published, so a runtime version
    /// that never went through the catalog can still be removed.
    #[test]
    fn owned_versions_reports_every_version_one_client_published() {
        let gate = AgentRuntimePublicationCapacityGate::new(10);
        gate.register_logical("c1", "public::a::1.0.0", 1, || {})
            .expect("1.0.0");
        gate.register_logical("c1", "public::a::3.0.0", 1, || {})
            .expect("3.0.0");
        gate.register_logical("c1", "public::other::1.0.0", 1, || {})
            .expect("another agent");
        gate.register_logical("c2", "public::a::2.0.0", 1, || {})
            .expect("another client");

        let mut owned = gate.owned_versions("c1", "public", "a");
        owned.sort();
        assert_eq!(owned, vec!["1.0.0".to_string(), "3.0.0".to_string()]);
    }

    #[test]
    fn owned_versions_is_empty_without_a_client() {
        let gate = AgentRuntimePublicationCapacityGate::new(10);
        gate.register_logical("", "public::a::1.0.0", 1, || {})
            .expect("unattributable");
        assert!(gate.owned_versions("", "public", "a").is_empty());
    }

    /// The version is the part after the last `::`, so a name containing the
    /// separator would be split at the wrong place — kept honest by using the
    /// same helper that built the key.
    #[test]
    fn owned_versions_round_trips_through_the_publication_key() {
        let gate = AgentRuntimePublicationCapacityGate::new(10);
        let key = AgentRuntimePublicationCapacityGate::publication_key("public", "a", "1.2.3");
        gate.register_logical("c1", &key, 1, || {})
            .expect("register");
        assert_eq!(gate.owned_versions("c1", "public", "a"), vec!["1.2.3"]);
    }

    #[test]
    fn publication_key_is_stable_per_agent_version() {
        assert_eq!(
            AgentRuntimePublicationCapacityGate::publication_key("public", "a", "1.0.0"),
            "public::a::1.0.0"
        );
    }

    #[test]
    #[should_panic(expected = "must be greater than 0")]
    fn non_positive_limit_is_rejected() {
        AgentRuntimePublicationCapacityGate::new(0);
    }
}
