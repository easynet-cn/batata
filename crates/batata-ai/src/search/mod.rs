//! AI resource search index — projection layer.
//!
//! Mirrors upstream Nacos `service/search/`: a resource version is projected
//! into one [`SearchDocument`] plus a set of [`SearchChunk`]s. The projection is
//! **deterministic** — the same input always yields the same `source_digest`
//! and `chunk_hash` values, which is what lets the index converge and lets
//! unchanged rebuilds be skipped.
//!
//! Out of scope for now (both optional upstream): the `llm_enhancement` stage
//! and vector/embedding storage.
//!
//! Hashing: upstream uses MD5. Batata uses SHA-256 — the digests are only
//! compared against themselves, and a 64-char hex digest still fits the
//! `varchar(64)` columns.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::repository::search;

pub mod consumer;
pub mod query;
pub mod service;
pub mod task;

/// Language tag recorded on chunks upstream for "undetermined".
pub const LANGUAGE_UND: &str = "und";

/// A bounded piece of source content fed into the index.
#[derive(Clone, Debug)]
pub struct SearchContent {
    /// Logical path of the content, e.g. `mcp-server.json`.
    pub path: String,
    /// Text used for keyword matching and enhancement.
    pub text: String,
}

/// The projected search document for one resource version.
#[derive(Clone, Debug)]
pub struct SearchDocument {
    /// Owning namespace ID.
    pub namespace_id: String,
    /// Resource type (e.g. `mcp`).
    pub resource_type: String,
    /// Resource name.
    pub resource_name: String,
    /// Resource version.
    pub resource_version: String,
    /// Name shown in search results.
    pub display_name: String,
    /// Optional description.
    pub description: Option<String>,
    /// Tags.
    pub tags: Vec<String>,
    /// Declared capabilities.
    pub capabilities: Vec<String>,
    /// Representative queries.
    pub representative_queries: Vec<String>,
    /// Structured metadata, serialized to JSON on the row.
    pub metadata: Map<String, Value>,
    /// Digest of the projected source; unchanged digests mean no rebuild.
    pub source_digest: String,
}

impl SearchDocument {
    /// Stable resource key, `namespace:type:name:version`.
    fn resource_key(&self) -> String {
        format!(
            "{}:{}:{}:{}",
            self.namespace_id, self.resource_type, self.resource_name, self.resource_version
        )
    }
}

/// One indexed chunk of a document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SearchChunk {
    /// Chunk type, one of the `search::CHUNK_TYPE_*` constants.
    pub chunk_type: String,
    /// Raw chunk text.
    pub chunk_text: String,
    /// Normalized text used for matching.
    pub canonical_text: String,
    /// Language tag.
    pub language: String,
    /// Optional serialized metadata.
    pub metadata: Option<String>,
    /// Deterministic hash of the chunk content.
    pub chunk_hash: String,
    /// Document this chunk belongs to.
    pub document_id: i64,
}

/// Deterministic hex digest of `input`.
fn digest(input: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    const_hex::encode(hasher.finalize())
}

/// Digest of a JSON value, used for `source_digest`.
///
/// `serde_json::Map` sorts keys, so the serialization is deterministic.
fn digest_json(value: &Value) -> String {
    digest(&value.to_string())
}

/// Build the canonical text upstream matches on.
///
/// `join([resourceType, displayName, chunkType, text], " ")` lowercased.
fn canonical_text(document: &SearchDocument, chunk_type: &str, text: &str) -> String {
    format!(
        "{} {} {} {}",
        document.resource_type, document.display_name, chunk_type, text
    )
    .to_lowercase()
}

/// Build one chunk, deriving `canonical_text` and `chunk_hash`.
fn chunk(
    document: &SearchDocument,
    chunk_type: &str,
    text: &str,
    metadata: Option<String>,
) -> Option<SearchChunk> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }
    let canonical = canonical_text(document, chunk_type, text);
    let chunk_hash = digest(&format!(
        "{}:{}:{}",
        document.resource_key(),
        chunk_type,
        canonical
    ));
    Some(SearchChunk {
        chunk_type: chunk_type.to_string(),
        chunk_text: text.to_string(),
        canonical_text: canonical,
        language: LANGUAGE_UND.to_string(),
        metadata,
        chunk_hash,
        document_id: 0,
    })
}

/// Chunks derived from the document's own fields.
///
/// Mirrors `AiResourceSearchChunkBuilder.buildChunks`.
pub fn build_chunks(document: &SearchDocument) -> Vec<SearchChunk> {
    let mut chunks = Vec::new();

    let description = document
        .description
        .as_deref()
        .filter(|d| !d.trim().is_empty())
        .unwrap_or(&document.display_name);
    chunks.extend(chunk(
        document,
        search::CHUNK_TYPE_DESCRIPTION,
        description,
        None,
    ));

    for capability in &document.capabilities {
        chunks.extend(chunk(
            document,
            search::CHUNK_TYPE_CAPABILITY,
            capability,
            None,
        ));
    }
    for query in &document.representative_queries {
        chunks.extend(chunk(
            document,
            search::CHUNK_TYPE_REPRESENTATIVE_QUERY,
            query,
            None,
        ));
    }
    for tag in &document.tags {
        chunks.extend(chunk(document, search::CHUNK_TYPE_TAG, tag, None));
    }

    // Metadata-derived chunks: only emitted when the keys are present.
    if let Some(c) = metadata_chunk(document, search::CHUNK_TYPE_METADATA_IO, &[
        "inputTypes",
        "outputTypes",
    ]) {
        chunks.push(c);
    }
    if let Some(c) = metadata_chunk(document, search::CHUNK_TYPE_METADATA_RISK, &[
        "sideEffects",
        "riskLevel",
    ]) {
        chunks.push(c);
    }
    if let Some(c) = metadata_chunk(document, search::CHUNK_TYPE_NOT_FOR, &["notFor"]) {
        chunks.push(c);
    }

    chunks
}

/// Build a chunk from selected metadata keys, `key:value` joined by spaces.
fn metadata_chunk(
    document: &SearchDocument,
    chunk_type: &str,
    keys: &[&str],
) -> Option<SearchChunk> {
    let mut parts = Vec::new();
    let mut selected = Map::new();
    for key in keys {
        let Some(value) = document.metadata.get(*key) else {
            continue;
        };
        selected.insert((*key).to_string(), value.clone());
        match value {
            Value::Array(items) => {
                for item in items {
                    parts.push(format!("{key}:{}", scalar_text(item)));
                }
            }
            other => parts.push(format!("{key}:{}", scalar_text(other))),
        }
    }
    if parts.is_empty() {
        return None;
    }
    chunk(
        document,
        chunk_type,
        &parts.join(" "),
        Some(Value::Object(selected).to_string()),
    )
}

/// Render a JSON scalar as plain text.
fn scalar_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

/// Chunks derived from source content, deduplicated by `chunk_hash`.
pub fn build_source_content_chunks(
    document: &SearchDocument,
    contents: &[SearchContent],
    chunk_type: &str,
) -> Vec<SearchChunk> {
    let mut chunks = Vec::new();
    for content in contents {
        if let Some(c) = chunk(document, chunk_type, &content.text, Some(content.path.clone())) {
            chunks.push(c);
        }
    }
    dedupe_by_hash(chunks)
}

/// Drop chunks with a hash already present, keeping the first occurrence.
fn dedupe_by_hash(chunks: Vec<SearchChunk>) -> Vec<SearchChunk> {
    let mut seen = std::collections::HashSet::new();
    chunks
        .into_iter()
        .filter(|c| seen.insert(c.chunk_hash.clone()))
        .collect()
}

/// Build the MCP search document for a server version.
///
/// Mirrors `AiResourceSearchDocumentBuilder.fromMcpServer`.
pub fn mcp_document(
    namespace_id: &str,
    name: &str,
    version: &str,
    description: Option<&str>,
    capabilities: Vec<String>,
    server_id: Option<&str>,
    protocol: Option<&str>,
    enabled: bool,
    status: Option<&str>,
) -> SearchDocument {
    let mut metadata = Map::new();
    metadata.insert("mcpName".to_string(), Value::String(name.to_string()));
    metadata.insert(
        "mcpServerId".to_string(),
        Value::String(server_id.unwrap_or_default().to_string()),
    );
    metadata.insert(
        "protocol".to_string(),
        Value::String(protocol.unwrap_or_default().to_string()),
    );
    metadata.insert("enabled".to_string(), Value::Bool(enabled));
    metadata.insert(
        "status".to_string(),
        Value::String(status.unwrap_or_default().to_string()),
    );

    // The digest covers every field that would change the projection.
    let digest_source = serde_json::json!({
        "name": name,
        "description": description.unwrap_or_default(),
        "version": version,
        "enabled": enabled,
        "status": status.unwrap_or_default(),
        "capabilities": capabilities,
        "metadata": metadata,
    });

    SearchDocument {
        namespace_id: namespace_id.to_string(),
        resource_type: "mcp".to_string(),
        resource_name: name.to_string(),
        resource_version: version.to_string(),
        display_name: name.to_string(),
        description: description.map(str::to_string),
        tags: Vec::new(),
        capabilities,
        representative_queries: Vec::new(),
        metadata,
        source_digest: digest_json(&digest_source),
    }
}

/// Render the MCP server spec as indexable text.
///
/// Mirrors `McpAiResourceSearchTypeHandler.mcpServerText`.
pub fn mcp_server_text(name: &str, description: Option<&str>, protocol: Option<&str>) -> String {
    let mut text = String::from("# MCP server\n");
    text.push_str(&format!("name: {name}\n"));
    text.push_str(&format!("description: {}\n", description.unwrap_or_default()));
    text.push_str(&format!("protocol: {}\n", protocol.unwrap_or_default()));
    text
}

/// Maximum characters taken from one MCP source document.
pub const MAX_MCP_CONTENT_CHARS: usize = 12_000;

/// Truncate text to [`MAX_MCP_CONTENT_CHARS`], as upstream bounds source content.
fn limit(text: &str) -> &str {
    match text.char_indices().nth(MAX_MCP_CONTENT_CHARS) {
        Some((idx, _)) => &text[..idx],
        None => text,
    }
}

/// Project an MCP server version into its document and chunks.
pub fn project_mcp(
    namespace_id: &str,
    name: &str,
    version: &str,
    description: Option<&str>,
    capabilities: Vec<String>,
    server_id: Option<&str>,
    protocol: Option<&str>,
    enabled: bool,
    status: Option<&str>,
    tools_json: Option<&str>,
) -> (SearchDocument, Vec<SearchChunk>) {
    let document = mcp_document(
        namespace_id,
        name,
        version,
        description,
        capabilities,
        server_id,
        protocol,
        enabled,
        status,
    );

    let mut chunks = build_chunks(&document);

    // Source contents: the server spec and, when present, the tool spec.
    let mut contents = vec![SearchContent {
        path: "mcp-server.json".to_string(),
        text: limit(&mcp_server_text(name, description, protocol)).to_string(),
    }];
    if let Some(tools) = tools_json.filter(|t| !t.trim().is_empty()) {
        contents.push(SearchContent {
            path: "mcp-tools.json".to_string(),
            text: limit(tools).to_string(),
        });
    }
    chunks.extend(build_source_content_chunks(
        &document,
        &contents,
        search::CHUNK_TYPE_MCP_CONTENT,
    ));

    (document, chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> SearchDocument {
        mcp_document(
            "public",
            "my-server",
            "1.0.0",
            Some("does things"),
            vec!["tool".to_string()],
            Some("id-1"),
            Some("http"),
            true,
            Some("ACTIVE"),
        )
    }

    #[test]
    fn projection_is_deterministic() {
        let (a, chunks_a) = project_mcp(
            "public",
            "s",
            "1.0.0",
            Some("d"),
            vec!["tool".to_string()],
            Some("id"),
            Some("http"),
            true,
            Some("ACTIVE"),
            None,
        );
        let (b, chunks_b) = project_mcp(
            "public",
            "s",
            "1.0.0",
            Some("d"),
            vec!["tool".to_string()],
            Some("id"),
            Some("http"),
            true,
            Some("ACTIVE"),
            None,
        );
        assert_eq!(a.source_digest, b.source_digest);
        assert_eq!(chunks_a.len(), chunks_b.len());
        for (x, y) in chunks_a.iter().zip(chunks_b.iter()) {
            assert_eq!(x.chunk_hash, y.chunk_hash);
        }
    }

    #[test]
    fn digest_changes_when_content_changes() {
        let a = mcp_document(
            "public",
            "s",
            "1.0.0",
            Some("one"),
            vec![],
            None,
            None,
            true,
            None,
        );
        let b = mcp_document(
            "public",
            "s",
            "1.0.0",
            Some("two"),
            vec![],
            None,
            None,
            true,
            None,
        );
        assert_ne!(
            a.source_digest, b.source_digest,
            "a content change must change the digest"
        );
    }

    #[test]
    fn digest_is_64_hex_chars() {
        let d = document();
        assert_eq!(d.source_digest.len(), 64, "must fit varchar(64)");
        assert!(
            d.source_digest.chars().all(|c| c.is_ascii_hexdigit()),
            "digest must be hex"
        );
    }

    #[test]
    fn canonical_text_is_lowercased_and_joined() {
        let d = document();
        let c = canonical_text(&d, search::CHUNK_TYPE_DESCRIPTION, "Hello World");
        assert_eq!(c, "mcp my-server description hello world");
    }

    #[test]
    fn chunk_hash_covers_resource_and_type() {
        let d = document();
        let a = chunk(&d, search::CHUNK_TYPE_DESCRIPTION, "same text", None).unwrap();
        let b = chunk(&d, search::CHUNK_TYPE_CAPABILITY, "same text", None).unwrap();
        assert_ne!(
            a.chunk_hash, b.chunk_hash,
            "chunk type must participate in the hash"
        );
        assert_ne!(a.canonical_text, b.canonical_text);
    }

    #[test]
    fn blank_text_produces_no_chunk() {
        let d = document();
        assert!(chunk(&d, search::CHUNK_TYPE_DESCRIPTION, "   ", None).is_none());
    }

    #[test]
    fn description_falls_back_to_display_name() {
        let mut d = document();
        d.description = None;
        let chunks = build_chunks(&d);
        let description = chunks
            .iter()
            .find(|c| c.chunk_type == search::CHUNK_TYPE_DESCRIPTION)
            .expect("description chunk");
        assert_eq!(description.chunk_text, "my-server");
    }

    #[test]
    fn metadata_chunks_only_when_keys_present() {
        let d = document();
        assert!(
            !build_chunks(&d)
                .iter()
                .any(|c| c.chunk_type == search::CHUNK_TYPE_NOT_FOR),
            "no notFor metadata means no not_for chunk"
        );

        let mut with_meta = document();
        with_meta
            .metadata
            .insert("notFor".to_string(), Value::String("prod".to_string()));
        assert!(
            build_chunks(&with_meta)
                .iter()
                .any(|c| c.chunk_type == search::CHUNK_TYPE_NOT_FOR),
            "notFor metadata must produce a not_for chunk"
        );
    }

    #[test]
    fn source_content_chunks_are_deduped_by_hash() {
        let d = document();
        let contents = vec![
            SearchContent {
                path: "a.json".to_string(),
                text: "same".to_string(),
            },
            SearchContent {
                path: "b.json".to_string(),
                text: "same".to_string(),
            },
        ];
        let chunks =
            build_source_content_chunks(&d, &contents, search::CHUNK_TYPE_MCP_CONTENT);
        assert_eq!(
            chunks.len(),
            1,
            "identical text must collapse to one chunk"
        );
    }

    #[test]
    fn oversized_content_is_truncated() {
        let long = "x".repeat(MAX_MCP_CONTENT_CHARS + 500);
        assert_eq!(limit(&long).chars().count(), MAX_MCP_CONTENT_CHARS);
    }

    #[test]
    fn mcp_projection_includes_server_and_tool_contents() {
        let (_d, chunks) = project_mcp(
            "public",
            "s",
            "1.0.0",
            Some("d"),
            vec![],
            None,
            None,
            true,
            None,
            Some(r#"{"tools":[]}"#),
        );
        let mcp_chunks: Vec<_> = chunks
            .iter()
            .filter(|c| c.chunk_type == search::CHUNK_TYPE_MCP_CONTENT)
            .collect();
        assert_eq!(mcp_chunks.len(), 2, "server spec and tool spec");
    }
}
