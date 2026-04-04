//! Stored-procedure access for connectivity, reputation, and health telemetry snapshots.
//!
//! # Design
//! - Exposes operator-facing read models for derived connectivity and reputation tables.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Returns explicit typed rows so UI/API layers stay free of database details.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const CONNECTIVITY_PROFILE_GET_CALL: &str = r"
    SELECT
        profile_exists,
        status::text,
        error_class::text,
        latency_p50_ms,
        latency_p95_ms,
        success_rate_1h::float8,
        success_rate_24h::float8,
        last_checked_at
    FROM indexer_connectivity_profile_get(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2
    )
";

const SOURCE_REPUTATION_LIST_CALL: &str = r"
    SELECT
        window_key::text,
        window_start,
        request_success_rate::float8,
        acquisition_success_rate::float8,
        fake_rate::float8,
        dmca_rate::float8,
        request_count,
        request_success_count,
        acquisition_count,
        acquisition_success_count,
        min_samples,
        computed_at
    FROM indexer_source_reputation_list(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        window_key_input => $3::reputation_window,
        limit_input => $4
    )
";

const HEALTH_EVENT_LIST_CALL: &str = r"
    SELECT
        occurred_at,
        event_type::text,
        latency_ms,
        http_status,
        error_class::text,
        detail
    FROM indexer_health_event_list(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        limit_input => $3
    )
";

/// Connectivity profile snapshot for an indexer instance.
#[derive(Debug, Clone, FromRow)]
pub struct IndexerConnectivityProfileRow {
    /// Whether the derived profile exists yet.
    pub profile_exists: bool,
    /// Derived connectivity status, when available.
    pub status: Option<String>,
    /// Dominant error class, when present.
    pub error_class: Option<String>,
    /// p50 latency in milliseconds.
    pub latency_p50_ms: Option<i32>,
    /// p95 latency in milliseconds.
    pub latency_p95_ms: Option<i32>,
    /// One-hour success rate.
    pub success_rate_1h: Option<f64>,
    /// Twenty-four-hour success rate.
    pub success_rate_24h: Option<f64>,
    /// Timestamp of the last profile refresh.
    pub last_checked_at: Option<DateTime<Utc>>,
}

/// Source reputation snapshot row.
#[derive(Debug, Clone, FromRow)]
pub struct IndexerSourceReputationRow {
    /// Window identifier (`1h`, `24h`, `7d`).
    pub window_key: String,
    /// Start time of the aggregation window.
    pub window_start: DateTime<Utc>,
    /// Request success rate for the window.
    pub request_success_rate: f64,
    /// Acquisition success rate for the window.
    pub acquisition_success_rate: f64,
    /// Fake-result rate for the window.
    pub fake_rate: f64,
    /// DMCA/removal rate for the window.
    pub dmca_rate: f64,
    /// Total request count.
    pub request_count: i32,
    /// Successful request count.
    pub request_success_count: i32,
    /// Total acquisition count.
    pub acquisition_count: i32,
    /// Successful acquisition count.
    pub acquisition_success_count: i32,
    /// Minimum samples threshold used for the rollup.
    pub min_samples: i32,
    /// Timestamp when the snapshot was computed.
    pub computed_at: DateTime<Utc>,
}

/// Health-event row for operator drill-down.
#[derive(Debug, Clone, FromRow)]
pub struct IndexerHealthEventRow {
    /// When the event occurred.
    pub occurred_at: DateTime<Utc>,
    /// Event type key.
    pub event_type: String,
    /// Request latency in milliseconds when known.
    pub latency_ms: Option<i32>,
    /// HTTP status code when known.
    pub http_status: Option<i32>,
    /// Error class when present.
    pub error_class: Option<String>,
    /// Optional diagnostic detail.
    pub detail: Option<String>,
}

/// Fetch connectivity profile state for an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_connectivity_profile_get(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
) -> Result<IndexerConnectivityProfileRow> {
    sqlx::query_as(CONNECTIVITY_PROFILE_GET_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("indexer connectivity profile get"))
}

/// List recent reputation rollups for an indexer instance and window.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_source_reputation_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    window_key: Option<&str>,
    limit: Option<i32>,
) -> Result<Vec<IndexerSourceReputationRow>> {
    sqlx::query_as(SOURCE_REPUTATION_LIST_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .bind(window_key)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(try_op("indexer source reputation list"))
}

/// List recent health events for an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_health_event_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    limit: Option<i32>,
) -> Result<Vec<IndexerHealthEventRow>> {
    sqlx::query_as(HEALTH_EVENT_LIST_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(try_op("indexer health event list"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataError;
    use sqlx::PgPool;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("connectivity tests").await
    }

    async fn insert_indexer_instance(pool: &PgPool) -> anyhow::Result<(i64, Uuid)> {
        let definition_id: i64 = sqlx::query_scalar(
            "INSERT INTO indexer_definition (
                upstream_source,
                upstream_slug,
                display_name,
                protocol,
                engine,
                schema_version,
                definition_hash,
                is_deprecated
            )
            VALUES ($1::upstream_source, $2, $3, $4::protocol, $5::engine, $6, $7, $8)
            RETURNING indexer_definition_id",
        )
        .bind("prowlarr_indexers")
        .bind(format!("connectivity-{}", Uuid::new_v4().simple()))
        .bind("Connectivity Definition")
        .bind("torrent")
        .bind("torznab")
        .bind(1_i32)
        .bind("b".repeat(64))
        .bind(false)
        .fetch_one(pool)
        .await?;

        let public_id = Uuid::new_v4();
        let instance_id: i64 = sqlx::query_scalar(
            "INSERT INTO indexer_instance (
                indexer_instance_public_id,
                indexer_definition_id,
                display_name,
                is_enabled,
                migration_state,
                enable_rss,
                enable_automatic_search,
                enable_interactive_search,
                priority,
                trust_tier_key,
                created_by_user_id,
                updated_by_user_id
            )
            VALUES (
                $1,
                $2,
                $3,
                TRUE,
                'ready'::indexer_instance_migration_state,
                TRUE,
                TRUE,
                TRUE,
                100,
                'public'::trust_tier_key,
                0,
                0
            )
            RETURNING indexer_instance_id",
        )
        .bind(public_id)
        .bind(definition_id)
        .bind(format!("Connectivity {}", Uuid::new_v4().simple()))
        .fetch_one(pool)
        .await?;

        Ok((instance_id, public_id))
    }

    #[tokio::test]
    async fn connectivity_profile_get_returns_missing_snapshot() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let (_, public_id) = insert_indexer_instance(test_db.pool()).await?;

        let row = indexer_connectivity_profile_get(test_db.pool(), actor, public_id).await?;
        assert!(!row.profile_exists);
        assert!(row.status.is_none());
        Ok(())
    }

    #[tokio::test]
    async fn connectivity_profile_get_returns_snapshot() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let now = test_db.now();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let (instance_id, public_id) = insert_indexer_instance(test_db.pool()).await?;

        sqlx::query(
            "INSERT INTO indexer_connectivity_profile (
                indexer_instance_id,
                status,
                error_class,
                latency_p50_ms,
                latency_p95_ms,
                success_rate_1h,
                success_rate_24h,
                last_checked_at
            )
            VALUES ($1, 'failing'::connectivity_status, 'cf_challenge'::error_class, 1200, 3500, 0.8500, 0.9100, $2)",
        )
        .bind(instance_id)
        .bind(now)
        .execute(test_db.pool())
        .await?;

        let row = indexer_connectivity_profile_get(test_db.pool(), actor, public_id).await?;
        assert!(row.profile_exists);
        assert_eq!(row.status.as_deref(), Some("failing"));
        assert_eq!(row.error_class.as_deref(), Some("cf_challenge"));
        assert_eq!(row.latency_p50_ms, Some(1200));
        Ok(())
    }

    #[tokio::test]
    async fn source_reputation_list_returns_recent_rows() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let (instance_id, public_id) = insert_indexer_instance(test_db.pool()).await?;

        sqlx::query(
            "INSERT INTO source_reputation (
                indexer_instance_id,
                window_key,
                window_start,
                request_success_rate,
                acquisition_success_rate,
                fake_rate,
                dmca_rate,
                request_count,
                request_success_count,
                acquisition_count,
                acquisition_success_count,
                min_samples,
                computed_at
            )
            VALUES
                ($1, '1h'::reputation_window, now() - interval '1 hour', 0.7500, 0.5000, 0.1000, 0.0500, 40, 30, 10, 5, 10, now() - interval '5 minutes'),
                ($1, '1h'::reputation_window, now() - interval '2 hour', 0.8000, 0.6000, 0.0500, 0.0200, 50, 40, 12, 7, 10, now() - interval '65 minutes')",
        )
        .bind(instance_id)
        .execute(test_db.pool())
        .await?;

        let rows =
            indexer_source_reputation_list(test_db.pool(), actor, public_id, Some("1h"), Some(1))
                .await?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].window_key, "1h");
        assert_eq!(rows[0].request_count, 40);
        Ok(())
    }

    #[tokio::test]
    async fn source_reputation_list_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err =
            indexer_source_reputation_list(test_db.pool(), actor, Uuid::new_v4(), Some("1h"), None)
                .await
                .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }

    #[tokio::test]
    async fn health_event_list_returns_recent_rows() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let now = test_db.now();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let (instance_id, public_id) = insert_indexer_instance(test_db.pool()).await?;

        sqlx::query(
            "INSERT INTO indexer_health_event (
                indexer_instance_id,
                occurred_at,
                event_type,
                latency_ms,
                http_status,
                error_class,
                detail
            )
            VALUES
                ($1, $2, 'identity_conflict'::health_event_type, 1450, 503, 'cf_challenge'::error_class, 'challenge observed'),
                ($1, $2 - INTERVAL '1 hour', 'identity_conflict'::health_event_type, NULL, NULL, NULL, 'older event')",
        )
        .bind(instance_id)
        .bind(now)
        .execute(test_db.pool())
        .await?;

        let rows = indexer_health_event_list(test_db.pool(), actor, public_id, Some(1)).await?;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event_type, "identity_conflict");
        assert_eq!(rows[0].http_status, Some(503));
        assert_eq!(rows[0].error_class.as_deref(), Some("cf_challenge"));
        assert_eq!(rows[0].detail.as_deref(), Some("challenge observed"));
        Ok(())
    }

    #[tokio::test]
    async fn health_event_list_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = indexer_health_event_list(test_db.pool(), actor, Uuid::new_v4(), Some(10))
            .await
            .expect_err("missing indexer should fail");
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }
}
