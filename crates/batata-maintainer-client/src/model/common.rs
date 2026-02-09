// Common model types

use serde::{Deserialize, Serialize};

/// Generic paginated response
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Page<T> {
    pub total_count: u64,
    pub page_number: u64,
    pub pages_available: u64,
    pub page_items: Vec<T>,
}

/// Generic API response wrapper (used internally)
#[derive(Debug, Deserialize)]
pub(crate) struct ApiResponse<T> {
    #[allow(dead_code)]
    pub code: i32,
    #[allow(dead_code)]
    pub message: String,
    pub data: T,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_serialization() {
        let page: Page<String> = Page {
            total_count: 10,
            page_number: 1,
            pages_available: 2,
            page_items: vec!["a".to_string(), "b".to_string()],
        };

        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("\"totalCount\":10"));
        assert!(json.contains("\"pageNumber\":1"));
        assert!(json.contains("\"pagesAvailable\":2"));

        let deserialized: Page<String> = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_count, 10);
        assert_eq!(deserialized.page_items.len(), 2);
    }

    #[test]
    fn test_api_response_deserialization() {
        let json = r#"{"code":200,"message":"ok","data":"hello"}"#;
        let resp: ApiResponse<String> = serde_json::from_str(json).unwrap();
        assert_eq!(resp.code, 200);
        assert_eq!(resp.data, "hello");
    }
}
