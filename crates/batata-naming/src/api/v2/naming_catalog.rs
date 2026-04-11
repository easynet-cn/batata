//! V2 Naming Catalog API handler
//!
//! Implements the Nacos V2 catalog API endpoint:
//! - GET /nacos/v2/ns/catalog/instances - List catalog instances

use std::collections::HashMap;
use std::sync::Arc;

use actix_web::{HttpRequest, Responder, get, web};
use serde::Serialize;

use batata_common::{
    ActionTypes, ApiType, DEFAULT_NAMESPACE_ID, SignType, default_page_no, default_page_size_small,
    impl_or_default,
};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, error, secured};

use batata_api::naming::NamingServiceProvider;

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CatalogInstancesQuery {
    #[serde(default, alias = "namespaceId")]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupName")]
    group_name: Option<String>,
    #[serde(alias = "serviceName")]
    service_name: String,
    #[serde(default, alias = "clusterName")]
    cluster_name: Option<String>,
    #[serde(default = "default_page_no", alias = "pageNo")]
    page_no: u64,
    #[serde(default = "default_page_size_small", alias = "pageSize")]
    page_size: u64,
}

impl CatalogInstancesQuery {
    impl_or_default!(namespace_id_or_default, namespace_id, DEFAULT_NAMESPACE_ID);

    impl_or_default!(group_name_or_default, group_name, "DEFAULT_GROUP");
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogInstanceResponse {
    ip: String,
    port: i32,
    weight: f64,
    healthy: bool,
    enabled: bool,
    ephemeral: bool,
    cluster_name: String,
    service_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    metadata: Option<HashMap<String, String>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CatalogInstanceListResponse {
    count: usize,
    list: Vec<CatalogInstanceResponse>,
}

/// List catalog instances
///
/// GET /nacos/v2/ns/catalog/instances
#[get("")]
pub async fn list_catalog_instances(
    req: HttpRequest,
    data: web::Data<AppState>,
    naming_service: web::Data<Arc<dyn NamingServiceProvider>>,
    params: web::Query<CatalogInstancesQuery>,
) -> impl Responder {
    if params.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "Required parameter 'serviceName' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();
    let clusters = params.cluster_name.as_deref().unwrap_or("");

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

    // Zero-copy snapshot — only paginated slice gets deep-cloned for response.
    let instances = naming_service.get_instances_snapshot(
        namespace_id,
        group_name,
        &params.service_name,
        clusters,
        false,
    );

    // Paginate
    let total = instances.len();
    let start = ((params.page_no - 1) * params.page_size) as usize;
    let end = std::cmp::min(start + params.page_size as usize, total);

    let page_items: Vec<CatalogInstanceResponse> = if start < total {
        instances[start..end]
            .iter()
            .map(|i| CatalogInstanceResponse {
                ip: i.ip.clone(),
                port: i.port,
                weight: i.weight,
                healthy: i.healthy,
                enabled: i.enabled,
                ephemeral: i.ephemeral,
                cluster_name: i.cluster_name.clone(),
                service_name: i.service_name.clone(),
                metadata: if i.metadata.is_empty() {
                    None
                } else {
                    Some(i.metadata.clone())
                },
            })
            .collect()
    } else {
        vec![]
    };

    let response = CatalogInstanceListResponse {
        count: total,
        list: page_items,
    };

    Result::<CatalogInstanceListResponse>::http_success(response)
}
