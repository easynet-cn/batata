//! Shared scalar readers for protocol-adapter plugin configuration.
//!
//! Consul and Apollo both build their config from `config::Config` with the
//! same "read a key, otherwise use a default" pattern. Keeping the scalar
//! readers here prevents the two `from_config` implementations from drifting
//! on defaults or on the CPU-based HTTP worker sizing.

/// Read a boolean plugin setting, falling back to `default` when the key is
/// absent or has the wrong type.
pub fn plugin_bool(cfg: &config::Config, key: &str, default: bool) -> bool {
    cfg.get_bool(key).unwrap_or(default)
}

/// Read an integer plugin setting, falling back to `default` when the key is
/// absent or has the wrong type.
pub fn plugin_int(cfg: &config::Config, key: &str, default: i64) -> i64 {
    cfg.get_int(key).unwrap_or(default)
}

/// Read a string plugin setting, falling back to `default` when the key is
/// absent or has the wrong type.
pub fn plugin_string(cfg: &config::Config, key: &str, default: impl Into<String>) -> String {
    cfg.get_string(key).unwrap_or_else(|_| default.into())
}

/// Read an optional string plugin setting, returning `None` when the key is
/// absent or has the wrong type.
pub fn plugin_string_opt(cfg: &config::Config, key: &str) -> Option<String> {
    cfg.get_string(key).ok()
}

/// Read the HTTP worker count: an explicit value wins; zero or an absent key
/// derives `min(4, cpus / 2)` with a floor of 2. Both protocol adapters use
/// the same rule so their serving capacity stays comparable.
pub fn plugin_http_workers(cfg: &config::Config, key: &str) -> usize {
    let configured = plugin_int(cfg, key, 0) as usize;
    if configured != 0 {
        return configured;
    }
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    std::cmp::min(4, cpus / 2).max(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    const KEY: &str = "batata.plugin.test.value";

    fn empty() -> config::Config {
        config::Config::builder().build().unwrap()
    }

    fn with<T: Into<config::Value>>(value: T) -> config::Config {
        config::Config::builder()
            .set_override(KEY, value)
            .unwrap()
            .build()
            .unwrap()
    }

    #[test]
    fn plugin_bool_uses_default_and_override() {
        assert!(!plugin_bool(&empty(), KEY, false));
        assert!(plugin_bool(&empty(), KEY, true));
        // An override of `false` must win over a `true` default.
        assert!(!plugin_bool(&with(false), KEY, true));
    }

    #[test]
    fn plugin_bool_falls_back_on_wrong_type() {
        assert!(plugin_bool(&with("not-a-bool"), KEY, true));
    }

    #[test]
    fn plugin_int_uses_default_and_override() {
        assert_eq!(plugin_int(&empty(), KEY, 8080), 8080);
        assert_eq!(plugin_int(&with(9090), KEY, 8080), 9090);
    }

    #[test]
    fn plugin_string_uses_default_and_override() {
        assert_eq!(plugin_string(&empty(), KEY, "dc1"), "dc1");
        assert_eq!(plugin_string(&with("dc2"), KEY, "dc1"), "dc2");
    }

    #[test]
    fn plugin_string_opt_distinguishes_missing_from_present() {
        assert_eq!(plugin_string_opt(&empty(), KEY), None);
        assert_eq!(
            plugin_string_opt(&with("token"), KEY).as_deref(),
            Some("token")
        );
    }

    #[test]
    fn plugin_http_workers_prefers_explicit_value() {
        assert_eq!(plugin_http_workers(&with(7), KEY), 7);
    }

    #[test]
    fn plugin_http_workers_derives_bounded_default() {
        let derived = plugin_http_workers(&empty(), KEY);
        assert!(
            (2..=4).contains(&derived),
            "unexpected worker count: {derived}"
        );
        // Explicit zero must take the same derived path as an absent key.
        assert_eq!(plugin_http_workers(&with(0), KEY), derived);
    }
}
