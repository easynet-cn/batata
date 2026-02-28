//! V3 Console instance management endpoints
//!
//! Provides HTTP handlers for instance operations on the main server.

use std::collections::HashMap;

use actix_web::{HttpMessage, HttpRequest, Responder, Scope, get, put, web};
use serde::{Deserialize, Serialize};

use batata_api::naming::model::Instance;
use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType, error, model::AppState, model::response::Result,
    secured,
};

const DEFAULT_NAMESPACE_ID: &str = "public";
const DEFAULT_GROUP: &str = "DEFAULT_GROUP";
const DEFAULT_CLUSTER: &str = "DEFAULT";

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

fn default_page_no() -> u64 {
    1
}

fn default_page_size() -> u64 {
    100
}

impl InstanceListQuery {
    fn namespace_id_or_default(&self) -> &str {
        self.namespace_id
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_NAMESPACE_ID)
    }

    fn group_name_or_default(&self) -> &str {
        self.group_name
            .as_deref()
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
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_NAMESPACE_ID)
    }

    fn group_name_or_default(&self) -> &str {
        self.group_name
            .as_deref()
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

    let total_count = all_instances.len();
    let page_no = params.page_no.max(1);
    let page_size = params.page_size.max(1);
    let start = ((page_no - 1) * page_size) as usize;
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
    form: web::Form<InstanceUpdateForm>,
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
        instance_id: batata_api::naming::model::generate_instance_id(
            &form.ip,
            form.port,
            cluster_name,
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

pub fn routes() -> Scope {
    web::scope("/instance")
        .service(list_instances)
        .service(update_instance)
}
