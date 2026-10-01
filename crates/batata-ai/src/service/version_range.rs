//! Agent runtime version range resolution.
//!
//! Mirrors upstream `RuntimeVersionRangeSupport`: a client discovers an agent
//! by a version *constraint* rather than an exact version, and the registry
//! answers with the highest published version that satisfies it.
//!
//! Supported forms:
//! - `latest` (or empty) — the highest published version.
//! - `1.2.3` — exact; returned only when that version is actually published.
//! - `1.2.x` / `1.2.*` — highest `1.2.*`.
//! - `1.x` / `1.*` — highest `1.*`.
//! - `^1.2.3` — highest `>= 1.2.3` below `2.0.0`, the usual caret range.
//!
//! Anything that parses as none of these is not a range; the caller should
//! treat it as an exact version and let the lookup fail naturally, rather than
//! silently resolving it to something else.

use batata_common::model::ai::skill::compare_versions;
use std::cmp::Ordering;

/// Pick the highest version in `available` that satisfies `requested`.
///
/// Returns `None` when nothing matches — including when `requested` is an exact
/// version that is not published, so a caller can report "no such version"
/// instead of serving endpoints for a different one.
pub fn select_version<'a>(available: &[&'a str], requested: &str) -> Option<&'a str> {
    let requested = requested.trim();
    if available.is_empty() {
        return None;
    }

    // `latest`, or no constraint at all: the highest published version.
    if requested.is_empty() || requested.eq_ignore_ascii_case("latest") {
        return available
            .iter()
            .copied()
            .max_by(|a, b| compare_versions(a, b));
    }

    // An exact version that is actually published wins outright.
    if let Some(exact) = available.iter().copied().find(|v| *v == requested) {
        return Some(exact);
    }

    let (lower, upper) = range_bounds(requested)?;
    available
        .iter()
        .copied()
        .filter(|v| compare_versions(v, &lower) != Ordering::Less)
        .filter(|v| match &upper {
            Some(upper) => compare_versions(v, upper) == Ordering::Less,
            None => true,
        })
        .max_by(|a, b| compare_versions(a, b))
}

/// Whether a constraint asks for a range at all.
///
/// Lets a caller distinguish "this is a range" from "this is an exact version
/// that happens not to be published".
pub fn is_range(requested: &str) -> bool {
    let requested = requested.trim();
    if requested.is_empty() || requested.eq_ignore_ascii_case("latest") {
        return true;
    }
    range_bounds(requested).is_some()
}

/// The half-open `[lower, upper)` bounds of a range, if `requested` is one.
fn range_bounds(requested: &str) -> Option<(String, Option<String>)> {
    let caret = requested.starts_with('^');
    let spec = requested.trim_start_matches('^');

    let mut parts = spec.split('.');
    let major = parts.next()?;
    let minor = parts.next();
    let patch = parts.next();

    let any_wild = [Some(major), minor, patch]
        .into_iter()
        .flatten()
        .any(is_wild);
    if !caret && !any_wild {
        // A plain exact version; not a range.
        return None;
    }

    let major_n = num_or_zero(major);
    let minor_n = minor.map(num_or_zero).unwrap_or(0);
    let patch_n = patch.map(num_or_zero).unwrap_or(0);

    let lower = format!("{major_n}.{minor_n}.{patch_n}");

    // The upper bound bumps the last component that is not a wildcard: `1.2.x`
    // stops at `1.3.0`, `1.x` stops at `2.0.0`.
    let upper = if is_wild(major) {
        None
    } else if caret {
        // A caret bumps the leftmost non-zero component: `^1.2.0` stops at
        // `2.0.0`, but `^0.2.0` stops at `0.3.0` and `^0.0.3` at `0.0.4`.
        if major_n > 0 {
            Some(format!("{}.0.0", major_n + 1))
        } else if minor_n > 0 {
            Some(format!("0.{}.0", minor_n + 1))
        } else {
            Some(format!("0.0.{}", patch_n + 1))
        }
    } else if minor.map(is_wild).unwrap_or(true) {
        Some(format!("{}.0.0", major_n + 1))
    } else if patch.map(is_wild).unwrap_or(true) {
        Some(format!("{major_n}.{}.0", minor_n + 1))
    } else {
        Some(format!("{major_n}.{minor_n}.{}", patch_n + 1))
    };

    Some((lower, upper))
}

fn is_wild(part: &str) -> bool {
    part == "x" || part == "X" || part == "*"
}

fn num_or_zero(part: &str) -> u32 {
    if is_wild(part) {
        0
    } else {
        part.parse().unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AVAILABLE: &[&str] = &["1.0.0", "1.2.0", "1.2.5", "1.4.1", "2.0.0", "2.1.3"];

    #[test]
    fn latest_picks_the_highest() {
        assert_eq!(select_version(AVAILABLE, "latest"), Some("2.1.3"));
        assert_eq!(select_version(AVAILABLE, ""), Some("2.1.3"));
    }

    #[test]
    fn an_exact_published_version_is_returned_as_is() {
        assert_eq!(select_version(AVAILABLE, "1.2.5"), Some("1.2.5"));
        assert_eq!(select_version(AVAILABLE, "2.0.0"), Some("2.0.0"));
    }

    /// An unpublished exact version must not silently resolve to another one —
    /// the caller needs to be able to say "no such version".
    #[test]
    fn an_exact_unpublished_version_matches_nothing() {
        assert_eq!(select_version(AVAILABLE, "1.2.6"), None);
        assert_eq!(select_version(AVAILABLE, "9.9.9"), None);
    }

    #[test]
    fn a_patch_wildcard_stays_inside_its_minor() {
        assert_eq!(select_version(AVAILABLE, "1.2.x"), Some("1.2.5"));
        assert_eq!(select_version(AVAILABLE, "1.2.*"), Some("1.2.5"));
        assert_eq!(select_version(AVAILABLE, "1.4.x"), Some("1.4.1"));
    }

    #[test]
    fn a_minor_wildcard_stays_inside_its_major() {
        assert_eq!(select_version(AVAILABLE, "1.x"), Some("1.4.1"));
        assert_eq!(select_version(AVAILABLE, "2.*"), Some("2.1.3"));
    }

    #[test]
    fn a_caret_range_stops_at_the_next_major() {
        assert_eq!(select_version(AVAILABLE, "^1.2.0"), Some("1.4.1"));
        assert_eq!(select_version(AVAILABLE, "^2.0.0"), Some("2.1.3"));
    }

    /// A caret range whose lower bound is above everything published in that
    /// major resolves to nothing, rather than spilling into the next major.
    #[test]
    fn a_caret_range_does_not_spill_into_the_next_major() {
        assert_eq!(select_version(AVAILABLE, "^3.0.0"), None);
    }

    #[test]
    fn nothing_available_matches_nothing() {
        assert_eq!(select_version(&[], "latest"), None);
        assert_eq!(select_version(&[], "1.x"), None);
    }

    #[test]
    fn ranges_and_exact_versions_are_distinguishable() {
        assert!(is_range("latest"));
        assert!(is_range("1.2.x"));
        assert!(is_range("^1.0.0"));
        assert!(!is_range("1.2.5"));
    }
}
