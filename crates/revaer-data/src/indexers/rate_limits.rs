//! Stored-procedure access for rate limit policies.
//!
//! # Design
//! - Encapsulates rate limit procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const RATE_LIMIT_POLICY_CREATE_CALL: &str = r"
    SELECT rate_limit_policy_create(
        actor_user_public_id => $1,
        display_name_input => $2,
        rpm_input => $3,
        burst_input => $4,
        concurrent_input => $5
    )
";

const RATE_LIMIT_POLICY_UPDATE_CALL: &str = r"
    SELECT rate_limit_policy_update(
        actor_user_public_id => $1,
        rate_limit_policy_public_id_input => $2,
        display_name_input => $3,
        rpm_input => $4,
        burst_input => $5,
        concurrent_input => $6
    )
";

const RATE_LIMIT_POLICY_SOFT_DELETE_CALL: &str = r"
    SELECT rate_limit_policy_soft_delete(
        actor_user_public_id => $1,
        rate_limit_policy_public_id_input => $2
    )
";

const INDEXER_INSTANCE_SET_RATE_LIMIT_CALL: &str = r"
    SELECT indexer_instance_set_rate_limit_policy(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        rate_limit_policy_public_id_input => $3
    )
";

const ROUTING_POLICY_SET_RATE_LIMIT_CALL: &str = r"
    SELECT routing_policy_set_rate_limit_policy(
        actor_user_public_id => $1,
        routing_policy_public_id_input => $2,
        rate_limit_policy_public_id_input => $3
    )
";

const RATE_LIMIT_TRY_CONSUME_CALL: &str = r"
    SELECT allowed, tokens_used
    FROM rate_limit_try_consume(
        scope_type_input => $1::rate_limit_scope,
        scope_id_input => $2,
        capacity_input => $3,
        tokens_input => $4
    )
";

/// Result from a rate limit token bucket consume attempt.
#[derive(Debug, Clone, Copy, FromRow)]
pub struct RateLimitConsumeResult {
    /// Whether the request is allowed.
    pub allowed: bool,
    /// Number of tokens consumed.
    pub tokens_used: i32,
}

/// Create a new rate limit policy.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn rate_limit_policy_create(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    display_name: &str,
    rpm: i32,
    burst: i32,
    concurrent: i32,
) -> Result<Uuid> {
    sqlx::query_scalar(RATE_LIMIT_POLICY_CREATE_CALL)
        .bind(actor_user_public_id)
        .bind(display_name)
        .bind(rpm)
        .bind(burst)
        .bind(concurrent)
        .fetch_one(pool)
        .await
        .map_err(try_op("rate limit policy create"))
}

/// Update an existing rate limit policy.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn rate_limit_policy_update(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
    display_name: Option<&str>,
    rpm: Option<i32>,
    burst: Option<i32>,
    concurrent: Option<i32>,
) -> Result<()> {
    sqlx::query(RATE_LIMIT_POLICY_UPDATE_CALL)
        .bind(actor_user_public_id)
        .bind(rate_limit_policy_public_id)
        .bind(display_name)
        .bind(rpm)
        .bind(burst)
        .bind(concurrent)
        .execute(pool)
        .await
        .map_err(try_op("rate limit policy update"))?;
    Ok(())
}

/// Soft delete a rate limit policy.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn rate_limit_policy_soft_delete(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    rate_limit_policy_public_id: Uuid,
) -> Result<()> {
    sqlx::query(RATE_LIMIT_POLICY_SOFT_DELETE_CALL)
        .bind(actor_user_public_id)
        .bind(rate_limit_policy_public_id)
        .execute(pool)
        .await
        .map_err(try_op("rate limit policy soft delete"))?;
    Ok(())
}

/// Assign a rate limit policy to an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_instance_set_rate_limit_policy(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    rate_limit_policy_public_id: Option<Uuid>,
) -> Result<()> {
    sqlx::query(INDEXER_INSTANCE_SET_RATE_LIMIT_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .bind(rate_limit_policy_public_id)
        .execute(pool)
        .await
        .map_err(try_op("indexer instance set rate limit policy"))?;
    Ok(())
}

/// Assign a rate limit policy to a routing policy.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn routing_policy_set_rate_limit_policy(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    routing_policy_public_id: Uuid,
    rate_limit_policy_public_id: Option<Uuid>,
) -> Result<()> {
    sqlx::query(ROUTING_POLICY_SET_RATE_LIMIT_CALL)
        .bind(actor_user_public_id)
        .bind(routing_policy_public_id)
        .bind(rate_limit_policy_public_id)
        .execute(pool)
        .await
        .map_err(try_op("routing policy set rate limit policy"))?;
    Ok(())
}

/// Attempt to consume tokens from a rate limit bucket.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn rate_limit_try_consume(
    pool: &PgPool,
    scope_type: &str,
    scope_id: i64,
    capacity: i32,
    tokens: i32,
) -> Result<RateLimitConsumeResult> {
    sqlx::query_as(RATE_LIMIT_TRY_CONSUME_CALL)
        .bind(scope_type)
        .bind(scope_id)
        .bind(capacity)
        .bind(tokens)
        .fetch_one(pool)
        .await
        .map_err(try_op("rate limit try consume"))
}

#[cfg(test)]
#[path = "rate_limits/tests.rs"]
mod tests;
