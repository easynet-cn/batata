//! AI Copilot for Batata — LLM-powered skill/prompt optimization and generation
//!
//! This crate provides:
//! - LLM provider abstraction (DashScope/OpenAI compatible)
//! - SSE streaming response processing
//! - Skill generation/optimization services
//! - Prompt optimization/debug services
//! - Console HTTP API endpoints
//! - Persistent configuration storage

pub mod agent;
pub mod api;
pub mod config;
pub mod model;
pub mod prompt;
pub mod service;
pub mod stream;

pub use agent::CopilotAgentManager;
pub use api::console_routes as copilot_console_routes;
pub use config::{CopilotConfig, CopilotConfigStorage};
