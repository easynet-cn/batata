//! Skill ZIP parsing and generation utilities
//!
//! Handles parsing skills from ZIP archives and generating ZIP archives from skills.
//! ZIP structure: `skillName/{SKILL.md, resources...}` with multi-level folder support.

use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use zip::write::SimpleFileOptions;

use crate::model::skill::{Skill, SkillResource};

/// Max decompressed ZIP size: 50 MB
const MAX_TOTAL_UNCOMPRESSED_BYTES: u64 = 50 * 1024 * 1024;

/// Max ZIP entries
const MAX_ZIP_ENTRIES: usize = 500;

/// Max upload ZIP size: 10 MB
pub const MAX_UPLOAD_ZIP_BYTES: usize = 10 * 1024 * 1024;

/// Binary file extensions (stored as base64)
const BINARY_EXTENSIONS: &[&str] = &[
    "ttf", "otf", "woff", "woff2", "eot", "png", "jpg", "jpeg", "gif", "webp", "ico", "cur", "pdf",
    "bin",
];

/// Parse a Skill from ZIP bytes.
pub fn parse_skill_from_zip(zip_bytes: &[u8], namespace_id: &str) -> anyhow::Result<Skill> {
    if zip_bytes.len() < 30 {
        anyhow::bail!("Invalid ZIP: too small");
    }

    // Check ZIP magic header: PK\x03\x04
    if zip_bytes[0] != b'P' || zip_bytes[1] != b'K' || zip_bytes[2] != 3 || zip_bytes[3] != 4 {
        anyhow::bail!("Invalid ZIP: bad magic header");
    }

    if zip_bytes.len() > MAX_UPLOAD_ZIP_BYTES {
        anyhow::bail!(
            "ZIP file too large: {} bytes (max {} bytes)",
            zip_bytes.len(),
            MAX_UPLOAD_ZIP_BYTES
        );
    }

    let cursor = Cursor::new(zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;

    if archive.len() > MAX_ZIP_ENTRIES {
        anyhow::bail!(
            "ZIP has too many entries: {} (max {})",
            archive.len(),
            MAX_ZIP_ENTRIES
        );
    }

    let mut entries: HashMap<String, Vec<u8>> = HashMap::new();
    let mut total_size: u64 = 0;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // Skip directories and macOS metadata
        if file.is_dir() || is_macos_metadata(&name) {
            continue;
        }

        // Path traversal prevention
        validate_path_safety(&name)?;

        let mut content = Vec::new();
        file.read_to_end(&mut content)?;
        total_size += content.len() as u64;

        if total_size > MAX_TOTAL_UNCOMPRESSED_BYTES {
            anyhow::bail!("ZIP decompressed size exceeds limit");
        }

        entries.insert(name, content);
    }

    // Find SKILL.md
    let skill_md_key = entries
        .keys()
        .find(|k| k == &"SKILL.md" || k.ends_with("/SKILL.md"))
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("ZIP does not contain SKILL.md"))?;

    let skill_md_bytes = entries.remove(&skill_md_key).unwrap();
    let skill_md_content = strip_bom(&String::from_utf8_lossy(&skill_md_bytes));

    // Parse YAML front matter
    let (front_matter, _body) = parse_yaml_front_matter(&skill_md_content);

    let name = front_matter
        .get("name")
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("SKILL.md missing required 'name' field in front matter"))?
        .trim()
        .to_string();

    let description = front_matter.get("description").cloned();

    // Determine the skill root prefix (e.g., "skillName/" or empty)
    let root_prefix = if skill_md_key.contains('/') {
        let idx = skill_md_key.find('/').unwrap();
        &skill_md_key[..=idx]
    } else {
        ""
    };

    // Parse resources
    let mut resources = HashMap::new();
    for (path, content) in &entries {
        // Strip root prefix
        let relative = if !root_prefix.is_empty() && path.starts_with(root_prefix) {
            &path[root_prefix.len()..]
        } else {
            path.as_str()
        };

        // Skip SKILL.md if found again at a different path
        if relative == "SKILL.md" {
            continue;
        }

        // Determine type and name from path
        let (res_type, res_name) = if let Some(last_slash) = relative.rfind('/') {
            (
                relative[..last_slash].to_string(),
                relative[last_slash + 1..].to_string(),
            )
        } else {
            (String::new(), relative.to_string())
        };

        let ext = res_name.rsplit('.').next().unwrap_or("");
        let is_binary = BINARY_EXTENSIONS.contains(&ext.to_lowercase().as_str());

        let (content_str, metadata) = if is_binary {
            use base64::Engine;
            let encoded = base64::engine::general_purpose::STANDARD.encode(content);
            let mut meta = HashMap::new();
            meta.insert("encoding".to_string(), "base64".to_string());
            (encoded, meta)
        } else {
            (String::from_utf8_lossy(content).to_string(), HashMap::new())
        };

        let resource = SkillResource {
            name: res_name,
            resource_type: res_type,
            content: Some(content_str),
            metadata,
        };
        resources.insert(resource.resource_identifier(), resource);
    }

    Ok(Skill {
        namespace_id: namespace_id.to_string(),
        name,
        description,
        skill_md: Some(skill_md_content),
        resource: resources,
    })
}

/// Convert a Skill to ZIP bytes.
pub fn skill_to_zip_bytes(skill: &Skill) -> anyhow::Result<Vec<u8>> {
    let buf = Vec::new();
    let cursor = Cursor::new(buf);
    let mut zip = zip::ZipWriter::new(cursor);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);

    let prefix = &skill.name;

    // Write SKILL.md
    if let Some(ref md) = skill.skill_md {
        let path = format!("{}/SKILL.md", prefix);
        zip.start_file(&path, options)?;
        zip.write_all(md.as_bytes())?;
    }

    // Write resources
    for res in skill.resource.values() {
        let path = if res.resource_type.is_empty() {
            format!("{}/{}", prefix, res.name)
        } else {
            format!("{}/{}/{}", prefix, res.resource_type, res.name)
        };

        validate_path_safety(&path)?;
        zip.start_file(&path, options)?;

        if let Some(ref content) = res.content {
            let bytes = resolve_resource_bytes(content, &res.metadata)?;
            zip.write_all(&bytes)?;
        }
    }

    let cursor = zip.finish()?;
    Ok(cursor.into_inner())
}

/// Resolve resource bytes (handles base64 encoded binary resources).
fn resolve_resource_bytes(
    content: &str,
    metadata: &HashMap<String, String>,
) -> anyhow::Result<Vec<u8>> {
    if metadata.get("encoding").map(|s| s.as_str()) == Some("base64") {
        use base64::Engine;
        Ok(base64::engine::general_purpose::STANDARD.decode(content)?)
    } else {
        Ok(content.as_bytes().to_vec())
    }
}

/// Parse YAML front matter from markdown content.
/// Returns (key-value map, body after front matter).
fn parse_yaml_front_matter(content: &str) -> (HashMap<String, String>, &str) {
    let mut map = HashMap::new();

    if !content.starts_with("---") {
        return (map, content);
    }

    // Find closing ---
    let after_first = &content[3..];
    let close_pos = after_first.find("\n---");
    let (yaml_block, body) = match close_pos {
        Some(pos) => {
            let yaml = &after_first[..pos];
            let rest_start = pos + 4; // skip \n---
            let body = if rest_start < after_first.len() {
                &after_first[rest_start..]
            } else {
                ""
            };
            (yaml, body)
        }
        None => return (map, content),
    };

    // Parse simple YAML key: value pairs
    let mut current_parent: Option<String> = None;
    for line in yaml_block.lines() {
        let trimmed = line.trim();

        // Skip empty lines and comments
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if let Some(colon_pos) = trimmed.find(':') {
            let key = trimmed[..colon_pos].trim();
            let value = trimmed[colon_pos + 1..].trim();

            // Check for nested block (key with no value, next lines indented)
            if value.is_empty() {
                current_parent = Some(key.to_string());
                continue;
            }

            let unquoted = unquote_yaml_value(value);

            if line.starts_with(' ') || line.starts_with('\t') {
                // Indented: child of current parent
                if let Some(ref parent) = current_parent {
                    map.insert(format!("{}.{}", parent, key), unquoted);
                }
            } else {
                current_parent = None;
                map.insert(key.to_string(), unquoted);
            }
        }
    }

    (map, body)
}

/// Unquote a YAML scalar value (handles double and single quotes).
fn unquote_yaml_value(value: &str) -> String {
    if value.starts_with('"') && value.ends_with('"') && value.len() >= 2 {
        let inner = &value[1..value.len() - 1];
        inner.replace("\\\\", "\\").replace("\\\"", "\"")
    } else if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        let inner = &value[1..value.len() - 1];
        inner.replace("''", "'")
    } else {
        value.to_string()
    }
}

/// Validate path safety (prevent path traversal).
fn validate_path_safety(path: &str) -> anyhow::Result<()> {
    if path.contains("..") {
        anyhow::bail!("Path traversal detected in ZIP entry: {}", path);
    }
    if path.starts_with('/') || path.starts_with('\\') {
        anyhow::bail!("Absolute path detected in ZIP entry: {}", path);
    }
    Ok(())
}

/// Check if a ZIP entry is macOS metadata.
fn is_macos_metadata(name: &str) -> bool {
    name.starts_with("__MACOSX/")
        || name.contains("/__MACOSX/")
        || name.rsplit('/').next().is_some_and(|f| f.starts_with("._"))
}

/// Strip UTF-8 BOM from string.
fn strip_bom(s: &str) -> String {
    s.strip_prefix('\u{FEFF}').unwrap_or(s).to_string()
}

/// Extract version from SKILL.md front matter.
pub fn extract_version_from_skill_md(skill: &Skill) -> Option<String> {
    let md = skill.skill_md.as_deref()?;
    let (fm, _) = parse_yaml_front_matter(md);
    fm.get("version")
        .or_else(|| fm.get("metadata.version"))
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_yaml_front_matter() {
        let content = "---\nname: test-skill\ndescription: A test\nversion: 1.0.0\n---\n# Body";
        let (map, body) = parse_yaml_front_matter(content);
        assert_eq!(map.get("name").unwrap(), "test-skill");
        assert_eq!(map.get("description").unwrap(), "A test");
        assert_eq!(map.get("version").unwrap(), "1.0.0");
        assert!(body.contains("# Body"));
    }

    #[test]
    fn test_parse_yaml_front_matter_quoted() {
        let content = "---\nname: \"my \\\"skill\\\"\"\ndescription: 'it''s great'\n---\n";
        let (map, _) = parse_yaml_front_matter(content);
        assert_eq!(map.get("name").unwrap(), "my \"skill\"");
        assert_eq!(map.get("description").unwrap(), "it's great");
    }

    #[test]
    fn test_parse_yaml_front_matter_nested() {
        let content = "---\nname: test\nmetadata:\n  version: 2.0.0\n  author: alice\n---\n";
        let (map, _) = parse_yaml_front_matter(content);
        assert_eq!(map.get("name").unwrap(), "test");
        assert_eq!(map.get("metadata.version").unwrap(), "2.0.0");
        assert_eq!(map.get("metadata.author").unwrap(), "alice");
    }

    #[test]
    fn test_validate_path_safety_ok() {
        assert!(validate_path_safety("skill/SKILL.md").is_ok());
        assert!(validate_path_safety("skill/resources/file.txt").is_ok());
    }

    #[test]
    fn test_validate_path_safety_traversal() {
        assert!(validate_path_safety("../etc/passwd").is_err());
        assert!(validate_path_safety("skill/../../etc").is_err());
    }

    #[test]
    fn test_validate_path_safety_absolute() {
        assert!(validate_path_safety("/etc/passwd").is_err());
    }

    #[test]
    fn test_is_macos_metadata() {
        assert!(is_macos_metadata("__MACOSX/"));
        assert!(is_macos_metadata("__MACOSX/._file"));
        assert!(is_macos_metadata("skill/__MACOSX/file"));
        assert!(is_macos_metadata("skill/._hidden"));
        assert!(!is_macos_metadata("skill/file.txt"));
    }

    #[test]
    fn test_strip_bom() {
        assert_eq!(strip_bom("\u{FEFF}hello"), "hello");
        assert_eq!(strip_bom("hello"), "hello");
    }

    #[test]
    fn test_unquote_yaml_value() {
        assert_eq!(unquote_yaml_value("plain"), "plain");
        assert_eq!(unquote_yaml_value("\"double\""), "double");
        assert_eq!(unquote_yaml_value("'single'"), "single");
        assert_eq!(
            unquote_yaml_value("\"has \\\"quotes\\\"\""),
            "has \"quotes\""
        );
        assert_eq!(unquote_yaml_value("'it''s'"), "it's");
    }

    #[test]
    fn test_roundtrip_zip() {
        let mut resources = HashMap::new();
        resources.insert(
            "tool::calc.py".to_string(),
            SkillResource {
                name: "calc.py".to_string(),
                resource_type: "tool".to_string(),
                content: Some("print('hello')".to_string()),
                metadata: HashMap::new(),
            },
        );

        let skill = Skill {
            namespace_id: "public".to_string(),
            name: "test-skill".to_string(),
            description: Some("A test".to_string()),
            skill_md: Some(
                "---\nname: test-skill\ndescription: A test\n---\n# Test Skill".to_string(),
            ),
            resource: resources,
        };

        let zip_bytes = skill_to_zip_bytes(&skill).unwrap();
        assert!(!zip_bytes.is_empty());

        let parsed = parse_skill_from_zip(&zip_bytes, "public").unwrap();
        assert_eq!(parsed.name, "test-skill");
        assert!(parsed.skill_md.is_some());
        assert_eq!(parsed.resource.len(), 1);
        let tool = parsed.resource.values().next().unwrap();
        assert_eq!(tool.name, "calc.py");
        assert_eq!(tool.resource_type, "tool");
        assert_eq!(tool.content.as_deref(), Some("print('hello')"));
    }

    #[test]
    fn test_extract_version_from_skill_md() {
        let skill = Skill {
            skill_md: Some("---\nname: s\nversion: 1.2.3\n---\n".to_string()),
            ..Default::default()
        };
        assert_eq!(
            extract_version_from_skill_md(&skill),
            Some("1.2.3".to_string())
        );
    }

    #[test]
    fn test_extract_version_from_nested_metadata() {
        let skill = Skill {
            skill_md: Some("---\nname: s\nmetadata:\n  version: 2.0.0\n---\n".to_string()),
            ..Default::default()
        };
        assert_eq!(
            extract_version_from_skill_md(&skill),
            Some("2.0.0".to_string())
        );
    }
}
