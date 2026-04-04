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
mod tests {
    use super::*;
    use crate::DataError;
    use crate::indexers::secrets::secret_create;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }
    #[tokio::test]
    async fn routing_policy_create_rejects_duplicate_name() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let first = routing_policy_create(pool, actor, "Routing", "direct").await?;
        assert_ne!(first, Uuid::nil());

        let err = routing_policy_create(pool, actor, "Routing", "direct")
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("display_name_already_exists"));
        Ok(())
    }
    #[tokio::test]
    async fn routing_policy_set_param_requires_policy() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = routing_policy_set_param(
            pool,
            actor,
            Uuid::new_v4(),
            "proxy_host",
            Some("localhost"),
            None,
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("routing_policy_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn routing_policy_bind_secret_requires_policy() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = routing_policy_bind_secret(
            pool,
            actor,
            Uuid::new_v4(),
            "http_proxy_auth",
            Uuid::new_v4(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("routing_policy_not_found"));
        Ok(())
    }

    #[tokio::test]
    async fn routing_policy_bind_secret_persists_binding() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let routing_policy_public_id =
            routing_policy_create(pool, actor, "Proxy Auth", "http_proxy").await?;
        let secret_public_id = secret_create(pool, actor, "password", "proxy-pass").await?;

        routing_policy_bind_secret(
            pool,
            actor,
            routing_policy_public_id,
            "http_proxy_auth",
            secret_public_id,
        )
        .await?;

        let binding: (String, Uuid) = sqlx::query_as(
            "SELECT sb.binding_name::text, s.secret_public_id
             FROM routing_policy rp
             JOIN routing_policy_parameter rpp
               ON rpp.routing_policy_id = rp.routing_policy_id
             JOIN secret_binding sb
               ON sb.bound_table = 'routing_policy_parameter'
              AND sb.bound_id = rpp.routing_policy_parameter_id
             JOIN secret s
               ON s.secret_id = sb.secret_id
             WHERE rp.routing_policy_public_id = $1
               AND rpp.param_key = 'http_proxy_auth'",
        )
        .bind(routing_policy_public_id)
        .fetch_one(pool)
        .await?;
        assert_eq!(binding.0, "proxy_password");
        assert_eq!(binding.1, secret_public_id);
        Ok(())
    }

    #[tokio::test]
    async fn routing_policy_get_returns_params_rate_limit_and_secret() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let routing_policy_public_id =
            routing_policy_create(pool, actor, "Proxy Detail", "http_proxy").await?;
        routing_policy_set_param(
            pool,
            actor,
            routing_policy_public_id,
            "proxy_host",
            Some("proxy.internal"),
            None,
            None,
        )
        .await?;
        routing_policy_set_param(
            pool,
            actor,
            routing_policy_public_id,
            "proxy_port",
            None,
            Some(8443),
            None,
        )
        .await?;
        let secret_public_id = secret_create(pool, actor, "password", "proxy-pass").await?;
        routing_policy_bind_secret(
            pool,
            actor,
            routing_policy_public_id,
            "http_proxy_auth",
            secret_public_id,
        )
        .await?;

        let rate_limit_policy_public_id: Uuid = sqlx::query_scalar(
            "INSERT INTO rate_limit_policy (
                rate_limit_policy_public_id,
                display_name,
                requests_per_minute,
                burst,
                concurrent_requests
            )
            VALUES ($1, $2, $3, $4, $5)
            RETURNING rate_limit_policy_public_id",
        )
        .bind(Uuid::new_v4())
        .bind("Proxy Rate Limit")
        .bind(90_i32)
        .bind(15_i32)
        .bind(3_i32)
        .fetch_one(pool)
        .await?;

        sqlx::query(
            "INSERT INTO routing_policy_rate_limit (routing_policy_id, rate_limit_policy_id)
             SELECT rp.routing_policy_id, rlp.rate_limit_policy_id
             FROM routing_policy rp
             CROSS JOIN rate_limit_policy rlp
             WHERE rp.routing_policy_public_id = $1
               AND rlp.rate_limit_policy_public_id = $2",
        )
        .bind(routing_policy_public_id)
        .bind(rate_limit_policy_public_id)
        .execute(pool)
        .await?;

        let rows = routing_policy_get(pool, actor, routing_policy_public_id).await?;
        assert!(!rows.is_empty());
        assert!(
            rows.iter()
                .any(|row| row.param_key.as_deref() == Some("proxy_host")
                    && row.value_plain.as_deref() == Some("proxy.internal"))
        );
        assert!(rows.iter().any(
            |row| row.param_key.as_deref() == Some("proxy_port") && row.value_int == Some(8443)
        ));
        assert!(
            rows.iter()
                .any(|row| row.param_key.as_deref() == Some("http_proxy_auth")
                    && row.secret_public_id == Some(secret_public_id)
                    && row.secret_binding_name.as_deref() == Some("proxy_password"))
        );
        let first = rows.first().expect("routing rows");
        assert_eq!(
            first.rate_limit_policy_public_id,
            Some(rate_limit_policy_public_id)
        );
        assert_eq!(
            first.rate_limit_display_name.as_deref(),
            Some("Proxy Rate Limit")
        );
        assert_eq!(first.rate_limit_requests_per_minute, Some(90));
        Ok(())
    }
}
