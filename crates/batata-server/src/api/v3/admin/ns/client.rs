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

const DEFAULT_NAMESPACE_ID: &str = "public";
const DEFAULT_GROUP: &str = "DEFAULT_GROUP";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct ServiceClientQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    #[serde(default)]
    ip: Option<String>,
    #[serde(default)]
    port: Option<i32>,
}

impl ServiceClientQuery {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, DEFAULT_GROUP);
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceClientInfo {
    client_id: String,
    client_ip: String,
    client_port: u16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceClientListResponse {
    count: i32,
    clients: Vec<ServiceClientInfo>,
}

/// GET /v3/admin/ns/client/service/publisher/list
///
/// Returns clients that published (registered instances to) a specific service.
#[get("service/publisher/list")]
async fn get_service_publishers(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<ConnectionManager>>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ServiceClientQuery>,
) -> impl Responder {
    if params.service_name.is_empty() {
        return Result::<Option<ServiceClientListResponse>>::http_response(
            400,
            400,
            "Required parameter 'serviceName' is missing".to_string(),
            None::<ServiceClientListResponse>,
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let publisher_ids =
        naming_service.get_publishers(namespace_id, group_name, &params.service_name);

    let clients: Vec<ServiceClientInfo> = publisher_ids
        .iter()
        .filter_map(|client_id| {
            connection_manager.get_client(client_id).map(|client| {
                let meta = &client.connection.meta_info;
                ServiceClientInfo {
                    client_id: client_id.clone(),
                    client_ip: meta.client_ip.clone(),
                    client_port: meta.remote_port,
                }
            })
        })
        .collect();

    let response = ServiceClientListResponse {
        count: clients.len() as i32,
        clients,
    };

    Result::<ServiceClientListResponse>::http_success(response)
}

/// GET /v3/admin/ns/client/service/subscriber/list
///
/// Returns clients that subscribed to a specific service.
#[get("service/subscriber/list")]
async fn get_service_subscribers(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<ConnectionManager>>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ServiceClientQuery>,
) -> impl Responder {
    if params.service_name.is_empty() {
        return Result::<Option<ServiceClientListResponse>>::http_response(
            400,
            400,
            "Required parameter 'serviceName' is missing".to_string(),
            None::<ServiceClientListResponse>,
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    let subscriber_ids =
        naming_service.get_subscribers(namespace_id, group_name, &params.service_name);

    let clients: Vec<ServiceClientInfo> = subscriber_ids
        .iter()
        .filter_map(|client_id| {
            connection_manager.get_client(client_id).map(|client| {
                let meta = &client.connection.meta_info;
                ServiceClientInfo {
                    client_id: client_id.clone(),
                    client_ip: meta.client_ip.clone(),
                    client_port: meta.remote_port,
                }
            })
        })
        .collect();

    let response = ServiceClientListResponse {
        count: clients.len() as i32,
        clients,
    };

    Result::<ServiceClientListResponse>::http_success(response)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct DistroQuery {
    ip: String,
    port: String,
}

/// GET /v3/admin/ns/client/distro
///
/// Returns the responsible server address for a given client IP:port.
#[get("distro")]
async fn get_distro(
    req: HttpRequest,
    data: web::Data<AppState>,
    _params: web::Query<DistroQuery>,
) -> impl Responder {
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::AdminApi)
            .build()
    );

    // In standalone/embedded mode, the responsible server is always self
    let self_addr = data
        .server_member_manager
        .as_ref()
        .map(|smm| smm.local_address().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let response = serde_json::json!({
        "responsibleServer": self_addr,
    });

    Result::<serde_json::Value>::http_success(response)
}

pub fn routes() -> actix_web::Scope {
    web::scope("/client")
        .service(list_clients)
        .service(get_client)
        .service(get_published)
        .service(get_subscribed)
        .service(get_service_publishers)
        .service(get_service_subscribers)
        .service(get_distro)
}
