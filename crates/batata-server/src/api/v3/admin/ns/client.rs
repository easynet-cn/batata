//! V3 Admin client info endpoints

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, get, web};
use serde::{Deserialize, Serialize};

use batata_core::service::remote::ConnectionManager;

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured, service::naming::NamingService,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClientDetailParam {
    client_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientListResponse {
    count: i32,
    client_ids: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientDetailResponse {
    client_id: String,
    client_type: String,
    client_ip: String,
    client_port: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    connect_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    app_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    create_time: i64,
    last_active_time: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientServiceInfo {
    namespace: String,
    group_name: String,
    service_name: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientServiceListResponse {
    count: i32,
    services: Vec<ClientServiceInfo>,
}

/// GET /v3/admin/ns/client/list
#[get("list")]
async fn list_clients(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<ConnectionManager>>,
) -> impl Responder {
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let client_ids = connection_manager.get_all_connection_ids();

    let response = ClientListResponse {
        count: client_ids.len() as i32,
        client_ids,
    };

    Result::<ClientListResponse>::http_success(response)
}

/// GET /v3/admin/ns/client
#[get("")]
async fn get_client(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<ConnectionManager>>,
    params: web::Query<ClientDetailParam>,
) -> impl Responder {
    if params.client_id.is_empty() {
        return Result::<Option<ClientDetailResponse>>::http_response(
            400,
            400,
            "Required parameter 'clientId' is missing".to_string(),
            None::<ClientDetailResponse>,
        );
    }

    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    match connection_manager.get_client(&params.client_id) {
        Some(grpc_client) => {
            let meta = &grpc_client.connection.meta_info;
            let response = ClientDetailResponse {
                client_id: meta.connection_id.clone(),
                client_type: if meta.is_sdk_source() {
                    "sdk"
                } else {
                    "cluster"
                }
                .to_string(),
                client_ip: meta.client_ip.clone(),
                client_port: meta.remote_port as i32,
                connect_type: if meta.connect_type.is_empty() {
                    None
                } else {
                    Some(meta.connect_type.clone())
                },
                app_name: if meta.app_name.is_empty() {
                    None
                } else {
                    Some(meta.app_name.clone())
                },
                version: if meta.version.is_empty() {
                    None
                } else {
                    Some(meta.version.clone())
                },
                create_time: meta.create_time,
                last_active_time: meta.last_active_time,
            };
            Result::<ClientDetailResponse>::http_success(response)
        }
        None => Result::<Option<ClientDetailResponse>>::http_response(
            404,
            404,
            format!("client {} not found", params.client_id),
            None::<ClientDetailResponse>,
        ),
    }
}

/// GET /v3/admin/ns/client/publish/list
#[get("publish/list")]
async fn get_published(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ClientDetailParam>,
) -> impl Responder {
    if params.client_id.is_empty() {
        return Result::<Option<ClientServiceListResponse>>::http_response(
            400,
            400,
            "Required parameter 'clientId' is missing".to_string(),
            None::<ClientServiceListResponse>,
        );
    }

    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let service_keys = naming_service.get_published_services(&params.client_id);
    let services: Vec<ClientServiceInfo> = service_keys
        .iter()
        .filter_map(|key| parse_service_key(key))
        .collect();

    let response = ClientServiceListResponse {
        count: services.len() as i32,
        services,
    };

    Result::<ClientServiceListResponse>::http_success(response)
}

/// GET /v3/admin/ns/client/subscribe/list
#[get("subscribe/list")]
async fn get_subscribed(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ClientDetailParam>,
) -> impl Responder {
    if params.client_id.is_empty() {
        return Result::<Option<ClientServiceListResponse>>::http_response(
            400,
            400,
            "Required parameter 'clientId' is missing".to_string(),
            None::<ClientServiceListResponse>,
        );
    }

    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let service_keys = naming_service.get_subscribed_services(&params.client_id);
    let services: Vec<ClientServiceInfo> = service_keys
        .iter()
        .filter_map(|key| parse_service_key(key))
        .collect();

    let response = ClientServiceListResponse {
        count: services.len() as i32,
        services,
    };

    Result::<ClientServiceListResponse>::http_success(response)
}

fn parse_service_key(key: &str) -> Option<ClientServiceInfo> {
    let parts: Vec<&str> = key.split("@@").collect();
    if parts.len() == 3 {
        Some(ClientServiceInfo {
            namespace: parts[0].to_string(),
            group_name: parts[1].to_string(),
            service_name: parts[2].to_string(),
        })
    } else {
        None
    }
}

pub fn routes() -> actix_web::Scope {
    web::scope("/client")
        .service(list_clients)
        .service(get_client)
        .service(get_published)
        .service(get_subscribed)
}
