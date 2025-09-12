use actix_web::{HttpMessage, HttpRequest, Responder, Scope, get, web};

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::model::Member,
    model::{self, common::AppState},
    secured,
};

#[get("nodes")]
async fn get_nodes(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let members = data.server_member_manager.all_members();

    model::common::Result::<Vec<Member>>::http_success(members)
}

pub fn routes() -> Scope {
    web::scope("/core/cluster").service(get_nodes)
}
