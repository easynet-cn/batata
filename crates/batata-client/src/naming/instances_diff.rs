//! Instance diff tracking for service discovery
//!
//! Tracks which instances were added, removed, or modified between
//! service updates, enabling efficient change processing.

use batata_api::naming::model::Instance;
use std::collections::HashMap;

/// Tracks the differences between two sets of service instances
#[derive(Debug, Clone, Default)]
pub struct InstancesDiff {
    /// Instances that were added
    pub added_instances: Vec<Instance>,
    /// Instances that were removed
    pub removed_instances: Vec<Instance>,
    /// Instances that were modified (new version)
    pub modified_instances: Vec<Instance>,
}

impl InstancesDiff {
    /// Compute the diff between old and new instance lists
    pub fn diff(old_instances: &[Instance], new_instances: &[Instance]) -> Self {
        let old_map: HashMap<String, &Instance> =
            old_instances.iter().map(|i| (i.key(), i)).collect();
        let new_map: HashMap<String, &Instance> =
            new_instances.iter().map(|i| (i.key(), i)).collect();

        let mut added = Vec::new();
        let mut removed = Vec::new();
        let mut modified = Vec::new();

        // Find added and modified
        for (key, new_inst) in &new_map {
            match old_map.get(key) {
                Some(old_inst) => {
                    if is_instance_modified(old_inst, new_inst) {
                        modified.push((*new_inst).clone());
                    }
                }
                None => {
                    added.push((*new_inst).clone());
                }
            }
        }

        // Find removed
        for (key, old_inst) in &old_map {
            if !new_map.contains_key(key) {
                removed.push((*old_inst).clone());
            }
        }

        Self {
            added_instances: added,
            removed_instances: removed,
            modified_instances: modified,
        }
    }

    /// Check if there are any differences
    pub fn has_diff(&self) -> bool {
        !self.added_instances.is_empty()
            || !self.removed_instances.is_empty()
            || !self.modified_instances.is_empty()
    }

    /// Total number of changes
    pub fn change_count(&self) -> usize {
        self.added_instances.len() + self.removed_instances.len() + self.modified_instances.len()
    }
}

/// Check if an instance has been modified
fn is_instance_modified(old: &Instance, new: &Instance) -> bool {
    old.weight != new.weight
        || old.healthy != new.healthy
        || old.enabled != new.enabled
        || old.metadata != new.metadata
        || old.ephemeral != new.ephemeral
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_instance(ip: &str, port: i32, healthy: bool) -> Instance {
        Instance {
            ip: ip.to_string(),
            port,
            healthy,
            weight: 1.0,
            enabled: true,
            ephemeral: true,
            cluster_name: "DEFAULT".to_string(),
            ..Default::default()
        }
    }

    #[test]
    fn test_no_diff() {
        let instances = vec![make_instance("10.0.0.1", 8080, true)];
        let diff = InstancesDiff::diff(&instances, &instances);
        assert!(!diff.has_diff());
        assert_eq!(diff.change_count(), 0);
    }

    #[test]
    fn test_added() {
        let old = vec![make_instance("10.0.0.1", 8080, true)];
        let new = vec![
            make_instance("10.0.0.1", 8080, true),
            make_instance("10.0.0.2", 8080, true),
        ];
        let diff = InstancesDiff::diff(&old, &new);
        assert!(diff.has_diff());
        assert_eq!(diff.added_instances.len(), 1);
        assert_eq!(diff.added_instances[0].ip, "10.0.0.2");
        assert!(diff.removed_instances.is_empty());
        assert!(diff.modified_instances.is_empty());
    }

    #[test]
    fn test_removed() {
        let old = vec![
            make_instance("10.0.0.1", 8080, true),
            make_instance("10.0.0.2", 8080, true),
        ];
        let new = vec![make_instance("10.0.0.1", 8080, true)];
        let diff = InstancesDiff::diff(&old, &new);
        assert!(diff.has_diff());
        assert!(diff.added_instances.is_empty());
        assert_eq!(diff.removed_instances.len(), 1);
        assert_eq!(diff.removed_instances[0].ip, "10.0.0.2");
    }

    #[test]
    fn test_modified() {
        let old = vec![make_instance("10.0.0.1", 8080, true)];
        let new = vec![make_instance("10.0.0.1", 8080, false)]; // healthy changed
        let diff = InstancesDiff::diff(&old, &new);
        assert!(diff.has_diff());
        assert!(diff.added_instances.is_empty());
        assert!(diff.removed_instances.is_empty());
        assert_eq!(diff.modified_instances.len(), 1);
        assert!(!diff.modified_instances[0].healthy);
    }

    #[test]
    fn test_mixed_changes() {
        let old = vec![
            make_instance("10.0.0.1", 8080, true),
            make_instance("10.0.0.2", 8080, true),
        ];
        let new = vec![
            make_instance("10.0.0.1", 8080, false), // modified
            make_instance("10.0.0.3", 8080, true),  // added
                                                    // 10.0.0.2 removed
        ];
        let diff = InstancesDiff::diff(&old, &new);
        assert!(diff.has_diff());
        assert_eq!(diff.added_instances.len(), 1);
        assert_eq!(diff.removed_instances.len(), 1);
        assert_eq!(diff.modified_instances.len(), 1);
        assert_eq!(diff.change_count(), 3);
    }

    #[test]
    fn test_empty_to_populated() {
        let old: Vec<Instance> = vec![];
        let new = vec![
            make_instance("10.0.0.1", 8080, true),
            make_instance("10.0.0.2", 8080, true),
        ];
        let diff = InstancesDiff::diff(&old, &new);
        assert!(diff.has_diff());
        assert_eq!(diff.added_instances.len(), 2);
        assert!(diff.removed_instances.is_empty());
        assert!(diff.modified_instances.is_empty());
    }

    #[test]
    fn test_populated_to_empty() {
        let old = vec![
            make_instance("10.0.0.1", 8080, true),
            make_instance("10.0.0.2", 8080, true),
        ];
        let new: Vec<Instance> = vec![];
        let diff = InstancesDiff::diff(&old, &new);
        assert!(diff.has_diff());
        assert!(diff.added_instances.is_empty());
        assert_eq!(diff.removed_instances.len(), 2);
        assert!(diff.modified_instances.is_empty());
    }

    #[test]
    fn test_both_empty() {
        let diff = InstancesDiff::diff(&[], &[]);
        assert!(!diff.has_diff());
        assert_eq!(diff.change_count(), 0);
    }

    #[test]
    fn test_weight_modified() {
        let old = vec![make_instance("10.0.0.1", 8080, true)];
        let mut new_inst = make_instance("10.0.0.1", 8080, true);
        new_inst.weight = 2.0;
        let new = vec![new_inst];

        let diff = InstancesDiff::diff(&old, &new);
        assert!(diff.has_diff());
        assert_eq!(diff.modified_instances.len(), 1);
        assert_eq!(diff.modified_instances[0].weight, 2.0);
    }

    #[test]
    fn test_enabled_modified() {
        let old = vec![make_instance("10.0.0.1", 8080, true)];
        let mut new_inst = make_instance("10.0.0.1", 8080, true);
        new_inst.enabled = false;
        let new = vec![new_inst];

        let diff = InstancesDiff::diff(&old, &new);
        assert!(diff.has_diff());
        assert_eq!(diff.modified_instances.len(), 1);
        assert!(!diff.modified_instances[0].enabled);
    }

    #[test]
    fn test_metadata_modified() {
        let old = vec![make_instance("10.0.0.1", 8080, true)];
        let mut new_inst = make_instance("10.0.0.1", 8080, true);
        new_inst
            .metadata
            .insert("version".to_string(), "2.0".to_string());
        let new = vec![new_inst];

        let diff = InstancesDiff::diff(&old, &new);
        assert!(diff.has_diff());
        assert_eq!(diff.modified_instances.len(), 1);
    }

    #[test]
    fn test_ephemeral_modified() {
        let old = vec![make_instance("10.0.0.1", 8080, true)];
        let mut new_inst = make_instance("10.0.0.1", 8080, true);
        new_inst.ephemeral = false;
        let new = vec![new_inst];

        let diff = InstancesDiff::diff(&old, &new);
        assert!(diff.has_diff());
        assert_eq!(diff.modified_instances.len(), 1);
    }

    #[test]
    fn test_default() {
        let diff = InstancesDiff::default();
        assert!(!diff.has_diff());
        assert_eq!(diff.change_count(), 0);
    }

    #[test]
    fn test_clone() {
        let old = vec![make_instance("10.0.0.1", 8080, true)];
        let new = vec![
            make_instance("10.0.0.1", 8080, true),
            make_instance("10.0.0.2", 8080, true),
        ];
        let diff = InstancesDiff::diff(&old, &new);
        let cloned = diff.clone();
        assert_eq!(cloned.added_instances.len(), diff.added_instances.len());
        assert_eq!(cloned.change_count(), diff.change_count());
    }
}
