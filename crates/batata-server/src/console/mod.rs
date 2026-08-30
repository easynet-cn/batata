//! Module `console` of the `batata-server` crate.
// Console web interface module
// AI console handlers have been moved to the batata-console crate.
// Non-AI console handlers have been moved to the batata-console crate.

/// `client` module.
pub mod client;

/// `v3` module.
pub mod v3 {
    /// Re-exported item.
    pub use batata_console::v3::ai_a2a;
    /// Re-exported item.
    pub use batata_console::v3::ai_agentspec;
    /// Re-exported item.
    pub use batata_console::v3::ai_import;
    /// Re-exported item.
    pub use batata_console::v3::ai_mcp;
    /// Re-exported item.
    pub use batata_console::v3::ai_pipeline;
    /// Re-exported item.
    pub use batata_console::v3::ai_plugin;
    /// Re-exported item.
    pub use batata_console::v3::ai_skill;
}
