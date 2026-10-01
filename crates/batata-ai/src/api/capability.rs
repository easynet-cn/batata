//! AI capability declaration — Nacos 3.3 compatible.
//!
//! Clients ask this once, before touching any resource, to learn which AI
//! subsystems the server actually provides. Upstream answers a static map and
//! so does this: the document depends on what is built into the server, not on
//! runtime state.
//!
//! `radV1` covers the client-facing RAD contract: publishing runtime endpoints,
//! deregistering them, heartbeating, and discovering an agent by version —
//! including version *ranges* (`1.2.x`, `^1.2.0`, `latest`). All of that is
//! implemented, backed by Naming ephemeral registrations.
//!
//! Not implemented: upstream's `AgentRuntimePublicationCapacityGate`, which
//! caps how many endpoints one publisher may hold. That is a server-side
//! protective limit rather than part of the contract, so it does not affect
//! the declaration.

use actix_web::{HttpResponse, Responder, Scope, get, web};
use batata_server_common::model::response::Result;
use serde_json::json;

/// Schema version of the capability document.
const SCHEMA_VERSION: u32 = 1;

/// GET /v3/client/ai/capabilities — Declare which AI subsystems are provided.
#[get("")]
async fn get_capabilities() -> impl Responder {
    HttpResponse::Ok().json(Result::success(json!({
        "schemaVersion": SCHEMA_VERSION,
        "capabilities": {
            "radV1": true,
            "mcp": true,
            "skill": true,
            "prompt": true,
            "agentSpec": true,
        }
    })))
}

/// Configure client capability routes at `/v3/client/ai/capabilities`.
pub fn client_routes() -> Scope {
    web::scope("/capabilities").service(get_capabilities)
}
