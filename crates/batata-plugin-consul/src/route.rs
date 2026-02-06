// Consul API route configuration
// Maps HTTP routes to handler functions

use actix_web::web;

use crate::{acl, agent, catalog, event, health, kv, lock, query, session, status};

/// Configure Consul Agent API routes (in-memory storage)
/// Returns a scope configured with all agent service endpoints
pub fn consul_agent_routes() -> actix_web::Scope {
    web::scope("/v1/agent")
        // Agent core endpoints
        .route("/self", web::get().to(agent::get_agent_self))
        .route("/members", web::get().to(agent::get_agent_members))
        .route("/host", web::get().to(agent::get_agent_host))
        .route("/version", web::get().to(agent::get_agent_version))
        .route("/join/{address}", web::put().to(agent::agent_join))
        .route("/leave", web::put().to(agent::agent_leave))
        .route(
            "/force-leave/{node}",
            web::put().to(agent::agent_force_leave),
        )
        .route("/reload", web::put().to(agent::agent_reload))
        .route("/maintenance", web::put().to(agent::agent_maintenance))
        .route("/metrics", web::get().to(agent::get_agent_metrics))
        .route("/monitor", web::get().to(agent::agent_monitor))
        .route(
            "/token/{token_type}",
            web::put().to(agent::update_agent_token),
        )
        // Service registration
        .route("/service/register", web::put().to(agent::register_service))
        // Service deregistration
        .route(
            "/service/deregister/{service_id}",
            web::put().to(agent::deregister_service),
        )
        // List all services
        .route("/services", web::get().to(agent::list_services))
        // Get single service
        .route("/service/{service_id}", web::get().to(agent::get_service))
        // Service maintenance mode
        .route(
            "/service/maintenance/{service_id}",
            web::put().to(agent::set_service_maintenance),
        )
        // Check endpoints
        .route("/check/register", web::put().to(health::register_check))
        .route(
            "/check/deregister/{check_id}",
            web::put().to(health::deregister_check),
        )
        .route("/check/pass/{check_id}", web::put().to(health::pass_check))
        .route("/check/warn/{check_id}", web::put().to(health::warn_check))
        .route("/check/fail/{check_id}", web::put().to(health::fail_check))
        .route(
            "/check/update/{check_id}",
            web::put().to(health::update_check),
        )
        .route("/checks", web::get().to(health::list_agent_checks))
}

/// Configure Consul Agent API routes (persistent database storage for checks)
/// Returns a scope configured with all agent service endpoints using database persistence for checks
pub fn consul_agent_routes_persistent() -> actix_web::Scope {
    web::scope("/v1/agent")
        // Agent core endpoints (same as non-persistent)
        .route("/self", web::get().to(agent::get_agent_self))
        .route("/members", web::get().to(agent::get_agent_members))
        .route("/host", web::get().to(agent::get_agent_host))
        .route("/version", web::get().to(agent::get_agent_version))
        .route("/join/{address}", web::put().to(agent::agent_join))
        .route("/leave", web::put().to(agent::agent_leave))
        .route(
            "/force-leave/{node}",
            web::put().to(agent::agent_force_leave),
        )
        .route("/reload", web::put().to(agent::agent_reload))
        .route("/maintenance", web::put().to(agent::agent_maintenance))
        .route("/metrics", web::get().to(agent::get_agent_metrics))
        .route("/monitor", web::get().to(agent::agent_monitor))
        .route(
            "/token/{token_type}",
            web::put().to(agent::update_agent_token),
        )
        // Service registration (same as non-persistent)
        .route("/service/register", web::put().to(agent::register_service))
        .route(
            "/service/deregister/{service_id}",
            web::put().to(agent::deregister_service),
        )
        .route("/services", web::get().to(agent::list_services))
        .route("/service/{service_id}", web::get().to(agent::get_service))
        .route(
            "/service/maintenance/{service_id}",
            web::put().to(agent::set_service_maintenance),
        )
        // Check endpoints - persistent versions
        .route(
            "/check/register",
            web::put().to(health::register_check_persistent),
        )
        .route(
            "/check/deregister/{check_id}",
            web::put().to(health::deregister_check_persistent),
        )
        .route(
            "/check/pass/{check_id}",
            web::put().to(health::pass_check_persistent),
        )
        .route(
            "/check/warn/{check_id}",
            web::put().to(health::warn_check_persistent),
        )
        .route(
            "/check/fail/{check_id}",
            web::put().to(health::fail_check_persistent),
        )
        .route(
            "/check/update/{check_id}",
            web::put().to(health::update_check_persistent),
        )
        .route(
            "/checks",
            web::get().to(health::list_agent_checks_persistent),
        )
}

/// Configure Consul Agent API routes (real cluster version)
/// Returns a scope configured with agent endpoints using real ServerMemberManager data
pub fn consul_agent_routes_real() -> actix_web::Scope {
    web::scope("/v1/agent")
        // Agent core endpoints - real cluster versions
        .route("/self", web::get().to(agent::get_agent_self_real))
        .route("/members", web::get().to(agent::get_agent_members_real))
        .route("/host", web::get().to(agent::get_agent_host))
        .route("/version", web::get().to(agent::get_agent_version))
        .route("/join/{address}", web::put().to(agent::agent_join))
        .route("/leave", web::put().to(agent::agent_leave))
        .route(
            "/force-leave/{node}",
            web::put().to(agent::agent_force_leave),
        )
        .route("/reload", web::put().to(agent::agent_reload))
        .route("/maintenance", web::put().to(agent::agent_maintenance))
        .route("/metrics", web::get().to(agent::get_agent_metrics_real)) // Real metrics with service/cluster counts
        .route("/monitor", web::get().to(agent::agent_monitor))
        .route(
            "/token/{token_type}",
            web::put().to(agent::update_agent_token),
        )
        // Service registration
        .route("/service/register", web::put().to(agent::register_service))
        .route(
            "/service/deregister/{service_id}",
            web::put().to(agent::deregister_service),
        )
        .route("/services", web::get().to(agent::list_services))
        .route("/service/{service_id}", web::get().to(agent::get_service))
        .route(
            "/service/maintenance/{service_id}",
            web::put().to(agent::set_service_maintenance),
        )
        // Check endpoints - use persistent versions for real cluster mode
        .route(
            "/check/register",
            web::put().to(health::register_check_persistent),
        )
        .route(
            "/check/deregister/{check_id}",
            web::put().to(health::deregister_check_persistent),
        )
        .route(
            "/check/pass/{check_id}",
            web::put().to(health::pass_check_persistent),
        )
        .route(
            "/check/warn/{check_id}",
            web::put().to(health::warn_check_persistent),
        )
        .route(
            "/check/fail/{check_id}",
            web::put().to(health::fail_check_persistent),
        )
        .route(
            "/check/update/{check_id}",
            web::put().to(health::update_check_persistent),
        )
        .route(
            "/checks",
            web::get().to(health::list_agent_checks_persistent),
        )
}

/// Configure Consul Health API routes (in-memory storage)
/// Returns a scope configured with all health check endpoints
pub fn consul_health_routes() -> actix_web::Scope {
    web::scope("/v1/health")
        // Service health (most commonly used)
        .route(
            "/service/{service}",
            web::get().to(health::get_service_health),
        )
        // Service checks
        .route(
            "/checks/{service}",
            web::get().to(health::get_service_checks),
        )
        // Checks by state
        .route("/state/{state}", web::get().to(health::get_checks_by_state))
        // Node checks
        .route("/node/{node}", web::get().to(health::get_node_checks))
        // Connect/mesh health (stub)
        .route(
            "/connect/{service}",
            web::get().to(health::get_connect_health),
        )
        // Ingress gateway health (stub)
        .route(
            "/ingress/{service}",
            web::get().to(health::get_ingress_health),
        )
}

/// Configure Consul Health API routes (persistent database storage)
/// Returns a scope configured with all health check endpoints using database persistence
pub fn consul_health_routes_persistent() -> actix_web::Scope {
    web::scope("/v1/health")
        // Service health (most commonly used)
        .route(
            "/service/{service}",
            web::get().to(health::get_service_health_persistent),
        )
        // Service checks
        .route(
            "/checks/{service}",
            web::get().to(health::get_service_checks_persistent),
        )
        // Checks by state
        .route(
            "/state/{state}",
            web::get().to(health::get_checks_by_state_persistent),
        )
        // Node checks
        .route(
            "/node/{node}",
            web::get().to(health::get_node_checks_persistent),
        )
        // Connect/mesh health (stub - same as regular)
        .route(
            "/connect/{service}",
            web::get().to(health::get_connect_health),
        )
        // Ingress gateway health (stub)
        .route(
            "/ingress/{service}",
            web::get().to(health::get_ingress_health),
        )
}

/// Configure Consul KV Store API routes (in-memory storage)
/// Returns a scope configured with all key-value store endpoints
pub fn consul_kv_routes() -> actix_web::Scope {
    web::scope("/v1")
        // KV Store - use wildcard for nested keys
        .route("/kv/{key:.*}", web::get().to(kv::get_kv))
        .route("/kv/{key:.*}", web::put().to(kv::put_kv))
        .route("/kv/{key:.*}", web::delete().to(kv::delete_kv))
        // Export/Import
        .route("/kv/export", web::get().to(kv::export_kv))
        .route("/kv/import", web::post().to(kv::import_kv))
        // Transaction
        .route("/txn", web::put().to(kv::txn))
}

/// Configure Consul KV Store API routes (persistent database storage)
/// Returns a scope configured with all key-value store endpoints using database persistence
pub fn consul_kv_routes_persistent() -> actix_web::Scope {
    web::scope("/v1")
        // KV Store - use wildcard for nested keys
        .route("/kv/{key:.*}", web::get().to(kv::get_kv_persistent))
        .route("/kv/{key:.*}", web::put().to(kv::put_kv_persistent))
        .route("/kv/{key:.*}", web::delete().to(kv::delete_kv_persistent))
        // Export/Import
        .route("/kv/export", web::get().to(kv::export_kv_persistent))
        .route("/kv/import", web::post().to(kv::import_kv_persistent))
        // Transaction
        .route("/txn", web::put().to(kv::txn_persistent))
}

/// Configure Consul Catalog API routes
/// Returns a scope configured with all catalog endpoints
pub fn consul_catalog_routes() -> actix_web::Scope {
    web::scope("/v1/catalog")
        // List all datacenters
        .route("/datacenters", web::get().to(catalog::list_datacenters))
        // List all services
        .route("/services", web::get().to(catalog::list_services))
        // Get service instances
        .route("/service/{service}", web::get().to(catalog::get_service))
        // List all nodes
        .route("/nodes", web::get().to(catalog::list_nodes))
        // Get node details
        .route("/node/{node}", web::get().to(catalog::get_node))
        // Register to catalog
        .route("/register", web::put().to(catalog::register))
        // Deregister from catalog
        .route("/deregister", web::put().to(catalog::deregister))
        // Connect/mesh service instances (stub)
        .route(
            "/connect/{service}",
            web::get().to(catalog::get_connect_service),
        )
        // Node services (alternative format)
        .route(
            "/node-services/{node}",
            web::get().to(catalog::get_node_services),
        )
        // Gateway services (stub)
        .route(
            "/gateway-services/{gateway}",
            web::get().to(catalog::get_gateway_services),
        )
}

/// Configure Consul ACL API routes
/// Returns a scope configured with all ACL management endpoints
pub fn consul_acl_routes() -> actix_web::Scope {
    web::scope("/v1/acl")
        // Bootstrap and auth endpoints (client-facing)
        .route("/bootstrap", web::put().to(acl::acl_bootstrap))
        .route("/login", web::post().to(acl::acl_login))
        .route("/logout", web::post().to(acl::acl_logout))
        // Token management
        .route("/tokens", web::get().to(acl::list_tokens))
        .route("/token/self", web::get().to(acl::get_token_self))
        .route("/token", web::put().to(acl::create_token))
        .route(
            "/token/{accessor_id}/clone",
            web::put().to(acl::clone_token),
        )
        .route("/token/{accessor_id}", web::get().to(acl::get_token))
        .route("/token/{accessor_id}", web::delete().to(acl::delete_token))
        // Policy management
        .route("/policies", web::get().to(acl::list_policies))
        .route("/policy", web::put().to(acl::create_policy))
        .route("/policy/{id}", web::get().to(acl::get_policy))
        // Role management
        .route("/roles", web::get().to(acl::list_roles))
        .route("/role", web::put().to(acl::create_role))
        .route("/role/{id}", web::get().to(acl::get_role))
        .route("/role/{id}", web::put().to(acl::update_role))
        .route("/role/{id}", web::delete().to(acl::delete_role))
        // Auth Method management
        .route("/auth-methods", web::get().to(acl::list_auth_methods))
        .route("/auth-method", web::put().to(acl::create_auth_method))
        .route("/auth-method/{name}", web::get().to(acl::get_auth_method))
        .route(
            "/auth-method/{name}",
            web::put().to(acl::update_auth_method),
        )
        .route(
            "/auth-method/{name}",
            web::delete().to(acl::delete_auth_method),
        )
}

/// Configure Consul ACL API routes (persistent database storage)
/// Returns a scope configured with all ACL management endpoints using database persistence
pub fn consul_acl_routes_persistent() -> actix_web::Scope {
    web::scope("/v1/acl")
        // Bootstrap and auth endpoints (client-facing) - persistent versions
        .route("/bootstrap", web::put().to(acl::acl_bootstrap_persistent))
        .route("/login", web::post().to(acl::acl_login_persistent))
        .route("/logout", web::post().to(acl::acl_logout_persistent))
        // Token management - persistent versions
        .route("/tokens", web::get().to(acl::list_tokens_persistent))
        .route("/token/self", web::get().to(acl::get_token_self_persistent))
        .route("/token", web::put().to(acl::create_token_persistent))
        .route(
            "/token/{accessor_id}/clone",
            web::put().to(acl::clone_token_persistent),
        )
        .route(
            "/token/{accessor_id}",
            web::get().to(acl::get_token_persistent),
        )
        .route(
            "/token/{accessor_id}",
            web::delete().to(acl::delete_token_persistent),
        )
        // Policy management - persistent versions
        .route("/policies", web::get().to(acl::list_policies_persistent))
        .route("/policy", web::put().to(acl::create_policy_persistent))
        .route("/policy/{id}", web::get().to(acl::get_policy_persistent))
        // Role management - persistent versions
        .route("/roles", web::get().to(acl::list_roles_persistent))
        .route("/role", web::put().to(acl::create_role_persistent))
        .route("/role/{id}", web::get().to(acl::get_role_persistent))
        .route("/role/{id}", web::put().to(acl::update_role_persistent))
        .route("/role/{id}", web::delete().to(acl::delete_role_persistent))
        // Auth Method management - persistent versions
        .route(
            "/auth-methods",
            web::get().to(acl::list_auth_methods_persistent),
        )
        .route(
            "/auth-method",
            web::put().to(acl::create_auth_method_persistent),
        )
        .route(
            "/auth-method/{name}",
            web::get().to(acl::get_auth_method_persistent),
        )
        .route(
            "/auth-method/{name}",
            web::put().to(acl::update_auth_method_persistent),
        )
        .route(
            "/auth-method/{name}",
            web::delete().to(acl::delete_auth_method_persistent),
        )
}

/// Configure Consul Session API routes
/// Returns a scope configured with all session management endpoints
pub fn consul_session_routes() -> actix_web::Scope {
    web::scope("/v1/session")
        .route("/create", web::put().to(session::create_session))
        .route("/destroy/{uuid}", web::put().to(session::destroy_session))
        .route("/info/{uuid}", web::get().to(session::get_session_info))
        .route("/list", web::get().to(session::list_sessions))
        .route("/node/{node}", web::get().to(session::list_node_sessions))
        .route("/renew/{uuid}", web::put().to(session::renew_session))
}

/// Configure Consul Session API routes (persistent database storage)
/// Returns a scope configured with all session management endpoints using database persistence
pub fn consul_session_routes_persistent() -> actix_web::Scope {
    web::scope("/v1/session")
        .route("/create", web::put().to(session::create_session_persistent))
        .route(
            "/destroy/{uuid}",
            web::put().to(session::destroy_session_persistent),
        )
        .route(
            "/info/{uuid}",
            web::get().to(session::get_session_info_persistent),
        )
        .route("/list", web::get().to(session::list_sessions_persistent))
        .route(
            "/node/{node}",
            web::get().to(session::list_node_sessions_persistent),
        )
        .route(
            "/renew/{uuid}",
            web::put().to(session::renew_session_persistent),
        )
}

/// Configure Consul Status API routes (fixed/fallback version)
/// Returns a scope configured with cluster status endpoints
pub fn consul_status_routes() -> actix_web::Scope {
    web::scope("/v1/status")
        .route("/leader", web::get().to(status::get_leader))
        .route("/peers", web::get().to(status::get_peers))
}

/// Configure Consul Status API routes (real cluster version)
/// Returns a scope configured with cluster status endpoints using ServerMemberManager
pub fn consul_status_routes_real() -> actix_web::Scope {
    web::scope("/v1/status")
        .route("/leader", web::get().to(status::get_leader_real))
        .route("/peers", web::get().to(status::get_peers_real))
}

/// Configure Consul Event API routes
/// Returns a scope configured with event endpoints
pub fn consul_event_routes() -> actix_web::Scope {
    web::scope("/v1/event")
        .route("/fire/{name}", web::put().to(event::fire_event))
        .route("/list", web::get().to(event::list_events))
}

/// Configure Consul Event API routes (persistent database storage)
/// Returns a scope configured with event endpoints using database persistence
pub fn consul_event_routes_persistent() -> actix_web::Scope {
    web::scope("/v1/event")
        .route("/fire/{name}", web::put().to(event::fire_event_persistent))
        .route("/list", web::get().to(event::list_events_persistent))
}

/// Configure Consul Prepared Query API routes
/// Returns a scope configured with prepared query endpoints
pub fn consul_query_routes() -> actix_web::Scope {
    web::scope("/v1/query")
        .route("", web::post().to(query::create_query))
        .route("", web::get().to(query::list_queries))
        .route("/{uuid}", web::get().to(query::get_query))
        .route("/{uuid}", web::put().to(query::update_query))
        .route("/{uuid}", web::delete().to(query::delete_query))
        .route("/{uuid}/execute", web::get().to(query::execute_query))
        .route("/{uuid}/explain", web::get().to(query::explain_query))
}

/// Configure Consul Prepared Query API routes (persistent database storage)
/// Returns a scope configured with prepared query endpoints using database persistence
pub fn consul_query_routes_persistent() -> actix_web::Scope {
    web::scope("/v1/query")
        .route("", web::post().to(query::create_query_persistent))
        .route("", web::get().to(query::list_queries_persistent))
        .route("/{uuid}", web::get().to(query::get_query_persistent))
        .route("/{uuid}", web::put().to(query::update_query_persistent))
        .route("/{uuid}", web::delete().to(query::delete_query_persistent))
        .route(
            "/{uuid}/execute",
            web::get().to(query::execute_query_persistent),
        )
        .route(
            "/{uuid}/explain",
            web::get().to(query::explain_query_persistent),
        )
}

/// Configure Consul Lock API routes
/// Returns a scope configured with lock and semaphore endpoints
pub fn consul_lock_routes() -> actix_web::Scope {
    web::scope("/v1")
        // Lock endpoints
        .route("/lock/acquire", web::post().to(lock::acquire_lock))
        .route("/lock/release/{key:.*}", web::put().to(lock::release_lock))
        .route("/lock/{key:.*}", web::get().to(lock::get_lock))
        .route("/lock/{key:.*}", web::delete().to(lock::destroy_lock))
        .route("/lock/renew/{key:.*}", web::put().to(lock::renew_lock))
        // Semaphore endpoints
        .route("/semaphore/acquire", web::post().to(lock::acquire_semaphore))
        .route(
            "/semaphore/release/{prefix:.*}",
            web::put().to(lock::release_semaphore),
        )
        .route("/semaphore/{prefix:.*}", web::get().to(lock::get_semaphore))
}

/// Configure all Consul API routes (in-memory storage)
/// This is the main entry point for integrating Consul API into the server
/// Note: Data is lost on server restart. For production use, use `consul_routes_persistent()`.
pub fn consul_routes() -> actix_web::Scope {
    web::scope("")
        .service(consul_agent_routes())
        .service(consul_health_routes())
        .service(consul_kv_routes())
        .service(consul_catalog_routes())
        .service(consul_acl_routes())
        .service(consul_session_routes())
        .service(consul_status_routes())
        .service(consul_event_routes())
        .service(consul_query_routes())
        .service(consul_lock_routes())
}

/// Configure all Consul API routes with database persistence
/// This is the recommended entry point for production use.
/// All data is persisted via ConfigService and survives server restarts.
///
/// Required dependencies to inject via `web::Data`:
/// - `AclService` or `AclServicePersistent`
/// - `ConsulAgentService`
/// - `ConsulKVServicePersistent`
/// - `ConsulHealthServicePersistent`
/// - `ConsulSessionServicePersistent`
/// - `ConsulEventServicePersistent`
/// - `ConsulQueryServicePersistent`
/// - `Arc<NamingService>`
/// - `Arc<DatabaseConnection>`
pub fn consul_routes_persistent() -> actix_web::Scope {
    web::scope("")
        .service(consul_agent_routes_persistent())
        .service(consul_health_routes_persistent())
        .service(consul_kv_routes_persistent())
        .service(consul_catalog_routes())
        .service(consul_acl_routes_persistent())
        .service(consul_session_routes_persistent())
        .service(consul_status_routes())
        .service(consul_event_routes_persistent())
        .service(consul_query_routes_persistent())
}

/// Configure all Consul API routes with full features
/// This is the most complete entry point with:
/// - Database persistence for all data
/// - Real cluster information from ServerMemberManager
/// - Real metrics from system and NamingService
///
/// Required dependencies to inject via `web::Data`:
/// - All dependencies from `consul_routes_persistent()`
/// - `Arc<ServerMemberManager>`
pub fn consul_routes_full() -> actix_web::Scope {
    web::scope("")
        .service(consul_agent_routes_real()) // Real cluster info + real metrics
        .service(consul_health_routes_persistent())
        .service(consul_kv_routes_persistent())
        .service(consul_catalog_routes())
        .service(consul_acl_routes_persistent())
        .service(consul_session_routes_persistent())
        .service(consul_status_routes_real()) // Real cluster status
        .service(consul_event_routes_persistent())
        .service(consul_query_routes_persistent())
}
