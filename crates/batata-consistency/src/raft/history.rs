//! Bounded helpers for config-history RocksDB scans.
//!
//! History keys are `tenant@@group@@data_id@@{id}` (see
//! [`super::state_machine::RocksStateMachine::config_history_key`]) and the
//! trailing id is a *decimal* string. A reverse lexicographic iterator would
//! therefore place `"9"` after `"100"`, so it cannot be used to serve an
//! id-descending page directly. These helpers rank candidates by the numeric
//! id parsed from the key while preserving the tie-break of the old stable
//! sort (RocksDB key order).

use std::cmp::{Ordering, Reverse};
use std::collections::BinaryHeap;

/// Upper bound on the number of candidates retained by one bounded history
/// query. `clamp_page` already caps `page_size` at 1000; a very deep page
/// would otherwise ask us to retain (and decode) an unbounded number of
/// entries, so such requests fall back to the streaming full-scan path.
pub(crate) const MAX_BOUNDED_HISTORY_KEYS: usize = 10_000;

/// Number of candidates a bounded query may retain, or `None` when the page
/// is too deep and the streaming full-scan path must be used instead.
pub(crate) fn bounded_history_cap(offset: u64, page_size: u64) -> Option<usize> {
    let cap = usize::try_from(offset.saturating_add(page_size)).unwrap_or(usize::MAX);
    (cap <= MAX_BOUNDED_HISTORY_KEYS).then_some(cap)
}

/// Parse the numeric id from the trailing `@@{id}` segment of a history key.
///
/// Returns `None` for keys that do not carry a plain decimal id. Callers must
/// treat that as "unknown" and fall back to decoding the stored record rather
/// than assuming the entry is not a match.
pub(crate) fn history_id_from_key(key: &[u8]) -> Option<i64> {
    let pos = key.windows(2).rposition(|window| window == b"@@")?;
    let tail = std::str::from_utf8(&key[pos + 2..]).ok()?;
    if tail.is_empty() || !tail.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    tail.parse::<i64>().ok()
}

/// A history candidate ordered by its rank in the final page.
///
/// `Ord` is a *priority* ordering: a larger numeric id is "greater"; for equal
/// ids the lexicographically smaller key is "greater", which reproduces the
/// order a stable `sort_by(|a, b| b.id.cmp(&a.id))` would leave them in when
/// the input arrives in RocksDB key order.
struct Ranked<T> {
    id: i64,
    key: Vec<u8>,
    value: T,
}

impl<T> PartialEq for Ranked<T> {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl<T> Eq for Ranked<T> {}

impl<T> PartialOrd for Ranked<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> Ord for Ranked<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.id
            .cmp(&other.id)
            .then_with(|| other.key.cmp(&self.key))
    }
}

/// A bounded top-K selector for id-descending history pages.
///
/// Keeps at most `cap` candidates in a min-heap (`BinaryHeap<Reverse<_>>`), so
/// memory is O(cap) instead of O(number of matching keys) and the ranking cost
/// is O(n log cap) instead of materializing every deserialized record and
/// sorting all of them.
pub(crate) struct HistoryTopK<T> {
    cap: usize,
    heap: BinaryHeap<Reverse<Ranked<T>>>,
}

impl<T> HistoryTopK<T> {
    /// Create a selector retaining at most `cap` candidates.
    pub(crate) fn new(cap: usize) -> Self {
        Self {
            cap,
            heap: BinaryHeap::new(),
        }
    }

    /// Number of candidates currently retained. Test builds use this to prove
    /// the fallback path stays bounded by the requested page depth.
    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.heap.len()
    }

    /// Offer a candidate. Candidates that cannot reach the first `cap` slots
    /// are dropped without keeping their (potentially large) value.
    pub(crate) fn push(&mut self, id: i64, key: Vec<u8>, value: T) {
        if self.cap == 0 {
            return;
        }
        let candidate = Ranked { id, key, value };
        if self.heap.len() < self.cap {
            self.heap.push(Reverse(candidate));
            return;
        }
        let better = self.heap.peek().is_some_and(|worst| candidate > worst.0);
        if better {
            self.heap.pop();
            self.heap.push(Reverse(candidate));
        }
    }

    /// Return the retained candidates as `(id, key, value)` in final page
    /// order: numeric id descending, RocksDB key ascending for ties.
    pub(crate) fn into_sorted_entries(self) -> Vec<(i64, Vec<u8>, T)> {
        let mut ranked: Vec<Ranked<T>> = self.heap.into_iter().map(|item| item.0).collect();
        ranked.sort_by(|a, b| b.cmp(a));
        ranked
            .into_iter()
            .map(|item| (item.id, item.key, item.value))
            .collect()
    }

    /// Return the retained candidates as `(id, value)` in final page order.
    pub(crate) fn into_sorted(self) -> Vec<(i64, T)> {
        self.into_sorted_entries()
            .into_iter()
            .map(|(id, _, value)| (id, value))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reference_page(mut entries: Vec<(i64, Vec<u8>, i64)>, cap: usize) -> Vec<i64> {
        // Stable sort by id descending, exactly like the old reader path.
        entries.sort_by(|a, b| b.0.cmp(&a.0));
        entries
            .into_iter()
            .take(cap)
            .map(|(_, _, value)| value)
            .collect()
    }

    fn fixture() -> Vec<(i64, Vec<u8>, i64)> {
        // Deliberately non-padded ids: lexicographic key order differs from
        // numeric id order, which is what defeats a reverse iterator.
        let ids = [1, 2, 9, 10, 99, 100, 101, 1000];
        let mut entries: Vec<(i64, Vec<u8>, i64)> = ids
            .iter()
            .map(|id| {
                (
                    *id,
                    format!("public@@DEFAULT_GROUP@@app.yml@@{}", id).into_bytes(),
                    *id,
                )
            })
            .collect();
        entries.sort_by(|a, b| a.1.cmp(&b.1)); // RocksDB key order
        entries
    }

    #[test]
    fn parses_numeric_id_from_history_key() {
        assert_eq!(
            history_id_from_key(b"public@@DEFAULT_GROUP@@app.yml@@100"),
            Some(100)
        );
        assert_eq!(history_id_from_key(b"a@@b@@c@@0"), Some(0));
        // Data ids may themselves contain "@@"; only the last segment counts.
        assert_eq!(history_id_from_key(b"t@@g@@d@@x@@42"), Some(42));
    }

    #[test]
    fn rejects_keys_without_a_decimal_id() {
        assert_eq!(history_id_from_key(b"public@@DEFAULT_GROUP@@app.yml"), None);
        assert_eq!(history_id_from_key(b"t@@g@@d@@"), None);
        assert_eq!(history_id_from_key(b"t@@g@@d@@12x"), None);
        assert_eq!(history_id_from_key(b"t@@g@@d@@-3"), None);
        assert_eq!(history_id_from_key(b"no-separator"), None);
        assert_eq!(
            history_id_from_key(b"t@@g@@d@@99999999999999999999999"),
            None
        );
    }

    #[test]
    fn top_k_matches_reference_order_for_every_cap() {
        let fixture = fixture();
        for cap in 0..=fixture.len() + 2 {
            let mut top = HistoryTopK::new(cap);
            for (id, key, value) in &fixture {
                top.push(*id, key.clone(), *value);
            }
            assert_eq!(
                top.into_sorted()
                    .into_iter()
                    .map(|(_, v)| v)
                    .collect::<Vec<_>>(),
                reference_page(fixture.clone(), cap),
                "cap={} must reproduce the old stable id-descending order",
                cap
            );
        }
    }

    #[test]
    fn equal_ids_keep_rocksdb_key_order() {
        let mut top = HistoryTopK::new(2);
        for key in [b"t@@g@@d2@@7".to_vec(), b"t@@g@@d1@@7".to_vec()] {
            let id = history_id_from_key(&key).unwrap();
            top.push(id, key.clone(), String::from_utf8(key).unwrap());
        }
        let values: Vec<String> = top.into_sorted().into_iter().map(|(_, v)| v).collect();
        assert_eq!(values, vec!["t@@g@@d1@@7", "t@@g@@d2@@7"]);
    }

    #[test]
    fn zero_capacity_drops_every_candidate() {
        let mut top = HistoryTopK::new(0);
        top.push(7, b"t@@g@@d@@7".to_vec(), 7);
        assert!(top.into_sorted().is_empty());
    }

    /// The streaming fallback re-reads its page by exact key, so the selector
    /// must expose the original RocksDB keys in final page order.
    #[test]
    fn into_sorted_entries_returns_ranked_keys_for_exact_reads() {
        let fixture = fixture();
        let mut top = HistoryTopK::new(3);
        for (id, key, _) in &fixture {
            top.push(*id, key.clone(), ());
        }
        assert_eq!(top.len(), 3, "selector must stay bounded by cap");

        let entries = top.into_sorted_entries();
        assert_eq!(
            entries.iter().map(|(id, _, _)| *id).collect::<Vec<_>>(),
            vec![1000, 101, 100]
        );
        assert_eq!(
            entries[0].1,
            b"public@@DEFAULT_GROUP@@app.yml@@1000".to_vec()
        );
        assert_eq!(
            entries[2].1,
            b"public@@DEFAULT_GROUP@@app.yml@@100".to_vec()
        );
    }

    #[test]
    fn selector_len_never_exceeds_capacity() {
        let mut top = HistoryTopK::new(4);
        for id in 0..50 {
            top.push(id, Vec::new(), id);
        }
        assert_eq!(top.len(), 4);
        assert_eq!(top.into_sorted().len(), 4);
    }
}
