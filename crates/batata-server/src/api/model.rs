// Common API models and constants for Batata application
// This file re-exports common types and constants from batata_api

// Re-export all common types and constants from batata_api
pub use batata_api::model::*;

#[cfg(test)]
mod tests {
    use super::*;

    // Constants tests
    #[test]
    fn test_api_constants() {
        assert_eq!(CLIENT_VERSION, "3.0.0");
        assert_eq!(DEFAULT_GROUP, "DEFAULT_GROUP");
        assert_eq!(DEFAULT_NAMESPACE_ID, "public");
        assert_eq!(DEFAULT_CLUSTER_NAME, "DEFAULT");
        assert_eq!(SDK_GRPC_PORT_DEFAULT_OFFSET, 1000);
        assert_eq!(CLUSTER_GRPC_PORT_DEFAULT_OFFSET, 1001);
    }

    #[test]
    fn test_timeout_constants() {
        assert_eq!(ONCE_TIMEOUT, 2000);
        assert_eq!(SO_TIMEOUT, 60000);
        assert_eq!(CONFIG_LONG_POLL_TIMEOUT, 30000);
        assert_eq!(DEFAULT_HEART_BEAT_TIMEOUT, 15000);
        assert_eq!(DEFAULT_IP_DELETE_TIMEOUT, 30000);
    }

    // Page tests
    #[test]
    fn test_page_default() {
        let page: Page<String> = Page::default();
        assert_eq!(page.total_count, 0);
        assert_eq!(page.page_number, 1);
        assert_eq!(page.pages_available, 0);
        assert!(page.page_items.is_empty());
    }

    #[test]
    fn test_page_new() {
        let items = vec![
            "item1".to_string(),
            "item2".to_string(),
            "item3".to_string(),
        ];
        let page = Page::new(10, 1, 3, items);
        assert_eq!(page.total_count, 10);
        assert_eq!(page.page_number, 1);
        assert_eq!(page.pages_available, 4); // ceil(10/3) = 4
        assert_eq!(page.page_items.len(), 3);
    }

    #[test]
    fn test_page_pages_calculation() {
        let page: Page<i32> = Page::new(25, 1, 10, vec![]);
        assert_eq!(page.pages_available, 3); // ceil(25/10) = 3

        let page2: Page<i32> = Page::new(30, 1, 10, vec![]);
        assert_eq!(page2.pages_available, 3); // exact division

        let page3: Page<i32> = Page::new(0, 1, 10, vec![]);
        assert_eq!(page3.pages_available, 0); // no items
    }

    #[test]
    fn test_page_serialization() {
        let page = Page::new(100, 2, 10, vec!["a".to_string(), "b".to_string()]);
        let json = serde_json::to_string(&page).unwrap();
        assert!(json.contains("totalCount"));
        assert!(json.contains("pageNumber"));
        assert!(json.contains("pagesAvailable"));
        assert!(json.contains("pageItems"));
    }

    // NodeState tests
    #[test]
    fn test_node_state_default() {
        let state = NodeState::default();
        assert!(matches!(state, NodeState::Up));
    }

    #[test]
    fn test_node_state_display() {
        assert_eq!(format!("{}", NodeState::Up), "UP");
        assert_eq!(format!("{}", NodeState::Down), "DOWN");
    }

    // Member and MemberBuilder tests
    #[test]
    fn test_member_builder_basic() {
        let member = MemberBuilder::new("192.168.1.1".to_string(), 8848).build();
        assert_eq!(member.ip, "192.168.1.1");
        assert_eq!(member.port, 8848);
        assert_eq!(member.address, "192.168.1.1:8848");
        assert!(matches!(member.state, NodeState::Up));
    }

    #[test]
    fn test_member_builder_with_state() {
        let member = MemberBuilder::new("10.0.0.1".to_string(), 9000)
            .node_state(NodeState::Starting)
            .build();
        assert!(matches!(member.state, NodeState::Starting));
    }

    #[test]
    fn test_member_builder_chaining() {
        let member = MemberBuilder::new("127.0.0.1".to_string(), 8080)
            .ip("192.168.0.1".to_string())
            .port(9999)
            .node_state(NodeState::Down)
            .build();
        assert_eq!(member.ip, "192.168.0.1");
        assert_eq!(member.port, 9999);
        assert!(matches!(member.state, NodeState::Down));
    }

    #[test]
    fn test_member_fail_access_cnt() {
        let member = MemberBuilder::new("localhost".to_string(), 8848).build();
        assert_eq!(member.fail_access_cnt, 0);
    }

    #[test]
    fn test_member_extend_info() {
        let member = MemberBuilder::new("node1".to_string(), 8848).build();
        let info = member.extend_info.read().unwrap();
        assert!(info.is_empty());
    }

    // Separator and pattern constants
    #[test]
    fn test_separator_constants() {
        assert_eq!(LINE_SEPARATOR, "\u{1}");
        assert_eq!(WORD_SEPARATOR, "\u{2}");
        assert_eq!(LONGPOLLING_LINE_SEPARATOR, "\r\n");
        assert_eq!(SERVICE_INFO_SPLITER, "@@");
    }

    #[test]
    fn test_pattern_constants() {
        assert_eq!(NUMBER_PATTERN_STRING, "^\\d+$");
        assert_eq!(ANY_PATTERN, ".*");
        assert_eq!(ALL_PATTERN, "*");
        assert_eq!(CLUSTER_NAME_PATTERN_STRING, "^[0-9a-zA-Z-]+$");
    }
}
