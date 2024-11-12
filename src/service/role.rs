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
        let query_result = query_select
            .paginate(db, page_size)
            .fetch_page(page_no - 1)
            .await?;
        let page_items = query_result
            .iter()
            .map(|role| RoleInfo {
                username: role.username.clone(),
                role: role.role.clone(),
            })
            .collect();

        let page_result = Page::<RoleInfo> {
            total_count: total_count,
            page_number: page_no,
            pages_available: (total_count as f64 / page_size as f64).ceil() as u64,
            page_items: page_items,
        };

        return anyhow::Ok(page_result);
    }

    let page_result = Page::<RoleInfo> {
        total_count: total_count,
        page_number: page_no,
        pages_available: 0,
        page_items: vec![],
    };

    return anyhow::Ok(page_result);
}
