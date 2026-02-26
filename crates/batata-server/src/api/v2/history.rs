//! V2 Config History API handlers
//!
//! Implements the Nacos V2 configuration history API endpoints:
//! - GET /nacos/v2/cs/history/list - Get history list with pagination
//! - GET /nacos/v2/cs/history - Get specific history entry
//! - GET /nacos/v2/cs/history/previous - Get previous history entry
//! - GET /nacos/v2/cs/history/configs - Get all configs in a namespace

use actix_web::{HttpMessage, HttpRequest, Responder, get, web};
use tracing::warn;

use batata_api::Page;
use batata_persistence::ConfigHistoryStorageData;

use crate::{
    ActionTypes, ApiType, Secured, SignType, error, model::common::AppState,
    model::response::Result, secured,
};

use super::model::{
    ConfigInfoResponse, HistoryDetailParam, HistoryItemResponse, HistoryListParam,
    HistoryPreviousParam, NamespaceConfigsParam,
};

/// Helper to convert ConfigHistoryStorageData to HistoryItemResponse (list view, no content)
fn history_storage_to_list_response(item: ConfigHistoryStorageData) -> HistoryItemResponse {
    HistoryItemResponse {
        id: item.id.to_string(),
        last_id: Some(-1),
        data_id: item.data_id,
        group: item.group,
        tenant: item.tenant,
        app_name: if item.app_name.is_empty() {
            None
        } else {
            Some(item.app_name)
        },
        md5: Some(item.md5),
        content: None, // Not included in list view
        src_ip: if item.src_ip.is_empty() {
            None
        } else {
            Some(item.src_ip)
        },
        src_user: if item.src_user.is_empty() {
            None
        } else {
            Some(item.src_user)
        },
        op_type: if item.op_type.is_empty() {
            None
        } else {
            Some(item.op_type)
        },
        publish_type: if item.publish_type.is_empty() {
            None
        } else {
            Some(item.publish_type)
        },
        ext_info: if item.ext_info.is_empty() {
            None
        } else {
            Some(item.ext_info)
        },
        r#type: None,
        created_time: Some(item.created_time.to_string()),
        last_modified_time: Some(item.modified_time.to_string()),
        encrypted_data_key: if item.encrypted_data_key.is_empty() {
            None
        } else {
            Some(item.encrypted_data_key)
        },
    }
}

/// Helper to convert ConfigHistoryStorageData to HistoryItemResponse (detail view, with content)
fn history_storage_to_detail_response(item: ConfigHistoryStorageData) -> HistoryItemResponse {
    HistoryItemResponse {
        id: item.id.to_string(),
        last_id: Some(-1),
        data_id: item.data_id,
        group: item.group,
        tenant: item.tenant,
        app_name: if item.app_name.is_empty() {
            None
        } else {
            Some(item.app_name)
        },
        md5: Some(item.md5),
        content: Some(item.content),
        src_ip: if item.src_ip.is_empty() {
            None
        } else {
            Some(item.src_ip)
        },
        src_user: if item.src_user.is_empty() {
            None
        } else {
            Some(item.src_user)
        },
        op_type: if item.op_type.is_empty() {
            None
        } else {
            Some(item.op_type)
        },
        publish_type: if item.publish_type.is_empty() {
            None
        } else {
            Some(item.publish_type)
        },
        ext_info: if item.ext_info.is_empty() {
            None
        } else {
            Some(item.ext_info)
        },
        r#type: None,
        created_time: Some(item.created_time.to_string()),
        last_modified_time: Some(item.modified_time.to_string()),
        encrypted_data_key: if item.encrypted_data_key.is_empty() {
            None
        } else {
            Some(item.encrypted_data_key)
        },
    }
}

/// Get config history list
///
/// GET /nacos/v2/cs/history/list
///
/// Retrieves a paginated list of configuration history entries.
#[get("list")]
pub async fn get_history_list(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<HistoryListParam>,
) -> impl Responder {
    // Validate required parameters
    if params.data_id.is_empty() {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameter 'dataId' is missing".to_string(),
            String::new(),
        );
    }

    if params.group.is_empty() {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameter 'group' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = params.namespace_id_or_default();

    // Check authorization
    let resource = format!(
        "{}:{}:config/{}",
        namespace_id, params.group, params.data_id
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Get history from persistence service
    let persistence = data.persistence();
    match persistence
        .config_history_search_page(
            &params.data_id,
            &params.group,
            namespace_id,
            params.page_no,
            params.page_size,
        )
        .await
    {
        Ok(page) => {
            // Transform to response format
            let response_page = Page {
                total_count: page.total_count,
                page_number: page.page_number,
                pages_available: page.pages_available,
                page_items: page
                    .page_items
                    .into_iter()
                    .map(history_storage_to_list_response)
                    .collect(),
            };

            Result::<Page<HistoryItemResponse>>::http_success(response_page)
        }
        Err(e) => {
            warn!(error = %e, "Failed to get history list");
            Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            )
        }
    }
}

/// Get specific history entry
///
/// GET /nacos/v2/cs/history
///
/// Retrieves a specific configuration history entry by nid.
#[get("")]
pub async fn get_history(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<HistoryDetailParam>,
) -> impl Responder {
    // Validate required parameters
    if params.data_id.is_empty() {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameter 'dataId' is missing".to_string(),
            String::new(),
        );
    }

    if params.group.is_empty() {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameter 'group' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = params.namespace_id_or_default();

    // Check authorization
    let resource = format!(
        "{}:{}:config/{}",
        namespace_id, params.group, params.data_id
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Get history entry by ID from persistence service
    let persistence = data.persistence();
    match persistence.config_history_find_by_id(params.nid).await {
        Ok(Some(item)) => {
            // Verify the history entry matches the requested config
            if item.data_id != params.data_id
                || item.group != params.group
                || item.tenant != namespace_id
            {
                return Result::<Option<HistoryItemResponse>>::http_response(
                    404,
                    404,
                    "history entry not found".to_string(),
                    None::<HistoryItemResponse>,
                );
            }

            let response = history_storage_to_detail_response(item);
            Result::<HistoryItemResponse>::http_success(response)
        }
        Ok(None) => Result::<Option<HistoryItemResponse>>::http_response(
            404,
            404,
            "history entry not found".to_string(),
            None::<HistoryItemResponse>,
        ),
        Err(e) => {
            warn!(error = %e, "Failed to get history entry");
            Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            )
        }
    }
}

/// Get previous history entry
///
/// GET /nacos/v2/cs/history/previous
///
/// Retrieves the history entry before the specified one.
#[get("previous")]
pub async fn get_previous_history(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<HistoryPreviousParam>,
) -> impl Responder {
    // Validate required parameters
    if params.data_id.is_empty() {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameter 'dataId' is missing".to_string(),
            String::new(),
        );
    }

    if params.group.is_empty() {
        return Result::<String>::http_response(
            400,
            400,
            "Required parameter 'group' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = params.namespace_id_or_default();

    // Check authorization
    let resource = format!(
        "{}:{}:config/{}",
        namespace_id, params.group, params.data_id
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Get the previous history entry from persistence service
    let persistence = data.persistence();
    match persistence
        .config_history_get_previous(&params.data_id, &params.group, namespace_id, params.id)
        .await
    {
        Ok(Some(item)) => {
            let response = history_storage_to_detail_response(item);
            Result::<HistoryItemResponse>::http_success(response)
        }
        Ok(None) => Result::<Option<HistoryItemResponse>>::http_response(
            404,
            404,
            "previous history entry not found".to_string(),
            None::<HistoryItemResponse>,
        ),
        Err(e) => {
            warn!(error = %e, "Failed to get previous history entry");
            Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            )
        }
    }
}

/// Get all configs in a namespace
///
/// GET /nacos/v2/cs/history/configs
///
/// Retrieves all configurations in a namespace (lightweight list without content).
#[get("configs")]
pub async fn get_namespace_configs(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<NamespaceConfigsParam>,
) -> impl Responder {
    let namespace_id = params.namespace_id_or_default();

    // Check authorization for namespace-level access
    let resource = format!("{}:*:config/*", namespace_id);
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::OpenApi)
            .build()
    );

    // Get configs from persistence service
    let persistence = data.persistence();
    match persistence.config_find_by_namespace(namespace_id).await {
        Ok(configs) => {
            let response: Vec<ConfigInfoResponse> = configs
                .into_iter()
                .map(|config| ConfigInfoResponse {
                    id: "0".to_string(),
                    data_id: config.data_id,
                    group: config.group,
                    tenant: config.tenant,
                    app_name: if config.app_name.is_empty() {
                        None
                    } else {
                        Some(config.app_name)
                    },
                    r#type: if config.config_type.is_empty() {
                        None
                    } else {
                        Some(config.config_type)
                    },
                })
                .collect();

            Result::<Vec<ConfigInfoResponse>>::http_success(response)
        }
        Err(e) => {
            warn!(error = %e, "Failed to get namespace configs");
            Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            )
        }
    }
}
