//! V3 Console service management endpoints
//!
//! Provides HTTP handlers for service discovery operations on the main server.

use std::collections::HashMap;

use actix_web::{HttpRequest, Responder, Scope, delete, get, post, put, web};
use serde::Deserialize;

use batata_common::{DEFAULT_GROUP, DEFAULT_NAMESPACE_ID, default_page_no, default_page_size};
use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType, error, model::AppState, model::response::Result,
    secured,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceListQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default, alias = "groupNameParam")]
    group_name: Option<String>,
    #[serde(default, alias = "serviceNameParam")]
    service_name: Option<String>,
    #[serde(default = "default_page_no")]
    page_no: u64,
    #[serde(default = "default_page_size")]
    page_size: u64,
    #[serde(default, alias = "ignoreEmptyService")]
    has_ip_count: Option<bool>,
    /// Accepted for Nacos compatibility; full withInstances support requires datasource changes.
    #[allow(dead_code)]
    #[serde(default)]
    with_instances: bool,
}

impl ServiceListQuery {
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
struct ServiceDetailQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
}

impl ServiceDetailQuery {
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
struct ServiceForm {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    /// Service-level ephemeral flag, matching upstream `ServiceForm.ephemeral`.
    /// Upstream only applies it on create (`ConsoleServiceController.createService`);
    /// `updateService` ignores it, so the update handler does not read it.
    #[serde(default)]
    ephemeral: Option<bool>,
    #[serde(default)]
    protect_threshold: Option<f64>,
    #[serde(default)]
    metadata: Option<serde_json::Value>,
    #[serde(default)]
    selector: Option<serde_json::Value>,
}

impl ServiceForm {
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
struct SubscriberQuery {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    #[serde(default = "default_page_no")]
    page_no: u64,
    #[serde(default = "default_page_size")]
    page_size: u64,
}

impl SubscriberQuery {
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

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UpdateClusterForm {
    #[serde(default)]
    namespace_id: Option<String>,
    #[serde(default)]
    group_name: Option<String>,
    service_name: String,
    cluster_name: String,
    #[serde(default)]
    health_checker: Option<serde_json::Value>,
    #[serde(default)]
    metadata: Option<HashMap<String, String>>,
    #[serde(default)]
    check_port: Option<i32>,
    #[serde(default)]
    use_instance_port4_check: Option<bool>,
}

impl UpdateClusterForm {
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

/// GET /ns/service/list
#[get("list")]
async fn list_services(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ServiceListQuery>,
) -> impl Responder {
    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

    let resource = format!("{}:{}:naming/*", namespace_id, group_name);
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let service_name_filter = params.service_name.as_deref().unwrap_or("");
    let has_ip_count = params.has_ip_count.unwrap_or(false);

    match data
        .console_datasource
        .service_list(
            namespace_id,
            group_name,
            service_name_filter,
            params.page_no,
            params.page_size,
            has_ip_count,
        )
        .await
    {
        Ok((count, service_list)) => {
            let response = serde_json::json!({
                "count": count,
                "serviceList": service_list,
            });
            Result::<serde_json::Value>::http_success(response)
        }
        Err(e) => Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// GET /ns/service
#[get("")]
async fn get_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ServiceDetailQuery>,
) -> impl Responder {
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
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data
        .console_datasource
        .service_get(namespace_id, group_name, &params.service_name)
        .await
    {
        Ok(Some(service)) => Result::<serde_json::Value>::http_success(service),
        Ok(None) => Result::<Option<serde_json::Value>>::http_response(
            404,
            20004,
            format!("service {} not found", params.service_name),
            None::<serde_json::Value>,
        ),
        Err(e) => Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// POST /ns/service
#[post("")]
async fn create_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Json<ServiceForm>,
) -> impl Responder {
    if form.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'serviceName' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = form.namespace_id_or_default();
    let group_name = form.group_name_or_default();

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

    let threshold = form.protect_threshold.unwrap_or(0.0);
    let ephemeral = form.ephemeral.unwrap_or(false);
    let metadata_str = form
        .metadata
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_default();
    let selector_str = form
        .selector
        .as_ref()
        .map(|v| v.to_string())
        .unwrap_or_default();

    match data
        .console_datasource
        .service_create(
            namespace_id,
            group_name,
            &form.service_name,
            threshold,
            &metadata_str,
            &selector_str,
            ephemeral,
        )
        .await
    {
        Ok(_result) => Result::<String>::http_success("ok".to_string()),
        Err(e) => Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// PUT /ns/service
#[put("")]
async fn update_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Json<ServiceForm>,
) -> impl Responder {
    if form.service_name.is_empty() {
        return Result::<String>::http_response(
            400,
            error::PARAMETER_MISSING.code,
            "Required parameter 'serviceName' is missing".to_string(),
            String::new(),
        );
    }

    let namespace_id = form.namespace_id_or_default();
    let group_name = form.group_name_or_default();

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

    let metadata_str = form.metadata.as_ref().map(|v| v.to_string());
    let selector_str = form.selector.as_ref().map(|v| v.to_string());

    match data
        .console_datasource
        .service_update(
            namespace_id,
            group_name,
            &form.service_name,
            form.protect_threshold,
            metadata_str.as_deref(),
            selector_str.as_deref(),
        )
        .await
    {
        Ok(_result) => Result::<String>::http_success("ok".to_string()),
        Err(e) => Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// DELETE /ns/service
#[delete("")]
async fn delete_service(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<ServiceDetailQuery>,
) -> impl Responder {
    let namespace_id = params.namespace_id_or_default();
    let group_name = params.group_name_or_default();

    let resource = format!(
        "{}:{}:naming/{}",
        namespace_id, group_name, params.service_name
    );
    secured!(
        Secured::builder(&req, &data, &resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Naming)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data
        .console_datasource
        .service_delete(namespace_id, group_name, &params.service_name)
        .await
    {
        Ok(_result) => Result::<String>::http_success("ok".to_string()),
        Err(e) => Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// GET /ns/service/subscribers
#[get("subscribers")]
async fn list_subscribers(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<SubscriberQuery>,
) -> impl Responder {
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
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    match data
        .console_datasource
        .service_subscriber_list(
            namespace_id,
            group_name,
            &params.service_name,
            params.page_no,
            params.page_size,
        )
        .await
    {
        Ok((count, subscribers)) => {
            let subscriber_list: Vec<serde_json::Value> = subscribers
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "addrStr": s,
                        "agent": s,
                    })
                })
                .collect();

            let response = serde_json::json!({
                "count": count,
                "subscribers": subscriber_list,
            });
            Result::<serde_json::Value>::http_success(response)
        }
        Err(e) => Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// GET /ns/service/selector/types
#[get("selector/types")]
async fn get_selector_types(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "console:naming")
            .action(ActionTypes::Read)
            .sign_type(SignType::Naming)
            .api_type(ApiType::ConsoleApi)
            .build()
    );

    let types = vec!["none".to_string(), "label".to_string()];
    Result::<Vec<String>>::http_success(types)
}

/// PUT /ns/service/cluster
#[put("cluster")]
async fn update_cluster(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Json<UpdateClusterForm>,
) -> impl Responder {
    let namespace_id = form.namespace_id_or_default();
    let group_name = form.group_name_or_default();

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

    let health_checker_type = form
        .health_checker
        .as_ref()
        .and_then(|v| {
            v.get("type")
                .and_then(|t| t.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| "TCP".to_string());

    match data
        .console_datasource
        .service_update_cluster(
            namespace_id,
            group_name,
            &form.service_name,
            &form.cluster_name,
            Some(health_checker_type.as_str()),
            form.metadata.clone(),
            form.check_port,
            form.use_instance_port4_check,
        )
        .await
    {
        Ok(_result) => Result::<String>::http_success("ok".to_string()),
        Err(e) => Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            e.to_string(),
            String::new(),
        ),
    }
}

/// Register the service discovery routes under `/ns`.
pub fn routes() -> Scope {
    web::scope("/ns")
        .service(
            web::scope("/service")
                .service(create_service)
                .service(delete_service)
                .service(update_service)
                .service(get_service)
                .service(list_services)
                .service(list_subscribers)
                .service(get_selector_types)
                .service(update_cluster),
        )
        .service(super::instance::routes())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Upstream `ServiceForm` carries `ephemeral` and `ConsoleServiceController`
    /// applies it on create, so the console form has to accept it.
    #[test]
    fn test_service_form_accepts_ephemeral_true() {
        let json = r#"{"serviceName":"svc","ephemeral":true,"protectThreshold":0.5}"#;
        let form: ServiceForm = serde_json::from_str(json).unwrap();
        assert_eq!(form.ephemeral, Some(true));
        assert_eq!(form.service_name, "svc");
        assert_eq!(form.protect_threshold, Some(0.5));
    }

    #[test]
    fn test_service_form_accepts_ephemeral_false() {
        let json = r#"{"serviceName":"svc","ephemeral":false}"#;
        let form: ServiceForm = serde_json::from_str(json).unwrap();
        assert_eq!(form.ephemeral, Some(false));
    }

    /// An absent flag stays `None`; `create_service` maps it to `false`, which
    /// matches upstream (a null `ServiceForm.ephemeral` defaults to false).
    #[test]
    fn test_service_form_ephemeral_absent_is_none() {
        let json = r#"{"serviceName":"svc"}"#;
        let form: ServiceForm = serde_json::from_str(json).unwrap();
        assert_eq!(form.ephemeral, None);
    }

    /// The camelCase aliases still resolve, so adding the field cannot break a
    /// real portal payload.
    #[test]
    fn test_service_form_camel_case_aliases_with_ephemeral() {
        let json =
            r#"{"serviceName":"svc","groupName":"G","protectThreshold":0.3,"ephemeral":true}"#;
        let form: ServiceForm = serde_json::from_str(json).unwrap();
        assert_eq!(form.group_name.as_deref(), Some("G"));
        assert_eq!(form.protect_threshold, Some(0.3));
        assert_eq!(form.ephemeral, Some(true));
    }

    #[test]
    fn test_service_form_protect_threshold_keeps_exact_wire_value() {
        let json = r#"{"serviceName":"svc","protectThreshold":0.7}"#;
        let form: ServiceForm = serde_json::from_str(json).unwrap();
        let threshold = form.protect_threshold.unwrap();
        // An f32 would serialize as 0.699999988079071 and the detail page
        // would not show the "0.7" the user just saved.
        assert_eq!(serde_json::to_string(&threshold).unwrap(), "0.7");
    }

    #[test]
    fn test_update_cluster_form_uses_upstream_field_names() {
        // Upstream `UpdateClusterForm` calls these `checkPort` and
        // `useInstancePort4Check`. Sending `healthyCheckPort` /
        // `useInstancePortForCheck` instead leaves them `None`, so the
        // cluster health-check port never changes.
        let json = r#"{"serviceName":"svc","clusterName":"DEFAULT","checkPort":8081,"useInstancePort4Check":true,"healthChecker":{"type":"HTTP"}}"#;
        let form: UpdateClusterForm = serde_json::from_str(json).unwrap();
        assert_eq!(form.check_port, Some(8081));
        assert_eq!(form.use_instance_port4_check, Some(true));
        assert!(form.health_checker.is_some());
        assert_eq!(form.namespace_id_or_default(), DEFAULT_NAMESPACE_ID);
        assert_eq!(form.group_name_or_default(), DEFAULT_GROUP);
    }
}
