pub mod ai;
pub mod plugin;

use serde::{Deserialize, Serialize};

/// Generic pagination wrapper for API responses
///
/// Serde aliases support Nacos-compatible deserialization where different
/// endpoints use different field names for the same concept.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    #[serde(alias = "count", default)]
    pub total_count: u64,
    #[serde(default)]
    pub page_number: u64,
    #[serde(default)]
    pub pages_available: u64,
    #[serde(
        alias = "serviceList",
        alias = "configList",
        alias = "hosts",
        alias = "subscribers",
        alias = "list",
        default
    )]
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

    pub fn empty() -> Self {
        Self::default()
    }
}
