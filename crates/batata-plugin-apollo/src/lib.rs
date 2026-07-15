pub mod bincode {
    pub use batata_common::bincode::{deserialize, serialize};
}
pub mod plugin;
pub mod migration;
pub mod entity;
pub mod model;
pub mod persistence;
pub mod api;
pub mod service;
pub mod route;
pub mod raft;

pub use plugin::ApolloPlugin;
pub use model::config::ApolloPluginConfig;
