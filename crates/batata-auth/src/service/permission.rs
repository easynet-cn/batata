//! Permission service

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::sync::LazyLock;
use std::time::Duration;

use batata_api::Page;
use batata_persistence::entity::permissions;
use batata_persistence::sea_orm::*;
use moka::sync::Cache;

use crate::model::PermissionInfo;

/// Cache entry that stores permissions
#[derive(Clone)]
struct CachedPermissions {
    permissions: Vec<PermissionInfo>,
}

// Cache for permissions by roles with 5-minute TTL
// Key is a hash of sorted roles for efficient lookup
static PERMISSIONS_CACHE: LazyLock<Cache<u64, CachedPermissions>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(5_000)
        .time_to_live(Duration::from_secs(300))
        .build()
});

// Secondary index: role -> set of cache keys that contain this role
// Enables O(1) targeted cache invalidation instead of O(n) full scan
static ROLE_CACHE_INDEX: LazyLock<dashmap::DashMap<String, Vec<u64>>> =
    LazyLock::new(dashmap::DashMap::new);

/// Generate cache key from roles using efficient hashing
/// This avoids allocating a new Vec and joining strings
fn make_cache_key(roles: &[String]) -> u64 {
    let mut hasher = DefaultHasher::new();
    // Sort roles in a stable way without allocating
    let mut sorted_indices: Vec<usize> = (0..roles.len()).collect();
    sorted_indices.sort_by(|&a, &b| roles[a].cmp(&roles[b]));

    for &idx in &sorted_indices {
        roles[idx].hash(&mut hasher);
        // Add separator to prevent collisions like ["ab", "c"] vs ["a", "bc"]
        0u8.hash(&mut hasher);
    }
    hasher.finish()
}

pub async fn find_by_id(
    db: &DatabaseConnection,
    role: &str,
    resource: &str,
    action: &str,
) -> anyhow::Result<Option<PermissionInfo>> {
    let permission = permissions::Entity::find_by_id((
        role.to_string(),
        resource.to_string(),
        action.to_string(),
    ))
    .one(db)
    .await?
    .map(PermissionInfo::from);

    Ok(permission)
}

pub async fn search_page(
    db: &DatabaseConnection,
    role: &str,
    page_no: u64,
    page_size: u64,
    accurate: bool,
) -> anyhow::Result<Page<PermissionInfo>> {
    let mut count_select = permissions::Entity::find();
    let mut query_select = permissions::Entity::find();

    if !role.is_empty() {
        if accurate {
            count_select = count_select.filter(permissions::Column::Role.eq(role));
            query_select = query_select.filter(permissions::Column::Role.eq(role));
        } else {
            count_select = count_select.filter(permissions::Column::Role.contains(role));
            query_select = query_select.filter(permissions::Column::Role.contains(role));
        }
    }

    let total_count = count_select.count(db).await?;

    if total_count > 0 {
        let page_items = query_select
            .paginate(db, page_size)
            .fetch_page(page_no - 1)
            .await?
            .iter()
            .map(PermissionInfo::from)
            .collect();

        return Ok(Page::<PermissionInfo>::new(
            total_count,
            page_no,
            page_size,
            page_items,
        ));
    }

    Ok(Page::<PermissionInfo>::default())
}

pub async fn find_by_roles(
    db: &DatabaseConnection,
    roles: Vec<String>,
) -> anyhow::Result<Vec<PermissionInfo>> {
    if roles.is_empty() {
        return Ok(vec![]);
    }

    let cache_key = make_cache_key(&roles);
    if let Some(cached) = PERMISSIONS_CACHE.get(&cache_key) {
        return Ok(cached.permissions);
    }

    let permissions: Vec<PermissionInfo> = permissions::Entity::find()
        .filter(permissions::Column::Role.is_in(&roles))
        .all(db)
        .await?
        .iter()
        .map(PermissionInfo::from)
        .collect();

    // Update secondary index for O(1) invalidation
    for role in &roles {
        ROLE_CACHE_INDEX
            .entry(role.clone())
            .or_default()
            .push(cache_key);
    }

    PERMISSIONS_CACHE.insert(
        cache_key,
        CachedPermissions {
            permissions: permissions.clone(),
        },
    );

    Ok(permissions)
}

/// Invalidate cache entries that contain the specified role
/// Uses secondary index for O(1) lookup instead of O(n) full scan
pub fn invalidate_permissions_cache_for_role(role: &str) {
    if let Some((_, keys)) = ROLE_CACHE_INDEX.remove(role) {
        for key in keys {
            PERMISSIONS_CACHE.invalidate(&key);
        }
    }
}

/// Invalidate all permission cache entries
/// Use sparingly - prefer `invalidate_permissions_cache_for_role` when possible
pub fn invalidate_all_permissions_cache() {
    PERMISSIONS_CACHE.invalidate_all();
    ROLE_CACHE_INDEX.clear();
}

pub async fn create(
    db: &DatabaseConnection,
    role: &str,
    resource: &str,
    action: &str,
) -> anyhow::Result<()> {
    let entity = permissions::ActiveModel {
        role: Set(role.to_owned()),
        resource: Set(resource.to_owned()),
        action: Set(action.to_owned()),
    };

    permissions::Entity::insert(entity).exec(db).await?;
    // Only invalidate cache entries that contain this role
    invalidate_permissions_cache_for_role(role);

    Ok(())
}

pub async fn delete(
    db: &DatabaseConnection,
    role: &str,
    resource: &str,
    action: &str,
) -> anyhow::Result<()> {
    permissions::Entity::delete_by_id((role.to_owned(), resource.to_owned(), action.to_owned()))
        .exec(db)
        .await?;

    // Only invalidate cache entries that contain this role
    invalidate_permissions_cache_for_role(role);

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_make_cache_key_deterministic() {
        let roles1 = vec!["admin".to_string(), "user".to_string()];
        let roles2 = vec!["admin".to_string(), "user".to_string()];

        assert_eq!(make_cache_key(&roles1), make_cache_key(&roles2));
    }

    #[test]
    fn test_make_cache_key_order_independent() {
        let roles1 = vec!["admin".to_string(), "user".to_string()];
        let roles2 = vec!["user".to_string(), "admin".to_string()];

        // Same roles in different order should produce the same key
        assert_eq!(make_cache_key(&roles1), make_cache_key(&roles2));
    }

    #[test]
    fn test_make_cache_key_different_roles() {
        let roles1 = vec!["admin".to_string()];
        let roles2 = vec!["user".to_string()];

        assert_ne!(make_cache_key(&roles1), make_cache_key(&roles2));
    }

    #[test]
    fn test_make_cache_key_no_collision() {
        // Test that ["ab", "c"] and ["a", "bc"] produce different keys
        let roles1 = vec!["ab".to_string(), "c".to_string()];
        let roles2 = vec!["a".to_string(), "bc".to_string()];

        assert_ne!(make_cache_key(&roles1), make_cache_key(&roles2));
    }

    #[test]
    fn test_make_cache_key_empty() {
        let roles: Vec<String> = vec![];
        // Should not panic
        let _ = make_cache_key(&roles);
    }

    #[test]
    fn test_cache_invalidation_for_role() {
        // Clear any existing cache entries
        invalidate_all_permissions_cache();

        // Insert some test entries and populate secondary index
        let roles1 = vec!["admin".to_string(), "user".to_string()];
        let roles2 = vec!["admin".to_string(), "guest".to_string()];
        let roles3 = vec!["guest".to_string()];

        let key1 = make_cache_key(&roles1);
        let key2 = make_cache_key(&roles2);
        let key3 = make_cache_key(&roles3);

        // Populate secondary index
        for role in &roles1 {
            ROLE_CACHE_INDEX.entry(role.clone()).or_default().push(key1);
        }
        for role in &roles2 {
            ROLE_CACHE_INDEX.entry(role.clone()).or_default().push(key2);
        }
        for role in &roles3 {
            ROLE_CACHE_INDEX.entry(role.clone()).or_default().push(key3);
        }

        PERMISSIONS_CACHE.insert(
            key1,
            CachedPermissions {
                permissions: vec![],
            },
        );
        PERMISSIONS_CACHE.insert(
            key2,
            CachedPermissions {
                permissions: vec![],
            },
        );
        PERMISSIONS_CACHE.insert(
            key3,
            CachedPermissions {
                permissions: vec![],
            },
        );

        // Invalidate cache for "admin" role
        invalidate_permissions_cache_for_role("admin");

        // Entries with "admin" should be invalidated
        assert!(PERMISSIONS_CACHE.get(&key1).is_none());
        assert!(PERMISSIONS_CACHE.get(&key2).is_none());
        // Entry without "admin" should remain
        assert!(PERMISSIONS_CACHE.get(&key3).is_some());

        // Cleanup
        invalidate_all_permissions_cache();
    }
}
