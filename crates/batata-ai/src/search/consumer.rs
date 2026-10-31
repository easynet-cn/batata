//! Consumer for durable search-index tasks.
//!
//! Mirrors upstream `AiResourceIndexTaskConsumer`: poll due tasks, claim one
//! with a lease, run the `base_index` stage, then complete or retry with
//! backoff. The `llm_enhancement` stage and vector convergence are not
//! implemented.
//!
//! Claiming is exclusive: the store hands out the lease only to the caller
//! whose (`revision`, `lease_token`) pair still matches, so several nodes can
//! poll the same queue without coordinating (upstream relies on the same
//! compare-and-set rather than on electing a leader). The lease token fences
//! the follow-up writes too — a worker whose lease was taken over cannot
//! complete or remove a task it no longer owns.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use batata_persistence::PersistenceService;
use batata_persistence::model::AiResourceTaskInfo;
use tokio::task::JoinHandle;
use tracing::warn;

use crate::repository::search;

use super::service::AiResourceSearchService;
use super::task::IndexTaskPayload;

/// Tasks fetched per poll, matching upstream `BATCH_SIZE`.
pub const BATCH_SIZE: u64 = 100;

/// How long a claimed task is owned before it becomes due again.
pub const LEASE_MILLIS: i64 = 60_000;

/// How often a held lease is pushed forward while a stage runs.
///
/// Upstream renews every 20s against a 60s lease, so a stage has to miss three
/// renewals before its task becomes claimable again. Without renewal a task
/// that legitimately takes longer than one lease would be picked up a second
/// time while the first worker is still running it.
pub const LEASE_RENEW_INTERVAL_SECONDS: u64 = 20;

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
    /// The task was skipped: already completed, still leased by another
    /// worker, or the lease was lost before the outcome could be recorded.
    Skipped,
}

/// Consumes search-index tasks.
pub struct AiResourceIndexConsumer {
    persistence: Arc<dyn PersistenceService>,
    search: AiResourceSearchService,
}

impl AiResourceIndexConsumer {
    /// Creates a consumer backed by the given persistence.
    pub fn new(persistence: Arc<dyn PersistenceService>) -> Self {
        let search = AiResourceSearchService::new(persistence.clone());
        Self {
            persistence,
            search,
        }
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
            // `task_find_due` answers on the scheduling columns only, so a
            // completed task can still come back. `task_claim` would reject it
            // anyway; skipping here keeps the outcome readable.
            if task.status == search::TASK_STATUS_COMPLETED {
                outcomes.push(TaskOutcome::Skipped);
                continue;
            }

            let Some(claimed) = self
                .persistence
                .task_claim(&task, LEASE_MILLIS, now)
                .await?
            else {
                // Not due, or another worker took the lease first.
                outcomes.push(TaskOutcome::Skipped);
                continue;
            };
            let lease_token = claimed.lease_token;
            // Kept alive for as long as the stage runs; dropped (and cancelled)
            // as soon as the outcome is recorded.
            let lease = TaskLease::start(self.persistence.clone(), &claimed);

            // An unreadable payload is treated as a failed stage rather than a
            // silent skip: leaving the task due would re-pick it every poll.
            let result = match serde_json::from_str::<IndexTaskPayload>(&claimed.task_payload) {
                Ok(payload) => {
                    self.search
                        .rebuild_latest_mcp(&claimed.namespace_id, &payload.subject.resource_name)
                        .await
                }
                Err(e) => Err(anyhow::anyhow!("unreadable index task payload: {e}")),
            };

            // Renew once more before recording, matching upstream's
            // `lease.renewNow()`; a stage that overran its renewal window has
            // already lost the task and must not write to it.
            let owned = lease.renew_now(self.persistence.as_ref()).await;
            if !owned {
                warn!(
                    task_key = %claimed.task_key,
                    "Lost the index task lease while running it; the outcome is discarded"
                );
                outcomes.push(TaskOutcome::Skipped);
                continue;
            }

            match result {
                Ok(true) => {
                    let mut done = claimed.clone();
                    done.status = search::TASK_STATUS_COMPLETED.to_string();
                    done.lease_token = 0;
                    done.lease_expire_at = None;
                    outcomes.push(if self
                        .persistence
                        .task_update_leased(&done, lease_token)
                        .await?
                    {
                        TaskOutcome::Completed
                    } else {
                        // The lease was taken over; the successor owns the task.
                        warn!(task_key = %claimed.task_key, "Lost the index task lease before completing it");
                        TaskOutcome::Skipped
                    });
                }
                Ok(false) => {
                    // Resource gone: drop the task rather than retry forever.
                    outcomes.push(
                        if self
                            .persistence
                            .task_delete_leased(&claimed.task_key, lease_token)
                            .await?
                        {
                            TaskOutcome::Removed
                        } else {
                            TaskOutcome::Skipped
                        },
                    );
                }
                Err(e) => {
                    let mut retry = claimed.clone();
                    retry.status = search::TASK_STATUS_PENDING.to_string();
                    retry.lease_token = 0;
                    retry.lease_expire_at = None;
                    retry.retry_count = claimed.retry_count + 1;
                    retry.next_execute_at = now + retry_delay_seconds(retry.retry_count) * 1000;
                    retry.last_error = Some(e.to_string());
                    outcomes.push(
                        if self
                            .persistence
                            .task_update_leased(&retry, lease_token)
                            .await?
                        {
                            TaskOutcome::Retried
                        } else {
                            TaskOutcome::Skipped
                        },
                    );
                }
            }
        }

        Ok(outcomes)
    }
}

/// A lease held on one claimed task.
///
/// Mirrors upstream's inner `TaskLease`: a background task pushes the expiry
/// forward so a long stage is not taken over while it is still running, and
/// [`Self::is_owned`] flips to `false` the moment a renewal is refused. The
/// renewal task is cancelled when the lease is dropped.
struct TaskLease {
    task: AiResourceTaskInfo,
    owned: Arc<AtomicBool>,
    renewal: Option<JoinHandle<()>>,
}

impl TaskLease {
    /// Take the lease and start renewing it in the background.
    fn start(persistence: Arc<dyn PersistenceService>, claimed: &AiResourceTaskInfo) -> Self {
        let owned = Arc::new(AtomicBool::new(true));
        let flag = owned.clone();
        let renewed = claimed.clone();
        let renewal = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(LEASE_RENEW_INTERVAL_SECONDS)).await;
                let now = now_millis();
                let held = persistence
                    .task_renew_lease(&renewed, LEASE_MILLIS, now)
                    .await
                    // A transport error is treated the same as a refused
                    // renewal: without a successful round trip the worker has
                    // no evidence it still owns the task.
                    .unwrap_or(false);
                if !held {
                    flag.store(false, Ordering::Release);
                    return;
                }
            }
        });
        Self {
            task: claimed.clone(),
            owned,
            renewal: Some(renewal),
        }
    }

    /// Renew immediately and report whether the lease is still held.
    async fn renew_now(&self, persistence: &dyn PersistenceService) -> bool {
        if !self.is_owned() {
            return false;
        }
        let held = persistence
            .task_renew_lease(&self.task, LEASE_MILLIS, now_millis())
            .await
            .unwrap_or(false);
        if !held {
            self.owned.store(false, Ordering::Release);
        }
        held
    }

    /// Whether the lease is still held.
    fn is_owned(&self) -> bool {
        self.owned.load(Ordering::Acquire)
    }
}

impl Drop for TaskLease {
    fn drop(&mut self) {
        if let Some(handle) = self.renewal.take() {
            handle.abort();
        }
    }
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
