//! V3 Console instance management endpoints
//!
//! Provides HTTP handlers for instance operations on the main server.

use std::collections::HashMap;

use actix_web::{HttpRequest, Responder, Scope, delete, get, put, web};
use serde::{Deserialize, Serialize};

use batata_api::naming::model::Instance;
use batata_common::{DEFAULT_GROUP, DEFAULT_NAMESPACE_ID, default_page_no, default_page_size};
use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType, error, model::AppState, model::response::Result,
    secured,
};

const DEFAULT_CLUSTER: &str = "DEFAULT";

/// Maximum page size accepted from the console instance-listing API.
const MAX_PAGE_SIZE: u64 = 500;

/// Clamp caller-supplied pagination to safe bounds and compute the slice
/// offset with saturating arithmetic. A huge `page_no * page_size` must not
/// overflow (and, in debug builds, panic) or force a giant allocation.
fn clamp_pagination(page_no: u64, page_size: u64) -> (u64, u64, usize) {
    let page_no = page_no.max(1);
    let page_size = page_size.clamp(1, MAX_PAGE_SIZE);
    let offset = (page_no - 1).saturating_mul(page_size);
    let start = usize::try_from(offset).unwrap_or(usize::MAX);
    (page_no, page_size, start)
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct InstanceListQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    #[serde(default)]
    cluster_name: Option<String>,
    #[serde(default = "default_page_no")]
    page_no: u64,
    #[serde(default = "default_page_size")]
    page_size: u64,
    #[serde(default)]
    healthy_only: Option<bool>,
}

impl InstanceListQuery {
    fn namespace_id_or_default(&self) -> &str {
        self.namespace_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_NAMESPACE_ID)
    }

    fn group_name_or_default(&self) -> &str {
        self.group_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_GROUP)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceUpdateForm {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    ip: String,
    port: i32,
    #[serde(default)]
    cluster_name: Option<String>,
    #[serde(default = "default_weight")]
    weight: f64,
    #[serde(default = "default_true")]
    enabled: bool,
    #[serde(default)]
    metadata: Option<HashMap<String, String>>,
    #[serde(default)]
    ephemeral: bool,
    #[serde(default = "default_true")]
    healthy: bool,
}

fn default_weight() -> f64 {
    1.0
}

fn default_true() -> bool {
    true
}

impl InstanceUpdateForm {
    fn namespace_id_or_default(&self) -> &str {
        self.namespace_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_NAMESPACE_ID)
    }

    fn group_name_or_default(&self) -> &str {
        self.group_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_GROUP)
    }

    fn cluster_name_or_default(&self) -> &str {
        self.cluster_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_CLUSTER)
    }
}

/// Query parameters for `DELETE /ns/instance`.
///
/// Mirrors upstream `ConsoleInstanceController#removeInstance(InstanceForm)`:
/// the console deregister call carries its parameters in the query string.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceDeleteQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    ip: String,
    port: i32,
    #[serde(default)]
    cluster_name: Option<String>,
    #[serde(default)]
    ephemeral: bool,
}

impl InstanceDeleteQuery {
    fn namespace_id_or_default(&self) -> &str {
        self.namespace_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_NAMESPACE_ID)
    }

    fn group_name_or_default(&self) -> &str {
        self.group_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_GROUP)
    }

    fn cluster_name_or_default(&self) -> &str {
        self.cluster_name
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_CLUSTER)
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct InstanceListResponse {
    total_count: usize,
    page_number: u64,
    pages_available: u64,
    list: Vec<Instance>,
}

/// GET /ns/instance/list
#[get("list")]
async fn list_instances(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<InstanceListQuery>,
) -> impl Responder {
    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();
    let cluster = params.cluster_name.as_deref().unwrap_or("");

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let all_instances = match data
        .console_datasource
        .instance_list(namespace_id, group_name, &params.service_name, cluster)
        .await
    {
        Ok(i) => i,
        Err(e) => {
            return Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                e.to_string(),
                String::new(),
            );
        }
    };

    // The console datasource exposes only a full-list API (no server-side
    // pagination), so bound the page size and slice in memory. Revisit if a
    // paginated datasource method becomes available.
    let total_count = all_instances.len();
    let (page_no, page_size, start) = clamp_pagination(params.page_no, params.page_size);
    let list: Vec<Instance> = all_instances
        .into_iter()
        .skip(start)
        .take(page_size as usize)
        .collect();
    let pages_available = if total_count == 0 {
        0
    } else {
        (total_count as u64).div_ceil(page_size)
    };

    let response = InstanceListResponse {
        total_count,
        page_number: page_no,
        pages_available,
        list,
    };

    Result::<InstanceListResponse>::http_success(response)
}

/// PUT /ns/instance
#[put("")]
async fn update_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Json<InstanceUpdateForm>,
) -> impl Responder {
    let namespace_id = form.namespace_id_or_default();
    let group_name = form.group_name_or_default();
    let cluster_name = form.cluster_name_or_default();

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, form.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let weight = batata_api::naming::model::clamp_weight(form.weight);

    let instance = Instance {
        instance_id: batata_api::naming::model::default_instance_id(
            &form.ip,
            form.port,
            cluster_name,
            group_name,
            &form.service_name,
        ),
        ip: form.ip.clone(),
        port: form.port,
        weight,
        healthy: form.healthy,
        enabled: form.enabled,
        ephemeral: form.ephemeral,
        cluster_name: cluster_name.to_string(),
        service_name: form.service_name.clone(),
        metadata: form.metadata.clone().unwrap_or_default(),
    };

    if let Err(e) = data
        .console_datasource
        .instance_update(namespace_id, group_name, &form.service_name, instance)
        .await
    {
        return Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        );
    }

    Result::<String>::http_success("ok".to_string())
}

/// DELETE /ns/instance
#[delete("")]
async fn delete_instance(
    req: HttpRequest,
    data: web::Data<AppState>,
    query: web::Query<InstanceDeleteQuery>,
) -> impl Responder {
    let namespace_id = query.namespace_id_or_default();
    let group_name = query.group_name_or_default();
    let cluster_name = query.cluster_name_or_default();

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, query.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    if query.service_name.is_empty() || query.ip.is_empty() || query.port <= 0 {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_VALIDATE_ERROR.code,
            "Required parameters 'serviceName', 'ip', 'port' are missing or invalid".to_string(),
            String::new(),
        );
    }

    let instance = Instance {
        instance_id: batata_api::naming::model::default_instance_id(
            &query.ip,
            query.port,
            cluster_name,
            group_name,
            &query.service_name,
        ),
        ip: query.ip.clone(),
        port: query.port,
        weight: 1.0,
        healthy: true,
        enabled: true,
        ephemeral: query.ephemeral,
        cluster_name: cluster_name.to_string(),
        service_name: query.service_name.clone(),
        metadata: HashMap::new(),
    };

    if let Err(e) = data
        .console_datasource
        .instance_delete(namespace_id, group_name, &query.service_name, instance)
        .await
    {
        return Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        );
    }

    Result::<String>::http_success("ok".to_string())
}

/// Register the instance management routes under `/instance`.
pub fn routes() -> Scope {
    web::scope("/instance")
        .service(list_instances)
        .service(update_instance)
        .service(delete_instance)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamp_pagination_normal_values() {
        // Success path: a normal page computes the expected offset.
        let (page_no, page_size, start) = clamp_pagination(3, 10);
        assert_eq!(page_no, 3);
        assert_eq!(page_size, 10);
        assert_eq!(start, 20);
    }

    #[test]
    fn clamp_pagination_clamps_page_size() {
        // Guard: an unbounded page_size must be clamped to MAX_PAGE_SIZE.
        let (_, page_size, _) = clamp_pagination(1, u64::MAX);
        assert_eq!(page_size, MAX_PAGE_SIZE);
        // A zero page_size is raised to 1.
        let (_, page_size, _) = clamp_pagination(1, 0);
        assert_eq!(page_size, 1);
    }

    #[test]
    fn clamp_pagination_no_overflow_on_huge_values() {
        // Guard: saturating/checked arithmetic must not overflow (which would
        // panic in debug builds) and must keep the page size bounded.
        let (page_no, page_size, start) = clamp_pagination(u64::MAX, u64::MAX);
        assert_eq!(page_no, u64::MAX);
        assert_eq!(page_size, MAX_PAGE_SIZE);
        // The offset saturates and converts to a large but valid skip.
        assert!(start > 0);
    }
}
