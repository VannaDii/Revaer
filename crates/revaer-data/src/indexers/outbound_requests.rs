//! Stored-procedure access for outbound request logging.
//!
//! # Design
//! - Encapsulates outbound request logging behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

const OUTBOUND_REQUEST_LOG_WRITE_CALL: &str = r"
    SELECT outbound_request_log_write(
        indexer_instance_public_id_input => $1,
        routing_policy_public_id_input => $2,
        search_request_public_id_input => $3,
        request_type_input => $4::outbound_request_type,
        correlation_id_input => $5,
        retry_seq_input => $6,
        started_at_input => $7,
        finished_at_input => $8,
        outcome_input => $9::outbound_request_outcome,
        via_mitigation_input => $10::outbound_via_mitigation,
        rate_limit_denied_scope_input => $11::rate_limit_scope,
        error_class_input => $12::error_class,
        http_status_input => $13,
        latency_ms_input => $14,
        parse_ok_input => $15,
        result_count_input => $16,
        cf_detected_input => $17,
        page_number_input => $18,
        page_cursor_key_input => $19
    )
";

/// Input payload for logging an outbound request.
#[derive(Debug, Clone)]
pub struct OutboundRequestLogInput<'a> {
    /// Indexer instance that issued the request.
    pub indexer_instance_public_id: Uuid,
    /// Optional routing policy used for the request.
    pub routing_policy_public_id: Option<Uuid>,
    /// Optional search request associated with the request.
    pub search_request_public_id: Option<Uuid>,
    /// Request type enum key.
    pub request_type: &'a str,
    /// Correlation id for the outbound request.
    pub correlation_id: Uuid,
    /// Retry sequence number.
    pub retry_seq: i16,
    /// Timestamp when the request started.
    pub started_at: DateTime<Utc>,
    /// Timestamp when the request finished.
    pub finished_at: DateTime<Utc>,
    /// Outcome enum key.
    pub outcome: &'a str,
    /// Mitigation enum key.
    pub via_mitigation: &'a str,
    /// Optional rate limit scope that denied the request.
    pub rate_limit_denied_scope: Option<&'a str>,
    /// Optional error class enum key.
    pub error_class: Option<&'a str>,
    /// Optional HTTP status code.
    pub http_status: Option<i32>,
    /// Optional request latency in milliseconds.
    pub latency_ms: Option<i32>,
    /// Optional parse success flag.
    pub parse_ok: Option<bool>,
    /// Optional result count.
    pub result_count: Option<i32>,
    /// Optional Cloudflare detection flag.
    pub cf_detected: Option<bool>,
    /// Optional page number for the request.
    pub page_number: Option<i32>,
    /// Optional cursor key for paging requests.
    pub page_cursor_key: Option<&'a str>,
}

/// Write an outbound request log entry.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn outbound_request_log_write(
    pool: &PgPool,
    input: &OutboundRequestLogInput<'_>,
) -> Result<()> {
    sqlx::query(OUTBOUND_REQUEST_LOG_WRITE_CALL)
        .bind(input.indexer_instance_public_id)
        .bind(input.routing_policy_public_id)
        .bind(input.search_request_public_id)
        .bind(input.request_type)
        .bind(input.correlation_id)
        .bind(input.retry_seq)
        .bind(input.started_at)
        .bind(input.finished_at)
        .bind(input.outcome)
        .bind(input.via_mitigation)
        .bind(input.rate_limit_denied_scope)
        .bind(input.error_class)
        .bind(input.http_status)
        .bind(input.latency_ms)
        .bind(input.parse_ok)
        .bind(input.result_count)
        .bind(input.cf_detected)
        .bind(input.page_number)
        .bind(input.page_cursor_key)
        .execute(pool)
        .await
        .map_err(try_op("outbound request log write"))?;
    Ok(())
}

#[cfg(test)]
#[path = "../../tests/unit/indexers/outbound_requests_tests.rs"]
mod tests;
