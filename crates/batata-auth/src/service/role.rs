//! Role service

use std::sync::LazyLock;
use std::time::Duration;

use batata_api::Page;
use batata_common::error::BatataError;
use batata_persistence::entity::{roles, users};
use batata_persistence::sea_orm::prelude::Expr;
use batata_persistence::sea_orm::sea_query::Asterisk;
use batata_persistence::sea_orm::*;
use moka::sync::Cache;

use crate::model::{GLOBAL_ADMIN_ROLE, RoleInfo};

// Cache for user roles with 5-minute TTL
static ROLES_CACHE: LazyLock<Cache<String, Vec<RoleInfo>>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(10_000)
        .time_to_live(Duration::from_secs(300))
        .build()
});

pub async fn find_all(db: &DatabaseConnection) -> anyhow::Result<Vec<RoleInfo>> {
    let roles = roles::Entity::find()
        .all(db)
        .await?
        .iter()
        .map(RoleInfo::from)
        .collect();

    Ok(roles)
}

pub async fn find_by_username(
    db: &DatabaseConnection,
    username: &str,
) -> anyhow::Result<Vec<RoleInfo>> {
    // Use get() with &str directly - moka supports Borrow<str> lookup for String keys
    if let Some(cached_roles) = ROLES_CACHE.get(username) {
        return Ok(cached_roles);
    }

    let roles: Vec<RoleInfo> = roles::Entity::find()
        .filter(roles::Column::Username.eq(username))
        .all(db)
        .await?
        .iter()
        .map(RoleInfo::from)
        .collect();

    ROLES_CACHE.insert(username.to_owned(), roles.clone());

    Ok(roles)
}

/// Invalidate cache for a specific user
pub fn invalidate_roles_cache(username: &str) {
    ROLES_CACHE.invalidate(username);
}

/// Invalidate cache entries for users who have a specific role
/// This is more efficient than invalidating the entire cache when deleting a role
pub fn invalidate_roles_cache_for_role(role: &str) {
    let keys_to_invalidate: Vec<String> = ROLES_CACHE
        .iter()
        .filter(|(_, roles)| roles.iter().any(|r| r.role == role))
        .map(|(key, _)| (*key).clone())
        .collect();

    for key in keys_to_invalidate {
        ROLES_CACHE.invalidate(&key);
    }
}

/// Invalidate all role cache entries
/// Use sparingly - prefer targeted invalidation when possible
pub fn invalidate_all_roles_cache() {
    ROLES_CACHE.invalidate_all();
}

pub async fn search_page(
    db: &DatabaseConnection,
    username: &str,
    role: &str,
    page_no: u64,
    page_size: u64,
    accurate: bool,
) -> anyhow::Result<Page<RoleInfo>> {
    let mut count_select = roles::Entity::find();
    let mut query_select =
        roles::Entity::find().columns([roles::Column::Role, roles::Column::Username]);

    if !username.is_empty() {
        if accurate {
            count_select = count_select.filter(roles::Column::Username.eq(username));
            query_select = query_select.filter(roles::Column::Username.eq(username));
        } else {
            count_select = count_select.filter(roles::Column::Username.contains(username));
            query_select = query_select.filter(roles::Column::Username.contains(username));
        }
    }

    if !role.is_empty() {
        if accurate {
            count_select = count_select.filter(roles::Column::Role.eq(role));
            query_select = query_select.filter(roles::Column::Role.eq(role));
        } else {
            count_select = count_select.filter(roles::Column::Role.contains(role));
            query_select = query_select.filter(roles::Column::Role.contains(role));
        }
    }

    let total_count = count_select
        .select_only()
        .column_as(Expr::col(Asterisk).count(), "count")
        .into_tuple::<i64>()
        .one(db)
        .await?
        .unwrap_or_default() as u64;

    if total_count > 0 {
        let page_items = query_select
            .paginate(db, page_size)
            .fetch_page(page_no - 1)
            .await?
            .iter()
            .map(RoleInfo::from)
            .collect();

        return Ok(Page::<RoleInfo>::new(
            total_count,
            page_no,
            page_size,
            page_items,
        ));
    }

    Ok(Page::<RoleInfo>::default())
}

pub async fn search(db: &DatabaseConnection, role: &str) -> anyhow::Result<Vec<String>> {
    let roles = roles::Entity::find()
        .column(roles::Column::Role)
        .filter(roles::Column::Role.contains(role))
        .all(db)
        .await?
        .iter()
        .map(|role| role.role.to_string())
        .collect();

    Ok(roles)
}

pub async fn create(db: &DatabaseConnection, role: &str, username: &str) -> anyhow::Result<()> {
    if users::Entity::find_by_id(username).one(db).await?.is_none() {
        return Err(BatataError::IllegalArgument(format!("user '{}' not found!", username)).into());
    }
    if GLOBAL_ADMIN_ROLE == role {
        return Err(BatataError::IllegalArgument(format!(
            "role '{}' is not permitted to create!",
            GLOBAL_ADMIN_ROLE
        ))
        .into());
    }
    if roles::Entity::find_by_id((username.to_owned(), role.to_owned()))
        .one(db)
        .await?
        .is_some()
    {
        return Err(BatataError::IllegalArgument(format!(
            "user '{}' already bound to the role '{}'!",
            username, role
        ))
        .into());
    }

    let entity = roles::ActiveModel {
        role: Set(role.to_owned()),
        username: Set(username.to_owned()),
    };

    roles::Entity::insert(entity).exec(db).await?;
    invalidate_roles_cache(username);

    Ok(())
}

pub async fn delete(db: &DatabaseConnection, role: &str, username: &str) -> anyhow::Result<()> {
    if GLOBAL_ADMIN_ROLE == role {
        return Err(BatataError::IllegalArgument(format!(
            "role '{}' is not permitted to delete!",
            GLOBAL_ADMIN_ROLE
        ))
        .into());
    }

    if username.is_empty() {
        roles::Entity::delete_many()
            .filter(roles::Column::Role.eq(role))
            .filter(roles::Column::Role.ne(GLOBAL_ADMIN_ROLE))
            .exec(db)
            .await?;
        // Invalidate only users who have this role, not entire cache
        invalidate_roles_cache_for_role(role);
    } else {
        roles::Entity::delete_by_id((role.to_owned(), username.to_owned()))
            .exec(db)
            .await?;
        invalidate_roles_cache(username);
    }

    Ok(())
}

pub async fn has_global_admin_role(db: &DatabaseConnection) -> anyhow::Result<bool> {
    let result = roles::Entity::find()
        .select_only()
        .column_as(Expr::cust("1"), "exists_flag")
        .filter(roles::Column::Role.eq(GLOBAL_ADMIN_ROLE))
        .into_tuple::<i32>()
        .one(db)
        .await?;

    Ok(result.is_some())
}

pub async fn has_global_admin_role_by_username(
    db: &DatabaseConnection,
    username: &str,
) -> anyhow::Result<bool> {
    let result = roles::Entity::find()
        .select_only()
        .column_as(Expr::cust("1"), "exists_flag")
        .filter(roles::Column::Username.eq(username))
        .filter(roles::Column::Role.eq(GLOBAL_ADMIN_ROLE))
        .into_tuple::<i32>()
        .one(db)
        .await?;

    Ok(result.is_some())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Use unique prefixes to avoid test interference since tests run in parallel
    // and share the same static cache

    #[test]
    fn test_invalidate_roles_cache_for_user() {
        let prefix = "test_user_";
        let user1 = format!("{}user1", prefix);
        let user2 = format!("{}user2", prefix);

        // Insert some test entries
        let roles1 = vec![RoleInfo {
            role: "guest".to_string(),
            username: user1.clone(),
        }];
        let roles2 = vec![RoleInfo {
            role: "guest".to_string(),
            username: user2.clone(),
        }];

        ROLES_CACHE.insert(user1.clone(), roles1);
        ROLES_CACHE.insert(user2.clone(), roles2);

        // Invalidate cache for user1
        invalidate_roles_cache(&user1);

        // user1 should be invalidated
        assert!(ROLES_CACHE.get(&user1).is_none());
        // user2 should remain
        assert!(ROLES_CACHE.get(&user2).is_some());

        // Cleanup
        ROLES_CACHE.invalidate(&user2);
    }

    #[test]
    fn test_invalidate_roles_cache_for_role() {
        let prefix = "test_role_";
        let user1 = format!("{}user1", prefix);
        let user2 = format!("{}user2", prefix);
        let user3 = format!("{}user3", prefix);

        // Insert some test entries
        let roles_user1 = vec![
            RoleInfo {
                role: "superadmin".to_string(),
                username: user1.clone(),
            },
            RoleInfo {
                role: "developer".to_string(),
                username: user1.clone(),
            },
        ];
        let roles_user2 = vec![RoleInfo {
            role: "superadmin".to_string(),
            username: user2.clone(),
        }];
        let roles_user3 = vec![RoleInfo {
            role: "viewer".to_string(),
            username: user3.clone(),
        }];

        ROLES_CACHE.insert(user1.clone(), roles_user1);
        ROLES_CACHE.insert(user2.clone(), roles_user2);
        ROLES_CACHE.insert(user3.clone(), roles_user3);

        // Invalidate cache for users with "superadmin" role
        invalidate_roles_cache_for_role("superadmin");

        // Users with "superadmin" role should be invalidated
        assert!(ROLES_CACHE.get(&user1).is_none());
        assert!(ROLES_CACHE.get(&user2).is_none());
        // User without "superadmin" role should remain
        assert!(ROLES_CACHE.get(&user3).is_some());

        // Cleanup
        ROLES_CACHE.invalidate(&user3);
    }

    #[test]
    fn test_cache_lookup_with_str() {
        let username = "test_lookup_testuser";

        let roles = vec![RoleInfo {
            role: "test".to_string(),
            username: username.to_string(),
        }];
        ROLES_CACHE.insert(username.to_string(), roles);

        // Should be able to lookup with &str directly
        let result = ROLES_CACHE.get(username);
        assert!(result.is_some());
        assert_eq!(result.unwrap().len(), 1);

        ROLES_CACHE.invalidate(username);
    }
}
