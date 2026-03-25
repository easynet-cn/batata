// Consul API route configuration
//
// All route registration is done in `crate::api::v1::routes()`.
// This module provides the public `routes()` entry point and test utilities.

/// Unified Consul API v1 routes.
///
/// All routes are under a single `/v1` scope to avoid actix-web scope shadowing.
/// Each sub-module uses a unique prefix (e.g., `/agent`, `/health`, `/kv`).
pub fn routes() -> actix_web::Scope {
    crate::api::v1::routes()
}

#[cfg(test)]
mod tests;
