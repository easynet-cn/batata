use sea_orm::*;

use crate::common;
use crate::common::model::{Page, User};
use crate::entity::users;

pub async fn find_by_username(db: &DatabaseConnection, username: &str) -> Option<User> {
    let user_entity = users::Entity::find()
        .filter(users::Column::Username.eq(username))
        .one(db)
        .await
        .unwrap();

    if user_entity.is_some() {
        let user_entity = user_entity.unwrap();

        Some(User {
            username: user_entity.username,
            password: user_entity.password,
        })
    } else {
        None
    }
}

pub async fn search_page(
    db: &DatabaseConnection,
    username: &str,
    page_no: u64,
    page_size: u64,
    accurate: bool,
) -> anyhow::Result<Page<User>> {
    let mut count_select = users::Entity::find();
    let mut query_select =
        users::Entity::find().columns([users::Column::Username, users::Column::Password]);

    if !username.is_empty() {
        if accurate {
            count_select = count_select.filter(users::Column::Username.eq(username));
            query_select = query_select.filter(users::Column::Username.eq(username));
        } else {
            count_select = count_select.filter(users::Column::Username.contains(username));
            query_select = query_select.filter(users::Column::Username.contains(username));
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
            .map(|user| User {
                username: user.username.clone(),
                password: user.password.clone(),
            })
            .collect();

        let page_result = Page::<User> {
            total_count: total_count,
            page_number: page_no,
            pages_available: (total_count as f64 / page_size as f64).ceil() as u64,
            page_items: page_items,
        };

        return anyhow::Ok(page_result);
    }

    let page_result = Page::<User> {
        total_count: total_count,
        page_number: page_no,
        pages_available: (total_count as f64 / page_size as f64).ceil() as u64,
        page_items: vec![],
    };

    return anyhow::Ok(page_result);
}

pub async fn create(db: &DatabaseConnection, username: &str, password: &str) -> anyhow::Result<()> {
    let entity = users::ActiveModel {
        username: Set(username.to_string()),
        password: Set(password.to_string()),
        enabled: Set(1),
    };

    users::Entity::insert(entity).exec(db).await?;

    anyhow::Ok(())
}

pub async fn update(
    db: &DatabaseConnection,
    username: &str,
    new_password: &str,
) -> anyhow::Result<()> {
    let user_option = users::Entity::find_by_id(username).one(db).await?;

    match user_option {
        Some(entity) => {
            let mut user: users::ActiveModel = entity.into();

            user.password = Set(bcrypt::hash(new_password, 10u32).ok().unwrap());

            user.update(db).await?;
        }
        None => {
            return Err(anyhow::Error::from(
                common::model::BusinessError::UserNotExist(username.to_string()),
            ))
        }
    }

    anyhow::Ok(())
}
