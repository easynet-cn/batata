//! MCP import validation.
//!
//! Mirrors upstream `McpServerValidationService.validateServers` and
//! `validateSingleServer`. Upstream validates a batch of `McpServerDetailInfo`
//! into per-server verdicts before any of them are written, so the console can
//! show what would fail.
//!
//! The rules are a pure function of the payload plus an injected "does this
//! server already exist?" predicate, which keeps them unit-testable without a
//! database. The HTTP layer resolves existence first and passes the answers in.

use std::collections::HashSet;

use serde_json::Value;

use super::mcp::{
    McpServerImportItem, McpServerImportValidationResult, McpServerRegistration, McpServerType,
    McpServerValidationItem, McpTransport, VALID_MCP_PROTOCOLS, validation_status,
};

/// The `stdio` protocol, which is the only one satisfied by a local config.
const PROTOCOL_STDIO: &str = "stdio";

/// Parse an import payload into the servers it contains.
///
/// Three shapes are accepted, because the console does not guarantee one:
///
/// - `{"servers": [ ... ]}` — the MCP registry shape
/// - `[ ... ]` — a bare array
/// - `{ "<name>": { ... } }` — a map keyed by server name; the key supplies
///   `name` when the entry omits it
pub fn parse_import_payload(content: &str) -> Result<Vec<McpServerImportItem>, String> {
    let value: Value = serde_json::from_str(content).map_err(|e| format!("Invalid JSON: {e}"))?;

    match value {
        Value::Array(items) => items.into_iter().map(deserialize_item).collect(),
        Value::Object(mut map) => match map.remove("servers") {
            Some(Value::Array(items)) => items.into_iter().map(deserialize_item).collect(),
            Some(_) => Err("\"servers\" must be an array".to_string()),
            // No `servers` key: treat the object as name -> server.
            None => map
                .into_iter()
                .map(|(key, entry)| {
                    let mut item = deserialize_item(entry)?;
                    if item.name.is_none() {
                        item.name = Some(key);
                    }
                    Ok(item)
                })
                .collect(),
        },
        _ => Err("Import content must be a JSON object or array".to_string()),
    }
}

fn deserialize_item(value: Value) -> Result<McpServerImportItem, String> {
    serde_json::from_value(value).map_err(|e| format!("Invalid server entry: {e}"))
}

/// Validate one import batch.
///
/// `exists` reports whether `(name, version)` is already present in the target
/// namespace. Duplicate detection covers both the batch itself and the
/// namespace, exactly as upstream does.
///
/// Upstream sets `valid` to `invalidCount == 0`: duplicates are reported in
/// their own count and do **not** by themselves make the batch invalid.
pub fn validate_servers<F>(
    servers: &[McpServerImportItem],
    exists: F,
) -> McpServerImportValidationResult
where
    F: Fn(&str, &str) -> bool,
{
    let mut items = Vec::with_capacity(servers.len());
    let mut seen: HashSet<String> = HashSet::new();
    let mut valid_count = 0u32;
    let mut invalid_count = 0u32;
    let mut duplicate_count = 0u32;

    for server in servers {
        let name = server.name.clone().unwrap_or_default();
        let version = server.version().to_string();
        let mut errors = Vec::new();
        let mut status = String::new();
        let mut already_exists = false;

        // ---- required fields ------------------------------------------------
        if name.trim().is_empty() {
            errors.push("Server name is required".to_string());
        }
        match server.protocol.as_deref() {
            None | Some("") => errors.push("Protocol is required".to_string()),
            Some(protocol) => {
                if !VALID_MCP_PROTOCOLS.contains(&protocol) {
                    errors.push(format!("Invalid protocol: {protocol}"));
                }
            }
        }
        if server
            .description
            .as_deref()
            .unwrap_or_default()
            .trim()
            .is_empty()
        {
            errors.push("Description is required".to_string());
        }

        // ---- duplicates inside this batch -----------------------------------
        // Upstream keys on name + version, so different versions of one server
        // are not duplicates of each other.
        let key = format!("{name}{version}");
        if seen.contains(&key) {
            errors.push(format!("Duplicate server name in import batch: {name}"));
            status = validation_status::DUPLICATE.to_string();
        } else {
            seen.insert(key);
        }

        // ---- already present in the namespace -------------------------------
        if !name.trim().is_empty() && exists(&name, &version) {
            already_exists = true;
            if status != validation_status::DUPLICATE {
                status = validation_status::DUPLICATE.to_string();
                errors.push(format!("Server already exists: {name}"));
            }
        }

        validate_protocol_specific_config(server, &mut errors);

        // ---- verdict --------------------------------------------------------
        if errors.is_empty() {
            status = validation_status::VALID.to_string();
        } else if status != validation_status::DUPLICATE {
            status = validation_status::INVALID.to_string();
        }

        match status.as_str() {
            validation_status::VALID => valid_count += 1,
            validation_status::DUPLICATE => duplicate_count += 1,
            _ => invalid_count += 1,
        }

        items.push(McpServerValidationItem {
            server_name: server.name.clone(),
            server_id: server.id.clone(),
            status,
            errors,
            exists: already_exists,
            selected: true,
        });
    }

    McpServerImportValidationResult {
        valid: invalid_count == 0,
        total_count: servers.len() as u32,
        valid_count,
        invalid_count,
        duplicate_count,
        servers: items,
        errors: Vec::new(),
    }
}

/// Convert one validated import item into a server registration.
///
/// `streamable` maps to `Http` because it is HTTP-based. `dubbo` has no Batata
/// equivalent and is **rejected** rather than silently downgraded to another
/// protocol. Tools are not mapped: the payload's tool shape is not Batata's
/// `McpTool`, and they can be re-fetched through `importToolsFromMcp`.
pub fn import_item_to_registration(
    namespace: &str,
    item: &McpServerImportItem,
) -> Result<McpServerRegistration, String> {
    let protocol = item.protocol.as_deref().unwrap_or_default();
    let server_type = match protocol {
        "stdio" => McpServerType::Stdio,
        "sse" => McpServerType::Sse,
        "http" | "streamable" => McpServerType::Http,
        other => return Err(format!("protocol '{other}' has no Batata server type")),
    };

    let endpoint = item
        .remote_server_config
        .as_ref()
        .and_then(|value| value.get("url"))
        .and_then(|url| url.as_str())
        .unwrap_or_default()
        .to_string();

    let command = item
        .local_server_config
        .as_ref()
        .and_then(|value| value.get("command"))
        .and_then(|command| command.as_str())
        .map(str::to_string);

    let name = item.name.clone().unwrap_or_default();
    let version = item.version();

    Ok(McpServerRegistration {
        name: name.clone(),
        display_name: name,
        description: item.description.clone().unwrap_or_default(),
        namespace: namespace.to_string(),
        version: if version.is_empty() {
            "1.0.0".to_string()
        } else {
            version.to_string()
        },
        endpoint,
        server_type,
        transport: McpTransport {
            transport_type: protocol.to_string(),
            command,
            url: if endpoint_empty(protocol) {
                None
            } else {
                Some(
                    item.remote_server_config
                        .as_ref()
                        .and_then(|v| v.get("url"))
                        .and_then(|u| u.as_str())
                        .unwrap_or_default()
                        .to_string(),
                )
            },
            ..Default::default()
        },
        capabilities: Default::default(),
        tools: Vec::new(),
        resources: Vec::new(),
        prompts: Vec::new(),
        metadata: std::collections::HashMap::new(),
        tags: Vec::new(),
        auto_fetch_tools: false,
        health_check: None,
    })
}

/// Whether the protocol carries no HTTP URL.
fn endpoint_empty(protocol: &str) -> bool {
    protocol == "stdio"
}

/// Choose the items an import should attempt.
///
/// Mirrors upstream `McpServerImportService.filterValidSelectedServers`, with
/// one deliberate difference: when the caller passes no explicit selection,
/// upstream returns **every** item, including `invalid` ones, which then fail
/// one by one — that contradicts the point of `skipInvalid`. Here the
/// unselected case keeps everything that is not `invalid`, so already-existing
/// servers (`duplicate`) can still be updated when overwrite is requested
/// while genuinely broken entries are never attempted.
///
/// When `selected_ids` is non-empty, upstream's rule applies exactly: only
/// `valid` items whose server id is selected.
///
/// Returns the **indices** of the chosen items, so the caller can look up the
/// matching source entry and its verdict together.
pub fn select_importable_items(
    items: &[McpServerValidationItem],
    selected_ids: &[String],
) -> Vec<usize> {
    if items.is_empty() {
        return Vec::new();
    }

    if selected_ids.is_empty() {
        return items
            .iter()
            .enumerate()
            .filter(|(_, item)| item.status != validation_status::INVALID)
            .map(|(index, _)| index)
            .collect();
    }

    items
        .iter()
        .enumerate()
        .filter(|(_, item)| item.status == validation_status::VALID)
        .filter(|(_, item)| {
            item.server_id
                .as_deref()
                .is_some_and(|id| selected_ids.iter().any(|s| s == id))
        })
        .map(|(index, _)| index)
        .collect()
}

/// Protocol-specific configuration checks.
///
/// `stdio` needs a local server config or packages; every other protocol needs
/// a remote server config. A supplied `toolSpec` must declare at least one
/// tool. Upstream runs the remote-config check even when the protocol is
/// blank, so this does too.
fn validate_protocol_specific_config(server: &McpServerImportItem, errors: &mut Vec<String>) {
    let protocol = server.protocol.as_deref().unwrap_or_default();

    if protocol == PROTOCOL_STDIO {
        let has_local = server.local_server_config.is_some();
        let has_packages = server
            .packages
            .as_ref()
            .map(|p| !p.is_empty())
            .unwrap_or(false);
        if !has_local && !has_packages {
            errors.push(
                "Local server configuration or packages are required for stdio protocol"
                    .to_string(),
            );
        }
    } else if server.remote_server_config.is_none() {
        errors.push(format!(
            "Remote server configuration is required for {protocol} protocol"
        ));
    }

    if let Some(spec) = &server.tool_spec {
        let empty = spec.tools.as_ref().map(|t| t.is_empty()).unwrap_or(true);
        if empty {
            errors.push("Tool specification should contain at least one tool".to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ai::mcp::{McpImportToolSpec, McpImportVersionDetail};

    fn item(name: &str, protocol: &str, version: &str) -> McpServerImportItem {
        McpServerImportItem {
            name: Some(name.to_string()),
            protocol: Some(protocol.to_string()),
            description: Some("a server".to_string()),
            version: Some(version.to_string()),
            remote_server_config: Some(Value::Object(Default::default())),
            ..Default::default()
        }
    }

    fn no_exist(_name: &str, _version: &str) -> bool {
        false
    }

    #[test]
    fn a_complete_server_is_valid() {
        let result = validate_servers(&[item("a", "http", "1.0.0")], no_exist);
        assert!(result.valid);
        assert_eq!(result.valid_count, 1);
        assert_eq!(result.invalid_count, 0);
        assert_eq!(result.servers[0].status, validation_status::VALID);
    }

    #[test]
    fn missing_fields_are_reported() {
        let result = validate_servers(&[McpServerImportItem::default()], no_exist);
        assert!(!result.valid);
        let errors = &result.servers[0].errors;
        assert!(errors.iter().any(|e| e.contains("Server name is required")));
        assert!(errors.iter().any(|e| e.contains("Protocol is required")));
        assert!(errors.iter().any(|e| e.contains("Description is required")));
    }

    #[test]
    fn an_unknown_protocol_is_rejected() {
        let result = validate_servers(&[item("a", "carrier-pigeon", "1.0.0")], no_exist);
        assert!(!result.valid);
        assert!(
            result.servers[0]
                .errors
                .iter()
                .any(|e| e.contains("Invalid protocol: carrier-pigeon"))
        );
    }

    #[test]
    fn all_upstream_protocols_are_accepted() {
        for protocol in VALID_MCP_PROTOCOLS {
            let mut server = item("a", protocol, "1.0.0");
            if protocol == PROTOCOL_STDIO {
                server.remote_server_config = None;
                server.local_server_config = Some(Value::Object(Default::default()));
            }
            let result = validate_servers(&[server], no_exist);
            assert!(result.valid, "protocol {protocol} must be accepted");
        }
    }

    #[test]
    fn stdio_needs_a_local_config_or_packages() {
        let server = McpServerImportItem {
            name: Some("a".to_string()),
            protocol: Some("stdio".to_string()),
            description: Some("d".to_string()),
            ..Default::default()
        };
        let result = validate_servers(&[server], no_exist);
        assert!(!result.valid);
        assert!(
            result.servers[0]
                .errors
                .iter()
                .any(|e| e.contains("required for stdio protocol"))
        );
    }

    #[test]
    fn non_stdio_needs_a_remote_config() {
        let server = McpServerImportItem {
            name: Some("a".to_string()),
            protocol: Some("http".to_string()),
            description: Some("d".to_string()),
            ..Default::default()
        };
        let result = validate_servers(&[server], no_exist);
        assert!(
            result.servers[0]
                .errors
                .iter()
                .any(|e| e.contains("Remote server configuration is required for http"))
        );
    }

    #[test]
    fn an_empty_tool_spec_is_rejected() {
        let mut server = item("a", "http", "1.0.0");
        server.tool_spec = Some(McpImportToolSpec {
            tools: Some(Vec::new()),
        });
        let result = validate_servers(&[server], no_exist);
        assert!(!result.valid);
        assert!(
            result.servers[0]
                .errors
                .iter()
                .any(|e| e.contains("at least one tool"))
        );
    }

    #[test]
    fn duplicates_inside_the_batch_are_counted_separately() {
        let servers = vec![item("a", "http", "1.0.0"), item("a", "http", "1.0.0")];
        let result = validate_servers(&servers, no_exist);
        assert_eq!(result.duplicate_count, 1);
        assert_eq!(result.servers[1].status, validation_status::DUPLICATE);
        // Duplicates do not make the batch invalid upstream.
        assert!(result.valid, "valid is invalidCount == 0");
    }

    #[test]
    fn the_same_name_at_different_versions_is_not_a_duplicate() {
        let servers = vec![item("a", "http", "1.0.0"), item("a", "http", "2.0.0")];
        let result = validate_servers(&servers, no_exist);
        assert_eq!(result.duplicate_count, 0);
        assert!(result.valid);
    }

    #[test]
    fn an_existing_server_is_marked_duplicate() {
        let result = validate_servers(&[item("a", "http", "1.0.0")], |name, _| name == "a");
        assert_eq!(result.duplicate_count, 1);
        assert!(result.servers[0].exists);
        assert!(
            result.servers[0]
                .errors
                .iter()
                .any(|e| e.contains("Server already exists: a"))
        );
    }

    #[test]
    fn version_detail_takes_precedence_over_version() {
        let mut server = item("a", "http", "1.0.0");
        server.version_detail = Some(McpImportVersionDetail {
            version: Some("2.0.0".to_string()),
        });
        assert_eq!(server.version(), "2.0.0");
        // The batch key uses the effective version, so these are not duplicates.
        let other = item("a", "http", "1.0.0");
        let result = validate_servers(&[server, other], no_exist);
        assert_eq!(result.duplicate_count, 0);
    }

    #[test]
    fn counts_add_up_to_the_total() {
        let servers = vec![
            item("ok", "http", "1.0.0"),
            item("bad", "nope", "1.0.0"),
            item("ok", "http", "1.0.0"),
        ];
        let result = validate_servers(&servers, no_exist);
        assert_eq!(result.total_count, 3);
        assert_eq!(
            result.valid_count + result.invalid_count + result.duplicate_count,
            result.total_count
        );
    }

    // ---- selection ----------------------------------------------------------

    fn verdict(name: &str, status: &str, id: Option<&str>) -> McpServerValidationItem {
        McpServerValidationItem {
            server_name: Some(name.to_string()),
            server_id: id.map(str::to_string),
            status: status.to_string(),
            errors: Vec::new(),
            exists: status == validation_status::DUPLICATE,
            selected: true,
        }
    }

    #[test]
    fn unselected_import_keeps_everything_but_invalid() {
        let items = vec![
            verdict("ok", validation_status::VALID, None),
            verdict("bad", validation_status::INVALID, None),
            verdict("dupe", validation_status::DUPLICATE, None),
        ];
        let chosen = select_importable_items(&items, &[]);
        let names: Vec<_> = chosen
            .iter()
            .map(|i| items[*i].server_name.as_deref().unwrap_or(""))
            .collect();
        assert_eq!(names, vec!["ok", "dupe"]);
    }

    #[test]
    fn an_explicit_selection_restricts_to_valid_and_selected() {
        let items = vec![
            verdict("ok", validation_status::VALID, Some("id-1")),
            verdict("other", validation_status::VALID, Some("id-2")),
            verdict("dupe", validation_status::DUPLICATE, Some("id-3")),
        ];
        let chosen = select_importable_items(&items, &["id-1".to_string()]);
        assert_eq!(chosen.len(), 1);
        assert_eq!(items[chosen[0]].server_name.as_deref(), Some("ok"));
    }

    #[test]
    fn no_items_means_no_selection() {
        assert!(select_importable_items(&[], &["id-1".to_string()]).is_empty());
    }

    // ---- payload parsing ----------------------------------------------------

    #[test]
    fn parses_the_servers_array_shape() {
        let content = r#"{"servers":[{"name":"a","protocol":"http","description":"d","remoteServerConfig":{}}]}"#;
        let servers = parse_import_payload(content).expect("parse");
        assert_eq!(servers.len(), 1);
        assert_eq!(servers[0].name.as_deref(), Some("a"));
    }

    #[test]
    fn parses_a_bare_array() {
        let servers = parse_import_payload(r#"[{"name":"a"}]"#).expect("parse");
        assert_eq!(servers.len(), 1);
    }

    #[test]
    fn a_keyed_map_supplies_the_name() {
        let servers = parse_import_payload(r#"{"my-server":{"protocol":"http"}}"#).expect("parse");
        assert_eq!(servers[0].name.as_deref(), Some("my-server"));
    }

    #[test]
    fn malformed_json_is_reported() {
        assert!(parse_import_payload("{not json").is_err());
    }
}
