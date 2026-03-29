//! Stored-procedure access for Cloudflare state management.
//!
//! # Design
//! - Encapsulates Cloudflare state procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::PgPool;
use uuid::Uuid;

const INDEXER_CF_STATE_RESET_CALL: &str = r"
    SELECT indexer_cf_state_reset(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        reason_input => $3
    )
";

const INDEXER_CF_STATE_GET_CALL: &str = r"
    SELECT
        state::text,
        last_changed_at,
        cf_session_expires_at,
        cooldown_until,
        backoff_seconds,
        consecutive_failures,
        last_error_class::text
    FROM indexer_cf_state_get(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2
    )
";

/// Cloudflare state row for an indexer instance.
#[derive(Debug, Clone)]
pub struct IndexerCfStateRow {
    /// Current Cloudflare mitigation state for the indexer instance.
    pub state: String,
    /// Timestamp when the state last changed.
    pub last_changed_at: chrono::DateTime<chrono::Utc>,
    /// Expiration time for the current Cloudflare session, if any.
    pub cf_session_expires_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Cooldown window end time, if the indexer is cooling down.
    pub cooldown_until: Option<chrono::DateTime<chrono::Utc>>,
    /// Backoff duration in seconds, if backoff is active.
    pub backoff_seconds: Option<i32>,
    /// Count of consecutive failures in the current mitigation window.
    pub consecutive_failures: i32,
    /// Last error class associated with Cloudflare failures, if recorded.
    pub last_error_class: Option<String>,
}

/// Fetch Cloudflare state for an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_cf_state_get(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
) -> Result<IndexerCfStateRow> {
    let record = sqlx::query_as::<
        _,
        (
            String,
            chrono::DateTime<chrono::Utc>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<chrono::DateTime<chrono::Utc>>,
            Option<i32>,
            i32,
            Option<String>,
        ),
    >(INDEXER_CF_STATE_GET_CALL)
    .bind(actor_user_public_id)
    .bind(indexer_instance_public_id)
    .fetch_one(pool)
    .await
    .map_err(try_op("indexer cf state get"))?;

    Ok(IndexerCfStateRow {
        state: record.0,
        last_changed_at: record.1,
        cf_session_expires_at: record.2,
        cooldown_until: record.3,
        backoff_seconds: record.4,
        consecutive_failures: record.5,
        last_error_class: record.6,
    })
}

/// Reset Cloudflare mitigation state for an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_cf_state_reset(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    reason: &str,
) -> Result<()> {
    sqlx::query(INDEXER_CF_STATE_RESET_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .bind(reason)
        .execute(pool)
        .await
        .map_err(try_op("indexer cf state reset"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataError;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }
    #[tokio::test]
    async fn cf_state_reset_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = indexer_cf_state_reset(pool, actor, Uuid::new_v4(), "manual reset")
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }

    #[tokio::test]
    async fn cf_state_get_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = indexer_cf_state_get(pool, actor, Uuid::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }
}
