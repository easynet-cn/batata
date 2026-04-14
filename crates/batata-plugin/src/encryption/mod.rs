//! Encryption Plugin - pluggable backends for config encryption/decryption.
//!
//! Mirrors Nacos `EncryptionPluginService` + `EncryptionPluginManager`.
//! External plugins register into [`EncryptionPluginRegistry`] at startup;
//! the config publish path resolves a plugin by name and uses it to
//! encrypt/decrypt sensitive config content.
//!
//! The [`EncryptionPlugin`] trait itself lives in `batata-common::crypto`
//! so lower layers that do not depend on `batata-plugin` (e.g. persistence
//! helpers) can still reference it.

mod noop;
mod registry;

pub use batata_common::crypto::EncryptionPlugin;
pub use noop::NoopEncryptionPlugin;
pub use registry::{EncryptionPluginRegistry, global_encryption_registry};
