use sea_orm::sea_query::Asterisk;
use sea_orm::*;
use sea_orm::{entity::ModelTrait, prelude::Expr};

use crate::api::model::Page;
use crate::auth::model::User;
use crate::entity::users;
use crate::error::BatataError;

pub async fn find_by_username(
    db: &DatabaseConnection,
    username: &str,
) -> anyhow::Result<Option<User>> {
    let user = users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(db)
        .await?
        .map(User::from);

    Ok(user)
}

pub async fn search_page(
    db: &DatabaseConnection,
    username: &str,
    page_no: u64,
    page_size: u64,
    accurate: bool,
) -> anyhow::Result<Page<User>> {
    let mut count_select = users::Entity::find();
    let mut query_select = users::Entity::find().columns([users::Column::Username]);

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
        .one(db)
        .await?
        .unwrap_or_default() as u64;

    if total_count > 0 {
        let offset = (page_no - 1) * page_size;
        let page_items = query_select
            .offset(offset)
            .limit(page_size)
            .all(db)
            .await?
            .iter()
            .map(User::from)
            .collect();

        return Ok(Page::<User>::new(
            total_count,
            page_no,
            page_size,
            page_items,
        ));
    }

    Ok(Page::<User>::default())
}

pub async fn search(db: &DatabaseConnection, username: &str) -> anyhow::Result<Vec<String>> {
    let users = users::Entity::find()
        .column(users::Column::Username)
        .filter(users::Column::Username.contains(username))
        .all(db)
        .await?
        .iter()
        .map(|user| user.username.to_string())
        .collect();

    return Ok(users);
}

pub async fn create(db: &DatabaseConnection, username: &str, password: &str) -> anyhow::Result<()> {
    let entity = users::ActiveModel {
        username: Set(username.to_string()),
        password: Set(password.to_string()),
        enabled: Set(1),
    };

    users::Entity::insert(entity).exec(db).await?;

    Ok(())
}

pub async fn update(
    db: &DatabaseConnection,
    username: &str,
    new_password: &str,
) -> anyhow::Result<()> {
    match users::Entity::find_by_id(username).one(db).await? {
        Some(entity) => {
            let mut user: users::ActiveModel = entity.into();

            user.password = Set(bcrypt::hash(new_password, 10u32).ok().unwrap());

            user.update(db).await?;

            Ok(())
        }
        None => Err(BatataError::UserNotExist(username.to_string()).into()),
    }
}

pub async fn delete(db: &DatabaseConnection, username: &str) -> anyhow::Result<()> {
    match users::Entity::find_by_id(username).one(db).await? {
        Some(entity) => {
            entity.delete(db).await?;

            Ok(())
        }
        None => Err(BatataError::UserNotExist(username.to_string()).into()),
    }
}
