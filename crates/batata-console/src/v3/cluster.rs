//! Cluster management console endpoints

use std::sync::Arc;

use actix_web::{Responder, Scope, get, post, web};
use serde::{Deserialize, Serialize};

use crate::datasource::ConsoleDataSource;
use crate::model::{ClusterHealthResponse, Member, SelfMemberResponse};

use super::namespace::ApiResult;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetNodesParam {
    pub keyword: Option<String>,
    pub with_health: Option<bool>,
}

#[get("nodes")]
pub async fn get_nodes(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    params: web::Query<GetNodesParam>,
) -> impl Responder {
    let mut members = datasource.cluster_members();

    if let Some(keyword) = &params.keyword
        && !keyword.is_empty()
    {
        members.retain(|e| e.address.contains(keyword));
    }

    ApiResult::<Vec<Member>>::http_success(members)
}

#[get("nodes/healthy")]
pub async fn get_healthy_nodes(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
) -> impl Responder {
    let members = datasource.cluster_healthy_members();
    ApiResult::<Vec<Member>>::http_success(members)
}

#[get("health")]
pub async fn get_health(datasource: web::Data<Arc<dyn ConsoleDataSource>>) -> impl Responder {
    let response = datasource.cluster_health();
    ApiResult::<ClusterHealthResponse>::http_success(response)
}

#[get("self")]
pub async fn get_self(datasource: web::Data<Arc<dyn ConsoleDataSource>>) -> impl Responder {
    let response = datasource.cluster_self();
    ApiResult::<SelfMemberResponse>::http_success(response)
}

#[get("node/{address}")]
pub async fn get_node(
    datasource: web::Data<Arc<dyn ConsoleDataSource>>,
    path: web::Path<String>,
) -> impl Responder {
    let address = path.into_inner();

    match datasource.cluster_member(&address) {
        Some(member) => ApiResult::<Member>::http_success(member),
        None => ApiResult::<String>::http_response(
            404,
            404,
            format!("Member not found: {}", address),
            String::new(),
        ),
    }
}

#[get("count")]
pub async fn get_member_count(datasource: web::Data<Arc<dyn ConsoleDataSource>>) -> impl Responder {
    let count = datasource.cluster_member_count();
    ApiResult::<usize>::http_success(count)
}

#[get("standalone")]
pub async fn check_standalone(datasource: web::Data<Arc<dyn ConsoleDataSource>>) -> impl Responder {
    let standalone_mode = datasource.cluster_is_standalone();
    ApiResult::<bool>::http_success(standalone_mode)
}

#[post("self/refresh")]
pub async fn refresh_self(datasource: web::Data<Arc<dyn ConsoleDataSource>>) -> impl Responder {
    datasource.cluster_refresh_self();
    ApiResult::<bool>::http_success(true)
}

pub fn routes() -> Scope {
    web::scope("/core/cluster")
        .service(get_nodes)
        .service(get_healthy_nodes)
        .service(get_health)
        .service(get_self)
        .service(get_node)
        .service(get_member_count)
        .service(check_standalone)
        .service(refresh_self)
}
