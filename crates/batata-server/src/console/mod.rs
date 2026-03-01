// Console web interface module
// AI console handlers have been moved to the batata-console crate.
// Non-AI console handlers have been moved to the batata-console crate.

pub mod client;

pub mod v3 {
    pub use batata_console::v3::ai_a2a;
    pub use batata_console::v3::ai_mcp;
    pub use batata_console::v3::ai_plugin;
}
