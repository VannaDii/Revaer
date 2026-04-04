//! Stored-procedure access for source metadata conflict resolution.
//!
//! # Design
//! - Encapsulates conflict resolution procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

const CONFLICT_RESOLVE_CALL: &str = r"
    SELECT source_metadata_conflict_resolve(
        actor_user_public_id => $1,
        conflict_id_input => $2,
        resolution_input => $3::conflict_resolution,
        resolution_note_input => $4
    )
";

const CONFLICT_REOPEN_CALL: &str = r"
    SELECT source_metadata_conflict_reopen(
        actor_user_public_id => $1,
        conflict_id_input => $2,
        resolution_note_input => $3
    )
";

const CONFLICT_LOG_CALL: &str = r"
    SELECT log_source_metadata_conflict(
        canonical_torrent_source_id_input => $1,
        indexer_instance_id_input => $2,
        conflict_type_input => $3::conflict_type,
        existing_value_input => $4,
        incoming_value_input => $5,
        observed_at_input => $6
    )
";

const CONFLICT_LIST_CALL: &str = r"
    SELECT
        conflict_id,
        conflict_type::text AS conflict_type,
        existing_value,
        incoming_value,
        observed_at,
        resolved_at,
        resolution::text AS resolution,
        resolution_note
    FROM source_metadata_conflict_list(
        actor_user_public_id => $1,
        include_resolved_input => $2,
        limit_input => $3
    )
";

/// Row returned by source metadata conflict listing.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SourceMetadataConflictRow {
    /// Numeric conflict identifier defined by the ERD resolve/reopen proc contract.
    pub conflict_id: i64,
    /// Conflict type label.
    pub conflict_type: String,
    /// Existing durable value.
    pub existing_value: String,
    /// Incoming conflicting value.
    pub incoming_value: String,
    /// First observed timestamp.
    pub observed_at: DateTime<Utc>,
    /// Resolution timestamp, when resolved.
    pub resolved_at: Option<DateTime<Utc>>,
    /// Resolution label, when resolved.
    pub resolution: Option<String>,
    /// Optional operator note recorded during resolution.
    pub resolution_note: Option<String>,
}

/// Resolve a source metadata conflict.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn source_metadata_conflict_resolve(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    conflict_id: i64,
    resolution: &str,
    resolution_note: Option<&str>,
) -> Result<()> {
    sqlx::query(CONFLICT_RESOLVE_CALL)
        .bind(actor_user_public_id)
        .bind(conflict_id)
        .bind(resolution)
        .bind(resolution_note)
        .execute(pool)
        .await
        .map_err(try_op("source metadata conflict resolve"))?;
    Ok(())
}

/// Reopen a resolved source metadata conflict.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn source_metadata_conflict_reopen(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    conflict_id: i64,
    resolution_note: Option<&str>,
) -> Result<()> {
    sqlx::query(CONFLICT_REOPEN_CALL)
        .bind(actor_user_public_id)
        .bind(conflict_id)
        .bind(resolution_note)
        .execute(pool)
        .await
        .map_err(try_op("source metadata conflict reopen"))?;
    Ok(())
}

/// Log a new source metadata conflict.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn log_source_metadata_conflict(
    pool: &PgPool,
    canonical_torrent_source_id: i64,
    indexer_instance_id: i64,
    conflict_type: &str,
    existing_value: Option<&str>,
    incoming_value: Option<&str>,
    observed_at: Option<DateTime<Utc>>,
) -> Result<()> {
    sqlx::query(CONFLICT_LOG_CALL)
        .bind(canonical_torrent_source_id)
        .bind(indexer_instance_id)
        .bind(conflict_type)
        .bind(existing_value)
        .bind(incoming_value)
        .bind(observed_at)
        .execute(pool)
        .await
        .map_err(try_op("source metadata conflict log"))?;
    Ok(())
}

/// List source metadata conflicts for operator review.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn source_metadata_conflict_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    include_resolved: Option<bool>,
    limit: Option<i32>,
) -> Result<Vec<SourceMetadataConflictRow>> {
    sqlx::query_as(CONFLICT_LIST_CALL)
        .bind(actor_user_public_id)
        .bind(include_resolved)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(try_op("source metadata conflict list"))
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
    async fn conflict_resolve_reopen_errors_on_missing_conflict() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = source_metadata_conflict_resolve(pool, actor, 1, "ignored", None)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("conflict_not_found"));

        let err = source_metadata_conflict_reopen(pool, actor, 1, None)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("conflict_not_resolved"));

        let err = log_source_metadata_conflict(
            pool,
            1,
            1,
            "hash",
            Some("old"),
            Some("new"),
            Some(Utc::now()),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_code(), Some("23503".to_string()));
        Ok(())
    }

    #[tokio::test]
    async fn conflict_list_requires_actor() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let err = source_metadata_conflict_list(test_db.pool(), Uuid::new_v4(), None, None)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("actor_not_found"));
        Ok(())
    }
}
