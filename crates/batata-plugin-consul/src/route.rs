// Consul API route configuration
// Maps HTTP routes to handler functions

use actix_web::web;

use crate::{
    acl, agent, catalog, config_entry, connect, connect_ca, coordinate, event, health, internal,
    kv, lock, operator, peering, query, session, snapshot, status,
};

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
        // Agent health service endpoints
        .route(
            "/health/service/id/{service_id}",
            web::get().to(agent::agent_health_service_by_id),
        )
        .route(
            "/health/service/name/{service_name}",
            web::get().to(agent::agent_health_service_by_name),
        )
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
        // Check endpoints (unified via InstanceCheckRegistry)
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
        // Agent health service endpoints
        .route(
            "/health/service/id/{service_id}",
            web::get().to(agent::agent_health_service_by_id),
        )
        .route(
            "/health/service/name/{service_name}",
            web::get().to(agent::agent_health_service_by_name),
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
        .route("/metrics", web::get().to(agent::get_agent_metrics_real))
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
        // Check endpoints (unified via InstanceCheckRegistry)
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
        // Agent health service endpoints
        .route(
            "/health/service/id/{service_id}",
            web::get().to(agent::agent_health_service_by_id),
        )
        .route(
            "/health/service/name/{service_name}",
            web::get().to(agent::agent_health_service_by_name),
        )
}

/// Configure Consul Health API routes (in-memory storage)
pub fn consul_health_routes() -> actix_web::Scope {
    web::scope("/v1/health")
        .route(
            "/service/{service}",
            web::get().to(health::get_service_health),
        )
        .route(
            "/checks/{service}",
            web::get().to(health::get_service_checks),
        )
        .route("/state/{state}", web::get().to(health::get_checks_by_state))
        .route("/node/{node}", web::get().to(health::get_node_checks))
        .route(
            "/connect/{service}",
            web::get().to(health::get_connect_health),
        )
        .route(
            "/ingress/{service}",
            web::get().to(health::get_ingress_health),
        )
}

/// Configure Consul KV Store API routes (in-memory storage)
pub fn consul_kv_routes() -> actix_web::Scope {
    web::scope("/v1")
        .route("/kv/{key:.*}", web::get().to(kv::get_kv))
        .route("/kv/{key:.*}", web::put().to(kv::put_kv))
        .route("/kv/{key:.*}", web::delete().to(kv::delete_kv))
        .route("/kv/export", web::get().to(kv::export_kv))
        .route("/kv/import", web::post().to(kv::import_kv))
        .route("/txn", web::put().to(kv::txn))
}

/// Configure Consul Catalog API routes
pub fn consul_catalog_routes() -> actix_web::Scope {
    web::scope("/v1/catalog")
        .route("/datacenters", web::get().to(catalog::list_datacenters))
        .route("/services", web::get().to(catalog::list_services))
        .route("/service/{service}", web::get().to(catalog::get_service))
        .route("/nodes", web::get().to(catalog::list_nodes))
        .route("/node/{node}", web::get().to(catalog::get_node))
        .route("/register", web::put().to(catalog::register))
        .route("/deregister", web::put().to(catalog::deregister))
        .route(
            "/connect/{service}",
            web::get().to(catalog::get_connect_service),
        )
        .route(
            "/node-services/{node}",
            web::get().to(catalog::get_node_services),
        )
        .route(
            "/gateway-services/{gateway}",
            web::get().to(catalog::get_gateway_services),
        )
}

/// Configure Consul UI API routes (existing, kept for backwards compatibility)
pub fn consul_ui_routes() -> actix_web::Scope {
    web::scope("/v1/internal/ui").route("/services", web::get().to(catalog::ui_services))
}

/// Configure all Consul Internal API routes (UI, Federation, VIP, ACL Authorize)
pub fn consul_internal_routes() -> actix_web::Scope {
    web::scope("/v1/internal")
        // UI endpoints
        .route("/ui/services", web::get().to(catalog::ui_services))
        .route("/ui/nodes", web::get().to(internal::ui_nodes))
        .route("/ui/node/{node}", web::get().to(internal::ui_node_info))
        .route(
            "/ui/exported-services",
            web::get().to(internal::ui_exported_services),
        )
        .route(
            "/ui/catalog-overview",
            web::get().to(internal::ui_catalog_overview),
        )
        .route(
            "/ui/gateway-services-nodes/{gateway}",
            web::get().to(internal::ui_gateway_services_nodes),
        )
        .route(
            "/ui/gateway-intentions/{gateway}",
            web::get().to(internal::ui_gateway_intentions),
        )
        .route(
            "/ui/service-topology/{service}",
            web::get().to(internal::ui_service_topology),
        )
        .route(
            "/ui/metrics-proxy/{path:.*}",
            web::get().to(internal::ui_metrics_proxy),
        )
        // Federation state endpoints
        .route(
            "/federation-states",
            web::get().to(internal::federation_state_list),
        )
        .route(
            "/federation-states/mesh-gateways",
            web::get().to(internal::federation_state_mesh_gateways),
        )
        .route(
            "/federation-state/{dc}",
            web::get().to(internal::federation_state_get),
        )
        // Service virtual IP
        .route(
            "/service-virtual-ip",
            web::put().to(internal::assign_service_virtual_ip),
        )
        // ACL authorize
        .route("/acl/authorize", web::post().to(acl::acl_authorize))
}

/// Configure Consul ACL API routes (in-memory)
pub fn consul_acl_routes() -> actix_web::Scope {
    web::scope("/v1/acl")
        // Bootstrap and auth endpoints
        .route("/bootstrap", web::put().to(acl::acl_bootstrap))
        .route("/login", web::post().to(acl::acl_login))
        .route("/logout", web::post().to(acl::acl_logout))
        // Replication status
        .route("/replication", web::get().to(acl::acl_replication))
        // Authorization
        .route("/authorize", web::post().to(acl::acl_authorize))
        // Token management
        .route("/tokens", web::get().to(acl::list_tokens))
        .route("/token/self", web::get().to(acl::get_token_self))
        .route("/token", web::put().to(acl::create_token))
        .route(
            "/token/{accessor_id}/clone",
            web::put().to(acl::clone_token),
        )
        .route("/token/{accessor_id}", web::get().to(acl::get_token))
        .route("/token/{accessor_id}", web::put().to(acl::update_token))
        .route("/token/{accessor_id}", web::delete().to(acl::delete_token))
        // Policy management
        .route("/policies", web::get().to(acl::list_policies))
        .route("/policy", web::put().to(acl::create_policy))
        .route(
            "/policy/name/{name}",
            web::get().to(acl::get_policy_by_name),
        )
        .route("/policy/{id}", web::get().to(acl::get_policy))
        .route("/policy/{id}", web::put().to(acl::update_policy))
        .route("/policy/{id}", web::delete().to(acl::delete_policy))
        // Role management
        .route("/roles", web::get().to(acl::list_roles))
        .route("/role", web::put().to(acl::create_role))
        .route("/role/name/{name}", web::get().to(acl::get_role_by_name))
        .route("/role/{id}", web::get().to(acl::get_role))
        .route("/role/{id}", web::put().to(acl::update_role))
        .route("/role/{id}", web::delete().to(acl::delete_role))
        // Binding Rule management
        .route("/binding-rules", web::get().to(acl::list_binding_rules))
        .route("/binding-rule", web::put().to(acl::create_binding_rule))
        .route("/binding-rule/{id}", web::get().to(acl::get_binding_rule))
        .route(
            "/binding-rule/{id}",
            web::put().to(acl::update_binding_rule),
        )
        .route(
            "/binding-rule/{id}",
            web::delete().to(acl::delete_binding_rule),
        )
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
        // Templated policies
        .route(
            "/templated-policies",
            web::get().to(acl::list_templated_policies),
        )
        .route(
            "/templated-policy/name/{name}",
            web::get().to(acl::get_templated_policy),
        )
        .route(
            "/templated-policy/preview/{name}",
            web::post().to(acl::preview_templated_policy),
        )
}

/// Configure Consul Session API routes
pub fn consul_session_routes() -> actix_web::Scope {
    web::scope("/v1/session")
        .route("/create", web::put().to(session::create_session))
        .route("/destroy/{uuid}", web::put().to(session::destroy_session))
        .route("/info/{uuid}", web::get().to(session::get_session_info))
        .route("/list", web::get().to(session::list_sessions))
        .route("/node/{node}", web::get().to(session::list_node_sessions))
        .route("/renew/{uuid}", web::put().to(session::renew_session))
}

/// Configure Consul Status API routes (fixed/fallback version)
pub fn consul_status_routes() -> actix_web::Scope {
    web::scope("/v1/status")
        .route("/leader", web::get().to(status::get_leader))
        .route("/peers", web::get().to(status::get_peers))
}

/// Configure Consul Status API routes (real cluster version)
pub fn consul_status_routes_real() -> actix_web::Scope {
    web::scope("/v1/status")
        .route("/leader", web::get().to(status::get_leader_real))
        .route("/peers", web::get().to(status::get_peers_real))
}

/// Configure Consul Event API routes
pub fn consul_event_routes() -> actix_web::Scope {
    web::scope("/v1/event")
        .route("/fire/{name}", web::put().to(event::fire_event))
        .route("/list", web::get().to(event::list_events))
}

/// Configure Consul Event API routes (persistent database storage)
pub fn consul_event_routes_persistent() -> actix_web::Scope {
    web::scope("/v1/event")
        .route("/fire/{name}", web::put().to(event::fire_event_persistent))
        .route("/list", web::get().to(event::list_events_persistent))
}

/// Configure Consul Prepared Query API routes
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

/// Configure Consul Lock API routes
pub fn consul_lock_routes() -> actix_web::Scope {
    web::scope("/v1")
        .route("/lock/acquire", web::post().to(lock::acquire_lock))
        .route("/lock/release/{key:.*}", web::put().to(lock::release_lock))
        .route("/lock/{key:.*}", web::get().to(lock::get_lock))
        .route("/lock/{key:.*}", web::delete().to(lock::destroy_lock))
        .route("/lock/renew/{key:.*}", web::put().to(lock::renew_lock))
        .route(
            "/semaphore/acquire",
            web::post().to(lock::acquire_semaphore),
        )
        .route(
            "/semaphore/release/{prefix:.*}",
            web::put().to(lock::release_semaphore),
        )
        .route("/semaphore/{prefix:.*}", web::get().to(lock::get_semaphore))
}

// ============================================================================
// New Tier 1 Routes
// ============================================================================

/// Configure Consul Snapshot API routes (in-memory)
pub fn consul_snapshot_routes() -> actix_web::Scope {
    web::scope("/v1")
        .route("/snapshot", web::get().to(snapshot::save_snapshot))
        .route("/snapshot", web::put().to(snapshot::restore_snapshot))
}

/// Configure Consul Snapshot API routes (persistent)
pub fn consul_snapshot_routes_persistent() -> actix_web::Scope {
    web::scope("/v1")
        .route(
            "/snapshot",
            web::get().to(snapshot::save_snapshot_persistent),
        )
        .route(
            "/snapshot",
            web::put().to(snapshot::restore_snapshot_persistent),
        )
}

/// Configure Consul Operator API routes (in-memory)
pub fn consul_operator_routes() -> actix_web::Scope {
    web::scope("/v1/operator")
        // Raft
        .route(
            "/raft/configuration",
            web::get().to(operator::get_raft_configuration),
        )
        .route(
            "/raft/transfer-leader",
            web::post().to(operator::transfer_leader),
        )
        .route("/raft/peer", web::delete().to(operator::remove_raft_peer))
        // Autopilot
        .route(
            "/autopilot/configuration",
            web::get().to(operator::get_autopilot_configuration),
        )
        .route(
            "/autopilot/configuration",
            web::put().to(operator::set_autopilot_configuration),
        )
        .route(
            "/autopilot/health",
            web::get().to(operator::get_autopilot_health),
        )
        .route(
            "/autopilot/state",
            web::get().to(operator::get_autopilot_state),
        )
        // Keyring
        .route("/keyring", web::get().to(operator::keyring_list))
        .route("/keyring", web::post().to(operator::keyring_install))
        .route("/keyring", web::put().to(operator::keyring_use))
        .route("/keyring", web::delete().to(operator::keyring_remove))
        // Usage & Utilization
        .route("/usage", web::get().to(operator::get_operator_usage))
        .route(
            "/utilization",
            web::get().to(operator::get_operator_utilization),
        )
}

/// Configure Consul Operator API routes (real cluster)
pub fn consul_operator_routes_real() -> actix_web::Scope {
    web::scope("/v1/operator")
        // Raft
        .route(
            "/raft/configuration",
            web::get().to(operator::get_raft_configuration_real),
        )
        .route(
            "/raft/transfer-leader",
            web::post().to(operator::transfer_leader_real),
        )
        .route(
            "/raft/peer",
            web::delete().to(operator::remove_raft_peer_real),
        )
        // Autopilot
        .route(
            "/autopilot/configuration",
            web::get().to(operator::get_autopilot_configuration_real),
        )
        .route(
            "/autopilot/configuration",
            web::put().to(operator::set_autopilot_configuration_real),
        )
        .route(
            "/autopilot/health",
            web::get().to(operator::get_autopilot_health_real),
        )
        .route(
            "/autopilot/state",
            web::get().to(operator::get_autopilot_state_real),
        )
        // Keyring
        .route("/keyring", web::get().to(operator::keyring_list_real))
        .route("/keyring", web::post().to(operator::keyring_install_real))
        .route("/keyring", web::put().to(operator::keyring_use_real))
        .route("/keyring", web::delete().to(operator::keyring_remove_real))
        // Usage & Utilization
        .route("/usage", web::get().to(operator::get_operator_usage_real))
        .route(
            "/utilization",
            web::get().to(operator::get_operator_utilization_real),
        )
}

/// Configure Consul Config Entry API routes (in-memory)
pub fn consul_config_entry_routes() -> actix_web::Scope {
    web::scope("/v1/config")
        .route("", web::put().to(config_entry::apply_config_entry))
        .route(
            "/{kind}/{name}",
            web::get().to(config_entry::get_config_entry),
        )
        .route(
            "/{kind}/{name}",
            web::delete().to(config_entry::delete_config_entry),
        )
        .route("/{kind}", web::get().to(config_entry::list_config_entries))
}

/// Configure Consul Config Entry API routes (persistent)
pub fn consul_config_entry_routes_persistent() -> actix_web::Scope {
    web::scope("/v1/config")
        .route(
            "",
            web::put().to(config_entry::apply_config_entry_persistent),
        )
        .route(
            "/{kind}/{name}",
            web::get().to(config_entry::get_config_entry_persistent),
        )
        .route(
            "/{kind}/{name}",
            web::delete().to(config_entry::delete_config_entry_persistent),
        )
        .route(
            "/{kind}",
            web::get().to(config_entry::list_config_entries_persistent),
        )
}

// ============================================================================
// New Tier 2 Routes
// ============================================================================

/// Configure Consul Coordinate API routes (in-memory)
pub fn consul_coordinate_routes() -> actix_web::Scope {
    web::scope("/v1/coordinate")
        .route(
            "/datacenters",
            web::get().to(coordinate::get_coordinate_datacenters),
        )
        .route("/nodes", web::get().to(coordinate::get_coordinate_nodes))
        .route(
            "/node/{node}",
            web::get().to(coordinate::get_coordinate_node),
        )
        .route("/update", web::put().to(coordinate::update_coordinate))
}

/// Configure Consul Coordinate API routes (persistent)
pub fn consul_coordinate_routes_persistent() -> actix_web::Scope {
    web::scope("/v1/coordinate")
        .route(
            "/datacenters",
            web::get().to(coordinate::get_coordinate_datacenters_persistent),
        )
        .route(
            "/nodes",
            web::get().to(coordinate::get_coordinate_nodes_persistent),
        )
        .route(
            "/node/{node}",
            web::get().to(coordinate::get_coordinate_node_persistent),
        )
        .route(
            "/update",
            web::put().to(coordinate::update_coordinate_persistent),
        )
}

/// Configure Consul Peering API routes (in-memory)
pub fn consul_peering_routes() -> actix_web::Scope {
    web::scope("/v1")
        .route(
            "/peering/token",
            web::post().to(peering::generate_peering_token),
        )
        .route(
            "/peering/establish",
            web::post().to(peering::establish_peering),
        )
        .route("/peering/{name}", web::get().to(peering::get_peering))
        .route("/peering/{name}", web::delete().to(peering::delete_peering))
        .route("/peerings", web::get().to(peering::list_peerings))
}

/// Configure Consul Peering API routes (persistent)
pub fn consul_peering_routes_persistent() -> actix_web::Scope {
    web::scope("/v1")
        .route(
            "/peering/token",
            web::post().to(peering::generate_peering_token_persistent),
        )
        .route(
            "/peering/establish",
            web::post().to(peering::establish_peering_persistent),
        )
        .route(
            "/peering/{name}",
            web::get().to(peering::get_peering_persistent),
        )
        .route(
            "/peering/{name}",
            web::delete().to(peering::delete_peering_persistent),
        )
        .route(
            "/peerings",
            web::get().to(peering::list_peerings_persistent),
        )
}

/// Configure Consul Connect/Service Mesh API routes (in-memory)
pub fn consul_connect_routes() -> actix_web::Scope {
    web::scope("/v1")
        .route(
            "/discovery-chain/{service}",
            web::get().to(connect::get_discovery_chain),
        )
        .route(
            "/discovery-chain/{service}",
            web::post().to(connect::post_discovery_chain),
        )
        .route(
            "/exported-services",
            web::get().to(connect::list_exported_services),
        )
        .route(
            "/imported-services",
            web::get().to(connect::list_imported_services),
        )
}

/// Configure Consul Connect/Service Mesh API routes (persistent)
pub fn consul_connect_routes_persistent() -> actix_web::Scope {
    web::scope("/v1")
        .route(
            "/discovery-chain/{service}",
            web::get().to(connect::get_discovery_chain_persistent),
        )
        .route(
            "/discovery-chain/{service}",
            web::post().to(connect::post_discovery_chain_persistent),
        )
        .route(
            "/exported-services",
            web::get().to(connect::list_exported_services_persistent),
        )
        .route(
            "/imported-services",
            web::get().to(connect::list_imported_services_persistent),
        )
}

// ============================================================================
// Tier 3 Routes: Connect CA and Intentions
// ============================================================================

/// Configure Consul Connect CA and Intentions routes (in-memory)
pub fn consul_connect_ca_routes() -> actix_web::Scope {
    web::scope("/v1")
        // CA endpoints
        .route("/connect/ca/roots", web::get().to(connect_ca::get_ca_roots))
        .route(
            "/connect/ca/configuration",
            web::get().to(connect_ca::get_ca_configuration),
        )
        .route(
            "/connect/ca/configuration",
            web::put().to(connect_ca::set_ca_configuration),
        )
        // Leaf cert
        .route(
            "/agent/connect/ca/leaf/{service}",
            web::get().to(connect_ca::get_leaf_cert),
        )
        // Agent authorize
        .route(
            "/agent/connect/authorize",
            web::post().to(connect_ca::connect_authorize),
        )
        // Intentions - check/match must come before {id} to avoid route conflicts
        .route(
            "/connect/intentions/check",
            web::get().to(connect_ca::check_intention),
        )
        .route(
            "/connect/intentions/match",
            web::get().to(connect_ca::match_intentions),
        )
        // Intentions - exact must come before {id}
        .route(
            "/connect/intentions/exact",
            web::get().to(connect_ca::get_intention_exact),
        )
        .route(
            "/connect/intentions/exact",
            web::put().to(connect_ca::upsert_intention_exact),
        )
        .route(
            "/connect/intentions/exact",
            web::delete().to(connect_ca::delete_intention_exact),
        )
        // Intentions CRUD
        .route(
            "/connect/intentions",
            web::get().to(connect_ca::list_intentions),
        )
        .route(
            "/connect/intentions",
            web::post().to(connect_ca::create_intention),
        )
        .route(
            "/connect/intentions/{id}",
            web::get().to(connect_ca::get_intention),
        )
        .route(
            "/connect/intentions/{id}",
            web::put().to(connect_ca::update_intention),
        )
        .route(
            "/connect/intentions/{id}",
            web::delete().to(connect_ca::delete_intention),
        )
}

/// Configure Consul Connect CA and Intentions routes (persistent)
pub fn consul_connect_ca_routes_persistent() -> actix_web::Scope {
    web::scope("/v1")
        // CA endpoints
        .route(
            "/connect/ca/roots",
            web::get().to(connect_ca::get_ca_roots_persistent),
        )
        .route(
            "/connect/ca/configuration",
            web::get().to(connect_ca::get_ca_configuration_persistent),
        )
        .route(
            "/connect/ca/configuration",
            web::put().to(connect_ca::set_ca_configuration_persistent),
        )
        // Leaf cert
        .route(
            "/agent/connect/ca/leaf/{service}",
            web::get().to(connect_ca::get_leaf_cert_persistent),
        )
        // Agent authorize
        .route(
            "/agent/connect/authorize",
            web::post().to(connect_ca::connect_authorize_persistent),
        )
        // Intentions - check/match must come before {id}
        .route(
            "/connect/intentions/check",
            web::get().to(connect_ca::check_intention_persistent),
        )
        .route(
            "/connect/intentions/match",
            web::get().to(connect_ca::match_intentions_persistent),
        )
        // Intentions - exact must come before {id}
        .route(
            "/connect/intentions/exact",
            web::get().to(connect_ca::get_intention_exact_persistent),
        )
        .route(
            "/connect/intentions/exact",
            web::put().to(connect_ca::upsert_intention_exact_persistent),
        )
        .route(
            "/connect/intentions/exact",
            web::delete().to(connect_ca::delete_intention_exact_persistent),
        )
        // Intentions CRUD
        .route(
            "/connect/intentions",
            web::get().to(connect_ca::list_intentions_persistent),
        )
        .route(
            "/connect/intentions",
            web::post().to(connect_ca::create_intention_persistent),
        )
        .route(
            "/connect/intentions/{id}",
            web::get().to(connect_ca::get_intention_persistent),
        )
        .route(
            "/connect/intentions/{id}",
            web::put().to(connect_ca::update_intention_persistent),
        )
        .route(
            "/connect/intentions/{id}",
            web::delete().to(connect_ca::delete_intention_persistent),
        )
}

// ============================================================================
// Combined Route Scopes
// ============================================================================

/// Configure all Consul API routes (in-memory storage)
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
        .service(consul_snapshot_routes())
        .service(consul_operator_routes())
        .service(consul_config_entry_routes())
        .service(consul_coordinate_routes())
        .service(consul_peering_routes())
        .service(consul_connect_routes())
        .service(consul_connect_ca_routes())
        .service(consul_internal_routes())
}

/// Configure all Consul API routes with database persistence
/// Note: KV, ACL, Session, Query now use RocksDB write-through persistence
/// via the unified service types (no separate persistent route functions needed).
pub fn consul_routes_persistent() -> actix_web::Scope {
    web::scope("")
        .service(consul_agent_routes_persistent())
        .service(consul_health_routes())
        .service(consul_kv_routes())
        .service(consul_catalog_routes())
        .service(consul_acl_routes())
        .service(consul_session_routes())
        .service(consul_status_routes())
        .service(consul_event_routes_persistent())
        .service(consul_query_routes())
        .service(consul_snapshot_routes_persistent())
        .service(consul_operator_routes())
        .service(consul_config_entry_routes_persistent())
        .service(consul_coordinate_routes_persistent())
        .service(consul_peering_routes_persistent())
        .service(consul_connect_routes_persistent())
        .service(consul_connect_ca_routes_persistent())
        .service(consul_internal_routes())
}

/// Configure a subset of Consul API routes for testing with the given data dependencies
/// Merges KV and snapshot routes into a single /v1 scope to avoid actix-web scope conflicts.
#[cfg(test)]
fn consul_test_routes() -> actix_web::Scope {
    web::scope("")
        .service(consul_agent_routes())
        .service(consul_health_routes())
        .service(consul_session_routes())
        .service(consul_event_routes())
        .service(consul_status_routes())
        // Merge KV and snapshot into a single /v1 scope (actix-web cannot have
        // two scopes with the same path prefix)
        .service(
            web::scope("/v1")
                .route("/kv/{key:.*}", web::get().to(kv::get_kv))
                .route("/kv/{key:.*}", web::put().to(kv::put_kv))
                .route("/kv/{key:.*}", web::delete().to(kv::delete_kv))
                .route("/txn", web::put().to(kv::txn))
                .route("/snapshot", web::get().to(snapshot::save_snapshot))
                .route("/snapshot", web::put().to(snapshot::restore_snapshot)),
        )
}

/// Configure all Consul API routes with full features
/// Database persistence + real cluster information
/// Note: KV, ACL, Session, Query now use RocksDB write-through persistence
/// via the unified service types (no separate persistent route functions needed).
pub fn consul_routes_full() -> actix_web::Scope {
    web::scope("")
        .service(consul_agent_routes_real())
        .service(consul_health_routes())
        .service(consul_kv_routes())
        .service(consul_catalog_routes())
        .service(consul_acl_routes())
        .service(consul_session_routes())
        .service(consul_status_routes_real())
        .service(consul_event_routes_persistent())
        .service(consul_query_routes())
        .service(consul_snapshot_routes_persistent())
        .service(consul_operator_routes_real())
        .service(consul_config_entry_routes_persistent())
        .service(consul_coordinate_routes_persistent())
        .service(consul_peering_routes_persistent())
        .service(consul_connect_routes_persistent())
        .service(consul_connect_ca_routes_persistent())
        .service(consul_internal_routes())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use actix_web::{App, test, web};

    use batata_naming::service::NamingService;

    use crate::acl::AclService;
    use crate::agent::ConsulAgentService;
    use crate::event::ConsulEventService;
    use crate::health::ConsulHealthService;
    use crate::kv::ConsulKVService;
    use crate::lock::{ConsulLockService, ConsulSemaphoreService};
    use crate::query::ConsulQueryService;
    use crate::session::ConsulSessionService;
    use crate::snapshot::ConsulSnapshotService;

    use super::consul_test_routes;

    /// Create a test app with all in-memory services configured
    async fn create_test_app() -> impl actix_web::dev::Service<
        actix_http::Request,
        Response = actix_web::dev::ServiceResponse,
        Error = actix_web::Error,
    > {
        let naming_service = Arc::new(NamingService::new());
        let registry = Arc::new(batata_naming::InstanceCheckRegistry::new(
            naming_service.clone(),
        ));
        let kv_service = ConsulKVService::new();
        let session_service = ConsulSessionService::new();
        let health_service = ConsulHealthService::new(naming_service.clone(), registry.clone());
        let agent_service = ConsulAgentService::new(naming_service.clone(), registry);
        let acl_service = AclService::disabled();
        let event_service = ConsulEventService::new();
        let snapshot_service = ConsulSnapshotService::new();
        let query_service = ConsulQueryService::new();
        let kv_arc = Arc::new(kv_service.clone());
        let session_arc = Arc::new(session_service.clone());
        let lock_service = ConsulLockService::new(kv_arc.clone(), session_arc.clone());
        let semaphore_service = ConsulSemaphoreService::new(kv_arc, session_arc);

        test::init_service(
            App::new()
                .app_data(web::Data::new(kv_service))
                .app_data(web::Data::new(session_service))
                .app_data(web::Data::new(health_service))
                .app_data(web::Data::new(agent_service))
                .app_data(web::Data::new(acl_service))
                .app_data(web::Data::new(event_service))
                .app_data(web::Data::new(snapshot_service))
                .app_data(web::Data::new(query_service))
                .app_data(web::Data::new(lock_service))
                .app_data(web::Data::new(semaphore_service))
                .service(consul_test_routes()),
        )
        .await
    }

    // ========================================================================
    // KV Store HTTP Tests
    // ========================================================================

    #[actix_web::test]
    async fn test_http_kv_put_and_get() {
        let app = create_test_app().await;

        // PUT a key
        let req = test::TestRequest::put()
            .uri("/v1/kv/http-test/key1")
            .set_payload("hello-world")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // GET the key
        let req = test::TestRequest::get()
            .uri("/v1/kv/http-test/key1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_array());
        let items = body.as_array().unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0]["Key"], "http-test/key1");
    }

    #[actix_web::test]
    async fn test_http_kv_get_nonexistent() {
        let app = create_test_app().await;

        let req = test::TestRequest::get()
            .uri("/v1/kv/nonexistent-http-key")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[actix_web::test]
    async fn test_http_kv_delete() {
        let app = create_test_app().await;

        // PUT then DELETE
        let req = test::TestRequest::put()
            .uri("/v1/kv/http-del/key1")
            .set_payload("to-delete")
            .to_request();
        test::call_service(&app, req).await;

        let req = test::TestRequest::delete()
            .uri("/v1/kv/http-del/key1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // Verify deleted
        let req = test::TestRequest::get()
            .uri("/v1/kv/http-del/key1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 404);
    }

    #[actix_web::test]
    async fn test_http_kv_keys_only() {
        let app = create_test_app().await;

        // PUT some keys
        for k in &["http-keys/a", "http-keys/b", "http-keys/c"] {
            let req = test::TestRequest::put()
                .uri(&format!("/v1/kv/{}", k))
                .set_payload("v")
                .to_request();
            test::call_service(&app, req).await;
        }

        // GET with ?keys
        let req = test::TestRequest::get()
            .uri("/v1/kv/http-keys/?keys")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_array());
        let keys = body.as_array().unwrap();
        assert!(keys.len() >= 3);
    }

    #[actix_web::test]
    async fn test_http_kv_cas() {
        let app = create_test_app().await;

        // PUT initial value
        let req = test::TestRequest::put()
            .uri("/v1/kv/http-cas/key1")
            .set_payload("initial")
            .to_request();
        test::call_service(&app, req).await;

        // GET to find the modify index
        let req = test::TestRequest::get()
            .uri("/v1/kv/http-cas/key1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: serde_json::Value = test::read_body_json(resp).await;
        let modify_index = body[0]["ModifyIndex"].as_u64().unwrap();

        // CAS with correct index should succeed
        let req = test::TestRequest::put()
            .uri(&format!("/v1/kv/http-cas/key1?cas={}", modify_index))
            .set_payload("updated")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body = test::read_body(resp).await;
        assert_eq!(body, "true");

        // CAS with old index should fail
        let req = test::TestRequest::put()
            .uri(&format!("/v1/kv/http-cas/key1?cas={}", modify_index))
            .set_payload("should-fail")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let body = test::read_body(resp).await;
        assert_eq!(body, "false");
    }

    // ========================================================================
    // Health Check HTTP Tests
    // ========================================================================

    #[actix_web::test]
    async fn test_http_register_and_get_check() {
        let app = create_test_app().await;

        // Register a check
        let check_json = serde_json::json!({
            "Name": "http-check-test",
            "CheckID": "http-chk-1",
            "TTL": "30s",
            "Status": "passing"
        });
        let req = test::TestRequest::put()
            .uri("/v1/agent/check/register")
            .set_json(&check_json)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // List agent checks
        let req = test::TestRequest::get()
            .uri("/v1/agent/checks")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_object());
        assert!(body.get("http-chk-1").is_some());
    }

    #[actix_web::test]
    async fn test_http_check_pass_warn_fail() {
        let app = create_test_app().await;

        // Register check
        let check_json = serde_json::json!({
            "Name": "status-check",
            "CheckID": "http-status-chk",
            "TTL": "30s"
        });
        let req = test::TestRequest::put()
            .uri("/v1/agent/check/register")
            .set_json(&check_json)
            .to_request();
        test::call_service(&app, req).await;

        // Pass
        let req = test::TestRequest::put()
            .uri("/v1/agent/check/pass/http-status-chk")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // Warn
        let req = test::TestRequest::put()
            .uri("/v1/agent/check/warn/http-status-chk")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // Fail
        let req = test::TestRequest::put()
            .uri("/v1/agent/check/fail/http-status-chk")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_web::test]
    async fn test_http_deregister_check() {
        let app = create_test_app().await;

        // Register
        let check_json = serde_json::json!({
            "Name": "dereg-check",
            "CheckID": "http-dereg-chk",
            "TTL": "30s"
        });
        let req = test::TestRequest::put()
            .uri("/v1/agent/check/register")
            .set_json(&check_json)
            .to_request();
        test::call_service(&app, req).await;

        // Deregister
        let req = test::TestRequest::put()
            .uri("/v1/agent/check/deregister/http-dereg-chk")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // ========================================================================
    // Health State HTTP Tests
    // ========================================================================

    #[actix_web::test]
    async fn test_http_health_state_any() {
        let app = create_test_app().await;

        let req = test::TestRequest::get()
            .uri("/v1/health/state/any")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // ========================================================================
    // Agent HTTP Tests
    // ========================================================================

    #[actix_web::test]
    async fn test_http_agent_self() {
        let app = create_test_app().await;

        let req = test::TestRequest::get().uri("/v1/agent/self").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.get("Config").is_some());
        assert!(body.get("Member").is_some());
    }

    #[actix_web::test]
    async fn test_http_agent_members() {
        let app = create_test_app().await;

        let req = test::TestRequest::get()
            .uri("/v1/agent/members")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_array());
        let members = body.as_array().unwrap();
        assert!(!members.is_empty());
        // First member should have Status=1 (alive)
        assert_eq!(members[0]["Status"], 1);
    }

    #[actix_web::test]
    async fn test_http_agent_version() {
        let app = create_test_app().await;

        let req = test::TestRequest::get()
            .uri("/v1/agent/version")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        let version = body["HumanVersion"].as_str().unwrap();
        assert!(version.contains("batata"));
    }

    #[actix_web::test]
    async fn test_http_agent_host() {
        let app = create_test_app().await;

        let req = test::TestRequest::get().uri("/v1/agent/host").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.get("Memory").is_some());
        assert!(body.get("Host").is_some());
    }

    #[actix_web::test]
    async fn test_http_agent_metrics() {
        let app = create_test_app().await;

        let req = test::TestRequest::get()
            .uri("/v1/agent/metrics")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.get("Gauges").is_some());
    }

    #[actix_web::test]
    async fn test_http_agent_service_register_and_list() {
        let app = create_test_app().await;

        // Register a service
        let svc_json = serde_json::json!({
            "Name": "http-test-web",
            "ID": "http-test-web-1",
            "Port": 8080,
            "Address": "10.0.0.1",
            "Tags": ["v1", "primary"]
        });
        let req = test::TestRequest::put()
            .uri("/v1/agent/service/register")
            .set_json(&svc_json)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // List services
        let req = test::TestRequest::get()
            .uri("/v1/agent/services")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_object());
    }

    // ========================================================================
    // Session HTTP Tests
    // ========================================================================

    #[actix_web::test]
    async fn test_http_session_create_and_list() {
        let app = create_test_app().await;

        // Create a session
        let session_json = serde_json::json!({
            "Name": "http-test-session",
            "TTL": "30s"
        });
        let req = test::TestRequest::put()
            .uri("/v1/session/create")
            .set_json(&session_json)
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.get("ID").is_some());

        // List sessions
        let req = test::TestRequest::get()
            .uri("/v1/session/list")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_array());
    }

    #[actix_web::test]
    async fn test_http_session_destroy() {
        let app = create_test_app().await;

        // Create
        let session_json = serde_json::json!({
            "Name": "http-destroy-session"
        });
        let req = test::TestRequest::put()
            .uri("/v1/session/create")
            .set_json(&session_json)
            .to_request();
        let resp = test::call_service(&app, req).await;
        let body: serde_json::Value = test::read_body_json(resp).await;
        let session_id = body["ID"].as_str().unwrap().to_string();

        // Destroy
        let req = test::TestRequest::put()
            .uri(&format!("/v1/session/destroy/{}", session_id))
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // ========================================================================
    // Event HTTP Tests
    // ========================================================================

    #[actix_web::test]
    async fn test_http_event_fire_and_list() {
        let app = create_test_app().await;

        // Fire an event
        let req = test::TestRequest::put()
            .uri("/v1/event/fire/http-test-evt")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert_eq!(body["Name"], "http-test-evt");
        assert!(body.get("ID").is_some());

        // List events
        let req = test::TestRequest::get().uri("/v1/event/list").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_array());
    }

    // ========================================================================
    // Status HTTP Tests
    // ========================================================================

    #[actix_web::test]
    async fn test_http_status_leader() {
        let app = create_test_app().await;

        let req = test::TestRequest::get()
            .uri("/v1/status/leader")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_web::test]
    async fn test_http_status_peers() {
        let app = create_test_app().await;

        let req = test::TestRequest::get()
            .uri("/v1/status/peers")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        let body: serde_json::Value = test::read_body_json(resp).await;
        assert!(body.is_array());
    }

    // ========================================================================
    // Snapshot HTTP Tests
    // ========================================================================

    #[actix_web::test]
    async fn test_http_snapshot_save_and_restore() {
        let app = create_test_app().await;

        // Save snapshot
        let req = test::TestRequest::get().uri("/v1/snapshot").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
        let snapshot_bytes = test::read_body(resp).await;
        assert!(!snapshot_bytes.is_empty());

        // Restore snapshot
        let req = test::TestRequest::put()
            .uri("/v1/snapshot")
            .set_payload(snapshot_bytes.to_vec())
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // ========================================================================
    // Agent Maintenance HTTP Tests
    // ========================================================================

    #[actix_web::test]
    async fn test_http_agent_maintenance() {
        let app = create_test_app().await;

        // Enable maintenance
        let req = test::TestRequest::put()
            .uri("/v1/agent/maintenance?enable=true&reason=testing")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);

        // Disable maintenance
        let req = test::TestRequest::put()
            .uri("/v1/agent/maintenance?enable=false")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    // ========================================================================
    // Agent Join/Leave/Reload Stubs
    // ========================================================================

    #[actix_web::test]
    async fn test_http_agent_join() {
        let app = create_test_app().await;

        let req = test::TestRequest::put()
            .uri("/v1/agent/join/10.0.0.1:8301")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_web::test]
    async fn test_http_agent_leave() {
        let app = create_test_app().await;

        let req = test::TestRequest::put().uri("/v1/agent/leave").to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_web::test]
    async fn test_http_agent_force_leave() {
        let app = create_test_app().await;

        let req = test::TestRequest::put()
            .uri("/v1/agent/force-leave/node-1")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }

    #[actix_web::test]
    async fn test_http_agent_reload() {
        let app = create_test_app().await;

        let req = test::TestRequest::put()
            .uri("/v1/agent/reload")
            .to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(resp.status(), 200);
    }
}
