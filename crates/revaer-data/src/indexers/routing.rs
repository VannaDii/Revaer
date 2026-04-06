//! Stored-procedure access for routing policy management.
//!
//! # Design
//! - Exposes typed wrappers around routing policy stored procedures.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Avoids extra dependencies by using string labels for enums.

use crate::error::{Result, try_op};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const ROUTING_POLICY_CREATE_CALL: &str = r"
    SELECT routing_policy_create(
        actor_user_public_id => $1,
        display_name_input => $2,
        mode_input => $3::routing_policy_mode
    )
";

const ROUTING_POLICY_SET_PARAM_CALL: &str = r"
    SELECT routing_policy_set_param(
        actor_user_public_id => $1,
        routing_policy_public_id_input => $2,
        param_key_input => $3::routing_param_key,
        value_plain_input => $4,
        value_int_input => $5,
        value_bool_input => $6
    )
";

const ROUTING_POLICY_BIND_SECRET_CALL: &str = r"
    SELECT routing_policy_bind_secret(
        actor_user_public_id => $1,
        routing_policy_public_id_input => $2,
        param_key_input => $3::routing_param_key,
        secret_public_id_input => $4
    )
";

const ROUTING_POLICY_GET_CALL: &str = r"
    SELECT
        routing_policy_public_id,
        display_name,
        mode::text,
        rate_limit_policy_public_id,
        rate_limit_display_name,
        rate_limit_requests_per_minute,
        rate_limit_burst,
        rate_limit_concurrent_requests,
        param_key::text,
        value_plain,
        value_int,
        value_bool,
        secret_public_id,
        secret_binding_name::text
    FROM routing_policy_get(
        actor_user_public_id => $1,
        routing_policy_public_id_input => $2
    )
";

/// One routing policy row plus an optional parameter projection.
#[derive(Debug, Clone, FromRow)]
pub struct RoutingPolicyDetailRow {
    /// Routing policy public identifier.
    pub routing_policy_public_id: Uuid,
    /// Operator-facing routing policy label.
    pub display_name: String,
    /// Routing mode (`direct`, `http_proxy`, `socks_proxy`, `flaresolverr`, etc).
    pub mode: String,
    /// Assigned rate-limit policy public identifier, when present.
    pub rate_limit_policy_public_id: Option<Uuid>,
    /// Assigned rate-limit policy display name, when present.
    pub rate_limit_display_name: Option<String>,
    /// Assigned requests-per-minute value, when present.
    pub rate_limit_requests_per_minute: Option<i32>,
    /// Assigned burst value, when present.
    pub rate_limit_burst: Option<i32>,
    /// Assigned concurrent-requests value, when present.
    pub rate_limit_concurrent_requests: Option<i32>,
    /// Parameter key for this row, when a parameter exists.
    pub param_key: Option<String>,
    /// Plain-text parameter value.
    pub value_plain: Option<String>,
    /// Integer parameter value.
    pub value_int: Option<i32>,
    /// Boolean parameter value.
    pub value_bool: Option<bool>,
    /// Secret bound to this parameter, when present.
    pub secret_public_id: Option<Uuid>,
    /// Binding name used for the secret, when present.
    pub secret_binding_name: Option<String>,
}

/// Create a routing policy.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn routing_policy_create(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    display_name: &str,
    mode: &str,
) -> Result<Uuid> {
    sqlx::query_scalar(ROUTING_POLICY_CREATE_CALL)
        .bind(actor_user_public_id)
        .bind(display_name)
        .bind(mode)
        .fetch_one(pool)
        .await
        .map_err(try_op("routing policy create"))
}

/// Set a routing policy parameter.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn routing_policy_set_param(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    routing_policy_public_id: Uuid,
    param_key: &str,
    value_plain: Option<&str>,
    value_int: Option<i32>,
    value_bool: Option<bool>,
) -> Result<()> {
    sqlx::query(ROUTING_POLICY_SET_PARAM_CALL)
        .bind(actor_user_public_id)
        .bind(routing_policy_public_id)
        .bind(param_key)
        .bind(value_plain)
        .bind(value_int)
        .bind(value_bool)
        .execute(pool)
        .await
        .map_err(try_op("routing policy set param"))?;
    Ok(())
}

/// Bind a secret to a routing policy parameter.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn routing_policy_bind_secret(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    routing_policy_public_id: Uuid,
    param_key: &str,
    secret_public_id: Uuid,
) -> Result<()> {
    sqlx::query(ROUTING_POLICY_BIND_SECRET_CALL)
        .bind(actor_user_public_id)
        .bind(routing_policy_public_id)
        .bind(param_key)
        .bind(secret_public_id)
        .execute(pool)
        .await
        .map_err(try_op("routing policy bind secret"))?;
    Ok(())
}

/// Fetch routing policy detail with operator-visible parameters and bindings.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn routing_policy_get(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    routing_policy_public_id: Uuid,
) -> Result<Vec<RoutingPolicyDetailRow>> {
    sqlx::query_as(ROUTING_POLICY_GET_CALL)
        .bind(actor_user_public_id)
        .bind(routing_policy_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("routing policy get"))
}

#[cfg(test)]
#[path = "routing/tests.rs"]
mod tests;
