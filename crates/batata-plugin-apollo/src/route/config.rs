use actix_web::{web, HttpResponse, Responder, HttpRequest};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use crate::persistence::traits::ApolloPersistenceService;
use crate::service::ReleaseService;
use crate::service::InstanceService;
use crate::service::GrayReleaseRuleService;
use crate::service::AppNamespaceService;
use crate::api::dto::{ApolloConfig, ErrorResponse, InstanceDTO, NotificationDTO, NotificationMessageDTO};

/// A resolved configuration bundle: merged kv map + merged release key.
#[derive(Debug, Clone)]
struct ResolvedConfig {
    configurations: HashMap<String, String>,
    release_key: String,
}

fn parse_labels(query: &web::Query<Value>) -> HashMap<String, String> {
    let mut labels = HashMap::new();
    if let Some(l) = query.get("label").and_then(|v| v.as_str()) {
        for pair in l.split(',') {
            if let Some((k, v)) = pair.split_once('=') {
                labels.insert(k.trim().to_string(), v.trim().to_string());
            }
        }
    }
    labels
}

/// Upstream passes the raw `?label=` token straight into gray-rule matching
/// (`clientLabel`), where rules list flat label tokens.
fn parse_client_label(query: &web::Query<Value>) -> Option<String> {
    query
        .get("label")
        .and_then(|v| v.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Upstream `AbstractConfigService.loadConfig`: walk the cluster fallback
/// chain — requested cluster (skipped when "default"), then dataCenter, then
/// "default" — at each step trying a matched gray release first, then the
/// latest active master release.
async fn find_release_for_cluster_chain(
    persistence: &Arc<dyn ApolloPersistenceService>,
    app_id: &str,
    client_ip: &str,
    labels: &HashMap<String, String>,
    cluster_name: &str,
    namespace_name: &str,
    data_center: Option<&str>,
    client_label: Option<&str>,
) -> anyhow::Result<Option<ResolvedConfig>> {
    let mut chain: Vec<&str> = Vec::new();
    if cluster_name != CLUSTER_NAME_DEFAULT {
        chain.push(cluster_name);
    }
    if let Some(dc) = data_center
        && !dc.is_empty() && dc != cluster_name {
            chain.push(dc);
        }
    chain.push(CLUSTER_NAME_DEFAULT);

    for cluster in chain {
        // Gray release wins when the client matches a rule for THIS cluster.
        if let Some((configs, key)) = get_gray_release_configuration(
            persistence,
            app_id,
            cluster,
            namespace_name,
            client_ip,
            labels,
            client_label,
        )
        .await?
        {
            return Ok(Some(ResolvedConfig {
                configurations: configs,
                release_key: key,
            }));
        }
        let release_service = ReleaseService::new(persistence.clone());
        if let Some(configurations) =
            release_service.get_configurations(app_id, cluster, namespace_name).await?
        {
            let release_key = release_service
                .get_latest_active(app_id, cluster, namespace_name)
                .await?
                .map(|r| r.release_key)
                .unwrap_or_default();
            return Ok(Some(ResolvedConfig {
                configurations,
                release_key,
            }));
        }
    }
    Ok(None)
}

/// Upstream `namespaceBelongsToAppId`: "application" always belongs; anything
/// else belongs only when this app defines that AppNamespace.
async fn namespace_belongs_to_app(
    persistence: &Arc<dyn ApolloPersistenceService>,
    app_id: &str,
    namespace: &str,
) -> bool {
    namespace == NAMESPACE_APPLICATION
        || AppNamespaceService::new(persistence.clone())
            .get(app_id, namespace)
            .await
            .map(|v| v.is_some())
            .unwrap_or(false)
}

/// Upstream `ConfigController.findPublicConfig`: exact-name lookup against
/// public AppNamespaces whose owner differs from the requesting app.
async fn find_public_namespace_owner(
    persistence: &Arc<dyn ApolloPersistenceService>,
    namespace: &str,
    requesting_app: &str,
) -> Option<String> {
    AppNamespaceService::new(persistence.clone())
        .list_public()
        .await
        .ok()?
        .into_iter()
        .find(|p| p.name == namespace && p.app_id != requesting_app)
        .map(|p| p.app_id)
}

/// Upstream `mergeReleaseConfigurations`: given `[private?, public?]`,
/// apply them reversed so **private keys override public keys** per-key
/// (a MERGE — public-only keys survive). Merged releaseKey is "+""-joined.
fn merge_release_configurations(private: Option<ResolvedConfig>, public: Option<ResolvedConfig>) -> Option<ResolvedConfig> {
    match (private, public) {
        (None, None) => None,
        (only @ Some(_), None) | (None, only @ Some(_)) => only,
        (Some(pri), Some(pub_cfg)) => {
            let mut merged: HashMap<String, String> = pub_cfg.configurations;
            for (k, v) in pri.configurations {
                merged.insert(k, v);
            }
            Some(ResolvedConfig {
                configurations: merged,
                release_key: format!("{}+{}", pri.release_key, pub_cfg.release_key),
            })
        }
    }
}

/// Full resolution pipeline shared by /configs and /configfiles*:
/// private chain first; when the namespace is not owned by the requesting app,
/// merge in the public owner's chain (private overrides public).
async fn resolve_effective_config(
    persistence: &Arc<dyn ApolloPersistenceService>,
    app_id: &str,
    cluster_name: &str,
    namespace_name: &str,
    data_center: Option<&str>,
    client_ip: &str,
    labels: &HashMap<String, String>,
    client_label: Option<&str>,
) -> anyhow::Result<Option<ResolvedConfig>> {
    let private = find_release_for_cluster_chain(
        persistence,
        app_id,
        client_ip,
        labels,
        cluster_name,
        namespace_name,
        data_center,
        client_label,
    )
    .await?;

    if namespace_belongs_to_app(persistence, app_id, namespace_name).await {
        return Ok(private);
    }

    let public_owner = find_public_namespace_owner(persistence, namespace_name, app_id).await;
    let public = match public_owner {
        Some(owner) => {
            find_release_for_cluster_chain(
                persistence,
                &owner,
                client_ip,
                labels,
                cluster_name,
                namespace_name,
                data_center,
                client_label,
            )
            .await?
        }
        None => None,
    };

    Ok(merge_release_configurations(private, public))
}

async fn query_config(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<Value>,
    req: HttpRequest,
) -> impl Responder {
    let (app_id, cluster_name, namespace) = path.into_inner();

    let release_key = query.get("releaseKey").and_then(|v| v.as_str()).unwrap_or("-1");

    if let Err(resp) = crate::middleware::access_key_auth::authenticate(&req, data.get_ref()).await {
        return resp;
    }

    let client_ip = get_client_ip(&req);
    let labels = parse_labels(&query);
    let client_label = parse_client_label(&query);
    let data_center = query.get("dataCenter").and_then(|v| v.as_str());

    match resolve_effective_config(
        data.get_ref(),
        &app_id,
        &cluster_name,
        &namespace,
        data_center,
        &client_ip,
        &labels,
        client_label.as_deref(),
    )
    .await
    {
        Ok(Some(resolved)) => {
            // Upstream auditReleases: record which release this instance
            // received (private AND public fetches both audited).
            crate::service::InstanceAuditService::new(data.get_ref().clone()).audit_async(
                &app_id,
                &cluster_name,
                data_center.unwrap_or(""),
                &client_ip,
                &app_id,
                normalize_namespace(&namespace),
                resolved.release_key.clone(),
            );
            if resolved.release_key == release_key {
                HttpResponse::NotModified().finish()
            } else {
                HttpResponse::Ok().json(ApolloConfig {
                    app_id: app_id.to_string(),
                    cluster: cluster_name.to_string(),
                    namespace_name: namespace.to_string(),
                    release_key: resolved.release_key,
                    configurations: resolved.configurations,
                })
            }
        }
        Ok(None) => HttpResponse::NotFound().json(ErrorResponse {
            status: 404,
            message: format!(
                "Could not load configurations with appId: {}, clusterName: {}, namespace: {}",
                app_id, cluster_name, namespace
            ),
        }),
        Err(e) => HttpResponse::InternalServerError().json(ErrorResponse {
            status: 500,
            message: e.to_string(),
        }),
    }
}

async fn query_config_file(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<Value>,
    req: HttpRequest,
) -> impl Responder {
    let (app_id, cluster_name, namespace) = path.into_inner();
    let release_key = query.get("releaseKey").and_then(|v| v.as_str()).unwrap_or("-1");

    if let Err(resp) = crate::middleware::access_key_auth::authenticate(&req, data.get_ref()).await {
        return resp;
    }

    let client_ip = get_client_ip(&req);
    let labels = parse_labels(&query);
    let client_label = parse_client_label(&query);
    let data_center = query.get("dataCenter").and_then(|v| v.as_str());

    match resolve_effective_config(
        data.get_ref(),
        &app_id,
        &cluster_name,
        &namespace,
        data_center,
        &client_ip,
        &labels,
        client_label.as_deref(),
    )
    .await
    {
        Ok(Some(resolved)) => {
            if resolved.release_key == release_key {
                return HttpResponse::NotModified().finish();
            }
            let content = render_config_file(&namespace, &resolved.configurations);
            HttpResponse::Ok()
                .content_type("text/plain; charset=UTF-8")
                .append_header(("Apollo-Release-Key", resolved.release_key))
                .body(content)
        }
        Ok(None) => HttpResponse::NotFound().body(format!(
            "Could not load configurations with appId: {}, clusterName: {}, namespace: {}",
            app_id, cluster_name, namespace
        )),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn query_config_file_raw(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<Value>,
    req: HttpRequest,
) -> impl Responder {
    let (app_id, cluster_name, namespace) = path.into_inner();
    let release_key = query.get("releaseKey").and_then(|v| v.as_str()).unwrap_or("-1");

    if let Err(resp) = crate::middleware::access_key_auth::authenticate(&req, data.get_ref()).await {
        return resp;
    }

    let client_ip = get_client_ip(&req);
    let labels = parse_labels(&query);
    let client_label = parse_client_label(&query);
    let data_center = query.get("dataCenter").and_then(|v| v.as_str());

    match resolve_effective_config(
        data.get_ref(),
        &app_id,
        &cluster_name,
        &namespace,
        data_center,
        &client_ip,
        &labels,
        client_label.as_deref(),
    )
    .await
    {
        Ok(Some(resolved)) => {
            if resolved.release_key == release_key {
                return HttpResponse::NotModified().finish();
            }
            let content = get_raw_config_content(&namespace, &resolved.configurations);
            let content_type = determine_content_type(&namespace);
            HttpResponse::Ok()
                .content_type(content_type)
                .append_header(("Apollo-Release-Key", resolved.release_key))
                .body(content)
        }
        Ok(None) => HttpResponse::NotFound().body(format!(
            "Could not load configurations with appId: {}, clusterName: {}, namespace: {}",
            app_id, cluster_name, namespace
        )),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

/// Upstream `ConfigConsts`.
const CLUSTER_NAME_DEFAULT: &str = "default";
const NAMESPACE_APPLICATION: &str = "application";
/// Upstream `BizConfig.longPollingTimeoutInMilli` default (clamped 1..90s).
const LONG_POLLING_TIMEOUT: Duration = Duration::from_secs(60);
/// Upstream deprecated `NotificationController.TIMEOUT`.

/// Upstream `NamespaceUtil.filterNamespaceName`: strip a trailing `.properties`.
fn normalize_namespace(ns: &str) -> &str {
    ns.strip_suffix(".properties").unwrap_or(ns)
}

fn watch_key(app_id: &str, cluster: &str, namespace_name: &str) -> String {
    crate::service::release_message_service::ReleaseMessageService::generate_message(
        app_id,
        cluster,
        namespace_name,
    )
}

/// Upstream `WatchKeysUtil.assembleAllWatchKeys` for one namespace:
/// always `{app}+default+{ns}`, plus `{app}+{cluster}+{ns}` when cluster is
/// not "default", plus `{app}+{dataCenter}+{ns}` when dataCenter differs.
fn assemble_watch_keys(
    app_id: &str,
    cluster: &str,
    namespace_name: &str,
    data_center: Option<&str>,
) -> Vec<String> {
    let mut keys = Vec::new();
    if cluster != CLUSTER_NAME_DEFAULT {
        keys.push(watch_key(app_id, cluster, namespace_name));
    }
    if let Some(dc) = data_center
        && !dc.is_empty() && dc != cluster {
            keys.push(watch_key(app_id, dc, namespace_name));
        }
    keys.push(watch_key(app_id, CLUSTER_NAME_DEFAULT, namespace_name));
    keys
}

async fn poll_notifications_v2_impl(
    data: &web::Data<Arc<dyn ApolloPersistenceService>>,
    query: &web::Query<Value>,
) -> HttpResponse {
    let app_id = query.get("appId").and_then(|v| v.as_str()).unwrap_or("");
    let cluster = query
        .get("cluster")
        .and_then(|v| v.as_str())
        .unwrap_or(CLUSTER_NAME_DEFAULT);
    let data_center = query.get("dataCenter").and_then(|v| v.as_str());
    let notifications_str = query
        .get("notifications")
        .and_then(|v| v.as_str())
        .unwrap_or("[]");
    let notifications: Vec<Value> = match serde_json::from_str(notifications_str) {
        Ok(v) => v,
        Err(_) => {
            // upstream BadRequestException.invalidNotificationsFormat → 400
            return HttpResponse::BadRequest().json(ErrorResponse {
                status: 400,
                message: format!("Invalid notifications format: {}", notifications_str),
            });
        }
    };

    // Parse + normalize; for a duplicated normalized namespace keep the entry
    // with the SMALLER client id (upstream dedupe rule).
    let mut entries: Vec<(String, i64)> = Vec::with_capacity(notifications.len());
    for notif in &notifications {
        let raw_ns = notif
            .get("namespaceName")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ns = normalize_namespace(raw_ns);
        if ns.is_empty() {
            continue;
        }
        let id = notif.get("notificationId").and_then(|v| v.as_i64()).unwrap_or(-1);
        if let Some(existing) = entries.iter_mut().find(|(n, _)| n == ns) {
            if id < existing.1 {
                existing.1 = id;
            }
        } else {
            entries.push((ns.to_string(), id));
        }
    }

    if entries.is_empty() {
        return HttpResponse::NotModified().finish();
    }

    // Watch-key expansion: per-namespace keys + public-namespace owner keys.
    let mut ns_keys: Vec<(String, Vec<String>)> = Vec::with_capacity(entries.len());
    let mut all_keys: Vec<String> = Vec::new();
    let public_namespaces = AppNamespaceService::new(data.get_ref().clone())
        .list_public()
        .await
        .unwrap_or_default();
    for (ns, _) in &entries {
        let mut keys = assemble_watch_keys(app_id, cluster, ns, data_center);
        // Public namespace owned by another app: also watch the owner's keys
        // (upstream findPublicConfigWatchKeys).
        if let Some(pns) = public_namespaces
            .iter()
            .find(|p| p.name == *ns && p.app_id != app_id)
        {
            keys.extend(assemble_watch_keys(&pns.app_id, cluster, ns, data_center));
        }
        all_keys.extend(keys.iter().cloned());
        ns_keys.push((ns.clone(), keys));
    }

    let hub = crate::service::notification_hub::hub();
    let message_service =
        crate::service::release_message_service::ReleaseMessageService::new(data.get_ref().clone());

    // Register waiters BEFORE the DB check to avoid lost wake-ups (upstream
    // registers deferred results first for the same reason).
    let notify = hub.register(&all_keys);

    match compute_changed(&message_service, &ns_keys, &entries).await {
        Ok(changed) if !changed.is_empty() => {
            return HttpResponse::Ok().json(changed);
        }
        _ => {}
    }

    // Suspend until any watched key is published or timeout → 304.
    let _woken = tokio::time::timeout(LONG_POLLING_TIMEOUT, notify.notified()).await;

    match compute_changed(&message_service, &ns_keys, &entries).await {
        Ok(changed) if !changed.is_empty() => HttpResponse::Ok().json(changed),
        _ => HttpResponse::NotModified().finish(),
    }
}

/// Latest notificationId per namespace = max over its watch keys (missing keys
/// count as upstream placeholder -1); emit entries newer than the client id.
async fn compute_changed(
    message_service: &crate::service::release_message_service::ReleaseMessageService,
    ns_keys: &[(String, Vec<String>)],
    entries: &[(String, i64)],
) -> anyhow::Result<Vec<NotificationDTO>> {
    let latest_map = message_service
        .find_latest_by_keys(
            &ns_keys
                .iter()
                .flat_map(|(_, keys)| keys.iter().cloned())
                .collect::<Vec<_>>(),
        )
        .await?;
    let mut result = Vec::new();
    for ((ns, keys), (_, client_id)) in ns_keys.iter().zip(entries.iter()) {
        let mut latest_id: i64 = -1;
        let mut details: HashMap<String, String> = HashMap::new();
        for key in keys {
            if let Some(msg) = latest_map.get(key) {
                details.insert(key.clone(), msg.id.to_string());
                latest_id = latest_id.max(msg.id);
            }
        }
        if latest_id > *client_id {
            result.push(NotificationDTO {
                namespace_name: ns.clone(),
                notification_id: latest_id,
                messages: NotificationMessageDTO { details },
            });
        }
    }
    Ok(result)
}

async fn get_notification_v2(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
    req: HttpRequest,
) -> impl Responder {
    if let Err(resp) = crate::middleware::access_key_auth::authenticate(&req, data.get_ref()).await {
        return resp;
    }
    poll_notifications_v2_impl(&data, &query).await
}

async fn register_instance(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<InstanceDTO>,
) -> impl Responder {
    let service = InstanceService::new(data.get_ref().clone());
    match service.register(body.into_inner()).await {
        Ok(instance) => HttpResponse::Ok().json(instance),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

async fn heartbeat(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    body: web::Json<Value>,
) -> impl Responder {
    let service = InstanceService::new(data.get_ref().clone());
    let app_id = body.get("appId").and_then(|v| v.as_str()).unwrap_or("");
    let ip = body.get("ip").and_then(|v| v.as_str()).unwrap_or("");
    let cluster = body.get("cluster").and_then(|v| v.as_str()).unwrap_or("default");
    let data_center = body.get("dataCenter").and_then(|v| v.as_str()).unwrap_or("default");

    match service.heartbeat(app_id, cluster, ip, data_center).await {
        Ok(_) => HttpResponse::Ok().json(serde_json::json!({"status": "ok"})),
        Err(e) => HttpResponse::BadRequest().json(ErrorResponse {
            status: 400,
            message: e.to_string(),
        }),
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ServiceDTO {
    app_name: String,
    instance_id: String,
    homepage_url: String,
}

fn build_service_dto(req: &HttpRequest, app_name: &str) -> ServiceDTO {
    let host = req
        .headers()
        .get("host")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("localhost:8080");
    ServiceDTO {
        app_name: app_name.to_string(),
        instance_id: "batata".to_string(),
        homepage_url: format!("http://{}", host),
    }
}

/// Upstream `DatabaseDiscoveryClientImpl` mapping: appName = ServiceName,
/// instanceId = homepageUrl = Uri; only rows inside the health-check window
/// (default 61s) are considered alive. When the registry has no live row
/// (fresh single-node install), fall back to this node itself so Java
/// clients keep working out of the box.
async fn discover(data: &web::Data<Arc<dyn ApolloPersistenceService>>, service_name: &str, req: &HttpRequest) -> Vec<ServiceDTO> {
    use crate::persistence::traits::ServiceRegistryPersistence;
    const HEALTH_WINDOW_SECS: i64 = 61;
    let mut dtos: Vec<ServiceDTO> = ServiceRegistryPersistence::find_alive(
        data.get_ref(),
        service_name,
        HEALTH_WINDOW_SECS,
    )
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|e| ServiceDTO {
        app_name: e.service_name,
        instance_id: e.uri.clone(),
        homepage_url: e.uri,
    })
    .collect();
    if dtos.is_empty() {
        dtos.push(build_service_dto(req, service_name));
    }
    dtos
}

async fn get_config_service(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    req: HttpRequest,
) -> impl Responder {
    let dtos = discover(&data, "apollo-configservice", &req).await;
    HttpResponse::Ok().json(dtos)
}

async fn get_admin_service(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    req: HttpRequest,
) -> impl Responder {
    let dtos = discover(&data, "apollo-adminservice", &req).await;
    HttpResponse::Ok().json(dtos)
}

async fn list_all_services(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    req: HttpRequest,
) -> impl Responder {
    let mut all = discover(&data, "apollo-configservice", &req).await;
    all.extend(discover(&data, "apollo-adminservice", &req).await);
    HttpResponse::Ok().json(all)
}

/// Performs the `configure_config_routes` operation.
pub fn configure_config_routes(cfg: &mut actix_web::web::ServiceConfig) {
    cfg.service(
        web::resource("/configs/{app_id}/{cluster_name}/{namespace}")
            .route(web::get().to(query_config)),
    )
    .service(
        web::resource("/configfiles/{app_id}/{cluster_name}/{namespace}")
            .route(web::get().to(query_config_file)),
    )
    .service(
        web::resource("/configfiles/json/{app_id}/{cluster_name}/{namespace}")
            .route(web::get().to(query_config_file)),
    )
    .service(
        web::resource("/configfiles/raw/{app_id}/{cluster_name}/{namespace}")
            .route(web::get().to(query_config_file_raw)),
    )
    .service(
        web::resource("/services/config")
            .route(web::get().to(get_config_service)),
    )
    .service(
        web::resource("/services/admin")
            .route(web::get().to(get_admin_service)),
    )
    .service(
        web::resource("/")
            .route(web::get().to(list_all_services)),
    )
    .service(
        web::resource("/notifications/v2")
            .route(web::get().to(get_notification_v2)),
    )
    .service(
        web::resource("/instances")
            .route(web::post().to(register_instance))
            .route(web::put().to(heartbeat)),
    );
}

fn get_client_ip(req: &HttpRequest) -> String {
    if let Some(ip) = req.headers().get("X-Forwarded-For")
        && let Ok(ip_str) = ip.to_str() {
            let ips: Vec<&str> = ip_str.split(',').collect();
            if let Some(first_ip) = ips.first() {
                return first_ip.trim().to_string();
            }
        }
    if let Some(ip) = req.headers().get("X-Real-IP")
        && let Ok(ip_str) = ip.to_str() {
            return ip_str.trim().to_string();
        }
    req.connection_info().peer_addr().unwrap_or("127.0.0.1").to_string()
}

async fn get_gray_release_configuration(persistence: &Arc<dyn ApolloPersistenceService>, app_id: &str, cluster_name: &str, namespace_name: &str, client_ip: &str, labels: &HashMap<String, String>, client_label: Option<&str>) -> Result<Option<(std::collections::HashMap<String, String>, String)>, anyhow::Error> {
    let gray_service = GrayReleaseRuleService::new(persistence.clone());
    if let Ok(Some(gray_release_id)) = gray_service
        .match_gray_release_rule_with_context(app_id, cluster_name, namespace_name, client_ip, Some(labels), None, client_label)
        .await
    {
        let release_service = ReleaseService::new(persistence.clone());
        if let Ok(Some(release)) = release_service.get_gray_release(gray_release_id).await {
            let configs: std::collections::HashMap<String, String> = serde_json::from_str(&release.configurations.unwrap_or_default()).unwrap_or_default();
            return Ok(Some((configs, release.release_key)));
        }
    }
    Ok(None)
}

fn render_config_file(namespace: &str, configurations: &std::collections::HashMap<String, String>) -> String {
    if namespace.ends_with(".properties") || !namespace.contains('.') {
        configurations.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n")
    } else if namespace.ends_with(".json") {
        let map: HashMap<&str, &str> = configurations.iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        serde_json::to_string_pretty(&map).unwrap_or_default()
    } else if namespace.ends_with(".xml") {
        let mut xml = String::from("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<configuration>\n");
        for (k, v) in configurations {
            xml.push_str(&format!("  <property name=\"{}\" value=\"{}\"/>\n", k, v));
        }
        xml.push_str("</configuration>");
        xml
    } else if namespace.ends_with(".yaml") || namespace.ends_with(".yml") {
        let mut yaml = String::new();
        for (k, v) in configurations {
            yaml.push_str(&format!("{}: {}\n", k, v));
        }
        yaml
    } else {
        configurations.iter()
            .map(|(k, v)| format!("{}={}", k, v))
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn get_raw_config_content(namespace: &str, configurations: &std::collections::HashMap<String, String>) -> String {
    // For .properties namespaces (or namespaces without a dot), behavior is same as regular /configfiles
    if namespace.ends_with(".properties") || !namespace.contains('.') {
        return render_config_file(namespace, configurations);
    }
    // For non-properties namespaces (json/yaml/xml), return the "content" field from configurations directly
    configurations.get("content").cloned().unwrap_or_default()
}

fn determine_content_type(namespace: &str) -> &'static str {
    if namespace.ends_with(".json") {
        "application/json;charset=UTF-8"
    } else if namespace.ends_with(".yml") || namespace.ends_with(".yaml") {
        "application/yaml;charset=UTF-8"
    } else if namespace.ends_with(".xml") {
        "application/xml;charset=UTF-8"
    } else {
        "text/plain;charset=UTF-8"
    }
}
