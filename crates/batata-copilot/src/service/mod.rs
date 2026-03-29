//! Copilot business logic services

pub mod prompt_debug;
pub mod prompt_optimization;
pub mod skill_generation;
pub mod skill_optimization;

pub use prompt_debug::PromptDebugService;
pub use prompt_optimization::PromptOptimizationService;
pub use skill_generation::SkillGenerationService;
pub use skill_optimization::SkillOptimizationService;
