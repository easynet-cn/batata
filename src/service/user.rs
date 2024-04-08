use sea_orm::*;

use crate::common::model::User;
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
