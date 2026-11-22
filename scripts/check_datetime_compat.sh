#!/usr/bin/env bash
# ==============================================================================
# Byte-level A/B check for the chrono -> time migration.
#
# Builds a throwaway crate that depends on BOTH the historical chrono 0.4.45
# and the new batata_common::datetime adapter. For each representative instant
# it serializes with chrono, deserializes + re-serializes with the adapter, and
# asserts the bytes are identical. Any mismatch exits non-zero with a diff.
#
# This is the compatibility proof that --full does not provide: the full gate
# only proves the new binary can read data it wrote itself, not that it can
# read bytes written by the pre-migration binary.
#
# Usage:
#   bash scripts/check_datetime_compat.sh
#   bash scripts/check_datetime_compat.sh --keep   # keep scratch dir for debug
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
COMMON_PATH="${REPO_ROOT}/crates/batata-common"
KEEP=0
[ "${1:-}" = "--keep" ] && KEEP=1

WORK_DIR="$(mktemp -d /tmp/batata-datetime-ab.XXXXXX)"
if [ "$KEEP" -eq 0 ]; then
    trap 'rm -rf "$WORK_DIR"' EXIT
fi

mkdir -p "${WORK_DIR}/src"

cat > "${WORK_DIR}/Cargo.toml" <<EOF
[package]
name = "batata-datetime-ab"
version = "0.1.0"
edition = "2021"

[dependencies]
chrono = { version = "=0.4.45", features = ["serde"] }
time = { version = "=0.3.55", features = ["std", "formatting", "parsing", "serde-human-readable", "serde-well-known"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
batata-common = { path = "${COMMON_PATH}" }

[workspace]
EOF

cat > "${WORK_DIR}/src/main.rs" <<'RS'
use batata_common::datetime as dt;
use chrono::{DateTime, NaiveDateTime, SecondsFormat, TimeZone, Utc};
use serde::{Deserialize, Serialize};

const CASE_SECS: i64 = 1_763_829_296; // 2025-11-22T16:34:56Z

#[derive(Serialize, Deserialize)]
struct NaiveWire {
    #[serde(with = "dt::naive_serde")]
    value: dt::NaiveDateTime,
}

#[derive(Serialize, Deserialize)]
struct NaiveOptWire {
    #[serde(with = "dt::naive_serde::option")]
    value: Option<dt::NaiveDateTime>,
}

#[derive(Serialize)]
struct UtcWire {
    #[serde(with = "time::serde::rfc3339")]
    value: time::OffsetDateTime,
}

fn check(label: &str, old: &str, new: &str, failures: &mut Vec<String>) {
    if old != new {
        failures.push(format!("{label}:\n    chrono (old): {old}\n    time   (new): {new}"));
    }
}

fn main() {
    let mut failures = Vec::new();
    let mut checks = 0usize;

    // Sub-second widths: none / millis / micros / nanos. These are the exact
    // shapes chrono emits with SecondsFormat::AutoSi, which the adapter must
    // reproduce byte-for-byte on disk and on the wire.
    for ns in [0i64, 123_000_000, 123_456_000, 123_456_789] {
        let old_utc: DateTime<Utc> = Utc.timestamp_opt(CASE_SECS, ns as u32).unwrap();
        let old_naive: NaiveDateTime = old_utc.naive_utc();
        let new_utc = dt::from_timestamp(CASE_SECS, ns as u32).unwrap();
        let new_naive = dt::naive(new_utc);
        let tag = format!("ns={ns}");

        // 1. embedded JSON / SeaORM naive serde (naive_serde adapter)
        let old_bytes = serde_json::to_string(&old_naive).unwrap();
        let wire = NaiveWire { value: new_naive };
        let new_wire = serde_json::to_string(&wire).unwrap();
        let new_bytes = new_wire
            .strip_prefix(r#"{"value":"#)
            .and_then(|s| s.strip_suffix('}'))
            .unwrap()
            .to_string();
        check(&format!("{tag} naive serde bytes"), &old_bytes, &new_bytes, &mut failures);
        checks += 1;

        // 1b. Option variant, Some and None
        let some_wire = NaiveOptWire { value: Some(new_naive) };
        let some_json = serde_json::to_string(&some_wire).unwrap();
        check(
            &format!("{tag} naive option Some"),
            &format!(r#"{{"value":{old_bytes}}}"#),
            &some_json,
            &mut failures,
        );
        checks += 1;
        let none_json = serde_json::to_string(&NaiveOptWire { value: None }).unwrap();
        check(
            &format!("{tag} naive option None"),
            r#"{"value":null}"#,
            &none_json,
            &mut failures,
        );
        checks += 1;

        // 1c. deserialize bytes written by the old binary, then re-serialize
        let decoded: NaiveWire = serde_json::from_str(&format!(r#"{{"value":{old_bytes}}}"#)).unwrap();
        check(
            &format!("{tag} old-bytes -> adapter round-trip"),
            &old_bytes,
            &serde_json::to_string(&NaiveWire { value: decoded.value }).unwrap()
                .strip_prefix(r#"{"value":"#).unwrap().strip_suffix('}').unwrap(),
            &mut failures,
        );
        checks += 1;

        // 2. UTC serde via time::serde::rfc3339 (Consul SDK DeletedAt)
        let old_utc_bytes = serde_json::to_string(&old_utc).unwrap();
        let new_utc_bytes = serde_json::to_string(&UtcWire { value: new_utc.into() }).unwrap();
        let new_utc_inner = new_utc_bytes
            .strip_prefix(r#"{"value":"#)
            .and_then(|s| s.strip_suffix('}'))
            .unwrap()
            .to_string();
        check(&format!("{tag} UTC rfc3339 serde bytes"), &old_utc_bytes, &new_utc_inner, &mut failures);
        checks += 1;

        // 3. RFC3339 default (chrono +00:00) and explicit Millis+Z
        check(
            &format!("{tag} rfc3339 +00:00"),
            &old_utc.to_rfc3339(),
            &dt::rfc3339(new_utc),
            &mut failures,
        );
        check(
            &format!("{tag} rfc3339 millis Z"),
            &old_utc.to_rfc3339_opts(SecondsFormat::Millis, true),
            &dt::rfc3339_z(new_utc, dt::Precision::Millis),
            &mut failures,
        );
        checks += 2;

        // 4. Display helpers used by Apollo routes and console audit
        check(
            &format!("{tag} apollo +00:00"),
            &old_naive.format("%Y-%m-%dT%H:%M:%S%.f+00:00").to_string(),
            &dt::NaiveDateTimeExt::format_apollo(&new_naive),
            &mut failures,
        );
        check(
            &format!("{tag} apollo seconds +00:00"),
            &old_naive.format("%Y-%m-%dT%H:%M:%S+00:00").to_string(),
            &dt::NaiveDateTimeExt::format_apollo_secs(&new_naive),
            &mut failures,
        );
        check(
            &format!("{tag} console space"),
            &old_naive.format("%Y-%m-%d %H:%M:%S").to_string(),
            &dt::DateTimeExt::format_space(&new_utc),
            &mut failures,
        );
        check(
            &format!("{tag} compact"),
            &old_naive.format("%Y%m%d%H%M%S").to_string(),
            &dt::DateTimeExt::format_compact(&new_utc),
            &mut failures,
        );
        checks += 4;
    }

    // 5. Legacy input forms the new parser must still accept
    for legacy in [
        "1970-01-01T00:00:00",
        "1970-01-01 00:00:00",
        "1970-01-01 00:00:00.0",
        "2025-11-22T16:34:56.123456789",
        "2025-11-22 16:34:56.123456",
    ] {
        let parsed = dt::parse_naive_str(legacy);
        check(&format!("legacy parse {legacy}"), "Ok", if parsed.is_ok() { "Ok" } else { "Err" }, &mut failures);
        checks += 1;
    }
    check(
        "legacy rfc3339 Z parse",
        "Ok",
        if dt::parse_rfc3339("2025-11-22T16:34:56Z").is_ok() { "Ok" } else { "Err" },
        &mut failures,
    );
    checks += 1;

    if failures.is_empty() {
        println!("A/B PASS: {checks} byte-level checks, chrono 0.4.45 bytes == batata_common::datetime bytes");
    } else {
        eprintln!("A/B FAIL: {} of {checks} checks differ:", failures.len());
        for f in &failures {
            eprintln!("  - {f}");
        }
        std::process::exit(1);
    }
}
RS

echo "== Building A/B crate (chrono 0.4.45 vs batata_common::datetime) =="
echo "   scratch dir: ${WORK_DIR}"
CARGO_TARGET_DIR="${WORK_DIR}/target" cargo run --offline --quiet --manifest-path "${WORK_DIR}/Cargo.toml"
