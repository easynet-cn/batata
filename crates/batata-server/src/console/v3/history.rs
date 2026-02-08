use actix_web::{HttpMessage, HttpRequest, Responder, Scope, get, post, web};
use serde::{Deserialize, Serialize};

use crate::{
    ActionTypes, ApiType, Secured, SignType,
    api::{
        config::model::{ConfigBasicInfo, ConfigHistoryBasicInfo, ConfigHistoryDetailInfo},
        model::Page,
    },
    model::{
        self,
        common::{AppState, DEFAULT_NAMESPACE_ID},
    },
    secured,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct FindOneParam {
    data_id: String,
    group_name: String,
    namespace_id: String,
    nid: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SearchParam {
    data_id: String,
    group_name: String,
    tenant: Option<String>,
    namespace_id: Option<String>,
    page_no: u64,
    page_size: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct FindConfigsbyNamespaceIdParam {
    namespace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DiffParam {
    nid1: u64,
    nid2: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RollbackParam {
    data_id: String,
    group_name: String,
    namespace_id: Option<String>,
    nid: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AdvancedSearchParam {
    data_id: Option<String>,
    group_name: Option<String>,
    namespace_id: Option<String>,
    op_type: Option<String>,
    src_user: Option<String>,
    start_time: Option<i64>,
    end_time: Option<i64>,
    page_no: Option<u64>,
    page_size: Option<u64>,
}

/// Response for diff operation
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffResponse {
    version1: ConfigHistoryDetailInfo,
    version2: ConfigHistoryDetailInfo,
    content_diff: Vec<DiffLine>,
}

/// A single diff line
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiffLine {
    line_number: usize,
    operation: String, // "add", "remove", "unchanged"
    content: String,
}

#[get("")]
async fn find_one(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<FindOneParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data.console_datasource.history_find_by_id(params.nid).await {
        Ok(config_info) => {
            model::common::Result::<Option<ConfigHistoryDetailInfo>>::http_success(config_info)
        }
        Err(e) => {
            tracing::error!("Failed to find history by id: {}", e);
            model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to find history: {}", e),
                String::new(),
            )
        }
    }
}

#[get("list")]
async fn search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SearchParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let data_id = &params.data_id;
    let group_name = &params.group_name;
    let mut namespace_id = params.tenant.clone().unwrap_or_default();

    if namespace_id.is_empty() {
        namespace_id = params
            .namespace_id
            .clone()
            .unwrap_or(DEFAULT_NAMESPACE_ID.to_string());
    }

    match data
        .console_datasource
        .history_search_page(
            data_id,
            group_name,
            &namespace_id,
            params.page_no,
            params.page_size,
        )
        .await
    {
        Ok(page_result) => {
            model::common::Result::<Page<ConfigHistoryBasicInfo>>::http_success(page_result)
        }
        Err(e) => {
            tracing::error!("Failed to search history: {}", e);
            model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to search history: {}", e),
                String::new(),
            )
        }
    }
}

#[get("configs")]
async fn find_configs_by_namespace_id(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<FindConfigsbyNamespaceIdParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data
        .console_datasource
        .history_find_configs_by_namespace_id(&params.namespace_id)
        .await
    {
        Ok(config_infos) => {
            model::common::Result::<Vec<ConfigBasicInfo>>::http_success(config_infos)
        }
        Err(e) => {
            tracing::error!("Failed to find configs by namespace: {}", e);
            model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to find configs: {}", e),
                String::new(),
            )
        }
    }
}

/// Compare two history versions
///
/// GET /v3/console/cs/history/diff
#[get("diff")]
async fn diff(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<DiffParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let v1 = match data
        .console_datasource
        .history_find_by_id(params.nid1)
        .await
    {
        Ok(Some(v)) => v,
        Ok(None) => {
            return model::common::Result::<String>::http_response(
                404,
                404,
                format!("Version {} not found", params.nid1),
                String::new(),
            );
        }
        Err(e) => {
            tracing::error!("Failed to find version {}: {}", params.nid1, e);
            return model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to find version: {}", e),
                String::new(),
            );
        }
    };

    let v2 = match data
        .console_datasource
        .history_find_by_id(params.nid2)
        .await
    {
        Ok(Some(v)) => v,
        Ok(None) => {
            return model::common::Result::<String>::http_response(
                404,
                404,
                format!("Version {} not found", params.nid2),
                String::new(),
            );
        }
        Err(e) => {
            tracing::error!("Failed to find version {}: {}", params.nid2, e);
            return model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to find version: {}", e),
                String::new(),
            );
        }
    };

    // Generate simple line-by-line diff
    let lines1: Vec<&str> = v1.content.lines().collect();
    let lines2: Vec<&str> = v2.content.lines().collect();

    let mut diff_lines = Vec::new();
    let max_lines = lines1.len().max(lines2.len());

    for i in 0..max_lines {
        let line1 = lines1.get(i).copied().unwrap_or("");
        let line2 = lines2.get(i).copied().unwrap_or("");

        if i >= lines1.len() {
            diff_lines.push(DiffLine {
                line_number: i + 1,
                operation: "add".to_string(),
                content: line2.to_string(),
            });
        } else if i >= lines2.len() {
            diff_lines.push(DiffLine {
                line_number: i + 1,
                operation: "remove".to_string(),
                content: line1.to_string(),
            });
        } else if line1 != line2 {
            diff_lines.push(DiffLine {
                line_number: i + 1,
                operation: "remove".to_string(),
                content: line1.to_string(),
            });
            diff_lines.push(DiffLine {
                line_number: i + 1,
                operation: "add".to_string(),
                content: line2.to_string(),
            });
        } else {
            diff_lines.push(DiffLine {
                line_number: i + 1,
                operation: "unchanged".to_string(),
                content: line1.to_string(),
            });
        }
    }

    let response = DiffResponse {
        version1: v1,
        version2: v2,
        content_diff: diff_lines,
    };
    model::common::Result::<DiffResponse>::http_success(response)
}

/// Rollback config to a previous version
///
/// POST /v3/console/cs/history/rollback
#[post("rollback")]
async fn rollback(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<RollbackParam>,
) -> impl Responder {
    let namespace_id = params
        .namespace_id
        .clone()
        .unwrap_or_else(|| DEFAULT_NAMESPACE_ID.to_string());

    // Check write permission
    let resource = format!(
        "{}:{}:config/{}",
        namespace_id, params.group_name, params.data_id
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    // Get the history version to rollback to
    match data.console_datasource.history_find_by_id(params.nid).await {
        Ok(Some(history)) => {
            // Verify it matches the requested config
            let basic = &history.config_history_basic_info.config_basic_info;
            if basic.data_id != params.data_id || basic.group_name != params.group_name {
                return model::common::Result::<String>::http_response(
                    400,
                    400,
                    "History version does not match requested config".to_string(),
                    String::new(),
                );
            }

            // Get client IP for audit
            let src_ip = req
                .connection_info()
                .realip_remote_addr()
                .map(|s| s.to_string())
                .unwrap_or_else(|| "unknown".to_string());

            // Publish the old content as a new version
            match data
                .console_datasource
                .config_create_or_update(
                    &params.data_id,
                    &params.group_name,
                    &namespace_id,
                    &history.content,
                    &basic.app_name, // app_name
                    "rollback",      // src_user
                    &src_ip,
                    "", // config_tags
                    "", // desc
                    "", // use
                    "", // effect
                    "", // type
                    "", // schema
                    &history.encrypted_data_key,
                )
                .await
            {
                Ok(_) => {
                    model::common::Result::<String>::http_success("Rollback successful".to_string())
                }
                Err(e) => {
                    tracing::error!("Failed to rollback config: {}", e);
                    model::common::Result::<String>::http_response(
                        500,
                        500,
                        format!("Failed to rollback config: {}", e),
                        String::new(),
                    )
                }
            }
        }
        Ok(None) => model::common::Result::<String>::http_response(
            404,
            404,
            "History version not found".to_string(),
            String::new(),
        ),
        Err(e) => {
            tracing::error!("Failed to find history version: {}", e);
            model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to find history version: {}", e),
                String::new(),
            )
        }
    }
}

/// Advanced search with filters
///
/// GET /v3/console/cs/history/search
#[get("search")]
async fn advanced_search(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<AdvancedSearchParam>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let namespace_id = params
        .namespace_id
        .clone()
        .unwrap_or_else(|| DEFAULT_NAMESPACE_ID.to_string());
    let data_id = params.data_id.clone().unwrap_or_default();
    let group_name = params.group_name.clone().unwrap_or_default();
    let page_no = params.page_no.unwrap_or(1);
    let page_size = params.page_size.unwrap_or(20);

    match data
        .console_datasource
        .history_search_with_filters(
            &data_id,
            &group_name,
            &namespace_id,
            params.op_type.as_deref(),
            params.src_user.as_deref(),
            params.start_time,
            params.end_time,
            page_no,
            page_size,
        )
        .await
    {
        Ok(page_result) => {
            model::common::Result::<Page<ConfigHistoryBasicInfo>>::http_success(page_result)
        }
        Err(e) => {
            tracing::error!("Failed to search history: {}", e);
            model::common::Result::<String>::http_response(
                500,
                500,
                format!("Failed to search history: {}", e),
                String::new(),
            )
        }
    }
}

pub fn routes() -> Scope {
    web::scope("/cs/history")
        .service(find_one)
        .service(search)
        .service(find_configs_by_namespace_id)
        .service(diff)
        .service(rollback)
        .service(advanced_search)
}
