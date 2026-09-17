//! AI Copilot for Batata — LLM-powered skill/prompt optimization and generation
//!
//! This crate provides:
//! - LLM provider abstraction (DashScope/OpenAI compatible)
//! - SSE streaming response processing
//! - Skill generation/optimization services
//! - Prompt optimization/debug services
//! - Console HTTP API endpoints
//! - Persistent configuration storage

#![warn(missing_docs)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::type_complexity)]
#![allow(clippy::unnecessary_sort_by)]
#![allow(clippy::assertions_on_constants)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::result_large_err)]
#![allow(clippy::empty_line_after_doc_comments)]

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
