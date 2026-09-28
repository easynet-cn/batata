//! Consumer for durable search-index tasks.
//!
//! Mirrors upstream `AiResourceIndexTaskConsumer`: poll due tasks, claim one
//! with a lease, run the `base_index` stage, then complete or retry with
//! backoff. The `llm_enhancement` stage and vector convergence are not
//! implemented.
//!
//! Claiming is **not** atomic here: the persistence layer exposes an upsert but
//! no compare-and-set for tasks, so two nodes could both claim the same task.
//! The projection is idempotent (it is keyed by `source_digest`), so the worst
//! case is duplicate work rather than a wrong index.

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use batata_persistence::PersistenceService;
use tracing::warn;

use crate::repository::search;

use super::service::AiResourceSearchService;
use super::task::IndexTaskPayload;

/// Tasks fetched per poll, matching upstream `BATCH_SIZE`.
pub const BATCH_SIZE: u64 = 100;

/// How long a claimed task is owned before it becomes due again.
pub const LEASE_MILLIS: i64 = 60_000;

/// Upper bound on the `base_index` retry delay.
pub const MAX_RETRY_SECONDS: i64 = 300;

/// Default seconds between polls.
pub const DEFAULT_INTERVAL_SECONDS: u64 = 5;

/// Retry delay in seconds: `min(MAX_RETRY_SECONDS, 5 << min(retry_count, 6))`.
pub fn retry_delay_seconds(retry_count: i32) -> i64 {
    let exponent = retry_count.clamp(0, 6) as u32;
    (5i64 << exponent).min(MAX_RETRY_SECONDS)
}

/// Current time in epoch milliseconds.
fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or_default()
}

/// Outcome of one task execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TaskOutcome {
    /// The index was (re)built; the task is complete.
    Completed,
    /// The resource no longer exists; the task was removed.
    Removed,
    /// The stage failed; the task was rescheduled.
    Retried,
    /// The task was skipped (already completed or still leased).
    Skipped,
}

/// Consumes search-index tasks.
pub struct AiResourceIndexConsumer {
    persistence: Arc<dyn PersistenceService>,
    search: AiResourceSearchService,
    /// Token generator; injected so tests can control lease tokens.
    lease_tokens: Box<dyn Fn() -> i64 + Send + Sync>,
}

impl AiResourceIndexConsumer {
    /// Creates a consumer backed by the given persistence.
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        let search = AiResourceSearchService::new(persistence.clone());
        Self {
            persistence,
            search,
            lease_tokens: Box::new(random_lease_token),
        }
    }

    /// Override the lease token generator (used by tests).
    pub fn with_lease_tokens<F>(mut self, tokens: F) -> Self
    where
        F: Fn() -> i64 + Send + Sync + 'static,
    {
        self.lease_tokens = Box::new(tokens);
        self
    }

    /// Poll once and process every due task. Returns the outcome per task.
    pub async fn consume_once(&self) -> anyhow::Result<Vec<TaskOutcome>> {
        let now = now_millis();
        let tasks = self
            .persistence
            .task_find_due(search::TASK_TYPE, now, BATCH_SIZE)
            .await?;

        let mut outcomes = Vec::with_capacity(tasks.len());
        for task in tasks {
            // Completed tasks stay in the table, so skip them explicitly.
            if task.status == search::TASK_STATUS_COMPLETED {
                outcomes.push(TaskOutcome::Skipped);
                continue;
            }
            // Still owned by another worker.
            if task.status == search::TASK_STATUS_PROCESSING
                && task.lease_expire_at.is_some_and(|expiry| expiry > now)
            {
                outcomes.push(TaskOutcome::Skipped);
                continue;
            }

            let subject = match serde_json::from_str::<IndexTaskPayload>(&task.task_payload) {
                Ok(payload) => payload.subject,
                Err(e) => {
                    warn!(task_key = %task.task_key, error = %e, "Unreadable index task payload");
                    outcomes.push(TaskOutcome::Skipped);
                    continue;
                }
            };

            // Claim.
            let mut claimed = task.clone();
            claimed.status = search::TASK_STATUS_PROCESSING.to_string();
            claimed.lease_token = (self.lease_tokens)();
            claimed.lease_expire_at = Some(now + LEASE_MILLIS);
            claimed.revision = task.revision + 1;
            self.persistence.task_upsert(&claimed).await?;

            match self
                .search
                .rebuild_latest_mcp(&task.namespace_id, &subject.resource_name)
                .await
            {
                Ok(true) => {
                    let mut done = claimed.clone();
                    done.status = search::TASK_STATUS_COMPLETED.to_string();
                    done.lease_token = 0;
                    done.lease_expire_at = None;
                    done.revision = claimed.revision + 1;
                    self.persistence.task_upsert(&done).await?;
                    outcomes.push(TaskOutcome::Completed);
                }
                Ok(false) => {
                    // Resource gone: drop the task rather than retry forever.
                    self.persistence.task_delete(&task.task_key).await?;
                    outcomes.push(TaskOutcome::Removed);
                }
                Err(e) => {
                    let mut retry = claimed.clone();
                    retry.status = search::TASK_STATUS_PENDING.to_string();
                    retry.lease_token = 0;
                    retry.lease_expire_at = None;
                    retry.retry_count = task.retry_count + 1;
                    retry.next_execute_at = now + retry_delay_seconds(retry.retry_count) * 1000;
                    retry.last_error = Some(e.to_string());
                    retry.revision = claimed.revision + 1;
                    self.persistence.task_upsert(&retry).await?;
                    outcomes.push(TaskOutcome::Retried);
                }
            }
        }

        Ok(outcomes)
    }
}

/// Default lease token source.
fn random_lease_token() -> i64 {
    // Monotonic-ish and distinct per process; uniqueness within a table is all
    // that matters.
    now_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_backoff_matches_upstream() {
        assert_eq!(retry_delay_seconds(0), 5);
        assert_eq!(retry_delay_seconds(1), 10);
        assert_eq!(retry_delay_seconds(2), 20);
        assert_eq!(retry_delay_seconds(6), 300, "5 << 6 = 320, capped at 300");
        assert_eq!(
            retry_delay_seconds(50),
            300,
            "exponent is clamped, so huge counts stay capped"
        );
    }

    #[test]
    fn negative_retry_count_is_clamped() {
        assert_eq!(retry_delay_seconds(-3), 5);
    }
}
