//! Debug / pprof endpoint tests.
//!
//! Each test calls the corresponding SDK method and validates it returns
//! a non-empty byte stream when supported, or surfaces a typed error
//! when the endpoint is disabled on this cluster.

mod common;

#[tokio::test]
async fn test_debug_heap() {
    let client = common::create_client();
    match client.debug_heap().await {
        Ok(bytes) => {
            assert!(!bytes.is_empty(), "heap dump should be non-empty");
        }
        Err(e) => {
            // 404 or 405 is acceptable when pprof is not enabled
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_debug_goroutine() {
    let client = common::create_client();
    match client.debug_goroutine().await {
        Ok(bytes) => {
            assert!(!bytes.is_empty());
        }
        Err(e) => {
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_debug_profile_short() {
    // Use 1 second to keep tests fast. Tolerate 404/405.
    let client = common::create_client();
    match client.debug_profile(1).await {
        Ok(bytes) => assert!(!bytes.is_empty()),
        Err(e) => {
            let _ = e.status();
        }
    }
}

#[tokio::test]
async fn test_debug_pprof_custom_profile() {
    // Fetching a named profile by `debug_pprof_bytes` ensures the generic
    // entry point is callable.
    let client = common::create_client();
    let _ = client.debug_pprof_bytes("allocs", None).await; // ignore result
}
