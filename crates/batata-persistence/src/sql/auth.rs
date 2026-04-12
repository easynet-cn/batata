//! AuthPersistence implementation for ExternalDbPersistService

use async_trait::async_trait;
use sea_orm::{prelude::Expr, sea_query::Asterisk, *};

use crate::entity::{permissions, roles, users};
use crate::model::*;
use crate::traits::*;

use super::ExternalDbPersistService;

#[async_trait]
impl AuthPersistence for ExternalDbPersistService {
    async fn user_find_by_username(&self, username: &str) -> anyhow::Result<Option<UserInfo>> {
        let user = users::Entity::find()
            .filter(users::Column::Username.eq(username))
            .one(&self.db)
            .await?
            .map(|m| UserInfo {
                username: m.username,
                password: m.password,
                enabled: m.enabled,
            });

        Ok(user)
    }

    async fn user_find_page(
        &self,
        username: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<UserInfo>> {
        let mut count_select = users::Entity::find();
        let mut query_select = users::Entity::find();

        if !username.is_empty() {
            if accurate {
                count_select = count_select.filter(users::Column::Username.eq(username));
                query_select = query_select.filter(users::Column::Username.eq(username));
            } else {
                count_select = count_select.filter(users::Column::Username.contains(username));
                query_select = query_select.filter(users::Column::Username.contains(username));
            }
        }

        let total_count = count_select
            .select_only()
            .column_as(Expr::col(Asterisk).count(), "count")
            .into_tuple::<i64>()
            .one(&self.db)
            .await?
            .unwrap_or_default() as u64;

        if total_count == 0 {
            return Ok(Page::empty());
        }

        let offset = (page_no - 1) * page_size;
        let items = query_select
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| UserInfo {
                username: m.username,
                password: m.password,
                enabled: m.enabled,
            })
            .collect();

        Ok(Page::new(total_count, page_no, page_size, items))
    }

    async fn user_create(
        &self,
        username: &str,
        password_hash: &str,
        _enabled: bool,
    ) -> anyhow::Result<()> {
        let entity = users::ActiveModel {
            username: Set(username.to_string()),
            password: Set(password_hash.to_string()),
            enabled: Set(true),
        };

        users::Entity::insert(entity).exec(&self.db).await?;
        Ok(())
    }

    async fn user_update_password(
        &self,
        username: &str,
        password_hash: &str,
    ) -> anyhow::Result<()> {
        match users::Entity::find_by_id(username).one(&self.db).await? {
            Some(entity) => {
                let mut user: users::ActiveModel = entity.into();
                user.password = Set(password_hash.to_string());
                user.update(&self.db).await?;
                Ok(())
            }
            None => Err(anyhow::anyhow!("User not found: {}", username)),
        }
    }

    async fn user_delete(&self, username: &str) -> anyhow::Result<()> {
        match users::Entity::find_by_id(username).one(&self.db).await? {
            Some(entity) => {
                entity.delete(&self.db).await?;
                Ok(())
            }
            None => Err(anyhow::anyhow!("User not found: {}", username)),
        }
    }

    async fn user_search(&self, username: &str) -> anyhow::Result<Vec<String>> {
        let usernames = users::Entity::find()
            .select_only()
            .column(users::Column::Username)
            .filter(users::Column::Username.like(format!("%{}%", username)))
            .into_tuple::<String>()
            .all(&self.db)
            .await?;

        Ok(usernames)
    }

    async fn role_find_by_username(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        let role_list = roles::Entity::find()
            .filter(roles::Column::Username.eq(username))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| RoleInfo {
                role: m.role,
                username: m.username,
            })
            .collect();

        Ok(role_list)
    }

    async fn role_find_page(
        &self,
        username: &str,
        role: &str,
        page_no: u64,
        page_size: u64,
        accurate: bool,
    ) -> anyhow::Result<Page<RoleInfo>> {
        let mut count_select = roles::Entity::find();
        let mut query_select = roles::Entity::find();

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
            .one(&self.db)
            .await?
            .unwrap_or_default() as u64;

        if total_count == 0 {
            return Ok(Page::empty());
        }

        let offset = (page_no - 1) * page_size;
        let items = query_select
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| RoleInfo {
                role: m.role,
                username: m.username,
            })
            .collect();

        Ok(Page::new(total_count, page_no, page_size, items))
    }

    async fn role_create(&self, role: &str, username: &str) -> anyhow::Result<()> {
        let entity = roles::ActiveModel {
            role: Set(role.to_string()),
            username: Set(username.to_string()),
        };

        roles::Entity::insert(entity).exec(&self.db).await?;
        Ok(())
    }

    async fn role_delete(&self, role: &str, username: &str) -> anyhow::Result<()> {
        if username.is_empty() {
            roles::Entity::delete_many()
                .filter(roles::Column::Role.eq(role))
                .exec(&self.db)
                .await?;
        } else {
            // PK order in entity is (username, role) — must match
            roles::Entity::delete_by_id((username.to_string(), role.to_string()))
                .exec(&self.db)
                .await?;
        }
        Ok(())
    }

    async fn role_has_global_admin(&self) -> anyhow::Result<bool> {
        let result = roles::Entity::find()
            .select_only()
            .column_as(Expr::cust("1"), "exists_flag")
            .filter(roles::Column::Role.eq("ROLE_ADMIN"))
            .into_tuple::<i32>()
            .one(&self.db)
            .await?;

        Ok(result.is_some())
    }

    async fn role_has_global_admin_by_username(&self, username: &str) -> anyhow::Result<bool> {
        let result = roles::Entity::find()
            .select_only()
            .column_as(Expr::cust("1"), "exists_flag")
            .filter(roles::Column::Username.eq(username))
            .filter(roles::Column::Role.eq("ROLE_ADMIN"))
            .into_tuple::<i32>()
            .one(&self.db)
            .await?;

        Ok(result.is_some())
    }

    async fn role_search(&self, role: &str) -> anyhow::Result<Vec<String>> {
        let role_names = roles::Entity::find()
            .select_only()
            .column(roles::Column::Role)
            .filter(roles::Column::Role.contains(role))
            .into_tuple::<String>()
            .all(&self.db)
            .await?;

        Ok(role_names)
    }

    async fn permission_find_by_role(&self, role: &str) -> anyhow::Result<Vec<PermissionInfo>> {
        let perms = permissions::Entity::find()
            .filter(permissions::Column::Role.eq(role))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| PermissionInfo {
                role: m.role,
                resource: m.resource,
                action: m.action,
            })
            .collect();

        Ok(perms)
    }

    async fn permission_find_by_roles(
        &self,
        role_list: Vec<String>,
    ) -> anyhow::Result<Vec<PermissionInfo>> {
        if role_list.is_empty() {
            return Ok(vec![]);
        }

        let perms = permissions::Entity::find()
            .filter(permissions::Column::Role.is_in(&role_list))
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| PermissionInfo {
                role: m.role,
                resource: m.resource,
                action: m.action,
            })
            .collect();

        Ok(perms)
    }

    async fn permission_find_page(
        &self,
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

        let total_count = count_select.count(&self.db).await?;

        if total_count == 0 {
            return Ok(Page::empty());
        }

        let offset = (page_no - 1) * page_size;
        let items = query_select
            .offset(offset)
            .limit(page_size)
            .all(&self.db)
            .await?
            .into_iter()
            .map(|m| PermissionInfo {
                role: m.role,
                resource: m.resource,
                action: m.action,
            })
            .collect();

        Ok(Page::new(total_count, page_no, page_size, items))
    }

    async fn permission_find_by_id(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<Option<PermissionInfo>> {
        let perm = permissions::Entity::find_by_id((
            role.to_string(),
            resource.to_string(),
            action.to_string(),
        ))
        .one(&self.db)
        .await?
        .map(|m| PermissionInfo {
            role: m.role,
            resource: m.resource,
            action: m.action,
        });

        Ok(perm)
    }

    async fn permission_grant(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        let entity = permissions::ActiveModel {
            role: Set(role.to_string()),
            resource: Set(resource.to_string()),
            action: Set(action.to_string()),
        };

        permissions::Entity::insert(entity).exec(&self.db).await?;
        Ok(())
    }

    async fn permission_revoke(
        &self,
        role: &str,
        resource: &str,
        action: &str,
    ) -> anyhow::Result<()> {
        permissions::Entity::delete_by_id((
            role.to_string(),
            resource.to_string(),
            action.to_string(),
        ))
        .exec(&self.db)
        .await?;
        Ok(())
    }
}
