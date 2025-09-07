use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PageParam {
    #[serde(default = "PageParam::default_page_no")]
    pub page_no: u64,
    #[serde(default = "PageParam::default_page_size")]
    pub page_size: u64,
}

impl PageParam {
    pub fn start(&self) -> u64 {
        (self.page_no - 1) * self.page_size
    }

    fn default_page_no() -> u64 {
        1
    }

    fn default_page_size() -> u64 {
        100
    }
}
