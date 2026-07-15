use actix_web::{web, HttpResponse, Responder, HttpRequest};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use crate::persistence::traits::ApolloPersistenceService;
use crate::service::ReleaseService;
use crate::service::InstanceService;
use crate::service::GrayReleaseRuleService;
use crate::api::dto::{ApolloConfig, ErrorResponse, InstanceDTO, NotificationDTO, NotificationMessageDTO};

async fn query_config(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    path: web::Path<(String, String, String)>,
    query: web::Query<Value>,
    req: HttpRequest,
) -> impl Responder {
    let (app_id, cluster_name, namespace) = path.into_inner();

    let release_key = query.get("releaseKey").and_then(|v| v.as_str()).unwrap_or("-1");

    let client_ip = get_client_ip(&req);
    let service = ReleaseService::new(data.get_ref().clone());

    match get_gray_release_configuration(data.get_ref(), &app_id, &cluster_name, &namespace, &client_ip).await {
        Ok(Some((configs, release_key_val))) => {
            if release_key_val == release_key {
                return HttpResponse::NotModified().finish();
            }
            return HttpResponse::Ok().json(ApolloConfig {
                app_id: app_id.to_string(),
                cluster: cluster_name.to_string(),
                namespace_name: namespace.to_string(),
                release_key: release_key_val,
                configurations: configs,
            });
        }
        Ok(None) => {}
        Err(e) => {
            return HttpResponse::InternalServerError().json(ErrorResponse {
                status: 500,
                message: e.to_string(),
            });
        }
    };

    match service.get_configurations(&app_id, &cluster_name, &namespace).await {
        Ok(Some(configurations)) => {
            let latest_release = service.get_latest_active(&app_id, &cluster_name, &namespace).await.unwrap_or(None);
            let current_release_key = latest_release.map(|r| r.release_key).unwrap_or_default();

            if current_release_key == release_key {
                return HttpResponse::NotModified().finish();
            }

            HttpResponse::Ok().json(ApolloConfig {
                app_id: app_id.to_string(),
                cluster: cluster_name.to_string(),
                namespace_name: namespace.to_string(),
                release_key: current_release_key,
                configurations,
            })
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

    let client_ip = get_client_ip(&req);

    if let Ok(Some((configurations, current_release_key))) = get_gray_release_configuration(data.get_ref(), &app_id, &cluster_name, &namespace, &client_ip).await {
        if current_release_key == release_key {
            return HttpResponse::NotModified().finish();
        }

        let content = render_config_file(&namespace, &configurations);
        return HttpResponse::Ok()
            .content_type("text/plain; charset=UTF-8")
            .append_header(("Apollo-Release-Key", current_release_key))
            .body(content);
    }

    let service = ReleaseService::new(data.get_ref().clone());
    match service.get_configurations(&app_id, &cluster_name, &namespace).await {
        Ok(Some(configurations)) => {
            let latest_release = service.get_latest_active(&app_id, &cluster_name, &namespace).await.unwrap_or(None);
            let current_release_key = latest_release.map(|r| r.release_key).unwrap_or_default();

            if current_release_key == release_key {
                return HttpResponse::NotModified().finish();
            }

            let content = render_config_file(&namespace, &configurations);

            HttpResponse::Ok()
                .content_type("text/plain; charset=UTF-8")
                .append_header(("Apollo-Release-Key", current_release_key))
                .body(content)
        }
        Ok(None) => HttpResponse::NotFound().body(format!(
            "Could not load configurations with appId: {}, clusterName: {}, namespace: {}",
            app_id, cluster_name, namespace
        )),
        Err(e) => HttpResponse::InternalServerError().body(e.to_string()),
    }
}

async fn poll_notifications(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    let app_id = query.get("appId").and_then(|v| v.as_str()).unwrap_or("");
    let cluster = query.get("cluster").and_then(|v| v.as_str()).unwrap_or("default");
    let notifications_str = query.get("notifications").and_then(|v| v.as_str()).unwrap_or("[]");

    let notifications: Vec<Value> = serde_json::from_str(notifications_str).unwrap_or_default();

    let service = ReleaseService::new(data.get_ref().clone());
    let mut result = Vec::new();

    for notif in &notifications {
        let namespace_name = notif.get("namespaceName").and_then(|v| v.as_str()).unwrap_or("application");
        let notification_id = notif.get("notificationId").and_then(|v| v.as_i64()).unwrap_or(-1);

        if let Ok(Some(release)) = service.get_latest_active(app_id, cluster, namespace_name).await {
            let release_id = release.id.unwrap_or(0) as i64;
            if release_id != notification_id {
                let mut details = HashMap::new();
                details.insert("appId".to_string(), app_id.to_string());
                details.insert("cluster".to_string(), cluster.to_string());
                details.insert("namespaceName".to_string(), namespace_name.to_string());

                result.push(NotificationDTO {
                    namespace_name: namespace_name.to_string(),
                    notification_id: release_id,
                    messages: NotificationMessageDTO { details },
                });
            }
        }
    }

    if !result.is_empty() {
        return HttpResponse::Ok().json(result);
    }

    HttpResponse::NotModified().finish()
}

async fn get_notification_v2(
    data: web::Data<Arc<dyn ApolloPersistenceService>>,
    query: web::Query<Value>,
) -> impl Responder {
    poll_notifications(data, query).await
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
        web::resource("/notifications/v2")
            .route(web::get().to(get_notification_v2)),
    )
    .service(
        web::resource("/notifications")
            .route(web::get().to(poll_notifications)),
    )
    .service(
        web::resource("/instances")
            .route(web::post().to(register_instance))
            .route(web::put().to(heartbeat)),
    );
}

fn get_client_ip(req: &HttpRequest) -> String {
    if let Some(ip) = req.headers().get("X-Forwarded-For") {
        if let Ok(ip_str) = ip.to_str() {
            let ips: Vec<&str> = ip_str.split(',').collect();
            if let Some(first_ip) = ips.first() {
                return first_ip.trim().to_string();
            }
        }
    }
    if let Some(ip) = req.headers().get("X-Real-IP") {
        if let Ok(ip_str) = ip.to_str() {
            return ip_str.trim().to_string();
        }
    }
    req.connection_info().peer_addr().unwrap_or("127.0.0.1").to_string()
}

async fn get_gray_release_configuration(persistence: &Arc<dyn ApolloPersistenceService>, app_id: &str, cluster_name: &str, namespace_name: &str, client_ip: &str) -> Result<Option<(std::collections::HashMap<String, String>, String)>, anyhow::Error> {
    let gray_service = GrayReleaseRuleService::new(persistence.clone());
    if let Ok(Some(gray_release_id)) = gray_service.match_gray_release_rule(app_id, cluster_name, namespace_name, client_ip).await {
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
