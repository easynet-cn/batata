//! Common infrastructure types
//!
//! These types are used across storage, consistency, and cluster layers.
//! They contain NO domain concepts (no service, instance, config, namespace, group).

use serde::{Deserialize, Serialize};

/// Paginated result — generic container for any domain type
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PagedResult<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub page_size: u32,
}

impl<T> PagedResult<T> {
    pub fn empty() -> Self {
        Self {
            items: Vec::new(),
            total: 0,
            page: 1,
            page_size: 10,
        }
    }

    pub fn new(items: Vec<T>, total: u64, page: u32, page_size: u32) -> Self {
        Self {
            items,
            total,
            page,
            page_size,
        }
    }

    pub fn total_pages(&self) -> u32 {
        if self.page_size == 0 {
            return 0;
        }
        ((self.total as f64) / (self.page_size as f64)).ceil() as u32
    }

    pub fn map<U>(self, f: impl FnMut(T) -> U) -> PagedResult<U> {
        PagedResult {
            items: self.items.into_iter().map(f).collect(),
            total: self.total,
            page: self.page,
            page_size: self.page_size,
        }
    }
}

/// Health status — generic tri-state, usable by both Nacos and Consul
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum HealthStatus {
    Passing,
    Warning,
    Critical,
    #[default]
    Unknown,
}

impl HealthStatus {
    /// Whether the status is considered healthy (Passing or Warning)
    pub fn is_healthy(&self) -> bool {
        matches!(self, HealthStatus::Passing | HealthStatus::Warning)
    }

    /// Whether the status is considered up (Passing only)
    pub fn is_passing(&self) -> bool {
        matches!(self, HealthStatus::Passing)
    }
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Passing => write!(f, "passing"),
            HealthStatus::Warning => write!(f, "warning"),
            HealthStatus::Critical => write!(f, "critical"),
            HealthStatus::Unknown => write!(f, "unknown"),
        }
    }
}

/// Change type for events
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChangeType {
    Created,
    Updated,
    Deleted,
}
