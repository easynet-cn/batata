//! Release key generation shared by every Apollo release path.

use std::hash::{DefaultHasher, Hash, Hasher};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicU32, Ordering};

/// Process-scoped stand-in for upstream `MachineUtil.getMachineIdentifier()`.
///
/// Two nodes in a cluster can publish in the same second with the same counter
/// value, so the machine identifier has to differ per process — otherwise the
/// generated keys collide and the `Release.ReleaseKey` unique index rejects one
/// of them.
fn machine_identifier() -> u32 {
    static MACHINE_IDENTIFIER: OnceLock<u32> = OnceLock::new();
    *MACHINE_IDENTIFIER.get_or_init(|| {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.subsec_nanos())
            .unwrap_or(0);
        nanos ^ (std::process::id().wrapping_mul(0x9E37_79B9))
    })
}

static COUNTER: AtomicU32 = AtomicU32::new(0);

/// Third byte of `value` (upstream `ByteUtil.int2`).
#[inline]
fn byte2(value: u32) -> u8 {
    ((value >> 16) & 0xff) as u8
}

/// Second byte of `value` (upstream `ByteUtil.int1`).
#[inline]
fn byte1(value: u32) -> u8 {
    ((value >> 8) & 0xff) as u8
}

/// Lowest byte of `value` (upstream `ByteUtil.int0`).
#[inline]
fn byte0(value: u32) -> u8 {
    (value & 0xff) as u8
}

/// Generate an Apollo release key.
///
/// Mirrors upstream `UniqueKeyGenerator.generate(Object... args)`: the key is
/// `{yyyyMMddHHmmss}-{16 hex chars}`, so it is always **31 characters** wide.
/// That matters because `InstanceConfig.ReleaseKey` is `varchar(64)` upstream
/// and MySQL/PostgreSQL reject longer values outright (error 1406 / `value too
/// long for type character varying(64)`).
///
/// The 16 hex chars encode eight bytes: two from `args`' hash, three from the
/// machine identifier and three from a monotonic counter — enough to keep keys
/// unique across hosts and within a burst of releases.
pub fn generate_release_key(args: &[&str]) -> String {
    let mut hasher = DefaultHasher::new();
    for arg in args {
        arg.hash(&mut hasher);
    }
    let key_hash = hasher.finish() as u32;

    let machine = machine_identifier();
    let counter = COUNTER.fetch_add(1, Ordering::Relaxed);

    let bytes: [u8; 8] = [
        byte1(key_hash),
        byte0(key_hash),
        byte2(machine),
        byte1(machine),
        byte0(machine),
        byte2(counter),
        byte1(counter),
        byte0(counter),
    ];

    let hex_id: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();
    format!("{}-{}", chrono::Utc::now().format("%Y%m%d%H%M%S"), hex_id)
}

#[cfg(test)]
mod tests {
    use super::generate_release_key;

    #[test]
    fn release_key_matches_upstream_shape() {
        let key = generate_release_key(&["app", "default", "application"]);
        assert_eq!(key.len(), 31, "release key must stay 31 chars: {}", key);
        let (ts, hex) = key.split_once('-').unwrap();
        assert_eq!(ts.len(), 14, "timestamp must be yyyyMMddHHmmss: {}", ts);
        assert!(ts.chars().all(|c| c.is_ascii_digit()));
        assert_eq!(hex.len(), 16);
        assert!(hex.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn release_key_fits_varchar64() {
        // `InstanceConfig.ReleaseKey` is varchar(64); gray merges concatenate
        // two keys with '+', so a single key must stay well under half that.
        let key = generate_release_key(&["some-long-app-id", "cluster", "namespace"]);
        assert!(key.len() * 2 + 1 <= 64);
    }
}
