use crate::common::model::RestResult;
use crate::core::model::Namespace;

use actix_web::{get, web};

#[get("")]
pub async fn get_namespaces() -> web::Json<RestResult<Vec<Namespace>>> {
    let namespaces: Vec<Namespace> = Vec::new();
    let rest_result = RestResult::<Vec<Namespace>>::success(namespaces);

    web::Json(rest_result)
}
