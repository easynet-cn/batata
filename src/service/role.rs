use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::{
    BatataError,
    entity::{roles, users},
    model::{
        self,
        auth::{self, RoleInfo},
        common::Page,
    },
};

pub async fn find_all(db: &DatabaseConnection) -> anyhow::Result<Vec<RoleInfo>> {
    let user_roles = roles::Entity::find()
        .all(db)
        .await
        .unwrap()
        .iter()
        .map(RoleInfo::from)
        .collect();

    Ok(user_roles)
}

pub async fn find_by_username(
    db: &DatabaseConnection,
    username: &str,
) -> anyhow::Result<Vec<RoleInfo>> {
    let user_roles = roles::Entity::find()
        .filter(roles::Column::Username.eq(username))
        .all(db)
        .await
        .unwrap()
        .iter()
        .map(RoleInfo::from)
        .collect();

    Ok(user_roles)
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

    return Ok(Page::<RoleInfo>::default());
}

pub async fn search(db: &DatabaseConnection, role: &str) -> anyhow::Result<Vec<String>> {
    let users = roles::Entity::find()
        .column(roles::Column::Role)
        .filter(roles::Column::Role.contains(role))
        .all(db)
        .await?
        .iter()
        .map(|role| role.role.to_string())
        .collect();

    return Ok(users);
}

pub async fn create(db: &DatabaseConnection, role: &str, username: &str) -> anyhow::Result<()> {
    if users::Entity::find_by_id(username).one(db).await?.is_none() {
        return Err(BatataError::IllegalArgument(format!("user '{}' not found!", username)).into());
    }
    if auth::GLOBAL_ADMIN_ROLE == role {
        return Err(BatataError::IllegalArgument(format!(
            "role '{}' is not permitted to create!",
            auth::GLOBAL_ADMIN_ROLE
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

    Ok(())
}

pub async fn delete(db: &DatabaseConnection, role: &str, username: &str) -> anyhow::Result<()> {
    if auth::GLOBAL_ADMIN_ROLE == role {
        return Err(BatataError::IllegalArgument(format!(
            "role '{}' is not permitted to delete!",
            auth::GLOBAL_ADMIN_ROLE
        ))
        .into());
    }

    if username.is_empty() {
        roles::Entity::delete_many()
            .filter(roles::Column::Role.eq(role))
            .filter(roles::Column::Role.ne(auth::GLOBAL_ADMIN_ROLE))
            .exec(db)
            .await?;
    } else {
        roles::Entity::delete_by_id((role.to_string(), username.to_string()))
            .exec(db)
            .await?;
    }

    Ok(())
}

pub async fn has_global_admin_role(db: &DatabaseConnection) -> anyhow::Result<bool> {
    let has = find_all(db)
        .await?
        .iter()
        .any(|role| role.role == model::auth::GLOBAL_ADMIN_ROLE);

    Ok(has)
}

pub async fn has_global_admin_role_by_username(
    db: &DatabaseConnection,
    username: &str,
) -> anyhow::Result<bool> {
    let has = find_by_username(db, username)
        .await?
        .iter()
        .any(|role| role.role == model::auth::GLOBAL_ADMIN_ROLE);

    Ok(has)
}
