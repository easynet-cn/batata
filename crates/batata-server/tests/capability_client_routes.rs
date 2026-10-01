//! Route-level test for the AI capability declaration.
//!
//! Clients read this before anything else to decide which subsystems to use,
//! so a wrong answer sends them to endpoints that do not exist. The handler
//! needs no services, so this runs without a database.

use actix_web::http::StatusCode;
use actix_web::{App, test, web};

/// GET /v3/client/ai/capabilities — declare the supported AI subsystems.
#[actix_web::test]
async fn capabilities_declares_what_the_server_provides() {
    let app = test::init_service(
        App::new()
            .service(web::scope("/v3/client/ai").service(batata_ai::capability_client_routes())),
    )
    .await;

    let req = test::TestRequest::get()
        .uri("/v3/client/ai/capabilities")
        .to_request();
    let resp = test::call_service(&app, req).await;
    assert_eq!(resp.status(), StatusCode::OK);

    let body = test::read_body(resp).await;
    let body = String::from_utf8_lossy(&body);

    // The subsystems Batata does provide.
    for name in ["mcp", "skill", "prompt", "agentSpec"] {
        assert!(body.contains(&format!("\"{name}\":true")), "{name}: {body}");
    }

    // RAD v1: runtime endpoints are published, deregistered and discovered
    // (including by version range), so the capability can be declared.
    assert!(body.contains("\"radV1\":true"), "radV1 must be true: {body}");
    assert!(body.contains("\"schemaVersion\":1"), "{body}");

    println!("ok: capability declaration = {body}");
}
