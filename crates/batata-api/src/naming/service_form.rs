//! Surface-independent service create/update payload.
//!
//! The Nacos ADMIN naming surface is form-encoded and carries `metadata` /
//! `selector` as JSON **text**. The CONSOLE surface is JSON and carries them as
//! structured values. Those wire types are not interchangeable, so each
//! surface keeps its own `ServiceForm` deserialization type and normalizes
//! into this shared DTO. Nothing here changes the wire contract.

use std::collections::HashMap;

/// Normalized service create/update fields.
///
/// `metadata` and `selector` are always JSON text here, regardless of which
/// surface produced them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ServiceFormData {
    /// Namespace id, blank/None falls back to the caller's default.
    pub namespace_id: Option<String>,
    /// Group name, blank/None falls back to the caller's default.
    pub group_name: Option<String>,
    /// Service name (required on the wire).
    pub service_name: String,
    /// Service-level ephemeral flag (create only per upstream).
    pub ephemeral: Option<bool>,
    /// Protection threshold.
    pub protect_threshold: Option<f64>,
    /// JSON text for the metadata map.
    pub metadata: Option<String>,
    /// JSON text for the selector object.
    pub selector: Option<String>,
}

/// Convert a CONSOLE-side structured JSON value into the shared JSON text.
///
/// A JSON string value is intentionally passed through as JSON text as well
/// (`.to_string()` quotes it); callers that need to reject double encoding
/// should validate the value before reaching this function.
pub fn normalize_console_value(value: Option<&serde_json::Value>) -> Option<String> {
    value.map(|v| v.to_string())
}

/// Canonicalize ADMIN-side JSON text so both surfaces produce identical text.
///
/// `serde_json::Value::to_string()` emits object keys in sorted order, while a
/// raw form field preserves the caller's order. Parsing and re-serializing
/// removes that cosmetic difference; malformed text is kept verbatim so the
/// existing lenient parse behaviour is unchanged.
pub fn normalize_json_text(text: Option<&str>) -> Option<String> {
    text.map(|raw| {
        serde_json::from_str::<serde_json::Value>(raw)
            .map(|value| value.to_string())
            .unwrap_or_else(|_| raw.to_string())
    })
}

impl ServiceFormData {
    /// Trimmed namespace id or `default` when missing/blank.
    pub fn namespace_id_or_default<'a>(&'a self, default: &'a str) -> &'a str {
        self.namespace_id
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(default)
    }

    /// Trimmed group name or `default` when missing/blank.
    pub fn group_name_or_default<'a>(&'a self, default: &'a str) -> &'a str {
        self.group_name
            .as_deref()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(default)
    }

    /// Parse metadata text, falling back to an empty map on malformed input.
    pub fn metadata_map(&self) -> HashMap<String, String> {
        self.metadata
            .as_deref()
            .and_then(|text| serde_json::from_str(text).ok())
            .unwrap_or_default()
    }

    /// Parse metadata text only when present and valid.
    ///
    /// The ADMIN update path must not overwrite stored metadata with an empty
    /// map when the payload is malformed.
    pub fn try_metadata_map(&self) -> Option<HashMap<String, String>> {
        self.metadata
            .as_deref()
            .and_then(|text| serde_json::from_str(text).ok())
    }

    /// Parse selector text into `(type, expression)`, or `None` when absent.
    pub fn selector_parts(&self) -> Option<(String, String)> {
        let text = self.selector.as_deref()?;
        let obj: serde_json::Value = serde_json::from_str(text).unwrap_or_default();
        Some((
            obj["type"].as_str().unwrap_or("none").to_string(),
            obj["expression"].as_str().unwrap_or("").to_string(),
        ))
    }

    /// Metadata text for datasources that take `&str` (empty when absent).
    pub fn metadata_text(&self) -> String {
        self.metadata.clone().unwrap_or_default()
    }

    /// Selector text for datasources that take `&str` (empty when absent).
    pub fn selector_text(&self) -> String {
        self.selector.clone().unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The canonical payload the two protocol surfaces must agree on.
    fn expected() -> ServiceFormData {
        ServiceFormData {
            namespace_id: Some("public".to_string()),
            group_name: Some("DEFAULT_GROUP".to_string()),
            service_name: "svc".to_string(),
            ephemeral: Some(true),
            protect_threshold: Some(0.5),
            metadata: Some(r#"{"env":"test"}"#.to_string()),
            selector: Some(r#"{"expression":"env=test","type":"label"}"#.to_string()),
        }
    }

    fn common() -> ServiceFormData {
        ServiceFormData {
            namespace_id: Some("public".to_string()),
            group_name: Some("DEFAULT_GROUP".to_string()),
            service_name: "svc".to_string(),
            ephemeral: Some(true),
            protect_threshold: Some(0.5),
            ..Default::default()
        }
    }

    fn from_admin() -> ServiceFormData {
        // ADMIN: metadata/selector arrive as JSON text in form fields.
        let mut form = common();
        form.metadata = normalize_json_text(Some(r#"{"env":"test"}"#));
        form.selector = normalize_json_text(Some(r#"{"type":"label","expression":"env=test"}"#));
        form
    }

    fn from_console() -> ServiceFormData {
        // CONSOLE: metadata/selector arrive as structured JSON values.
        let metadata = Some(json!({ "env": "test" }));
        let selector = Some(json!({ "type": "label", "expression": "env=test" }));
        let mut form = common();
        form.metadata = normalize_console_value(metadata.as_ref());
        form.selector = normalize_console_value(selector.as_ref());
        form
    }

    #[test]
    fn both_protocol_surfaces_normalize_to_the_same_dto() {
        assert_eq!(from_admin(), expected());
        assert_eq!(from_console(), expected());
    }

    #[test]
    fn admin_text_canonicalizes_keys_but_keeps_malformed_text() {
        assert_eq!(
            normalize_json_text(Some(r#"{"type":"label","expression":"x"}"#)),
            Some(r#"{"expression":"x","type":"label"}"#.to_string())
        );
        assert_eq!(
            normalize_json_text(Some("not json")),
            Some("not json".to_string())
        );
        assert_eq!(normalize_json_text(None), None);
    }

    #[test]
    fn namespace_and_group_defaults_trim_and_fall_back() {
        let mut form = ServiceFormData::default();
        assert_eq!(form.namespace_id_or_default("public"), "public");
        assert_eq!(form.group_name_or_default("DEFAULT_GROUP"), "DEFAULT_GROUP");

        form.namespace_id = Some("  ns-a  ".to_string());
        form.group_name = Some("   ".to_string());
        assert_eq!(form.namespace_id_or_default("public"), "ns-a");
        assert_eq!(form.group_name_or_default("DEFAULT_GROUP"), "DEFAULT_GROUP");
    }

    #[test]
    fn metadata_parse_is_lenient_but_update_is_strict() {
        let form = ServiceFormData {
            metadata: Some(r#"{"env":"test"}"#.to_string()),
            ..Default::default()
        };
        assert_eq!(
            form.metadata_map().get("env").map(String::as_str),
            Some("test")
        );
        assert_eq!(
            form.try_metadata_map().and_then(|m| m.get("env").cloned()),
            Some("test".to_string())
        );

        let broken = ServiceFormData {
            metadata: Some("not json".to_string()),
            ..Default::default()
        };
        assert!(broken.metadata_map().is_empty());
        assert!(broken.try_metadata_map().is_none());
    }

    #[test]
    fn selector_parts_distinguish_absent_from_malformed() {
        let absent = ServiceFormData::default();
        assert_eq!(absent.selector_parts(), None);

        let malformed = ServiceFormData {
            selector: Some("not json".to_string()),
            ..Default::default()
        };
        assert_eq!(
            malformed.selector_parts(),
            Some(("none".to_string(), String::new()))
        );

        assert_eq!(
            from_admin().selector_parts(),
            Some(("label".to_string(), "env=test".to_string()))
        );
        assert_eq!(from_admin().metadata_text(), r#"{"env":"test"}"#);
        assert_eq!(
            from_admin().selector_text(),
            r#"{"expression":"env=test","type":"label"}"#
        );
    }
}
