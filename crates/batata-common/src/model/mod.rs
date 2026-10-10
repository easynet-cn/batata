use serde::{Deserialize, Serialize};

/// AI-related model types.
pub mod ai;
/// Plugin model types.
pub mod plugin;

/// Generic pagination wrapper for API responses
///
/// Serde aliases support Nacos-compatible deserialization where different
/// endpoints use different field names for the same concept.
///
/// NOTE: This is the server-internal copy of `Page<T>` (the "service-trait"
/// contract). The wire/client copy lives in `batata-api::model::Page`. The two
/// are intentionally identical so they serialize the same way over the wire,
/// but they are distinct types to keep `batata-common` free of a dependency on
/// `batata-api` (and thus keep the client crate off server-only deps).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    #[serde(alias = "count", default)]
    /// The `total_count` field.
    pub total_count: u64,
    #[serde(default)]
    /// The `page_number` field.
    pub page_number: u64,
    #[serde(default)]
    /// The `pages_available` field.
    pub pages_available: u64,
    #[serde(
        alias = "serviceList",
        alias = "configList",
        alias = "hosts",
        alias = "subscribers",
        alias = "list",
        default
    )]
    /// The `page_items` field.
    pub page_items: Vec<T>,
}

impl<T> Default for Page<T> {
    fn default() -> Self {
        Self {
            total_count: 0,
            page_number: 1,
            pages_available: 0,
            page_items: vec![],
        }
    }
}

impl<T> Page<T> {
    /// Creates a new `Page` from the given counts and items.
    pub fn new(total_count: u64, page_number: u64, page_size: u64, page_items: Vec<T>) -> Self {
        Self {
            total_count,
            page_number,
            pages_available: if page_size > 0 {
                (total_count as f64 / page_size as f64).ceil() as u64
            } else {
                0
            },
            page_items,
        }
    }

    /// Creates an empty `Page`.
    pub fn empty() -> Self {
        Self::default()
    }
}
