//! Client-side naming selectors (Nacos 3.3.0 GA `NamingSelector` parity).
//!
//! GA defines `NamingSelector` as a **client-side instance filter**:
//! `select(NamingContext) -> NamingResult`, where `NamingResult.getResult()`
//! is the filtered `List<Instance>`. `NamingSelectorWrapper` applies it to the
//! current instance list and to each diff bucket of a change event before the
//! user listener is invoked (`client/.../NamingSelectorWrapper.java`). Nothing
//! is sent to the server, so this module is a pure client-side capability:
//! `NacosNamingService.subscribe(..., selector, listener)` just builds the
//! wrapper and registers it with the local change notifier.
//!
//! GA's Java overloads are expressed here as explicit named methods, since
//! Rust has no method overloading. `subscribe(serviceName, groupName,
//! NamingSelector, EventListener)` maps to
//! [`super::BatataNamingService::subscribe_with_selector`] and
//! [`crate::traits::NamingService::subscribe_with_selector`], while
//! `subscribe(serviceName, NamingSelector, EventListener)` (default group) maps
//! to
//! [`super::BatataNamingService::subscribe_with_selector_default_group`] and
//! [`crate::traits::NamingService::subscribe_with_selector_default_group`].

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::Arc;

use batata_api::naming::model::Instance;
use regex::Regex;

use super::instances_diff::InstancesDiff;
use super::listener::{EventListener, NamingEvent};
use crate::error::{ClientError, Result};

/// Context handed to a [`NamingSelector`]; mirrors GA `NamingContext`.
///
/// The instance slice is borrowed so filtering an event does not clone the
/// whole list before the predicate runs.
#[derive(Clone, Debug)]
pub struct NamingContext<'a> {
    /// Name of the service whose instances are being filtered.
    pub service_name: String,
    /// Group of the service.
    pub group_name: String,
    /// Cluster filter string for the subscription (empty = all clusters).
    pub clusters: String,
    /// Candidate instances to filter.
    pub instances: &'a [Instance],
}

impl<'a> NamingContext<'a> {
    /// Build a context for one selector invocation.
    pub fn new(
        service_name: &str,
        group_name: &str,
        clusters: &str,
        instances: &'a [Instance],
    ) -> Self {
        Self {
            service_name: service_name.to_string(),
            group_name: group_name.to_string(),
            clusters: clusters.to_string(),
            instances,
        }
    }
}

/// Client-side instance filter, mirroring GA `NamingSelector`.
///
/// Implementations return the subset of `context.instances` the subscriber is
/// interested in. They must be `Send + Sync` because the filter runs on the
/// push-dispatch task.
pub trait NamingSelector: Send + Sync {
    /// Return the selected instances for `context`.
    fn select(&self, context: &NamingContext<'_>) -> Vec<Instance>;
}

/// Predicate-backed selector, mirroring GA `DefaultNamingSelector`.
///
/// GA's `DefaultNamingSelector` holds a `Predicate<Instance>` and returns
/// `Collections.emptyList()` for a null instance list; a borrowed empty slice
/// here maps to the same empty result.
pub struct DefaultNamingSelector<F>
where
    F: Fn(&Instance) -> bool + Send + Sync + 'static,
{
    filter: F,
}

impl<F> DefaultNamingSelector<F>
where
    F: Fn(&Instance) -> bool + Send + Sync + 'static,
{
    /// Build a selector from an instance predicate.
    pub fn new(filter: F) -> Self {
        Self { filter }
    }
}

impl<F> NamingSelector for DefaultNamingSelector<F>
where
    F: Fn(&Instance) -> bool + Send + Sync + 'static,
{
    fn select(&self, context: &NamingContext<'_>) -> Vec<Instance> {
        context
            .instances
            .iter()
            .filter(|instance| (self.filter)(instance))
            .cloned()
            .collect()
    }
}

/// Factory for the GA built-in selectors (`NamingSelectorFactory.java`).
pub struct NamingSelectorFactory;

impl NamingSelectorFactory {
    /// GA `EMPTY_SELECTOR`: returns every instance unchanged.
    pub fn empty_selector() -> Arc<dyn NamingSelector> {
        Arc::new(DefaultNamingSelector::new(|_| true))
    }

    /// GA `HEALTHY_SELECTOR`: keeps only healthy instances.
    pub fn healthy_selector() -> Arc<dyn NamingSelector> {
        Arc::new(DefaultNamingSelector::new(|instance| instance.healthy))
    }

    /// GA `newClusterSelector`: keeps instances whose `cluster_name` is in the
    /// given collection. An empty collection yields [`Self::empty_selector`].
    pub fn new_cluster_selector(clusters: &[String]) -> Arc<dyn NamingSelector> {
        if clusters.is_empty() {
            return Self::empty_selector();
        }
        let set: HashSet<String> = clusters.iter().cloned().collect();
        Arc::new(DefaultNamingSelector::new(move |instance| {
            set.contains(&instance.cluster_name)
        }))
    }

    /// GA `newIpSelector`: keeps instances whose IP fully matches `pattern`.
    ///
    /// Java's `Pattern.matches` is a whole-string match, so the expression is
    /// anchored with `^(?:...)$` before compilation. An invalid expression is
    /// reported as an error (GA throws `IllegalArgumentException` for a null
    /// argument and lets `Pattern` reject malformed expressions).
    pub fn new_ip_selector(pattern: &str) -> Result<Arc<dyn NamingSelector>> {
        let anchored = format!("^(?:{pattern})$");
        let regex = Regex::new(&anchored).map_err(|e| {
            ClientError::Other(anyhow::anyhow!(
                "invalid IP selector regex {pattern:?}: {e}"
            ))
        })?;
        Ok(Arc::new(DefaultNamingSelector::new(move |instance| {
            regex.is_match(&instance.ip)
        })))
    }

    /// GA `newMetadataSelector`: matches instance metadata.
    ///
    /// With `is_any = false` **all** entries must match (and the instance must
    /// have at least as many metadata entries); with `is_any = true` the GA
    /// implementation ORs each entry match with that size check, so any single
    /// matching entry is enough. The quirk is preserved deliberately.
    pub fn new_metadata_selector(
        metadata: HashMap<String, String>,
        is_any: bool,
    ) -> Arc<dyn NamingSelector> {
        let entries: Vec<(String, String)> = metadata.into_iter().collect();
        let expected = entries.len();
        Arc::new(DefaultNamingSelector::new(move |instance| {
            let mut matched = instance.metadata.len() >= expected;
            for (key, value) in &entries {
                let eq = instance
                    .metadata
                    .get(key)
                    .is_some_and(|actual| actual == value);
                matched = if is_any { matched || eq } else { matched && eq };
            }
            matched
        }))
    }

    /// GA `getUniqueClusterString`: sorted, de-duplicated, comma-joined.
    pub fn get_unique_cluster_string(clusters: &[String]) -> String {
        let set: BTreeSet<&str> = clusters.iter().map(String::as_str).collect();
        set.into_iter().collect::<Vec<_>>().join(",")
    }
}

/// Wraps a listener so every delivered event is filtered first.
///
/// Mirrors GA `NamingSelectorWrapper.buildListenerEvent`: the current instance
/// list and each diff bucket (`added` / `removed` / `modified`) are selected
/// independently before the wrapped listener is invoked.
pub struct SelectorEventListener {
    selector: Arc<dyn NamingSelector>,
    listener: Arc<dyn EventListener>,
}

impl SelectorEventListener {
    /// Wrap `listener` with `selector`.
    pub fn new(selector: Arc<dyn NamingSelector>, listener: Arc<dyn EventListener>) -> Self {
        Self { selector, listener }
    }

    /// Build the listener registration for a selector-based subscription.
    ///
    /// The concrete type is returned so the subscription path can reuse
    /// [`Self::filter_event`] for the initial snapshot; it coerces to
    /// `Arc<dyn EventListener>` wherever a listener is registered.
    pub fn wrap(selector: Arc<dyn NamingSelector>, listener: Arc<dyn EventListener>) -> Arc<Self> {
        Arc::new(Self::new(selector, listener))
    }

    /// Apply the selector to one instance list with the event's identity.
    pub fn filter_instances(
        &self,
        service_name: &str,
        group_name: &str,
        clusters: &str,
        instances: &[Instance],
    ) -> Vec<Instance> {
        if instances.is_empty() {
            return Vec::new();
        }
        let context = NamingContext::new(service_name, group_name, clusters, instances);
        self.selector.select(&context)
    }

    /// Build the filtered event that the wrapped listener should receive.
    pub fn filter_event(&self, event: &NamingEvent) -> NamingEvent {
        let instances = self.filter_instances(
            &event.service_name,
            &event.group_name,
            &event.clusters,
            &event.instances,
        );

        let diff = event.diff.as_ref().map(|diff| InstancesDiff {
            added_instances: self.filter_instances(
                &event.service_name,
                &event.group_name,
                &event.clusters,
                &diff.added_instances,
            ),
            removed_instances: self.filter_instances(
                &event.service_name,
                &event.group_name,
                &event.clusters,
                &diff.removed_instances,
            ),
            modified_instances: self.filter_instances(
                &event.service_name,
                &event.group_name,
                &event.clusters,
                &diff.modified_instances,
            ),
        });

        NamingEvent {
            service_name: event.service_name.clone(),
            group_name: event.group_name.clone(),
            clusters: event.clusters.clone(),
            instances,
            diff,
        }
    }
}

impl EventListener for SelectorEventListener {
    fn on_event(&self, event: NamingEvent) {
        self.listener.on_event(self.filter_event(&event));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn instance(ip: &str, healthy: bool, cluster: &str) -> Instance {
        Instance {
            instance_id: format!("{ip}#8080#{cluster}"),
            ip: ip.to_string(),
            port: 8080,
            weight: 1.0,
            healthy,
            enabled: true,
            ephemeral: true,
            cluster_name: cluster.to_string(),
            service_name: "svc".to_string(),
            metadata: HashMap::new(),
        }
    }

    fn context(instances: &[Instance]) -> NamingContext<'_> {
        NamingContext::new("svc", "DEFAULT_GROUP", "", instances)
    }

    fn ips(instances: &[Instance]) -> Vec<String> {
        instances.iter().map(|i| i.ip.clone()).collect()
    }

    fn metadata(kv: &[(&str, &str)]) -> HashMap<String, String> {
        kv.iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn empty_selector_keeps_every_instance() {
        let all = vec![
            instance("10.0.0.1", true, "A"),
            instance("10.0.0.2", false, "B"),
        ];
        let selected = NamingSelectorFactory::empty_selector().select(&context(&all));
        assert_eq!(ips(&selected), vec!["10.0.0.1", "10.0.0.2"]);
    }

    #[test]
    fn healthy_selector_drops_unhealthy_instances() {
        let all = vec![
            instance("10.0.0.1", true, "A"),
            instance("10.0.0.2", false, "B"),
            instance("10.0.0.3", true, "B"),
        ];
        let selected = NamingSelectorFactory::healthy_selector().select(&context(&all));
        assert_eq!(ips(&selected), vec!["10.0.0.1", "10.0.0.3"]);
    }

    #[test]
    fn cluster_selector_matches_only_named_clusters() {
        let all = vec![
            instance("10.0.0.1", true, "A"),
            instance("10.0.0.2", true, "B"),
            instance("10.0.0.3", true, "C"),
        ];
        let clusters = vec!["A".to_string(), "C".to_string()];
        let selected =
            NamingSelectorFactory::new_cluster_selector(&clusters).select(&context(&all));
        assert_eq!(ips(&selected), vec!["10.0.0.1", "10.0.0.3"]);
    }

    #[test]
    fn empty_cluster_selector_is_identity() {
        let all = vec![
            instance("10.0.0.1", true, "A"),
            instance("10.0.0.2", false, "B"),
        ];
        let selected = NamingSelectorFactory::new_cluster_selector(&[]).select(&context(&all));
        assert_eq!(ips(&selected), vec!["10.0.0.1", "10.0.0.2"]);
    }

    #[test]
    fn ip_selector_is_a_whole_string_match() {
        let all = vec![
            instance("10.0.0.1", true, "A"),
            instance("10.0.0.20", true, "A"),
            instance("10.0.1.1", true, "A"),
        ];
        let selector = NamingSelectorFactory::new_ip_selector(r"10\.0\.0\.\d+").unwrap();
        let selected = selector.select(&context(&all));
        // "10.0.0.20" matches \d+, but "10.0.1.1" must not match as a substring.
        assert_eq!(ips(&selected), vec!["10.0.0.1", "10.0.0.20"]);
    }

    #[test]
    fn ip_selector_rejects_invalid_regex() {
        assert!(NamingSelectorFactory::new_ip_selector("[").is_err());
    }

    #[test]
    fn metadata_selector_all_entries_must_match() {
        let mut first = instance("10.0.0.1", true, "A");
        first.metadata = metadata(&[("env", "prod"), ("zone", "cn")]);
        let mut second = instance("10.0.0.2", true, "A");
        second.metadata = metadata(&[("env", "prod"), ("zone", "us")]);
        let mut third = instance("10.0.0.3", true, "A");
        third.metadata = metadata(&[("env", "prod")]);

        let all = vec![first, second, third];
        let selector = NamingSelectorFactory::new_metadata_selector(
            metadata(&[("env", "prod"), ("zone", "cn")]),
            false,
        );
        let selected = selector.select(&context(&all));
        assert_eq!(ips(&selected), vec!["10.0.0.1"]);
    }

    #[test]
    fn metadata_selector_any_entry_can_match() {
        let mut first = instance("10.0.0.1", true, "A");
        // Fewer metadata entries than requested, but one entry matches: only
        // the OR branch can admit this instance.
        first.metadata = metadata(&[("env", "prod")]);
        let mut second = instance("10.0.0.2", true, "A");
        // Enough entries to satisfy GA's initial size check, none matching:
        // GA still admits it because the size predicate is ORed in.
        second.metadata = metadata(&[("env", "dev"), ("zone", "us")]);
        let mut third = instance("10.0.0.3", true, "A");
        // Too few entries and no match at all.
        third.metadata = metadata(&[("env", "dev")]);

        let all = vec![first, second, third];
        let selector = NamingSelectorFactory::new_metadata_selector(
            metadata(&[("env", "prod"), ("zone", "cn")]),
            true,
        );
        let selected = selector.select(&context(&all));
        assert_eq!(ips(&selected), vec!["10.0.0.1", "10.0.0.2"]);
    }

    #[test]
    fn unique_cluster_string_is_sorted_and_deduplicated() {
        let clusters = vec![
            "C".to_string(),
            "A".to_string(),
            "C".to_string(),
            "B".to_string(),
        ];
        assert_eq!(
            NamingSelectorFactory::get_unique_cluster_string(&clusters),
            "A,B,C"
        );
        assert_eq!(NamingSelectorFactory::get_unique_cluster_string(&[]), "");
    }

    #[test]
    fn wrapper_filters_current_instances_and_every_diff_bucket() {
        let healthy = instance("10.0.0.1", true, "A");
        let unhealthy = instance("10.0.0.2", false, "A");
        let unhealthy_modified = instance("10.0.0.3", false, "A");
        let healthy_modified = instance("10.0.0.4", true, "A");

        let event = NamingEvent {
            service_name: "svc".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            clusters: "A".to_string(),
            instances: vec![healthy.clone(), unhealthy.clone(), healthy_modified.clone()],
            diff: Some(InstancesDiff {
                added_instances: vec![healthy.clone(), unhealthy.clone()],
                removed_instances: vec![unhealthy.clone(), healthy_modified.clone()],
                modified_instances: vec![unhealthy_modified, healthy.clone()],
            }),
        };

        let selector = NamingSelectorFactory::healthy_selector();
        let wrapper = SelectorEventListener::new(selector, Arc::new(NoopListener));
        let filtered = wrapper.filter_event(&event);

        assert_eq!(ips(&filtered.instances), vec!["10.0.0.1", "10.0.0.4"]);
        let diff = filtered.diff.expect("diff must be preserved");
        assert_eq!(ips(&diff.added_instances), vec!["10.0.0.1"]);
        assert_eq!(ips(&diff.removed_instances), vec!["10.0.0.4"]);
        assert_eq!(ips(&diff.modified_instances), vec!["10.0.0.1"]);
    }

    #[test]
    fn wrapper_invokes_inner_listener_once_with_filtered_event() {
        let seen = Arc::new(Mutex::new(Vec::<Vec<String>>::new()));
        let count = Arc::new(AtomicUsize::new(0));
        let listener = FnEventListener::new({
            let seen = Arc::clone(&seen);
            let count = Arc::clone(&count);
            move |event: NamingEvent| {
                seen.lock().unwrap().push(ips(&event.instances));
                count.fetch_add(1, Ordering::SeqCst);
            }
        });

        let wrapper = SelectorEventListener::new(
            NamingSelectorFactory::healthy_selector(),
            Arc::new(listener),
        );
        wrapper.on_event(NamingEvent {
            service_name: "svc".to_string(),
            group_name: "DEFAULT_GROUP".to_string(),
            clusters: String::new(),
            instances: vec![
                instance("10.0.0.1", true, "A"),
                instance("10.0.0.2", false, "A"),
            ],
            diff: None,
        });

        assert_eq!(count.load(Ordering::SeqCst), 1);
        assert_eq!(*seen.lock().unwrap(), vec![vec!["10.0.0.1".to_string()]]);
    }

    struct NoopListener;
    impl EventListener for NoopListener {
        fn on_event(&self, _event: NamingEvent) {}
    }

    struct FnEventListener<F>
    where
        F: Fn(NamingEvent) + Send + Sync + 'static,
    {
        f: F,
    }

    impl<F> FnEventListener<F>
    where
        F: Fn(NamingEvent) + Send + Sync + 'static,
    {
        fn new(f: F) -> Self {
            Self { f }
        }
    }

    impl<F> EventListener for FnEventListener<F>
    where
        F: Fn(NamingEvent) + Send + Sync + 'static,
    {
        fn on_event(&self, event: NamingEvent) {
            (self.f)(event);
        }
    }
}
