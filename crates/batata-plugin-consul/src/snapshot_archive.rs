//! Consul-compatible snapshot archive format.
//!
//! Implements the same tar-based format as `consul/snapshot/archive.go`:
//!
//! A snapshot archive is a tar file containing three members:
//! - `meta.json`   — JSON-encoded Raft snapshot metadata
//! - `state.bin`   — Binary-encoded Raft state (opaque blob)
//! - `SHA256SUMS`  — SHA-256 hashes of `meta.json` and `state.bin`
//!
//! The `SHA256SUMS` file uses the same format as Consul's `hashList.Encode`:
//! ```text
//! <hex(sha256)>  <filename>
//! ```
//!
//! Archives created by Batata can be inspected by Consul's snapshot tools,
//! and archives produced by Consul can be opened here.  The `state.bin`
//! payload is intentionally opaque at the archive layer — the backing
//! Raft implementation decides what goes inside.

use std::io::{Read, Write};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// File names used inside the tar archive (must match Consul byte-for-byte).
pub const META_FILE: &str = "meta.json";
pub const STATE_FILE: &str = "state.bin";
pub const SHA256SUMS_FILE: &str = "SHA256SUMS";

/// Raft snapshot metadata, wire-compatible with Consul's `raft.SnapshotMeta`.
///
/// Consul stores this JSON-encoded in `meta.json` inside the archive.
/// The field names and casing match `github.com/hashicorp/raft.SnapshotMeta`
/// serialization so Go consumers can unmarshal it directly.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct SnapshotMeta {
    /// Raft protocol version (3 for modern Raft).
    pub version: u8,
    /// Snapshot identifier (free-form; Consul uses `term-index-id`).
    pub id: String,
    /// Raft log index covered by this snapshot.
    pub index: u64,
    /// Raft term at the time of snapshot.
    pub term: u64,
    /// Peers list (legacy, nil for version >= 1).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peers: Option<Vec<String>>,
    /// Full Raft configuration (voters, non-voters, learners).
    #[serde(rename = "Configuration")]
    pub configuration: serde_json::Value,
    /// Raft log index of the configuration change that produced this set.
    #[serde(rename = "ConfigurationIndex")]
    pub configuration_index: u64,
    /// Size of the state payload in bytes (used by Consul for progress).
    pub size: i64,
}

impl Default for SnapshotMeta {
    fn default() -> Self {
        Self {
            version: 1,
            id: String::new(),
            index: 0,
            term: 0,
            peers: None,
            configuration: serde_json::json!({"Servers": []}),
            configuration_index: 0,
            size: 0,
        }
    }
}

/// Write a Consul-compatible snapshot archive to `out`.
///
/// `state` is the opaque Raft snapshot binary (goes into `state.bin`).
pub fn write_archive<W: Write>(
    out: W,
    mut meta: SnapshotMeta,
    state: &[u8],
) -> std::io::Result<()> {
    // Update size field so meta.json reflects the actual state length.
    meta.size = state.len() as i64;
    let meta_bytes = serde_json::to_vec_pretty(&meta)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    // Compute SHA-256 hashes of meta.json and state.bin
    let meta_hash = {
        let mut h = Sha256::new();
        h.update(&meta_bytes);
        h.finalize()
    };
    let state_hash = {
        let mut h = Sha256::new();
        h.update(state);
        h.finalize()
    };
    // Consul uses the exact format: "<hex>  <filename>\n" (two spaces)
    let sha256sums = format!(
        "{:x}  {}\n{:x}  {}\n",
        meta_hash, META_FILE, state_hash, STATE_FILE
    );

    let mut builder = tar::Builder::new(out);
    append_file(&mut builder, META_FILE, &meta_bytes)?;
    append_file(&mut builder, STATE_FILE, state)?;
    append_file(&mut builder, SHA256SUMS_FILE, sha256sums.as_bytes())?;
    builder.finish()?;
    Ok(())
}

fn append_file<W: Write>(
    builder: &mut tar::Builder<W>,
    name: &str,
    data: &[u8],
) -> std::io::Result<()> {
    let mut header = tar::Header::new_gnu();
    header.set_size(data.len() as u64);
    header.set_mode(0o600);
    header.set_mtime(chrono::Utc::now().timestamp() as u64);
    header.set_cksum();
    builder.append_data(&mut header, name, data)
}

/// A parsed snapshot archive.
#[derive(Debug, Clone)]
pub struct ParsedArchive {
    pub meta: SnapshotMeta,
    pub state: Vec<u8>,
}

/// Read and verify a Consul-compatible snapshot archive from `input`.
///
/// The SHA256SUMS file is checked against the actual contents; a mismatch
/// returns `ErrorKind::InvalidData`.
pub fn read_archive<R: Read>(input: R) -> std::io::Result<ParsedArchive> {
    let mut archive = tar::Archive::new(input);

    let mut meta_bytes: Option<Vec<u8>> = None;
    let mut state_bytes: Option<Vec<u8>> = None;
    let mut sums_bytes: Option<Vec<u8>> = None;

    for entry_result in archive.entries()? {
        let mut entry = entry_result?;
        let name = entry.path()?.to_string_lossy().to_string();
        let mut buf = Vec::new();
        entry.read_to_end(&mut buf)?;
        match name.as_str() {
            META_FILE => meta_bytes = Some(buf),
            STATE_FILE => state_bytes = Some(buf),
            SHA256SUMS_FILE => sums_bytes = Some(buf),
            other => {
                // Unknown members are tolerated (matches Consul's behavior)
                tracing::debug!("snapshot archive: ignoring unknown entry {}", other);
            }
        }
    }

    let meta_bytes = meta_bytes
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing meta.json"))?;
    let state_bytes = state_bytes
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidData, "missing state.bin"))?;
    let sums_bytes = sums_bytes.ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "missing SHA256SUMS")
    })?;

    // Verify hashes
    let meta_hash = {
        let mut h = Sha256::new();
        h.update(&meta_bytes);
        h.finalize()
    };
    let state_hash = {
        let mut h = Sha256::new();
        h.update(&state_bytes);
        h.finalize()
    };

    let sums_str = std::str::from_utf8(&sums_bytes).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("SHA256SUMS utf8: {}", e),
        )
    })?;
    let mut seen_meta = false;
    let mut seen_state = false;
    for line in sums_str.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        // Format: "<hex>  <filename>"
        let (hex, file) = match line.split_once("  ") {
            Some((h, f)) => (h.trim(), f.trim()),
            None => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("malformed SHA256SUMS line: {}", line),
                ));
            }
        };
        let expected = hex_decode(hex)?;
        match file {
            META_FILE => {
                if expected != meta_hash.as_slice() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "meta.json SHA-256 mismatch",
                    ));
                }
                seen_meta = true;
            }
            STATE_FILE => {
                if expected != state_hash.as_slice() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        "state.bin SHA-256 mismatch",
                    ));
                }
                seen_state = true;
            }
            _ => {
                // Ignore unknown hashed file entries (forward compat)
            }
        }
    }
    if !seen_meta || !seen_state {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "SHA256SUMS missing required entries",
        ));
    }

    let meta: SnapshotMeta = serde_json::from_slice(&meta_bytes).map_err(|e| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, format!("meta.json: {}", e))
    })?;

    Ok(ParsedArchive {
        meta,
        state: state_bytes,
    })
}

fn hex_decode(s: &str) -> std::io::Result<Vec<u8>> {
    if s.len() % 2 != 0 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "odd-length hex",
        ));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for chunk in s.as_bytes().chunks(2) {
        let hi = hex_nibble(chunk[0])?;
        let lo = hex_nibble(chunk[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_nibble(c: u8) -> std::io::Result<u8> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("invalid hex char {:?}", c as char),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_empty_state() {
        let meta = SnapshotMeta {
            version: 1,
            id: "1-100-abc".to_string(),
            index: 100,
            term: 1,
            ..Default::default()
        };
        let state = b"";
        let mut buf = Vec::new();
        write_archive(&mut buf, meta.clone(), state).unwrap();

        let parsed = read_archive(&buf[..]).unwrap();
        assert_eq!(parsed.meta.id, meta.id);
        assert_eq!(parsed.meta.index, meta.index);
        assert_eq!(parsed.meta.term, meta.term);
        assert_eq!(parsed.state, state);
    }

    #[test]
    fn round_trip_with_state() {
        let meta = SnapshotMeta {
            id: "2-200-xyz".to_string(),
            index: 200,
            term: 2,
            ..Default::default()
        };
        let state = b"some raft snapshot bytes";
        let mut buf = Vec::new();
        write_archive(&mut buf, meta, state).unwrap();

        let parsed = read_archive(&buf[..]).unwrap();
        assert_eq!(parsed.state, state);
    }

    #[test]
    fn large_state_round_trip() {
        // 1MB state
        let state: Vec<u8> = (0..(1024 * 1024)).map(|i| (i % 256) as u8).collect();
        let meta = SnapshotMeta {
            id: "big".to_string(),
            index: 9999,
            term: 3,
            ..Default::default()
        };
        let mut buf = Vec::new();
        write_archive(&mut buf, meta, &state).unwrap();

        let parsed = read_archive(&buf[..]).unwrap();
        assert_eq!(parsed.state.len(), state.len());
        assert_eq!(parsed.state, state);
    }

    #[test]
    fn tampered_state_detected() {
        let meta = SnapshotMeta::default();
        let state = b"original";
        let mut buf = Vec::new();
        write_archive(&mut buf, meta, state).unwrap();

        // Read, tamper with state bytes, write back with the ORIGINAL sums
        // The easiest way: manually re-encode a corrupted archive
        let mut corrupted = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut corrupted);
            // Original hashes from untampered archive
            let original = read_archive(&buf[..]).unwrap();
            let meta_bytes = serde_json::to_vec_pretty(&original.meta).unwrap();
            let meta_h = {
                let mut h = Sha256::new();
                h.update(&meta_bytes);
                h.finalize()
            };
            let state_h = {
                let mut h = Sha256::new();
                h.update(state); // original state hash
                h.finalize()
            };
            let sums = format!(
                "{:x}  {}\n{:x}  {}\n",
                meta_h, META_FILE, state_h, STATE_FILE
            );
            append_file(&mut builder, META_FILE, &meta_bytes).unwrap();
            // Append DIFFERENT state bytes but keep the original hash entry
            append_file(&mut builder, STATE_FILE, b"TAMPERED").unwrap();
            append_file(&mut builder, SHA256SUMS_FILE, sums.as_bytes()).unwrap();
            builder.finish().unwrap();
        }

        let err = read_archive(&corrupted[..]).unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
        assert!(err.to_string().contains("SHA-256 mismatch"));
    }

    #[test]
    fn missing_state_rejected() {
        let mut corrupted = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut corrupted);
            append_file(&mut builder, META_FILE, b"{}").unwrap();
            append_file(&mut builder, SHA256SUMS_FILE, b"").unwrap();
            builder.finish().unwrap();
        }
        let err = read_archive(&corrupted[..]).unwrap_err();
        assert!(err.to_string().contains("missing state.bin"));
    }
}
