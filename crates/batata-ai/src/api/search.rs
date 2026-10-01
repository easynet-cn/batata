//! Resource search HTTP API — Nacos 3.3 compatible.
//!
//! Client: `/v3/client/ai/resources/search`
//!
//! The index itself is built by `crate::search::consumer` as versions are
//! published; this is the read side.
//!
//! Two deliberate differences from upstream, both documented in
//! `docs/compat/nacos/ai/README.md`:
//! - Upstream pages with a `cursor` and caps with `limit`; Batata pages with
//!   `pageNo` / `pageSize`, as every other AI list endpoint here does.
//! - Upstream can blend a vector channel when pgvector is available. Batata has
//!   no pgvector equivalent, so keyword hits *are* the ranking.

use actix_web::{HttpRequest, HttpResponse, Responder, Scope, get, web};
use batata_common::{ActionTypes, ApiType, SignType};
use batata_server_common::model::app_state::AppState;
use batata_server_common::model::response::Result;
use batata_server_common::{Secured, secured};
use serde::Deserialize;

use crate::search::query;

/// Empty namespaces mean the default one.
fn normalize_namespace(ns: &str) -> &str {
    if ns.is_empty() {
        batata_common::DEFAULT_NAMESPACE_ID
    } else {
        ns
    }
}

/// Query parameters of the client search endpoint.
#[derive(Debug, Deserialize)]
pub struct ResourceSearchForm {
    /// Namespace to search in; empty means `public`.
    #[serde(default, alias = "namespaceId")]
    pub namespace_id: String,
    /// Search text.
    #[serde(default)]
    pub query: String,
    /// Resource types to restrict to; empty means all.
    #[serde(default, alias = "resourceTypes")]
    pub resource_types: Vec<String>,
    /// 1-based page number.
    #[serde(default = "default_page_no", alias = "pageNo")]
    pub page_no: u64,
    /// Page size; 0 means the default.
    #[serde(default, alias = "pageSize")]
    pub page_size: u64,
}

fn default_page_no() -> u64 {
    1
}

/// GET /v3/client/ai/resources/search — Keyword search over the resource index.
#[get("")]
async fn search_resources(
    req: HttpRequest,
    data: web::Data<AppState>,
    form: web::Query<ResourceSearchForm>,
) -> impl Responder {
    secured!(
        Secured::builder(&req, &data, "")
            .action(ActionTypes::Read)
            .sign_type(SignType::Ai)
            .api_type(ApiType::OpenApi)
            .build()
    );

    let form = form.into_inner();
    if form.query.trim().is_empty() {
        return Result::<()>::http_bad_request(
            &batata_common::error::PARAMETER_MISSING,
            "query is required",
        );
    }

    let Some(persistence) = data.persistence.as_ref() else {
        return Result::<()>::http_internal_error(anyhow::anyhow!(
            "search needs a persistence backend, and none is configured"
        ));
    };
    let persistence: &dyn batata_persistence::PersistenceService = persistence.as_ref();

    let types: Vec<&str> = form
        .resource_types
        .iter()
        .map(|t| t.as_str())
        .filter(|t| !t.is_empty())
        .collect();
    let ns = normalize_namespace(&form.namespace_id);

    match query::search(
        persistence,
        ns,
        &form.query,
        &types,
        form.page_no,
        form.page_size,
    )
    .await
    {
        Ok(page) => HttpResponse::Ok().json(Result::success(page)),
        Err(e) => Result::<()>::http_internal_error(e),
    }
}

/// Configure client search routes at `/v3/client/ai/resources/search`.
pub fn client_routes() -> Scope {
    web::scope("/resources/search").service(search_resources)
}
