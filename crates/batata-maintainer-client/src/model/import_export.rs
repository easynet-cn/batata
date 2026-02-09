// Configuration import/export model types

use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use std::str::FromStr;

/// Import operation result summary
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub success_count: u32,
    pub skip_count: u32,
    pub fail_count: u32,
    pub fail_data: Vec<ImportFailItem>,
}

/// Details of a failed import item
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFailItem {
    pub data_id: String,
    pub group: String,
    pub reason: String,
}

/// Import conflict resolution policy
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SameConfigPolicy {
    /// Stop import on first conflict
    #[default]
    Abort,
    /// Skip conflicting configs, continue with others
    Skip,
    /// Overwrite existing configs with imported data
    Overwrite,
}

impl Display for SameConfigPolicy {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SameConfigPolicy::Abort => write!(f, "ABORT"),
            SameConfigPolicy::Skip => write!(f, "SKIP"),
            SameConfigPolicy::Overwrite => write!(f, "OVERWRITE"),
        }
    }
}

impl FromStr for SameConfigPolicy {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_uppercase().as_str() {
            "ABORT" => Ok(SameConfigPolicy::Abort),
            "SKIP" => Ok(SameConfigPolicy::Skip),
            "OVERWRITE" => Ok(SameConfigPolicy::Overwrite),
            _ => Err(format!(
                "Invalid policy: {}. Valid values: ABORT, SKIP, OVERWRITE",
                s
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_import_result_serialization() {
        let result = ImportResult {
            success_count: 5,
            skip_count: 1,
            fail_count: 2,
            fail_data: vec![ImportFailItem {
                data_id: "app.yaml".to_string(),
                group: "DEFAULT_GROUP".to_string(),
                reason: "duplicate".to_string(),
            }],
        };

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"successCount\":5"));
        assert!(json.contains("\"failCount\":2"));

        let deserialized: ImportResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.success_count, 5);
        assert_eq!(deserialized.fail_data.len(), 1);
    }

    #[test]
    fn test_same_config_policy_from_str() {
        assert_eq!(
            "ABORT".parse::<SameConfigPolicy>().unwrap(),
            SameConfigPolicy::Abort
        );
        assert_eq!(
            "skip".parse::<SameConfigPolicy>().unwrap(),
            SameConfigPolicy::Skip
        );
        assert_eq!(
            "Overwrite".parse::<SameConfigPolicy>().unwrap(),
            SameConfigPolicy::Overwrite
        );
        assert!("invalid".parse::<SameConfigPolicy>().is_err());
    }

    #[test]
    fn test_same_config_policy_display() {
        assert_eq!(format!("{}", SameConfigPolicy::Abort), "ABORT");
        assert_eq!(format!("{}", SameConfigPolicy::Skip), "SKIP");
        assert_eq!(format!("{}", SameConfigPolicy::Overwrite), "OVERWRITE");
    }
}
