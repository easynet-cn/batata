//! Visibility constants — aligned with Nacos VisibilityConstants

/// Public scope — resource visible to all users in the namespace
pub const SCOPE_PUBLIC: &str = "PUBLIC";

/// Private scope — resource visible only to owner and admins
pub const SCOPE_PRIVATE: &str = "PRIVATE";

/// Read action identifier
pub const ACTION_READ: &str = "r";

/// Write action identifier
pub const ACTION_WRITE: &str = "w";
