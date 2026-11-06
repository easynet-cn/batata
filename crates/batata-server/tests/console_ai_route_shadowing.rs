//! Regression test for console AI route shadowing.
//!
//! `startup::http` composes the console scope as
//! `configure_v3_console_routes()` followed by the sibling AI scopes
//! (`/ai/a2a`, `/ai/mcp`, `/ai/agents`, ...). If the AI resource importer is
//! registered as a bare `web::scope("/ai")`, actix matches that scope for
//! *every* `/ai/*` path and never falls back to the siblings registered after
//! it: the console then answers `/v3/console/ai/a2a/list` with 404.
//!
//! The importer must therefore be its own fully-qualified `/ai/import` scope.

use actix_web::{App, HttpResponse, http::StatusCode, test, web};

/// Sibling scopes registered exactly the way `startup::http` registers
/// `console_a2a::routes()` and `console_mcp::routes()`: as separate services
/// under `/v3/console`, not nested inside a shared `/ai` scope.
///
/// The production handlers need `AppState` and AI service trait objects;
/// routing is what is under test here, so plain 200 responders keep the
/// assertions exact and free of unrelated dependencies.
fn configure_ai_siblings(cfg: &mut web::ServiceConfig) {
    cfg.service(web::scope("/ai/a2a").route(
        "/list",
        web::get().to(|| async { HttpResponse::Ok().body("a2a") }),
    ))
    .service(web::scope("/ai/mcp").route(
        "/list",
        web::get().to(|| async { HttpResponse::Ok().body("mcp") }),
    ));
}

#[actix_web::test]
async fn console_importer_does_not_shadow_ai_sibling_scopes() {
    let app = test::init_service(
        App::new().service(
            web::scope("/v3/console")
                .configure(batata_console::configure_v3_console_routes)
                .configure(configure_ai_siblings),
        ),
    )
    .await;

    for (path, expected) in [
        ("/v3/console/ai/a2a/list", "a2a"),
        ("/v3/console/ai/mcp/list", "mcp"),
    ] {
        let req = test::TestRequest::get().uri(path).to_request();
        let resp = test::call_service(&app, req).await;
        assert_eq!(
            resp.status(),
            StatusCode::OK,
            "{path} must reach its sibling AI scope, not be swallowed by /ai"
        );
        let body = test::read_body(resp).await;
        assert_eq!(body, expected, "unexpected handler for {path}");
    }
}

#[actix_web::test]
async fn console_importer_still_resolves_after_prefix_change() {
    let app = test::init_service(
        App::new().service(
            web::scope("/v3/console")
                .configure(batata_console::configure_v3_console_routes)
                .configure(configure_ai_siblings),
        ),
    )
    .await;

    // No AppState is registered, so the `secured!` extractor cannot return
    // 200. What matters is that the route is still *matched*: a missing route
    // would be 404, an extractor failure is a 500.
    let req = test::TestRequest::get()
        .uri("/v3/console/ai/import/sources")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_ne!(
        resp.status(),
        StatusCode::NOT_FOUND,
        "/v3/console/ai/import/sources must still be mounted"
    );
}
