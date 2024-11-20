use anyhow::Ok;
use sea_orm::*;

use crate::{
    common::model::{Page, RoleInfo},
    entity::roles,
};

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
        .map(|role| RoleInfo {
            role: role.role.clone(),
            username: role.username.clone(),
        })
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

    let total_count = count_select.count(db).await?;

    if total_count > 0 {
        let page_items = query_select
            .paginate(db, page_size)
            .fetch_page(page_no - 1)
            .await?
            .iter()
            .map(|entity| RoleInfo::from(entity.clone()))
            .collect();

        return anyhow::Ok(Page::<RoleInfo>::new(
            total_count,
            page_no,
            page_size,
            page_items,
        ));
    }

    return anyhow::Ok(Page::<RoleInfo>::default());
}

pub async fn search(db: &DatabaseConnection, role: &str) -> anyhow::Result<Vec<String>> {
    let users = roles::Entity::find()
        .column(roles::Column::Role)
        .filter(roles::Column::Role.contains(role))
        .all(db)
        .await?
        .iter()
        .map(|role| role.role.clone())
        .collect();

    return anyhow::Ok(users);
}

pub async fn create(db: &DatabaseConnection, role: &str, username: &str) -> anyhow::Result<()> {
    let entity = roles::ActiveModel {
        role: Set(role.to_string()),
        username: Set(username.to_string()),
    };

    roles::Entity::insert(entity).exec(db).await?;

    anyhow::Ok(())
}

pub async fn delete(db: &DatabaseConnection, role: &str, username: &str) -> anyhow::Result<()> {
    if username.is_empty() {
        roles::Entity::delete_many()
            .filter(roles::Column::Role.eq(role))
            .exec(db)
            .await?;
    } else {
        roles::Entity::delete_by_id((role.to_string(), username.to_string()))
            .exec(db)
            .await?;
    }

    anyhow::Ok(())
}
