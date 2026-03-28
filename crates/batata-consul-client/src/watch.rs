//! Blocking query watch helpers
//!
//! Provides a `watch` function that wraps Consul's blocking query mechanism
//! into an async stream, automatically tracking the `WaitIndex` between calls.
//!
//! This is the Rust equivalent of the Go `consul/api/watch` package.

use std::time::Duration;

use crate::error::Result;
use crate::model::{QueryMeta, QueryOptions};

/// Default wait time for blocking queries (matches Consul default: 5 minutes)
const DEFAULT_WATCH_WAIT: Duration = Duration::from_secs(300);

/// Default retry delay on error
const DEFAULT_ERROR_RETRY_DELAY: Duration = Duration::from_secs(1);

/// Configuration for a watch operation
#[derive(Clone, Debug)]
pub struct WatchConfig {
    /// Maximum time to block per query. Default: 5 minutes (Consul default).
    pub wait_time: Duration,
    /// Delay before retrying after an error. Default: 1 second.
    pub error_retry_delay: Duration,
    /// Maximum consecutive errors before stopping. 0 = unlimited.
    pub max_errors: u32,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            wait_time: DEFAULT_WATCH_WAIT,
            error_retry_delay: DEFAULT_ERROR_RETRY_DELAY,
            max_errors: 0,
        }
    }
}

/// Run a blocking query watch loop, calling `f` repeatedly with updated WaitIndex.
///
/// Each call to `f` should perform a blocking query (e.g., `kv_get`, `catalog_services`)
/// with the provided `QueryOptions`. When the index changes, the new data is sent
/// to the callback.
///
/// # Example
/// ```ignore
/// use batata_consul_client::watch::{watch, WatchConfig};
///
/// let client = ConsulClient::new(config)?;
/// let (tx, mut rx) = tokio::sync::mpsc::channel(16);
///
/// tokio::spawn(async move {
///     watch(
///         WatchConfig::default(),
///         QueryOptions::default(),
///         |opts| async {
///             client.catalog_services(opts).await
///         },
///         |data, meta| {
///             let _ = tx.try_send((data, meta));
///         },
///     ).await;
/// });
///
/// while let Some((services, meta)) = rx.recv().await {
///     println!("Services changed: {:?}", services);
/// }
/// ```
pub async fn watch<T, F, Fut, H>(
    config: WatchConfig,
    mut opts: QueryOptions,
    query_fn: F,
    mut handler: H,
) where
    F: Fn(QueryOptions) -> Fut,
    Fut: std::future::Future<Output = Result<(T, QueryMeta)>>,
    H: FnMut(T, QueryMeta),
{
    opts.wait_time = Some(config.wait_time);
    let mut consecutive_errors = 0u32;

    loop {
        match query_fn(opts.clone()).await {
            Ok((data, meta)) => {
                consecutive_errors = 0;

                // Only notify if the index actually changed
                if meta.last_index > opts.wait_index {
                    opts.wait_index = meta.last_index;
                    handler(data, meta);
                } else if meta.last_index > 0 {
                    // Index didn't change but is valid — update and continue
                    opts.wait_index = meta.last_index;
                }
            }
            Err(e) => {
                consecutive_errors += 1;
                tracing::warn!("Watch query failed (attempt {}): {}", consecutive_errors, e);

                if config.max_errors > 0 && consecutive_errors >= config.max_errors {
                    tracing::error!(
                        "Watch stopped after {} consecutive errors",
                        consecutive_errors
                    );
                    break;
                }

                // Reset index on error to avoid stale blocking
                opts.wait_index = 0;
                tokio::time::sleep(config.error_retry_delay).await;
            }
        }
    }
}

/// Watch a single KV key and call the handler when it changes.
///
/// Convenience wrapper around `watch()` for the common KV watch pattern.
/// The client must be wrapped in Arc for the async closure.
pub async fn watch_key<H>(
    client: std::sync::Arc<crate::client::ConsulClient>,
    key: String,
    config: WatchConfig,
    opts: QueryOptions,
    mut handler: H,
) where
    H: FnMut(Option<crate::model::KVPair>, QueryMeta),
{
    watch(
        config,
        opts,
        |opts| {
            let key = key.clone();
            let client = client.clone();
            async move { client.kv_get(&key, &opts).await }
        },
        move |data, meta| {
            handler(data, meta);
        },
    )
    .await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watch_config_defaults() {
        let config = WatchConfig::default();
        assert_eq!(config.wait_time, Duration::from_secs(300));
        assert_eq!(config.error_retry_delay, Duration::from_secs(1));
        assert_eq!(config.max_errors, 0);
    }
}
