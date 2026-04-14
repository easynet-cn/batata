//! Consul Operator API handlers with scope-relative route macros.
//!
//! These use `#[get("/raft/configuration")]` style macros under an "/operator" scope.

use actix_web::{HttpRequest, HttpResponse, Scope, delete, get, post, put, web};

use crate::acl::AclService;
use crate::catalog::ConsulCatalogService;
use crate::operator::{
    AutopilotConfigParams, AutopilotConfiguration, ConsulOperatorService,
    ConsulOperatorServiceReal, KeyringParams, KeyringRequest, OperatorQueryParams, RaftPeerParams,
    TransferLeaderParams,
};

// ============================================================================
// In-memory handlers
// ============================================================================

#[get("/raft/configuration")]
async fn get_raft_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_raft_configuration(req, acl_service, operator_service, _query).await
}

#[post("/raft/transfer-leader")]
async fn transfer_leader(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<TransferLeaderParams>,
) -> HttpResponse {
    crate::operator::transfer_leader(req, acl_service, operator_service, query).await
}

#[delete("/raft/peer")]
async fn remove_raft_peer(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<RaftPeerParams>,
) -> HttpResponse {
    crate::operator::remove_raft_peer(req, acl_service, operator_service, query).await
}

#[get("/autopilot/configuration")]
async fn get_autopilot_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_autopilot_configuration(req, acl_service, operator_service, _query).await
}

#[put("/autopilot/configuration")]
async fn set_autopilot_configuration(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    query: web::Query<AutopilotConfigParams>,
    body: web::Json<AutopilotConfiguration>,
) -> HttpResponse {
    crate::operator::set_autopilot_configuration(req, acl_service, operator_service, query, body)
        .await
}

#[get("/autopilot/health")]
async fn get_autopilot_health(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_autopilot_health(req, acl_service, operator_service, _query).await
}

#[get("/autopilot/state")]
async fn get_autopilot_state(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_autopilot_state(req, acl_service, operator_service, _query).await
}

#[get("/keyring")]
async fn keyring_list(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    _query: web::Query<KeyringParams>,
) -> HttpResponse {
    crate::operator::keyring_list(req, acl_service, operator_service, _query).await
}

#[post("/keyring")]
async fn keyring_install(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    crate::operator::keyring_install(req, acl_service, operator_service, body).await
}

#[put("/keyring")]
async fn keyring_use(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    crate::operator::keyring_use(req, acl_service, operator_service, body).await
}

#[delete("/keyring")]
async fn keyring_remove(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    crate::operator::keyring_remove(req, acl_service, operator_service, body).await
}

#[get("/usage")]
async fn get_operator_usage(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorService>,
    catalog_service: web::Data<ConsulCatalogService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_operator_usage(req, acl_service, operator_service, catalog_service, _query)
        .await
}

#[get("/utilization")]
async fn get_operator_utilization(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_operator_utilization(req, acl_service, _query).await
}

// ============================================================================
// Real cluster handlers
// ============================================================================

#[get("/raft/configuration")]
async fn get_raft_configuration_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_raft_configuration_real(req, acl_service, operator_service, _query).await
}

#[post("/raft/transfer-leader")]
async fn transfer_leader_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<TransferLeaderParams>,
) -> HttpResponse {
    crate::operator::transfer_leader_real(req, acl_service, _operator_service, _query).await
}

#[delete("/raft/peer")]
async fn remove_raft_peer_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _operator_service: web::Data<ConsulOperatorServiceReal>,
    query: web::Query<RaftPeerParams>,
) -> HttpResponse {
    crate::operator::remove_raft_peer_real(req, acl_service, _operator_service, query).await
}

#[get("/autopilot/configuration")]
async fn get_autopilot_configuration_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_autopilot_configuration_real(req, acl_service, operator_service, _query)
        .await
}

#[put("/autopilot/configuration")]
async fn set_autopilot_configuration_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    query: web::Query<AutopilotConfigParams>,
    body: web::Json<AutopilotConfiguration>,
) -> HttpResponse {
    crate::operator::set_autopilot_configuration_real(
        req,
        acl_service,
        operator_service,
        query,
        body,
    )
    .await
}

#[get("/autopilot/health")]
async fn get_autopilot_health_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_autopilot_health_real(req, acl_service, operator_service, _query).await
}

#[get("/autopilot/state")]
async fn get_autopilot_state_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_autopilot_state_real(req, acl_service, operator_service, _query).await
}

#[get("/keyring")]
async fn keyring_list_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    _query: web::Query<KeyringParams>,
) -> HttpResponse {
    crate::operator::keyring_list_real(req, acl_service, operator_service, _query).await
}

#[post("/keyring")]
async fn keyring_install_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    crate::operator::keyring_install_real(req, acl_service, operator_service, body).await
}

#[put("/keyring")]
async fn keyring_use_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    crate::operator::keyring_use_real(req, acl_service, operator_service, body).await
}

#[delete("/keyring")]
async fn keyring_remove_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    body: web::Json<KeyringRequest>,
) -> HttpResponse {
    crate::operator::keyring_remove_real(req, acl_service, operator_service, body).await
}

#[get("/usage")]
async fn get_operator_usage_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    operator_service: web::Data<ConsulOperatorServiceReal>,
    catalog_service: web::Data<ConsulCatalogService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_operator_usage_real(
        req,
        acl_service,
        operator_service,
        catalog_service,
        _query,
    )
    .await
}

#[get("/utilization")]
async fn get_operator_utilization_real(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    _query: web::Query<OperatorQueryParams>,
) -> HttpResponse {
    crate::operator::get_operator_utilization_real(req, acl_service, _query).await
}

// ============================================================================
// Route registration
// ============================================================================

/// GET /v1/operator/segment - Network segment list.
/// Enterprise-only feature; OSS/Batata returns an empty array to match
/// Consul OSS behavior rather than 404, so clients can detect feature absence.
#[get("/segment")]
async fn operator_segment_list() -> HttpResponse {
    HttpResponse::Ok().json(Vec::<String>::new())
}

/// GET /v1/operator/license - Enterprise license info (stub).
/// Returns a valid-looking payload with no features enabled so enterprise
/// clients don't error out on the license check.
#[get("/license")]
async fn operator_license() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({
        "Valid": true,
        "License": {
            "LicenseID": "",
            "CustomerID": "",
            "InstallationID": "*",
            "IssueTime": "1970-01-01T00:00:00Z",
            "StartTime": "1970-01-01T00:00:00Z",
            "ExpirationTime": "2099-12-31T23:59:59Z",
            "TerminationTime": "2099-12-31T23:59:59Z",
            "Product": "consul",
            "Flags": {},
            "Features": [],
        },
        "Warnings": [],
    }))
}

pub fn routes() -> Scope {
    web::scope("/operator")
        .service(get_raft_configuration)
        .service(transfer_leader)
        .service(remove_raft_peer)
        .service(get_autopilot_configuration)
        .service(set_autopilot_configuration)
        .service(get_autopilot_health)
        .service(get_autopilot_state)
        .service(keyring_list)
        .service(keyring_install)
        .service(keyring_use)
        .service(keyring_remove)
        .service(get_operator_usage)
        .service(get_operator_utilization)
        .service(operator_segment_list)
        .service(operator_license)
}

pub fn routes_real() -> Scope {
    web::scope("/operator")
        .service(get_raft_configuration_real)
        .service(transfer_leader_real)
        .service(remove_raft_peer_real)
        .service(get_autopilot_configuration_real)
        .service(set_autopilot_configuration_real)
        .service(get_autopilot_health_real)
        .service(get_autopilot_state_real)
        .service(keyring_list_real)
        .service(keyring_install_real)
        .service(keyring_use_real)
        .service(keyring_remove_real)
        .service(get_operator_usage_real)
        .service(get_operator_utilization_real)
        .service(operator_segment_list)
        .service(operator_license)
}
