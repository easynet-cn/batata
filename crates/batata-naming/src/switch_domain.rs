//! Runtime-mutable switch domain for Nacos naming module.
//!
//! Matches Nacos's `SwitchDomain` bean which holds runtime-tunable parameters
//! for the naming service. Values can be changed via PUT /operator/switches.

use std::sync::atomic::{AtomicBool, AtomicI32, AtomicI64, Ordering};

/// Runtime-mutable naming switch domain.
///
/// Uses atomics for lock-free read/write. Initialized from Configuration at startup.
/// Can be updated at runtime via the admin switches API.
pub struct SwitchDomain {
    pub push_enabled: AtomicBool,
    pub health_check_enabled: AtomicBool,
    pub distro_enabled: AtomicBool,
    pub default_push_cache_millis: AtomicI64,
    pub client_beat_interval: AtomicI64,
    pub default_cache_millis: AtomicI64,
    pub check_times: AtomicI32,
    pub default_instance_ephemeral: AtomicBool,
}

impl SwitchDomain {
    /// Create with default values matching Nacos defaults
    pub fn new() -> Self {
        Self {
            push_enabled: AtomicBool::new(true),
            health_check_enabled: AtomicBool::new(true),
            distro_enabled: AtomicBool::new(false),
            default_push_cache_millis: AtomicI64::new(10000),
            client_beat_interval: AtomicI64::new(5000),
            default_cache_millis: AtomicI64::new(3000),
            check_times: AtomicI32::new(3),
            default_instance_ephemeral: AtomicBool::new(true),
        }
    }

    /// Initialize from Configuration
    pub fn from_config(config: &batata_server_common::Configuration) -> Self {
        Self {
            push_enabled: AtomicBool::new(true),
            health_check_enabled: AtomicBool::new(config.is_health_check()),
            distro_enabled: AtomicBool::new(!config.is_standalone()),
            default_push_cache_millis: AtomicI64::new(10000),
            client_beat_interval: AtomicI64::new(5000),
            default_cache_millis: AtomicI64::new(3000),
            check_times: AtomicI32::new(config.max_health_check_fail_count()),
            default_instance_ephemeral: AtomicBool::new(true),
        }
    }

    /// Update a switch entry by name. Returns true if the entry was recognized.
    pub fn update(&self, entry: &str, value: &str) -> bool {
        match entry {
            "pushEnabled" => {
                self.push_enabled
                    .store(value.parse().unwrap_or(true), Ordering::Relaxed);
                true
            }
            "healthCheckEnabled" => {
                self.health_check_enabled
                    .store(value.parse().unwrap_or(true), Ordering::Relaxed);
                true
            }
            "distroEnabled" => {
                self.distro_enabled
                    .store(value.parse().unwrap_or(false), Ordering::Relaxed);
                true
            }
            "defaultPushCacheMillis" => {
                if let Ok(v) = value.parse() {
                    self.default_push_cache_millis.store(v, Ordering::Relaxed);
                }
                true
            }
            "clientBeatInterval" => {
                if let Ok(v) = value.parse() {
                    self.client_beat_interval.store(v, Ordering::Relaxed);
                }
                true
            }
            "defaultCacheMillis" => {
                if let Ok(v) = value.parse() {
                    self.default_cache_millis.store(v, Ordering::Relaxed);
                }
                true
            }
            "checkTimes" => {
                if let Ok(v) = value.parse() {
                    self.check_times.store(v, Ordering::Relaxed);
                }
                true
            }
            "defaultInstanceEphemeral" => {
                self.default_instance_ephemeral
                    .store(value.parse().unwrap_or(true), Ordering::Relaxed);
                true
            }
            _ => false,
        }
    }

    // Getters
    pub fn is_push_enabled(&self) -> bool {
        self.push_enabled.load(Ordering::Relaxed)
    }
    pub fn is_health_check_enabled(&self) -> bool {
        self.health_check_enabled.load(Ordering::Relaxed)
    }
    pub fn is_distro_enabled(&self) -> bool {
        self.distro_enabled.load(Ordering::Relaxed)
    }
    pub fn get_check_times(&self) -> i32 {
        self.check_times.load(Ordering::Relaxed)
    }
}

impl Default for SwitchDomain {
    fn default() -> Self {
        Self::new()
    }
}
