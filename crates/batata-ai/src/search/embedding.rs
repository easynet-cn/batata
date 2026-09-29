//! Text embedding for AI resource retrieval.
//!
//! Mirrors upstream `AiResourceEmbeddingService` and its default
//! implementation `HashingAiResourceEmbeddingService`: a deterministic,
//! model-free embedder that hashes tokens into a fixed-dimension vector and
//! L2-normalizes the result. Upstream ships this as the default "until a
//! deployment provides a model-backed implementation", which is exactly the
//! case here — it needs no model, no network and no pgvector, so it can be
//! tested outright.
//!
//! Because vectors are normalized, [`cosine_similarity`] is a plain dot
//! product.
//!
//! Known divergence: upstream tokenizes over UTF-16 code units and rejects
//! tokens of `length <= 1`. This port counts Unicode scalar values instead, so
//! a lone non-ASCII character (one scalar value, but 1–2 UTF-16 units) can be
//! kept where Java would drop it. ASCII input — every identifier this indexes
//! in practice — matches exactly.

/// Model identifier persisted alongside vector rows.
pub const HASHING_MODEL: &str = "nacos-local-hashing-embedding-v1";

/// Vector dimension produced by [`HashingAiResourceEmbeddingService`].
pub const HASHING_DIMENSION: usize = 384;

/// Embedding abstraction for AI resource retrieval.
///
/// Mirrors upstream `AiResourceEmbeddingService`.
pub trait AiResourceEmbeddingService: Send + Sync {
    /// Embedding model identifier persisted with vector rows.
    fn model(&self) -> &str;

    /// Embedding vector dimension.
    fn dimension(&self) -> usize;

    /// Embed `text` into a normalized vector of [`Self::dimension`] values.
    fn embed(&self, text: &str) -> Vec<f32>;
}

/// Deterministic local embedding used until a deployment provides a
/// model-backed implementation.
///
/// Mirrors upstream `HashingAiResourceEmbeddingService`.
#[derive(Clone, Copy, Debug, Default)]
pub struct HashingAiResourceEmbeddingService;

impl HashingAiResourceEmbeddingService {
    /// Creates the hashing embedder.
    pub fn new() -> Self {
        Self
    }
}

impl AiResourceEmbeddingService for HashingAiResourceEmbeddingService {
    fn model(&self) -> &str {
        HASHING_MODEL
    }

    fn dimension(&self) -> usize {
        HASHING_DIMENSION
    }

    fn embed(&self, text: &str) -> Vec<f32> {
        embed(text, HASHING_DIMENSION)
    }
}

/// CRC-32 (IEEE 802.3, reflected, polynomial `0xEDB88320`).
///
/// Upstream hashes tokens with `java.util.zip.CRC32`; this is the same
/// algorithm. Verified against the standard check value in the tests.
pub fn crc32(bytes: &[u8]) -> u32 {
    let table = crc32_table();
    let mut crc = 0xFFFF_FFFFu32;
    for &byte in bytes {
        let index = ((crc ^ byte as u32) & 0xFF) as usize;
        crc = table[index] ^ (crc >> 8);
    }
    !crc
}

/// The CRC-32 lookup table, built once.
fn crc32_table() -> &'static [u32; 256] {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        const POLY: u32 = 0xEDB8_8320;
        let mut table = [0u32; 256];
        for (index, entry) in table.iter_mut().enumerate() {
            let mut value = index as u32;
            for _ in 0..8 {
                value = if value & 1 != 0 {
                    POLY ^ (value >> 1)
                } else {
                    value >> 1
                };
            }
            *entry = value;
        }
        table
    })
}

/// Split `text` into the tokens upstream hashes.
///
/// Every maximal run of letters/digits longer than one character, plus every
/// adjacent character pair that does not span whitespace. The text is
/// lowercased first, and blank text yields no tokens.
pub fn tokens(text: &str) -> Vec<String> {
    let normalized = text.to_lowercase();
    if normalized.trim().is_empty() {
        return Vec::new();
    }

    let mut result = Vec::new();

    // Maximal runs of letters and digits, keeping runs longer than one char.
    let mut current = String::new();
    for c in normalized.chars() {
        if c.is_alphanumeric() {
            current.push(c);
        } else {
            if current.chars().count() > 1 {
                result.push(std::mem::take(&mut current));
            }
            current.clear();
        }
    }
    if current.chars().count() > 1 {
        result.push(current);
    }

    // Character bigrams that do not cross whitespace.
    let chars: Vec<char> = normalized.chars().collect();
    for pair in chars.windows(2) {
        if !pair[0].is_whitespace() && !pair[1].is_whitespace() {
            result.push(pair.iter().collect());
        }
    }

    result
}

/// Embed `text` into `dimension` values, mirroring upstream's hashing embedder.
///
/// Each token is hashed to a bucket and contributes `+1` or `-1` depending on
/// the hash parity; the vector is then L2-normalized. A zero vector stays zero
/// (upstream returns early when the norm is not positive).
pub fn embed(text: &str, dimension: usize) -> Vec<f32> {
    let mut vector = vec![0.0f32; dimension];
    for token in tokens(text) {
        let hash = crc32(token.as_bytes());
        let index = (hash % dimension as u32) as usize;
        vector[index] += if hash & 1 == 0 { 1.0 } else { -1.0 };
    }
    normalize(&mut vector);
    vector
}

/// Scale `vector` to unit length, leaving a zero vector untouched.
fn normalize(vector: &mut [f32]) {
    let norm: f32 = vector.iter().map(|v| v * v).sum();
    if norm <= 0.0 {
        return;
    }
    let scale = norm.sqrt();
    for value in vector.iter_mut() {
        *value /= scale;
    }
}

/// Cosine similarity of two vectors.
///
/// Both operands are expected to be normalized, which makes this a dot
/// product. Returns `0.0` for mismatched or empty input.
pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn embedder() -> HashingAiResourceEmbeddingService {
        HashingAiResourceEmbeddingService::new()
    }

    #[test]
    fn crc32_matches_the_standard_check_value() {
        // The canonical CRC-32 check value; also covers the empty input.
        assert_eq!(crc32(b""), 0);
        assert_eq!(crc32(b"123456789"), 0xCBF4_3926);
    }

    #[test]
    fn crc32_is_stable_and_input_sensitive() {
        assert_eq!(crc32(b"weather"), crc32(b"weather"));
        assert_ne!(crc32(b"weather"), crc32(b"whether"));
    }

    #[test]
    fn model_and_dimension_match_upstream() {
        let svc = embedder();
        assert_eq!(svc.model(), "nacos-local-hashing-embedding-v1");
        assert_eq!(svc.dimension(), 384);
    }

    #[test]
    fn embed_has_the_model_dimension() {
        assert_eq!(embedder().embed("hello world").len(), HASHING_DIMENSION);
    }

    #[test]
    fn embed_is_deterministic() {
        let svc = embedder();
        let a = svc.embed("mcp weather server");
        let b = svc.embed("mcp weather server");
        assert_eq!(a, b, "the same text must always yield the same vector");
    }

    #[test]
    fn embed_is_normalized() {
        let vector = embedder().embed("mcp weather server");
        let norm: f32 = vector.iter().map(|v| v * v).sum();
        assert!(
            (norm - 1.0).abs() < 1e-4,
            "the vector must be unit length, got norm {norm}"
        );
    }

    #[test]
    fn blank_text_embeds_to_a_zero_vector() {
        for text in ["", "   ", "\t\n"] {
            let vector = embedder().embed(text);
            assert!(
                vector.iter().all(|v| *v == 0.0),
                "blank text must embed to zero, got {vector:?}"
            );
        }
    }

    #[test]
    fn different_texts_embed_differently() {
        let svc = embedder();
        assert_ne!(svc.embed("weather server"), svc.embed("database backup"));
    }

    #[test]
    fn tokens_cover_words_longer_than_one_char() {
        let tokens = tokens("a bb ccc");
        assert!(tokens.contains(&"bb".to_string()));
        assert!(tokens.contains(&"ccc".to_string()));
        assert!(
            !tokens.contains(&"a".to_string()),
            "single-character words are dropped upstream"
        );
    }

    #[test]
    fn tokens_include_bigrams_but_not_across_whitespace() {
        let tokens = tokens("ab cd");
        assert!(tokens.contains(&"ab".to_string()));
        assert!(tokens.contains(&"cd".to_string()));
        assert!(
            !tokens.contains(&" c".to_string()) && !tokens.contains(&"c ".to_string()),
            "bigrams must not span whitespace"
        );
    }

    #[test]
    fn tokens_are_lowercased() {
        assert!(tokens("Weather").contains(&"weather".to_string()));
    }

    #[test]
    fn related_text_is_more_similar_than_unrelated() {
        let svc = embedder();
        let base = svc.embed("mcp weather server");
        let related = svc.embed("mcp weather tool");
        let unrelated = svc.embed("database backup utility");
        let near = cosine_similarity(&base, &related);
        let far = cosine_similarity(&base, &unrelated);
        assert!(
            near > far,
            "shared vocabulary must score higher: related={near}, unrelated={far}"
        );
    }

    #[test]
    fn cosine_similarity_of_identical_vectors_is_one() {
        let vector = embedder().embed("some text");
        let score = cosine_similarity(&vector, &vector);
        assert!(
            (score - 1.0).abs() < 1e-4,
            "a vector is identical to itself, got {score}"
        );
    }

    #[test]
    fn cosine_similarity_rejects_mismatched_lengths() {
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[1.0]), 0.0);
        assert_eq!(cosine_similarity(&[], &[]), 0.0);
    }
}
