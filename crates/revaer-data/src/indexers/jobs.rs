//! Stored-procedure access for background job runners.
//!
//! # Design
//! - Encapsulates job runner procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{DataError, Result, try_op};
use sqlx::PgPool;

const JOB_CLAIM_NEXT_CALL: &str = r"
    SELECT job_claim_next(
        job_key_input => $1::job_key
    )
";

const JOB_RUN_RETENTION_PURGE_CALL: &str = "SELECT * FROM job_run_retention_purge_v2()";
const JOB_RUN_CONNECTIVITY_REFRESH_CALL: &str =
    "SELECT * FROM job_run_connectivity_profile_refresh_v2()";
const JOB_RUN_REPUTATION_ROLLUP_CALL: &str = r"
    SELECT * FROM job_run_reputation_rollup_v2(
        window_key_input => $1::reputation_window
    )
";
const JOB_RUN_CANONICAL_BACKFILL_CALL: &str =
    "SELECT * FROM job_run_canonical_backfill_best_source_v2()";
const JOB_RUN_CANONICAL_PRUNE_CALL: &str =
    "SELECT * FROM job_run_canonical_prune_low_confidence_v2()";
const JOB_RUN_BASE_SCORE_REFRESH_CALL: &str =
    "SELECT * FROM job_run_base_score_refresh_recent_v2()";
const JOB_RUN_RSS_BACKFILL_CALL: &str = "SELECT * FROM job_run_rss_subscription_backfill_v2()";
const JOB_RUN_POLICY_SNAPSHOT_GC_CALL: &str = "SELECT * FROM job_run_policy_snapshot_gc_v2()";
const JOB_RUN_POLICY_SNAPSHOT_REPAIR_CALL: &str =
    "SELECT * FROM job_run_policy_snapshot_refcount_repair_v2()";
const JOB_RUN_RATE_LIMIT_STATE_PURGE_CALL: &str =
    "SELECT * FROM job_run_rate_limit_state_purge_v2()";

#[derive(Debug, sqlx::FromRow)]
struct JobRunResult {
    ok: bool,
    error_code: Option<String>,
    error_detail: Option<String>,
}

impl JobRunResult {
    fn into_result(self, operation: &'static str, job_key: &'static str) -> Result<()> {
        if self.ok {
            return Ok(());
        }

        Err(DataError::JobFailed {
            operation,
            job_key,
            error_code: self.error_code,
            error_detail: self.error_detail,
        })
    }
}

/// Claim the next due job for the given key.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_claim_next(pool: &PgPool, job_key: &str) -> Result<()> {
    sqlx::query(JOB_CLAIM_NEXT_CALL)
        .bind(job_key)
        .execute(pool)
        .await
        .map_err(try_op("job claim next"))?;
    Ok(())
}

/// Run the retention purge job.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_run_retention_purge(pool: &PgPool) -> Result<()> {
    let result: JobRunResult = sqlx::query_as(JOB_RUN_RETENTION_PURGE_CALL)
        .fetch_one(pool)
        .await
        .map_err(try_op("job run retention purge"))?;
    result.into_result("job run retention purge", "retention_purge")
}

/// Refresh connectivity profiles.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_run_connectivity_profile_refresh(pool: &PgPool) -> Result<()> {
    let result: JobRunResult = sqlx::query_as(JOB_RUN_CONNECTIVITY_REFRESH_CALL)
        .fetch_one(pool)
        .await
        .map_err(try_op("job run connectivity profile refresh"))?;
    result.into_result(
        "job run connectivity profile refresh",
        "connectivity_profile_refresh",
    )
}

/// Run reputation rollups for a window.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_run_reputation_rollup(pool: &PgPool, window_key: &str) -> Result<()> {
    let result: JobRunResult = sqlx::query_as(JOB_RUN_REPUTATION_ROLLUP_CALL)
        .bind(window_key)
        .fetch_one(pool)
        .await
        .map_err(try_op("job run reputation rollup"))?;
    let job_key = match window_key {
        "1h" => "reputation_rollup_1h",
        "24h" => "reputation_rollup_24h",
        "7d" => "reputation_rollup_7d",
        _ => "reputation_rollup",
    };
    result.into_result("job run reputation rollup", job_key)
}

/// Run canonical best-source backfill.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_run_canonical_backfill_best_source(pool: &PgPool) -> Result<()> {
    let result: JobRunResult = sqlx::query_as(JOB_RUN_CANONICAL_BACKFILL_CALL)
        .fetch_one(pool)
        .await
        .map_err(try_op("job run canonical backfill best source"))?;
    result.into_result(
        "job run canonical backfill best source",
        "canonical_backfill_best_source",
    )
}

/// Run canonical prune for low-confidence entries.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_run_canonical_prune_low_confidence(pool: &PgPool) -> Result<()> {
    let result: JobRunResult = sqlx::query_as(JOB_RUN_CANONICAL_PRUNE_CALL)
        .fetch_one(pool)
        .await
        .map_err(try_op("job run canonical prune low confidence"))?;
    result.into_result(
        "job run canonical prune low confidence",
        "canonical_prune_low_confidence",
    )
}

/// Run base score refresh for recent sources.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_run_base_score_refresh_recent(pool: &PgPool) -> Result<()> {
    let result: JobRunResult = sqlx::query_as(JOB_RUN_BASE_SCORE_REFRESH_CALL)
        .fetch_one(pool)
        .await
        .map_err(try_op("job run base score refresh recent"))?;
    result.into_result(
        "job run base score refresh recent",
        "base_score_refresh_recent",
    )
}

/// Run RSS subscription backfill.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_run_rss_subscription_backfill(pool: &PgPool) -> Result<()> {
    let result: JobRunResult = sqlx::query_as(JOB_RUN_RSS_BACKFILL_CALL)
        .fetch_one(pool)
        .await
        .map_err(try_op("job run rss subscription backfill"))?;
    result.into_result(
        "job run rss subscription backfill",
        "rss_subscription_backfill",
    )
}

/// Run policy snapshot garbage collection.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_run_policy_snapshot_gc(pool: &PgPool) -> Result<()> {
    let result: JobRunResult = sqlx::query_as(JOB_RUN_POLICY_SNAPSHOT_GC_CALL)
        .fetch_one(pool)
        .await
        .map_err(try_op("job run policy snapshot gc"))?;
    result.into_result("job run policy snapshot gc", "policy_snapshot_gc")
}

/// Run policy snapshot refcount repair.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_run_policy_snapshot_refcount_repair(pool: &PgPool) -> Result<()> {
    let result: JobRunResult = sqlx::query_as(JOB_RUN_POLICY_SNAPSHOT_REPAIR_CALL)
        .fetch_one(pool)
        .await
        .map_err(try_op("job run policy snapshot refcount repair"))?;
    result.into_result(
        "job run policy snapshot refcount repair",
        "policy_snapshot_refcount_repair",
    )
}

/// Purge old rate limit state.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn job_run_rate_limit_state_purge(pool: &PgPool) -> Result<()> {
    let result: JobRunResult = sqlx::query_as(JOB_RUN_RATE_LIMIT_STATE_PURGE_CALL)
        .fetch_one(pool)
        .await
        .map_err(try_op("job run rate limit state purge"))?;
    result.into_result("job run rate limit state purge", "rate_limit_state_purge")
}

#[cfg(test)]
#[path = "jobs/tests.rs"]
mod tests;
