//! Auth Functional Tests
//!
//! Tests JWT login, AK/SK signing, SecurityProxy.

mod common;

use std::sync::Arc;
use batata_client::auth::{
    AccessKeyAuthProvider, ClientAuthService, JwtAuthProvider, RequestResource, SecurityProxy,
};

#[tokio::test]
#[ignore]
async fn test_jwt_login_success() {
    common::init_tracing();
    let provider = JwtAuthProvider::new(common::USERNAME, common::PASSWORD, "/nacos");
    let servers = vec![common::SERVER_URL.to_string()];

    let ok = provider.login(&servers).await;
    assert!(ok, "JWT login should succeed");

    let ctx = provider.get_identity_context(&RequestResource::default());
    assert!(
        ctx.headers.contains_key("accessToken"),
        "Should have accessToken header"
    );
    assert!(
        !ctx.headers.get("accessToken").unwrap().is_empty(),
        "Token should not be empty"
    );
}

#[tokio::test]
#[ignore]
async fn test_jwt_login_failure() {
    common::init_tracing();
    let provider = JwtAuthProvider::new("wrong_user", "wrong_pass", "/nacos");
    let servers = vec![common::SERVER_URL.to_string()];

    let ok = provider.login(&servers).await;
    assert!(!ok, "JWT login with wrong credentials should fail");
}

#[test]
fn test_aksk_signing() {
    let provider = AccessKeyAuthProvider::new("test-ak", "test-sk");
    assert!(provider.is_enabled());
    assert_eq!(provider.name(), "accesskey");

    let ctx = provider.get_identity_context(&RequestResource {
        namespace: "public".to_string(),
        ..Default::default()
    });
    assert!(ctx.headers.contains_key("ak"), "Should have ak header");
    assert!(ctx.headers.contains_key("sign"), "Should have sign header");
    assert!(
        ctx.headers.contains_key("data"),
        "Should have data (timestamp) header"
    );
}

#[tokio::test]
#[ignore]
async fn test_security_proxy_merge() {
    common::init_tracing();
    let mut proxy = SecurityProxy::new();

    let jwt = Arc::new(JwtAuthProvider::new(
        common::USERNAME,
        common::PASSWORD,
        "/nacos",
    ));
    let aksk = Arc::new(AccessKeyAuthProvider::new("my-ak", "my-sk"));

    proxy.add_provider(jwt);
    proxy.add_provider(aksk);

    assert_eq!(proxy.provider_count(), 2);

    let servers = vec![common::SERVER_URL.to_string()];
    let ok = proxy.login(&servers).await;
    assert!(ok, "At least one provider should succeed");

    let ctx = proxy.get_identity_context(&RequestResource::default());
    // Should have headers from BOTH providers
    assert!(
        ctx.headers.contains_key("accessToken"),
        "JWT token should be present"
    );
    assert!(
        ctx.headers.contains_key("ak"),
        "AK should be present"
    );
}

#[test]
fn test_disabled_provider() {
    let provider = JwtAuthProvider::new("", "", "/nacos");
    assert!(!provider.is_enabled(), "Empty credentials should disable provider");

    let provider2 = AccessKeyAuthProvider::new("", "");
    assert!(!provider2.is_enabled(), "Empty AK/SK should disable provider");
}
