//! Module `api::v3::client::route` of the `batata-server` crate.
use actix_web::{Scope, web};

use super::{cs, ns};

/// `client_routes` function.
///
/// # Returns
/// `Scope`.
pub fn client_routes() -> Scope {
    web::scope("/v3/client")
        .service(web::scope("/ns").service(ns::instance::routes()))
        .service(web::scope("/cs").service(cs::config::routes()))
        .service(
            web::scope("/ai")
                .service(batata_ai::prompt_client_routes())
                .service(batata_ai::skill_client_routes())
                .service(batata_ai::agentspec_client_routes())
                .service(batata_ai::agent_client_routes())
                .service(batata_ai::capability_client_routes())
                .service(batata_ai::resource_search_client_routes()),
        )
}
