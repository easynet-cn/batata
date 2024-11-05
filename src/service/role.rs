use anyhow::Ok;
use sea_orm::*;

use crate::{common::model::RoleInfo, entity::roles};

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
