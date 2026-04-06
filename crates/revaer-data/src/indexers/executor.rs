//! Stored-procedure access for executor handoff operations (RSS polling, test probes, secrets).
//!
//! # Design
//! - Exposes typed wrappers around stored procedures so callers never embed SQL.
//! - Keeps enum-like DB values as strings to avoid extra dependencies.
//! - Error handling maps SQL failures to `DataError::QueryFailed` with constant messages.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const RSS_POLL_CLAIM_CALL: &str = r"
    SELECT *
    FROM rss_poll_claim(
        limit_input => $1
    )
";

const RSS_POLL_APPLY_CALL: &str = r"
    SELECT *
    FROM rss_poll_apply(
        rss_subscription_id_input => $1,
        correlation_id_input => $2,
        retry_seq_input => $3,
        started_at_input => $4,
        finished_at_input => $5,
        outcome_input => $6::outbound_request_outcome,
        error_class_input => $7::error_class,
        http_status_input => $8,
        latency_ms_input => $9,
        parse_ok_input => $10,
        result_count_input => $11,
        via_mitigation_input => $12::outbound_via_mitigation,
        rate_limit_denied_scope_input => $13::rate_limit_scope,
        cf_detected_input => $14,
        cf_retryable_input => $15,
        item_guid_input => $16::varchar[],
        infohash_v1_input => $17::char(40)[],
        infohash_v2_input => $18::char(64)[],
        magnet_hash_input => $19::char(64)[]
    )
";

const INDEXER_TEST_PREPARE_CALL: &str = r"
    SELECT
        can_execute,
        error_class::text AS error_class,
        error_code,
        detail,
        engine::text AS engine,
        routing_policy_public_id,
        connect_timeout_ms,
        read_timeout_ms,
        field_names,
        field_types::text[] AS field_types,
        value_plain,
        value_int,
        value_decimal::text[] AS value_decimal,
        value_bool,
        secret_public_ids
    FROM indexer_instance_test_prepare(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2
    )
";

const INDEXER_TEST_FINALIZE_CALL: &str = r"
    SELECT
        ok,
        error_class::text AS error_class,
        error_code,
        detail,
        result_count
    FROM indexer_instance_test_finalize(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        ok_input => $3,
        error_class_input => $4::error_class,
        error_code_input => $5,
        detail_input => $6,
        result_count_input => $7
    )
";

const SECRET_READ_CALL: &str = r"
    SELECT
        secret_type::text AS secret_type,
        cipher_text,
        key_id
    FROM secret_read(
        actor_user_public_id => $1,
        secret_public_id_input => $2
    )
";

/// RSS subscription claim result.
#[derive(Debug, Clone, FromRow)]
pub struct RssPollClaimRow {
    /// RSS subscription internal id.
    pub rss_subscription_id: i64,
    /// Public id of the indexer instance.
    pub indexer_instance_public_id: Uuid,
    /// Public id of the routing policy, if configured.
    pub routing_policy_public_id: Option<Uuid>,
    /// Subscription polling interval in seconds.
    pub interval_seconds: i32,
    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: i32,
    /// Read timeout in milliseconds.
    pub read_timeout_ms: i32,
    /// Correlation id for outbound request logging.
    pub correlation_id: Uuid,
    /// Retry sequence for the poll attempt.
    pub retry_seq: i16,
}

/// RSS poll application input payload.
#[derive(Debug, Clone)]
pub struct RssPollApplyInput<'a> {
    /// RSS subscription internal id.
    pub rss_subscription_id: i64,
    /// Correlation id for outbound request logging.
    pub correlation_id: Uuid,
    /// Retry sequence for the poll attempt.
    pub retry_seq: i16,
    /// Start timestamp for the HTTP request.
    pub started_at: DateTime<Utc>,
    /// Finish timestamp for the HTTP request.
    pub finished_at: DateTime<Utc>,
    /// Outcome label (`success` or `failure`).
    pub outcome: &'a str,
    /// Error class label when outcome is `failure`.
    pub error_class: Option<&'a str>,
    /// HTTP status code observed by the executor.
    pub http_status: Option<i32>,
    /// Request latency in milliseconds.
    pub latency_ms: Option<i32>,
    /// Whether the RSS response parsed successfully.
    pub parse_ok: bool,
    /// Total number of RSS items parsed.
    pub result_count: Option<i32>,
    /// Mitigation path used (`none`, `proxy`, `flaresolverr`).
    pub via_mitigation: &'a str,
    /// Rate-limit scope when denied.
    pub rate_limit_denied_scope: Option<&'a str>,
    /// Whether Cloudflare indicators were detected.
    pub cf_detected: bool,
    /// Whether Cloudflare failures are retryable (flaresolverr available).
    pub cf_retryable: bool,
    /// Item GUIDs (aligned by index).
    pub item_guid: Option<&'a [String]>,
    /// Infohash v1 values (aligned by index).
    pub infohash_v1: Option<&'a [String]>,
    /// Infohash v2 values (aligned by index).
    pub infohash_v2: Option<&'a [String]>,
    /// Magnet hashes (aligned by index).
    pub magnet_hash: Option<&'a [String]>,
}

/// RSS poll application result.
#[derive(Debug, Clone, FromRow)]
pub struct RssPollApplyResult {
    /// Parsed RSS item count from the executor.
    pub items_parsed: i32,
    /// Items with at least one valid identifier.
    pub items_eligible: i32,
    /// New rows inserted into the dedupe table.
    pub items_inserted: i32,
    /// Whether the subscription was treated as succeeded.
    pub subscription_succeeded: bool,
}

/// Indexer test prepare output.
#[derive(Debug, Clone, FromRow)]
pub struct IndexerTestPrepareRow {
    /// Whether the executor can proceed.
    pub can_execute: bool,
    /// Error class label when preparation fails.
    pub error_class: Option<String>,
    /// Error code when preparation fails.
    pub error_code: Option<String>,
    /// Detail string for UI display.
    pub detail: Option<String>,
    /// Indexer engine label.
    pub engine: String,
    /// Routing policy public id, if configured.
    pub routing_policy_public_id: Option<Uuid>,
    /// Connection timeout in milliseconds.
    pub connect_timeout_ms: i32,
    /// Read timeout in milliseconds.
    pub read_timeout_ms: i32,
    /// Field names aligned with config arrays.
    pub field_names: Option<Vec<String>>,
    /// Field types aligned with config arrays.
    pub field_types: Option<Vec<String>>,
    /// Plain string values aligned with config arrays.
    pub value_plain: Option<Vec<Option<String>>>,
    /// Integer values aligned with config arrays.
    pub value_int: Option<Vec<Option<i32>>>,
    /// Decimal values aligned with config arrays, rendered as strings.
    pub value_decimal: Option<Vec<Option<String>>>,
    /// Boolean values aligned with config arrays.
    pub value_bool: Option<Vec<Option<bool>>>,
    /// Secret public ids aligned with config arrays.
    pub secret_public_ids: Option<Vec<Option<Uuid>>>,
}

/// Indexer test finalize output.
#[derive(Debug, Clone, FromRow)]
pub struct IndexerTestFinalizeRow {
    /// Whether the test succeeded.
    pub ok: bool,
    /// Error class label when the test failed.
    pub error_class: Option<String>,
    /// Error code when the test failed.
    pub error_code: Option<String>,
    /// Detail string for UI display.
    pub detail: Option<String>,
    /// Parsed result count from the executor.
    pub result_count: Option<i32>,
}

/// Secret read output (encrypted data).
#[derive(Debug, Clone, FromRow)]
pub struct SecretCipherRow {
    /// Secret type label.
    pub secret_type: String,
    /// Cipher text payload.
    pub cipher_text: Vec<u8>,
    /// Key identifier used for encryption.
    pub key_id: String,
}

/// Claim due RSS subscriptions for polling.
///
/// # Errors
///
/// Returns an error if the database call fails.
pub async fn rss_poll_claim(pool: &PgPool, limit: i32) -> Result<Vec<RssPollClaimRow>> {
    sqlx::query_as(RSS_POLL_CLAIM_CALL)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(try_op("rss poll claim"))
}

/// Apply the outcome of an RSS poll attempt.
///
/// # Errors
///
/// Returns an error if the database call fails.
pub async fn rss_poll_apply(
    pool: &PgPool,
    input: &RssPollApplyInput<'_>,
) -> Result<RssPollApplyResult> {
    sqlx::query_as(RSS_POLL_APPLY_CALL)
        .bind(input.rss_subscription_id)
        .bind(input.correlation_id)
        .bind(input.retry_seq)
        .bind(input.started_at)
        .bind(input.finished_at)
        .bind(input.outcome)
        .bind(input.error_class)
        .bind(input.http_status)
        .bind(input.latency_ms)
        .bind(input.parse_ok)
        .bind(input.result_count)
        .bind(input.via_mitigation)
        .bind(input.rate_limit_denied_scope)
        .bind(input.cf_detected)
        .bind(input.cf_retryable)
        .bind(input.item_guid)
        .bind(input.infohash_v1)
        .bind(input.infohash_v2)
        .bind(input.magnet_hash)
        .fetch_one(pool)
        .await
        .map_err(try_op("rss poll apply"))
}

/// Prepare an indexer instance for executor testing.
///
/// # Errors
///
/// Returns an error if the database call fails.
pub async fn indexer_instance_test_prepare(
    pool: &PgPool,
    actor_user_public_id: Option<Uuid>,
    indexer_instance_public_id: Uuid,
) -> Result<IndexerTestPrepareRow> {
    sqlx::query_as(INDEXER_TEST_PREPARE_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("indexer test prepare"))
}

/// Input payload for finalizing an indexer test run.
#[derive(Debug, Clone, Copy)]
pub struct IndexerTestFinalizeInput<'a> {
    /// Actor user public id for audit.
    pub actor_user_public_id: Option<Uuid>,
    /// Indexer instance public id.
    pub indexer_instance_public_id: Uuid,
    /// Whether the test succeeded.
    pub ok: bool,
    /// Optional error classification.
    pub error_class: Option<&'a str>,
    /// Optional error code string.
    pub error_code: Option<&'a str>,
    /// Optional detail string.
    pub detail: Option<&'a str>,
    /// Optional result count for diagnostics.
    pub result_count: Option<i32>,
}

/// Finalize an indexer test run.
///
/// # Errors
///
/// Returns an error if the database call fails.
pub async fn indexer_instance_test_finalize(
    pool: &PgPool,
    input: &IndexerTestFinalizeInput<'_>,
) -> Result<IndexerTestFinalizeRow> {
    sqlx::query_as(INDEXER_TEST_FINALIZE_CALL)
        .bind(input.actor_user_public_id)
        .bind(input.indexer_instance_public_id)
        .bind(input.ok)
        .bind(input.error_class)
        .bind(input.error_code)
        .bind(input.detail)
        .bind(input.result_count)
        .fetch_one(pool)
        .await
        .map_err(try_op("indexer test finalize"))
}

/// Read encrypted secret payloads for executor use.
///
/// # Errors
///
/// Returns an error if the database call fails.
pub async fn secret_read(
    pool: &PgPool,
    actor_user_public_id: Option<Uuid>,
    secret_public_id: Uuid,
) -> Result<SecretCipherRow> {
    sqlx::query_as(SECRET_READ_CALL)
        .bind(actor_user_public_id)
        .bind(secret_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("secret read"))
}

#[cfg(test)]
#[path = "executor/tests.rs"]
mod tests;
