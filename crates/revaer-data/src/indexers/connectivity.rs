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
#[path = "connectivity/tests.rs"]
mod tests;
