//! Initializer module - trait-driven service initialization
//!
//! Only the shared traits remain. Concrete initializers were an unused
//! parallel startup path; the real server is wired exclusively through
//! `AppBuilder::run()`.

/// `traits` module.
pub mod traits;

/// Re-exported item.
pub use traits::*;
