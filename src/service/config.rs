use std::collections::HashMap;

use crate::common::model::{ConfigInfo, Page};

pub async fn find_config_info_like_4_page(
    page_no: i32,
    page_size: i32,
    data_id: String,
    group: String,
    tenant: String,
    config_advance_info: HashMap<String, String>,
) -> Page<ConfigInfo> {
    let mut page_result = Page::<ConfigInfo> {
        total_count: 1,
        page_number: 1,
        pages_available: 1,
        page_items: vec![],
    };

    page_result
}
