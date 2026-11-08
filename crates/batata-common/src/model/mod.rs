use serde::{Deserialize, Serialize};

/// Maximum number of rows a single paginated read may return.
///
/// Query parameters are untrusted: without a cap, `pageSize = u64::MAX` forces
/// the server to materialize an entire namespace / column family in memory.
pub const MAX_PAGE_SIZE: u64 = 1000;

/// Normalize untrusted pagination inputs into a bounded `(offset, page_size)`.
///
/// `saturating_*` arithmetic prevents `page_no = u64::MAX` from overflowing in
/// debug builds or wrapping to an arbitrary offset in release builds.
pub fn clamp_page(page_no: u64, page_size: u64) -> (u64, u64) {
    let page_size = page_size.min(MAX_PAGE_SIZE);
    let offset = page_no.saturating_sub(1).saturating_mul(page_size);
    (offset, page_size)
}

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

#[cfg(test)]
mod tests {
    use super::{MAX_PAGE_SIZE, clamp_page};

    #[test]
    fn clamp_page_bounds_untrusted_page_size() {
        assert_eq!(clamp_page(1, u64::MAX), (0, MAX_PAGE_SIZE));
        assert_eq!(clamp_page(3, 25), (50, 25));
        assert_eq!(clamp_page(0, 25), (0, 25), "page_no=0 must not underflow");
    }

    #[test]
    fn clamp_page_never_overflows_on_max_page_no() {
        let (offset, page_size) = clamp_page(u64::MAX, u64::MAX);
        assert_eq!(page_size, MAX_PAGE_SIZE);
        assert_eq!(offset, u64::MAX, "the product must saturate, not wrap");
    }
}
