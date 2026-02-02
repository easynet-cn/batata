//! Service selector evaluation for instance filtering
//!
//! This module provides selector evaluation for filtering service instances
//! based on metadata labels and other criteria.

use std::collections::HashMap;

use regex::Regex;
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::Instance;

/// Selector type for filtering instances
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SelectorType {
    /// No filtering (return all instances)
    None,
    /// Filter by label equality
    Label,
    /// Filter by expression (more complex queries)
    Expression,
}

impl Default for SelectorType {
    fn default() -> Self {
        SelectorType::None
    }
}

/// Label requirement for selector
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LabelRequirement {
    /// Label key
    pub key: String,
    /// Operator for comparison
    pub operator: LabelOperator,
    /// Values for comparison (for In/NotIn operators)
    #[serde(default)]
    pub values: Vec<String>,
}

impl LabelRequirement {
    /// Create a new label requirement with Equals operator
    pub fn equals(key: &str, value: &str) -> Self {
        Self {
            key: key.to_string(),
            operator: LabelOperator::Equals,
            values: vec![value.to_string()],
        }
    }

    /// Create a new label requirement with NotEquals operator
    pub fn not_equals(key: &str, value: &str) -> Self {
        Self {
            key: key.to_string(),
            operator: LabelOperator::NotEquals,
            values: vec![value.to_string()],
        }
    }

    /// Create a new label requirement with In operator
    pub fn in_set(key: &str, values: Vec<&str>) -> Self {
        Self {
            key: key.to_string(),
            operator: LabelOperator::In,
            values: values.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Create a new label requirement with NotIn operator
    pub fn not_in_set(key: &str, values: Vec<&str>) -> Self {
        Self {
            key: key.to_string(),
            operator: LabelOperator::NotIn,
            values: values.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    /// Create a new label requirement with Exists operator
    pub fn exists(key: &str) -> Self {
        Self {
            key: key.to_string(),
            operator: LabelOperator::Exists,
            values: vec![],
        }
    }

    /// Create a new label requirement with DoesNotExist operator
    pub fn does_not_exist(key: &str) -> Self {
        Self {
            key: key.to_string(),
            operator: LabelOperator::DoesNotExist,
            values: vec![],
        }
    }

    /// Create a new label requirement with Regex operator
    pub fn regex(key: &str, pattern: &str) -> Self {
        Self {
            key: key.to_string(),
            operator: LabelOperator::Regex,
            values: vec![pattern.to_string()],
        }
    }

    /// Check if an instance's metadata matches this requirement
    pub fn matches(&self, metadata: &HashMap<String, String>) -> bool {
        match &self.operator {
            LabelOperator::Equals => {
                metadata
                    .get(&self.key)
                    .map(|v| self.values.first().map_or(false, |expected| v == expected))
                    .unwrap_or(false)
            }
            LabelOperator::NotEquals => {
                metadata
                    .get(&self.key)
                    .map(|v| self.values.first().map_or(true, |expected| v != expected))
                    .unwrap_or(true)
            }
            LabelOperator::In => {
                metadata
                    .get(&self.key)
                    .map(|v| self.values.contains(v))
                    .unwrap_or(false)
            }
            LabelOperator::NotIn => {
                metadata
                    .get(&self.key)
                    .map(|v| !self.values.contains(v))
                    .unwrap_or(true)
            }
            LabelOperator::Exists => metadata.contains_key(&self.key),
            LabelOperator::DoesNotExist => !metadata.contains_key(&self.key),
            LabelOperator::Regex => {
                if let Some(pattern) = self.values.first() {
                    if let Ok(re) = Regex::new(pattern) {
                        metadata
                            .get(&self.key)
                            .map(|v| re.is_match(v))
                            .unwrap_or(false)
                    } else {
                        warn!("Invalid regex pattern: {}", pattern);
                        false
                    }
                } else {
                    false
                }
            }
            LabelOperator::GreaterThan => {
                if let Some(expected) = self.values.first() {
                    metadata.get(&self.key).map_or(false, |v| {
                        v.parse::<f64>()
                            .ok()
                            .and_then(|val| expected.parse::<f64>().ok().map(|exp| val > exp))
                            .unwrap_or(false)
                    })
                } else {
                    false
                }
            }
            LabelOperator::LessThan => {
                if let Some(expected) = self.values.first() {
                    metadata.get(&self.key).map_or(false, |v| {
                        v.parse::<f64>()
                            .ok()
                            .and_then(|val| expected.parse::<f64>().ok().map(|exp| val < exp))
                            .unwrap_or(false)
                    })
                } else {
                    false
                }
            }
        }
    }
}

/// Label operator for comparison
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum LabelOperator {
    /// Equal to value
    Equals,
    /// Not equal to value
    NotEquals,
    /// Value is in set
    In,
    /// Value is not in set
    NotIn,
    /// Key exists (value doesn't matter)
    Exists,
    /// Key does not exist
    DoesNotExist,
    /// Value matches regex pattern
    Regex,
    /// Value is greater than (numeric comparison)
    GreaterThan,
    /// Value is less than (numeric comparison)
    LessThan,
}

/// Service selector for filtering instances
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceSelector {
    /// Selector type
    #[serde(default)]
    pub selector_type: SelectorType,
    /// Label requirements (all must match - AND logic)
    #[serde(default)]
    pub requirements: Vec<LabelRequirement>,
    /// Expression string (for backward compatibility with Nacos)
    #[serde(default)]
    pub expression: String,
}

impl ServiceSelector {
    /// Create a new empty selector (matches all instances)
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a label selector with requirements
    pub fn with_requirements(requirements: Vec<LabelRequirement>) -> Self {
        Self {
            selector_type: SelectorType::Label,
            requirements,
            expression: String::new(),
        }
    }

    /// Create a selector from expression string
    pub fn with_expression(expression: &str) -> Self {
        Self {
            selector_type: SelectorType::Expression,
            requirements: vec![],
            expression: expression.to_string(),
        }
    }

    /// Parse selector from JSON string (Nacos format)
    ///
    /// Nacos selector format:
    /// ```json
    /// {"type":"label","expression":"key1=value1,key2=value2"}
    /// ```
    pub fn parse(selector_str: &str) -> Result<Self, String> {
        if selector_str.is_empty() {
            return Ok(Self::new());
        }

        // Try to parse as JSON
        if selector_str.starts_with('{') {
            return serde_json::from_str(selector_str)
                .map_err(|e| format!("Failed to parse selector JSON: {}", e));
        }

        // Parse as simple expression string (key1=value1,key2=value2)
        let requirements = Self::parse_expression(selector_str)?;
        Ok(Self {
            selector_type: SelectorType::Label,
            requirements,
            expression: selector_str.to_string(),
        })
    }

    /// Parse expression string into label requirements
    ///
    /// Supported formats:
    /// - key=value (equals)
    /// - key!=value (not equals)
    /// - key (exists)
    /// - !key (does not exist)
    /// - key in (v1,v2) (in set)
    /// - key notin (v1,v2) (not in set)
    fn parse_expression(expression: &str) -> Result<Vec<LabelRequirement>, String> {
        let mut requirements = Vec::new();

        // First, handle "in" and "notin" expressions which contain commas in their values
        // We need to parse these before splitting by comma
        let expression = expression.trim();

        // Check if the entire expression is a single "in" or "notin" expression
        if let Some(idx) = expression.find(" in (") {
            let key = expression[..idx].trim();
            if let Some(end_idx) = expression.find(')') {
                let values_str = &expression[idx + 5..end_idx];
                let values: Vec<String> = values_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                requirements.push(LabelRequirement {
                    key: key.to_string(),
                    operator: LabelOperator::In,
                    values,
                });
                // Check if there's more after the closing paren
                if end_idx + 1 < expression.len() {
                    let rest = expression[end_idx + 1..].trim();
                    if rest.starts_with(',') {
                        let more = Self::parse_expression(&rest[1..])?;
                        requirements.extend(more);
                    }
                }
                return Ok(requirements);
            }
        }

        if let Some(idx) = expression.find(" notin (") {
            let key = expression[..idx].trim();
            if let Some(end_idx) = expression.find(')') {
                let values_str = &expression[idx + 8..end_idx];
                let values: Vec<String> = values_str
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .collect();
                requirements.push(LabelRequirement {
                    key: key.to_string(),
                    operator: LabelOperator::NotIn,
                    values,
                });
                // Check if there's more after the closing paren
                if end_idx + 1 < expression.len() {
                    let rest = expression[end_idx + 1..].trim();
                    if rest.starts_with(',') {
                        let more = Self::parse_expression(&rest[1..])?;
                        requirements.extend(more);
                    }
                }
                return Ok(requirements);
            }
        }

        // For simple expressions without "in" or "notin", split by comma
        for part in expression.split(',') {
            let part = part.trim();
            if part.is_empty() {
                continue;
            }

            // Parse "key!=value" format
            if let Some(idx) = part.find("!=") {
                let key = part[..idx].trim();
                let value = part[idx + 2..].trim();
                requirements.push(LabelRequirement::not_equals(key, value));
                continue;
            }

            // Parse "key=value" format
            if let Some(idx) = part.find('=') {
                let key = part[..idx].trim();
                let value = part[idx + 1..].trim();
                requirements.push(LabelRequirement::equals(key, value));
                continue;
            }

            // Parse "!key" format (does not exist)
            if part.starts_with('!') {
                let key = part[1..].trim();
                requirements.push(LabelRequirement::does_not_exist(key));
                continue;
            }

            // Parse "key" format (exists)
            requirements.push(LabelRequirement::exists(part));
        }

        Ok(requirements)
    }

    /// Check if an instance matches this selector
    pub fn matches(&self, instance: &Instance) -> bool {
        match self.selector_type {
            SelectorType::None => true,
            SelectorType::Label => {
                // All requirements must match (AND logic)
                self.requirements
                    .iter()
                    .all(|req| req.matches(&instance.metadata))
            }
            SelectorType::Expression => {
                // Parse expression on-demand if requirements are empty
                if self.requirements.is_empty() && !self.expression.is_empty() {
                    if let Ok(requirements) = Self::parse_expression(&self.expression) {
                        return requirements
                            .iter()
                            .all(|req| req.matches(&instance.metadata));
                    }
                }
                self.requirements
                    .iter()
                    .all(|req| req.matches(&instance.metadata))
            }
        }
    }

    /// Filter instances based on selector
    pub fn filter<'a>(&self, instances: &'a [Instance]) -> Vec<&'a Instance> {
        instances.iter().filter(|inst| self.matches(inst)).collect()
    }

    /// Filter instances and return owned copies
    pub fn filter_owned(&self, instances: &[Instance]) -> Vec<Instance> {
        instances
            .iter()
            .filter(|inst| self.matches(inst))
            .cloned()
            .collect()
    }
}

/// Builder for creating service selectors
pub struct SelectorBuilder {
    requirements: Vec<LabelRequirement>,
}

impl SelectorBuilder {
    /// Create a new selector builder
    pub fn new() -> Self {
        Self {
            requirements: Vec::new(),
        }
    }

    /// Add an equals requirement
    pub fn equals(mut self, key: &str, value: &str) -> Self {
        self.requirements.push(LabelRequirement::equals(key, value));
        self
    }

    /// Add a not equals requirement
    pub fn not_equals(mut self, key: &str, value: &str) -> Self {
        self.requirements
            .push(LabelRequirement::not_equals(key, value));
        self
    }

    /// Add an in set requirement
    pub fn in_set(mut self, key: &str, values: Vec<&str>) -> Self {
        self.requirements
            .push(LabelRequirement::in_set(key, values));
        self
    }

    /// Add a not in set requirement
    pub fn not_in_set(mut self, key: &str, values: Vec<&str>) -> Self {
        self.requirements
            .push(LabelRequirement::not_in_set(key, values));
        self
    }

    /// Add an exists requirement
    pub fn exists(mut self, key: &str) -> Self {
        self.requirements.push(LabelRequirement::exists(key));
        self
    }

    /// Add a does not exist requirement
    pub fn does_not_exist(mut self, key: &str) -> Self {
        self.requirements
            .push(LabelRequirement::does_not_exist(key));
        self
    }

    /// Add a regex requirement
    pub fn regex(mut self, key: &str, pattern: &str) -> Self {
        self.requirements.push(LabelRequirement::regex(key, pattern));
        self
    }

    /// Build the selector
    pub fn build(self) -> ServiceSelector {
        ServiceSelector::with_requirements(self.requirements)
    }
}

impl Default for SelectorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_instance_with_metadata(metadata: HashMap<String, String>) -> Instance {
        Instance {
            metadata,
            ..Default::default()
        }
    }

    #[test]
    fn test_label_equals() {
        let req = LabelRequirement::equals("env", "prod");

        let mut meta = HashMap::new();
        meta.insert("env".to_string(), "prod".to_string());
        assert!(req.matches(&meta));

        meta.insert("env".to_string(), "dev".to_string());
        assert!(!req.matches(&meta));

        let empty_meta = HashMap::new();
        assert!(!req.matches(&empty_meta));
    }

    #[test]
    fn test_label_not_equals() {
        let req = LabelRequirement::not_equals("env", "prod");

        let mut meta = HashMap::new();
        meta.insert("env".to_string(), "dev".to_string());
        assert!(req.matches(&meta));

        meta.insert("env".to_string(), "prod".to_string());
        assert!(!req.matches(&meta));

        // Missing key should match (not equals)
        let empty_meta = HashMap::new();
        assert!(req.matches(&empty_meta));
    }

    #[test]
    fn test_label_in() {
        let req = LabelRequirement::in_set("region", vec!["us-east", "us-west"]);

        let mut meta = HashMap::new();
        meta.insert("region".to_string(), "us-east".to_string());
        assert!(req.matches(&meta));

        meta.insert("region".to_string(), "eu-west".to_string());
        assert!(!req.matches(&meta));
    }

    #[test]
    fn test_label_not_in() {
        let req = LabelRequirement::not_in_set("region", vec!["us-east", "us-west"]);

        let mut meta = HashMap::new();
        meta.insert("region".to_string(), "eu-west".to_string());
        assert!(req.matches(&meta));

        meta.insert("region".to_string(), "us-east".to_string());
        assert!(!req.matches(&meta));
    }

    #[test]
    fn test_label_exists() {
        let req = LabelRequirement::exists("version");

        let mut meta = HashMap::new();
        meta.insert("version".to_string(), "1.0".to_string());
        assert!(req.matches(&meta));

        let empty_meta = HashMap::new();
        assert!(!req.matches(&empty_meta));
    }

    #[test]
    fn test_label_does_not_exist() {
        let req = LabelRequirement::does_not_exist("deprecated");

        let meta = HashMap::new();
        assert!(req.matches(&meta));

        let mut meta_with_key = HashMap::new();
        meta_with_key.insert("deprecated".to_string(), "true".to_string());
        assert!(!req.matches(&meta_with_key));
    }

    #[test]
    fn test_label_regex() {
        let req = LabelRequirement::regex("version", r"^v\d+\.\d+$");

        let mut meta = HashMap::new();
        meta.insert("version".to_string(), "v1.0".to_string());
        assert!(req.matches(&meta));

        meta.insert("version".to_string(), "1.0".to_string());
        assert!(!req.matches(&meta));
    }

    #[test]
    fn test_selector_multiple_requirements() {
        let selector = SelectorBuilder::new()
            .equals("env", "prod")
            .exists("version")
            .not_in_set("region", vec!["eu-west"])
            .build();

        let mut meta = HashMap::new();
        meta.insert("env".to_string(), "prod".to_string());
        meta.insert("version".to_string(), "1.0".to_string());
        meta.insert("region".to_string(), "us-east".to_string());

        let instance = create_instance_with_metadata(meta);
        assert!(selector.matches(&instance));
    }

    #[test]
    fn test_selector_parse_expression() {
        // Test simple equals
        let selector = ServiceSelector::parse("env=prod").unwrap();
        let mut meta = HashMap::new();
        meta.insert("env".to_string(), "prod".to_string());
        let instance = create_instance_with_metadata(meta);
        assert!(selector.matches(&instance));

        // Test multiple conditions
        let selector = ServiceSelector::parse("env=prod,region=us-east").unwrap();
        let mut meta = HashMap::new();
        meta.insert("env".to_string(), "prod".to_string());
        meta.insert("region".to_string(), "us-east".to_string());
        let instance = create_instance_with_metadata(meta);
        assert!(selector.matches(&instance));
    }

    #[test]
    fn test_selector_parse_in_expression() {
        let selector = ServiceSelector::parse("region in (us-east,us-west)").unwrap();

        let mut meta = HashMap::new();
        meta.insert("region".to_string(), "us-east".to_string());
        let instance = create_instance_with_metadata(meta);
        assert!(selector.matches(&instance));

        let mut meta2 = HashMap::new();
        meta2.insert("region".to_string(), "eu-west".to_string());
        let instance2 = create_instance_with_metadata(meta2);
        assert!(!selector.matches(&instance2));
    }

    #[test]
    fn test_selector_filter() {
        let selector = SelectorBuilder::new().equals("env", "prod").build();

        let instances = vec![
            {
                let mut meta = HashMap::new();
                meta.insert("env".to_string(), "prod".to_string());
                create_instance_with_metadata(meta)
            },
            {
                let mut meta = HashMap::new();
                meta.insert("env".to_string(), "dev".to_string());
                create_instance_with_metadata(meta)
            },
            {
                let mut meta = HashMap::new();
                meta.insert("env".to_string(), "prod".to_string());
                create_instance_with_metadata(meta)
            },
        ];

        let filtered = selector.filter(&instances);
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_empty_selector_matches_all() {
        let selector = ServiceSelector::new();
        let instance = Instance::default();
        assert!(selector.matches(&instance));
    }

    #[test]
    fn test_selector_json_parse() {
        let json = r#"{"selectorType":"label","requirements":[{"key":"env","operator":"equals","values":["prod"]}]}"#;
        let selector = ServiceSelector::parse(json).unwrap();

        let mut meta = HashMap::new();
        meta.insert("env".to_string(), "prod".to_string());
        let instance = create_instance_with_metadata(meta);
        assert!(selector.matches(&instance));
    }
}
