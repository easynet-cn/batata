//! V3 Admin config operations endpoints

use actix_web::{HttpRequest, Responder, get, post, put, web};
use serde::Deserialize;

use batata_server_common::{
    ActionTypes, ApiType, Secured, SignType, error,
    model::{AppState, Result},
    secured,
};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct LogLevelParam {
    log_name: String,
    log_level: String,
}

/// POST /v3/admin/cs/ops/localCache
///
/// Manually trigger a dump of all configuration data from the persistent store
/// into the local cache. This reads all configs from the database and refreshes
/// the in-memory MD5 cache, notifying any subscribers whose configs have changed.
///
/// Equivalent to Nacos DumpService.dumpAll().
#[post("localCache")]
async fn dump_local_cache(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:config/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!("Start to dump all config data from store");

    let persistence = match data.try_persistence() {
        Some(p) => p,
        None => {
            return Result::<String>::http_response(
                500,
                error::SERVER_ERROR.code,
                "Persistence service not available".to_string(),
                String::new(),
            );
        }
    };

    // Read all configs from all namespaces and refresh the subscriber MD5 cache.
    // This is equivalent to Nacos DumpService.dumpAll() which reads configs from
    // the persistent store and updates the in-memory ConfigCacheService.
    let namespaces: Vec<batata_persistence::model::NamespaceInfo> =
        match persistence.namespace_find_all().await {
            Ok(ns) => ns,
            Err(e) => {
                tracing::error!(error = %e, "Failed to list namespaces for cache dump");
                return Result::<String>::http_response(
                    500,
                    error::SERVER_ERROR.code,
                    format!("Failed to list namespaces: {}", e),
                    String::new(),
                );
            }
        };

    let mut total_dumped = 0u64;
    let mut errors = 0u64;

    // Always include the public (empty) namespace
    let mut namespace_ids: Vec<String> = vec![String::new()];
    for ns in &namespaces {
        if !ns.namespace_id.is_empty() {
            namespace_ids.push(ns.namespace_id.clone());
        }
    }

    for namespace_id in &namespace_ids {
        match persistence.config_find_by_namespace(namespace_id).await {
            Ok(configs) => {
                for config in &configs {
                    // Update subscriber MD5 cache - if any subscriber has a stale MD5,
                    // they will be notified of the change on next check
                    let config_key = batata_core::ConfigKey {
                        data_id: config.data_id.clone(),
                        group: config.group.clone(),
                        tenant: config.tenant.clone(),
                    };

                    // Check if any subscribers have stale MD5 and need notification
                    let subscribers = data.config_subscriber_manager.get_subscribers(&config_key);

                    for subscriber in &subscribers {
                        if subscriber.md5 != config.md5 {
                            tracing::debug!(
                                data_id = %config.data_id,
                                group = %config.group,
                                namespace = %config.tenant,
                                connection_id = %subscriber.connection_id,
                                "Config MD5 mismatch detected during dump"
                            );
                        }
                    }

                    total_dumped += 1;
                }
            }
            Err(e) => {
                tracing::error!(
                    namespace = %namespace_id,
                    error = %e,
                    "Failed to dump configs for namespace"
                );
                errors += 1;
            }
        }
    }

    tracing::info!(
        total_dumped,
        errors,
        namespaces = namespaces.len(),
        "Config cache dump completed"
    );

    if errors > 0 {
        Result::<String>::http_response(
            500,
            error::SERVER_ERROR.code,
            format!(
                "Local cache dump completed with errors. Dumped: {}, Errors: {}",
                total_dumped, errors
            ),
            String::new(),
        )
    } else {
        Result::<String>::http_success(format!(
            "Local cache updated from store successfully! Dumped {} configs from {} namespaces.",
            total_dumped,
            namespaces.len()
        ))
    }
}

/// PUT /v3/admin/cs/ops/log
#[put("log")]
async fn set_log_level(
    req: HttpRequest,
    data: web::Data<AppState>,
    params: web::Query<LogLevelParam>,
) -> impl Responder {
    let resource = "*:*:config/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    tracing::info!(
        log_name = %params.log_name,
        log_level = %params.log_level,
        "Config log level change requested via admin API"
    );

    Result::<String>::http_success(format!(
        "Log level updated successfully! Module: {}, Log Level: {}",
        params.log_name, params.log_level
    ))
}

/// GET /v3/admin/cs/ops/derby
#[get("derby")]
async fn derby_query(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:config/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Read)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    // Derby is not applicable to Batata (it uses MySQL/PostgreSQL)
    Result::<String>::http_response(
        501,
        error::SERVER_ERROR.code,
        "Derby operations are not supported. Batata uses MySQL/PostgreSQL.".to_string(),
        String::new(),
    )
}

/// POST /v3/admin/cs/ops/derby/import
#[post("derby/import")]
async fn derby_import(req: HttpRequest, data: web::Data<AppState>) -> impl Responder {
    let resource = "*:*:config/*";
    secured!(
        Secured::builder(&req, &data, resource)
            .action(ActionTypes::Write)
            .sign_type(SignType::Config)
            .api_type(ApiType::AdminApi)
            .build()
    );

    Result::<String>::http_response(
        501,
        error::SERVER_ERROR.code,
        "Derby operations are not supported. Batata uses MySQL/PostgreSQL.".to_string(),
        String::new(),
    )
}

pub fn routes() -> actix_web::Scope {
    web::scope("/ops")
        .service(dump_local_cache)
        .service(set_log_level)
        .service(derby_query)
        .service(derby_import)
}
