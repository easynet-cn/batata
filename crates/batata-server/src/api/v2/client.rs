//! V2 Client API handlers
//!
//! Implements the Nacos V2 client management API endpoints:
//! - GET /nacos/v2/ns/client/list - Get client list
//! - GET /nacos/v2/ns/client - Get client detail
//! - GET /nacos/v2/ns/client/publish/list - Get published services
//! - GET /nacos/v2/ns/client/subscribe/list - Get subscribed services
//! - GET /nacos/v2/ns/client/service/publisher/list - Get service publishers
//! - GET /nacos/v2/ns/client/service/subscriber/list - Get service subscribers

use std::sync::Arc;

use actix_web::{HttpMessage, HttpRequest, Responder, get, web};

use batata_core::service::remote::ConnectionManager;

use crate::{
    ActionTypes, ApiType, Secured, SignType, model::common::AppState, model::response::Result,
    secured, service::naming::NamingService,
};

use super::model::{
    ClientDetailParam, ClientDetailResponse, ClientListParam, ClientListResponse,
    ClientServiceInfo, ClientServiceListParam, ClientServiceListResponse, ServiceClientInfo,
    ServiceClientListParam, ServiceClientListResponse,
};

/// Get client list
///
/// GET /nacos/v2/ns/client/list
///
/// Returns a list of all connected client IDs.
#[get("list")]
pub async fn get_client_list(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<ConnectionManager>>,
    _params: web::Query<ClientListParam>,
) -> impl Responder {
    // Check authorization for client operations
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let client_ids = connection_manager.get_all_connection_ids();

    let response = ClientListResponse {
        count: client_ids.len() as i32,
        client_ids,
    };

    Result::<ClientListResponse>::http_success(response)
}

/// Get client detail
///
/// GET /nacos/v2/ns/client
///
/// Returns detailed information about a specific client.
#[get("")]
pub async fn get_client_detail(
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

    // Check authorization
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let client = connection_manager.get_client(&params.client_id);

    match client {
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
                client_port: meta.remote_port,
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

/// Get client published services
///
/// GET /nacos/v2/ns/client/publish/list
///
/// Returns a list of services published by a specific client.
#[get("publish/list")]
pub async fn get_client_published_services(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ClientServiceListParam>,
) -> impl Responder {
    if params.client_id.is_empty() {
        return Result::<Option<ClientServiceListResponse>>::http_response(
            400,
            400,
            "Required parameter 'clientId' is missing".to_string(),
            None::<ClientServiceListResponse>,
        );
    }

    // Check authorization
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
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

/// Get client subscribed services
///
/// GET /nacos/v2/ns/client/subscribe/list
///
/// Returns a list of services subscribed by a specific client.
#[get("subscribe/list")]
pub async fn get_client_subscribed_services(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ClientServiceListParam>,
) -> impl Responder {
    if params.client_id.is_empty() {
        return Result::<Option<ClientServiceListResponse>>::http_response(
            400,
            400,
            "Required parameter 'clientId' is missing".to_string(),
            None::<ClientServiceListResponse>,
        );
    }

    // Check authorization
    let resource = "*:*:naming/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
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

/// Get service publisher list
///
/// GET /nacos/v2/ns/client/service/publisher/list
///
/// Returns a list of clients that published instances to a specific service.
#[get("service/publisher/list")]
pub async fn get_service_publishers(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<ConnectionManager>>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ServiceClientListParam>,
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

    // Check authorization
    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
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

/// Get service subscriber list
///
/// GET /nacos/v2/ns/client/service/subscriber/list
///
/// Returns a list of clients that subscribed to a specific service.
#[get("service/subscriber/list")]
pub async fn get_service_subscribers(
    req: HttpRequest,
    data: web::Data<AppState>,
    connection_manager: web::Data<Arc<ConnectionManager>>,
    naming_service: web::Data<Arc<NamingService>>,
    params: web::Query<ServiceClientListParam>,
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

    // Check authorization
    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::OpenApi)
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

/// Parse a service key (namespace@@group@@service) into components
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
