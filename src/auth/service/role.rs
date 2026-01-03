use std::sync::LazyLock;
use std::time::Duration;

use moka::sync::Cache;
use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::{
    api::model::Page,
    auth::model::{GLOBAL_ADMIN_ROLE, RoleInfo},
    entity::{roles, users},
    error::BatataError,
};

// Cache for user roles with 5-minute TTL
// Key: username, Value: Vec<RoleInfo>
static ROLES_CACHE: LazyLock<Cache<String, Vec<RoleInfo>>> = LazyLock::new(|| {
    Cache::builder()
        .max_capacity(10_000)
        .time_to_live(Duration::from_secs(300)) // 5 minutes TTL
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
    // Check cache first
    if let Some(cached_roles) = ROLES_CACHE.get(&username.to_string()) {
        return Ok(cached_roles);
    }

    // Cache miss - query database
    let roles: Vec<RoleInfo> = roles::Entity::find()
        .filter(roles::Column::Username.eq(username))
        .all(db)
        .await?
        .iter()
        .map(RoleInfo::from)
        .collect();

    // Store in cache
    ROLES_CACHE.insert(username.to_string(), roles.clone());

    Ok(roles)
}

/// Invalidate roles cache for a specific user
pub fn invalidate_roles_cache(username: &str) {
    ROLES_CACHE.invalidate(&username.to_string());
}

/// Invalidate all roles cache
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
    if roles::Entity::find_by_id((username.to_string(), role.to_string()))
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
        role: Set(role.to_string()),
        username: Set(username.to_string()),
    };

    roles::Entity::insert(entity).exec(db).await?;

    // Invalidate cache for this user
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
        // Invalidate all caches since multiple users may be affected
        invalidate_all_roles_cache();
    } else {
        roles::Entity::delete_by_id((role.to_string(), username.to_string()))
            .exec(db)
            .await?;
        // Invalidate cache for this specific user
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
