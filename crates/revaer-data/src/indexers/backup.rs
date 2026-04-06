//! Stored-procedure access for indexer backup export reads.
//!
//! # Design
//! - Exposes typed wrappers around normalized backup export procedures.
//! - Keeps runtime SQL confined to stored-procedure invocations with named binds.
//! - Returns flattened rows that higher layers can assemble into snapshot documents.

use crate::error::{Result, try_op};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const TAG_LIST_CALL: &str = r"
    SELECT
        tag_public_id,
        tag_key,
        display_name
    FROM indexer_backup_export_tag_list(
        actor_user_public_id => $1
    )
";

const RATE_LIMIT_LIST_CALL: &str = r"
    SELECT
        rate_limit_policy_public_id,
        display_name,
        requests_per_minute,
        burst,
        concurrent_requests,
        is_system
    FROM indexer_backup_export_rate_limit_policy_list(
        actor_user_public_id => $1
    )
";

const ROUTING_LIST_CALL: &str = r"
    SELECT
        routing_policy_public_id,
        display_name,
        mode::text,
        rate_limit_policy_public_id,
        rate_limit_display_name,
        param_key::text,
        value_plain,
        value_int,
        value_bool,
        secret_public_id,
        secret_type::text
    FROM indexer_backup_export_routing_policy_list(
        actor_user_public_id => $1
    )
";

const INSTANCE_LIST_CALL: &str = r"
    SELECT
        indexer_instance_public_id,
        upstream_slug,
        display_name,
        CASE WHEN is_enabled THEN 'enabled' ELSE 'disabled' END AS instance_status,
        CASE WHEN enable_rss THEN 'enabled' ELSE 'disabled' END AS rss_status,
        CASE
            WHEN enable_automatic_search THEN 'enabled'
            ELSE 'disabled'
        END AS automatic_search_status,
        CASE
            WHEN enable_interactive_search THEN 'enabled'
            ELSE 'disabled'
        END AS interactive_search_status,
        priority,
        trust_tier_key::text,
        routing_policy_public_id,
        routing_policy_display_name,
        connect_timeout_ms,
        read_timeout_ms,
        max_parallel_requests,
        rate_limit_policy_public_id,
        rate_limit_display_name,
        rss_subscription_enabled,
        rss_interval_seconds,
        media_domain_key::text,
        tag_key,
        field_name,
        field_type::text,
        value_plain,
        value_int,
        value_decimal,
        value_bool,
        secret_public_id,
        secret_type::text
    FROM indexer_backup_export_indexer_instance_list(
        actor_user_public_id => $1
    )
";

/// Exportable tag row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct BackupTagRow {
    /// Tag public identifier.
    pub tag_public_id: Uuid,
    /// Stable tag key.
    pub tag_key: String,
    /// Operator-facing display name.
    pub display_name: String,
}

/// Exportable rate-limit policy row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct BackupRateLimitPolicyRow {
    /// Rate-limit policy public identifier.
    pub rate_limit_policy_public_id: Uuid,
    /// Operator-facing display name.
    pub display_name: String,
    /// Requests-per-minute limit.
    pub requests_per_minute: i32,
    /// Burst capacity.
    pub burst: i32,
    /// Concurrent request cap.
    pub concurrent_requests: i32,
    /// Whether this is a system-seeded policy.
    pub is_system: bool,
}

/// Flattened routing-policy export row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct BackupRoutingPolicyRow {
    /// Routing policy public identifier.
    pub routing_policy_public_id: Uuid,
    /// Operator-facing display name.
    pub display_name: String,
    /// Routing mode.
    pub mode: String,
    /// Optional assigned rate-limit policy public identifier.
    pub rate_limit_policy_public_id: Option<Uuid>,
    /// Optional assigned rate-limit policy display name.
    pub rate_limit_display_name: Option<String>,
    /// Optional parameter key.
    pub param_key: Option<String>,
    /// Optional plain parameter value.
    pub value_plain: Option<String>,
    /// Optional integer parameter value.
    pub value_int: Option<i32>,
    /// Optional boolean parameter value.
    pub value_bool: Option<bool>,
    /// Optional bound secret public identifier.
    pub secret_public_id: Option<Uuid>,
    /// Optional bound secret type.
    pub secret_type: Option<String>,
}

/// Flattened indexer-instance export row.
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct BackupIndexerInstanceRow {
    /// Indexer instance public identifier.
    pub indexer_instance_public_id: Uuid,
    /// Upstream definition slug.
    pub upstream_slug: String,
    /// Operator-facing display name.
    pub display_name: String,
    /// Whether the instance is enabled.
    pub instance_status: String,
    /// Whether RSS is enabled on the instance.
    pub rss_status: String,
    /// Whether automatic search is enabled.
    pub automatic_search_status: String,
    /// Whether interactive search is enabled.
    pub interactive_search_status: String,
    /// Priority override.
    pub priority: i32,
    /// Optional trust tier key.
    pub trust_tier_key: Option<String>,
    /// Optional routing policy public identifier.
    pub routing_policy_public_id: Option<Uuid>,
    /// Optional routing policy display name.
    pub routing_policy_display_name: Option<String>,
    /// Connect timeout in milliseconds.
    pub connect_timeout_ms: i32,
    /// Read timeout in milliseconds.
    pub read_timeout_ms: i32,
    /// Maximum parallel requests.
    pub max_parallel_requests: i32,
    /// Optional direct rate-limit policy public identifier.
    pub rate_limit_policy_public_id: Option<Uuid>,
    /// Optional direct rate-limit policy display name.
    pub rate_limit_display_name: Option<String>,
    /// Optional RSS subscription enabled value.
    pub rss_subscription_enabled: Option<bool>,
    /// Optional RSS subscription interval.
    pub rss_interval_seconds: Option<i32>,
    /// Optional media-domain key from a joined row.
    pub media_domain_key: Option<String>,
    /// Optional tag key from a joined row.
    pub tag_key: Option<String>,
    /// Optional field name from a joined row.
    pub field_name: Option<String>,
    /// Optional field type from a joined row.
    pub field_type: Option<String>,
    /// Optional plain field value.
    pub value_plain: Option<String>,
    /// Optional integer field value.
    pub value_int: Option<i32>,
    /// Optional decimal field value rendered as text.
    pub value_decimal: Option<String>,
    /// Optional boolean field value.
    pub value_bool: Option<bool>,
    /// Optional bound secret public identifier.
    pub secret_public_id: Option<Uuid>,
    /// Optional bound secret type.
    pub secret_type: Option<String>,
}

/// List exportable tags.
///
/// # Errors
///
/// Returns an error if the export procedure rejects the actor or query.
pub async fn indexer_backup_export_tag_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
) -> Result<Vec<BackupTagRow>> {
    sqlx::query_as(TAG_LIST_CALL)
        .bind(actor_user_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("indexer backup export tag list"))
}

/// List exportable rate-limit policies.
///
/// # Errors
///
/// Returns an error if the export procedure rejects the actor or query.
pub async fn indexer_backup_export_rate_limit_policy_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
) -> Result<Vec<BackupRateLimitPolicyRow>> {
    sqlx::query_as(RATE_LIMIT_LIST_CALL)
        .bind(actor_user_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("indexer backup export rate limit list"))
}

/// List exportable routing policies in flattened form.
///
/// # Errors
///
/// Returns an error if the export procedure rejects the actor or query.
pub async fn indexer_backup_export_routing_policy_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
) -> Result<Vec<BackupRoutingPolicyRow>> {
    sqlx::query_as(ROUTING_LIST_CALL)
        .bind(actor_user_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("indexer backup export routing list"))
}

/// List exportable indexer instances in flattened form.
///
/// # Errors
///
/// Returns an error if the export procedure rejects the actor or query.
pub async fn indexer_backup_export_indexer_instance_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
) -> Result<Vec<BackupIndexerInstanceRow>> {
    sqlx::query_as(INSTANCE_LIST_CALL)
        .bind(actor_user_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("indexer backup export instance list"))
}

#[cfg(test)]
#[path = "backup/tests.rs"]
mod tests;
