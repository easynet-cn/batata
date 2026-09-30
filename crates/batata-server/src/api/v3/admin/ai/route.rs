//! Module `api::v3::admin::ai::route` of the `batata-server` crate.
use actix_web::{Scope, web};

use super::{a2a, mcp};

/// `routes` function.
///
/// # Returns
/// `Scope`.
pub fn routes() -> Scope {
    web::scope("/ai")
        .service(mcp::routes())
        .service(a2a::routes())
        .service(batata_ai::agent_admin_routes())
        .service(batata_ai::prompt_admin_routes())
        .service(batata_ai::skill_admin_routes())
        .service(batata_ai::agentspec_admin_routes())
        .service(batata_ai::pipeline_admin_routes())
        // AI resource importer (F-NAC-ADM-AI-008)
        .service(batata_console::v3::ai_import::routes())
}
