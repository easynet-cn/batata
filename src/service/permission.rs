use sea_orm::*;

use crate::{entity::permissions, model::auth::PermissionInfo, model::common::Page};

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
            .map(|entity| PermissionInfo::from(entity.clone()))
            .collect();

        return anyhow::Ok(Page::<PermissionInfo>::new(
            total_count,
            page_no,
            page_size,
            page_items,
        ));
    }

    return anyhow::Ok(Page::<PermissionInfo>::default());
}

pub async fn find_by_roles(
    db: &DatabaseConnection,
    roles: Vec<String>,
) -> anyhow::Result<Vec<PermissionInfo>> {
    if roles.is_empty() {
        return anyhow::Ok(vec![]);
    } else {
        let permissions = permissions::Entity::find()
            .filter(permissions::Column::Role.is_in(roles))
            .all(db)
            .await?
            .iter()
            .map(|entity| PermissionInfo::from(entity.clone()))
            .collect();

        return anyhow::Ok(permissions);
    }
}

pub async fn create(
    db: &DatabaseConnection,
    role: &str,
    resource: &str,
    action: &str,
) -> anyhow::Result<()> {
    let entity = permissions::ActiveModel {
        role: Set(role.to_string()),
        resource: Set(resource.to_string()),
        action: Set(action.to_string()),
    };

    permissions::Entity::insert(entity).exec(db).await?;

    anyhow::Ok(())
}

pub async fn delete(
    db: &DatabaseConnection,
    role: &str,
    resource: &str,
    action: &str,
) -> anyhow::Result<()> {
    permissions::Entity::delete_by_id((role.to_string(), resource.to_string(), action.to_string()))
        .exec(db)
        .await?;

    anyhow::Ok(())
}
