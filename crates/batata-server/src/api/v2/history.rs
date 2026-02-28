//! V2 Config History API handlers
//!
//! Implements the Nacos V2 configuration history API endpoints:
//! - GET /nacos/v2/cs/history/list - Get history list with pagination
//! - GET /nacos/v2/cs/history - Get specific history entry
//! - GET /nacos/v2/cs/history/detail - Get history detail with original/updated content comparison
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
    ConfigHistoryInfoDetail, ConfigInfoResponse, HistoryDetailParam, HistoryItemResponse,
    HistoryListParam, HistoryPreviousParam, NamespaceConfigsParam,
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

/// Get config history detail with original/updated content for comparison
///
/// GET /nacos/v2/cs/history/detail
///
/// Returns a history entry with both original and updated content for version comparison.
/// The behavior depends on the operation type:
/// - INSERT: originalContent is empty, updatedContent is the new content
/// - UPDATE: originalContent is this version, updatedContent is the current config
/// - DELETE: originalContent is the deleted content, updatedContent is empty
#[get("detail")]
pub async fn get_history_detail(
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

    let persistence = data.persistence();

    // Get history entry by nid
    let history_item = match persistence.config_history_find_by_id(params.nid).await {
        Ok(Some(item)) => item,
        Ok(None) => {
            return Result::<Option<ConfigHistoryInfoDetail>>::http_response(
                404,
                404,
                "history entry not found".to_string(),
                None::<ConfigHistoryInfoDetail>,
            );
        }
        Err(e) => {
            warn!(error = %e, "Failed to get history entry");
            return Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            );
        }
    };

    // Verify the history entry matches the requested config
    if history_item.data_id != params.data_id
        || history_item.group != params.group
        || history_item.tenant != namespace_id
    {
        return Result::<Option<ConfigHistoryInfoDetail>>::http_response(
            404,
            404,
            "Please check dataId, group or namespaceId.".to_string(),
            None::<ConfigHistoryInfoDetail>,
        );
    }

    let op_type = history_item.op_type.trim().to_string();

    // Build base detail from history entry
    let mut detail = ConfigHistoryInfoDetail {
        id: history_item.id.to_string(),
        last_id: Some(-1),
        data_id: history_item.data_id.clone(),
        group: history_item.group.clone(),
        tenant: history_item.tenant.clone(),
        op_type: Some(op_type.clone()),
        publish_type: if history_item.publish_type.is_empty() {
            None
        } else {
            Some(history_item.publish_type.clone())
        },
        gray_name: if history_item.gray_name.is_empty() {
            None
        } else {
            Some(history_item.gray_name.clone())
        },
        app_name: if history_item.app_name.is_empty() {
            None
        } else {
            Some(history_item.app_name.clone())
        },
        src_ip: if history_item.src_ip.is_empty() {
            None
        } else {
            Some(history_item.src_ip.clone())
        },
        src_user: if history_item.src_user.is_empty() {
            None
        } else {
            Some(history_item.src_user.clone())
        },
        original_md5: None,
        original_content: None,
        original_encrypted_data_key: None,
        original_ext_info: None,
        updated_md5: None,
        updated_content: None,
        updated_encrypted_data_key: None,
        update_ext_info: None,
        created_time: Some(history_item.created_time.to_string()),
        last_modified_time: Some(history_item.modified_time.to_string()),
    };

    // Populate original/updated content based on operation type
    // Matches Nacos HistoryService.getConfigHistoryInfoDetail logic
    match op_type.as_str() {
        "I" => {
            // INSERT: no original, updated is the new content
            detail.updated_content = Some(history_item.content.clone());
            detail.updated_md5 = Some(history_item.md5.clone());
            detail.updated_encrypted_data_key = if history_item.encrypted_data_key.is_empty() {
                None
            } else {
                Some(history_item.encrypted_data_key.clone())
            };
            detail.update_ext_info = if history_item.ext_info.is_empty() {
                None
            } else {
                Some(history_item.ext_info.clone())
            };
            detail.original_content = Some(String::new());
            detail.original_md5 = Some(String::new());
            detail.original_encrypted_data_key = Some(String::new());
            detail.original_ext_info = Some(String::new());
        }
        "U" => {
            // UPDATE: original is this record's content, updated is the current config
            detail.original_content = Some(history_item.content.clone());
            detail.original_md5 = Some(history_item.md5.clone());
            detail.original_encrypted_data_key = if history_item.encrypted_data_key.is_empty() {
                None
            } else {
                Some(history_item.encrypted_data_key.clone())
            };
            detail.original_ext_info = if history_item.ext_info.is_empty() {
                None
            } else {
                Some(history_item.ext_info.clone())
            };

            // Fall back to current config as the "updated" version
            match persistence
                .config_find_one(&params.data_id, &params.group, namespace_id)
                .await
            {
                Ok(Some(current)) => {
                    detail.updated_content = Some(current.content);
                    detail.updated_md5 = Some(current.md5);
                    detail.updated_encrypted_data_key = if current.encrypted_data_key.is_empty() {
                        None
                    } else {
                        Some(current.encrypted_data_key)
                    };
                }
                _ => {
                    // Config may have been deleted since; leave updated as None
                }
            }
        }
        "D" => {
            // DELETE: original is the deleted content, no updated
            detail.original_content = Some(history_item.content.clone());
            detail.original_md5 = Some(history_item.md5.clone());
            detail.original_encrypted_data_key = if history_item.encrypted_data_key.is_empty() {
                None
            } else {
                Some(history_item.encrypted_data_key.clone())
            };
            detail.original_ext_info = if history_item.ext_info.is_empty() {
                None
            } else {
                Some(history_item.ext_info.clone())
            };
        }
        _ => {
            // Unknown op type: just include raw content
            detail.original_content = Some(history_item.content.clone());
            detail.original_md5 = Some(history_item.md5.clone());
        }
    }

    Result::<ConfigHistoryInfoDetail>::http_success(detail)
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
