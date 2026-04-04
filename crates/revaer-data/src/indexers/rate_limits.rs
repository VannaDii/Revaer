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
mod tests {
    use super::*;
    use crate::DataError;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }
    #[tokio::test]
    async fn rate_limit_policy_roundtrip_and_assignments() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let policy_id = rate_limit_policy_create(pool, actor, "Test", 60, 10, 2).await?;

        rate_limit_policy_update(
            pool,
            actor,
            policy_id,
            Some("Updated"),
            Some(120),
            Some(20),
            Some(4),
        )
        .await?;

        let consume = rate_limit_try_consume(pool, "indexer_instance", 42, 5, 1).await?;
        assert!(consume.allowed);
        assert_eq!(consume.tokens_used, 1);

        let err =
            indexer_instance_set_rate_limit_policy(pool, actor, Uuid::new_v4(), Some(policy_id))
                .await
                .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));

        let err =
            routing_policy_set_rate_limit_policy(pool, actor, Uuid::new_v4(), Some(policy_id))
                .await
                .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("routing_policy_not_found"));

        rate_limit_policy_soft_delete(pool, actor, policy_id).await?;
        Ok(())
    }

    #[tokio::test]
    async fn rate_limit_try_consume_enforces_bucket_capacity() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();

        let first = rate_limit_try_consume(pool, "indexer_instance", 404, 5, 3).await?;
        assert!(first.allowed);
        assert_eq!(first.tokens_used, 3);

        let second = rate_limit_try_consume(pool, "indexer_instance", 404, 5, 3).await?;
        assert!(!second.allowed);
        assert_eq!(second.tokens_used, 3);
        Ok(())
    }

    #[tokio::test]
    async fn rate_limit_try_consume_enforces_bucket_capacity_for_routing_scope()
    -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();

        let first = rate_limit_try_consume(pool, "routing_policy", 505, 4, 2).await?;
        assert!(first.allowed);
        assert_eq!(first.tokens_used, 2);

        let second = rate_limit_try_consume(pool, "routing_policy", 505, 4, 2).await?;
        assert!(second.allowed);
        assert_eq!(second.tokens_used, 4);

        let third = rate_limit_try_consume(pool, "routing_policy", 505, 4, 1).await?;
        assert!(!third.allowed);
        assert_eq!(third.tokens_used, 4);
        Ok(())
    }

    #[tokio::test]
    async fn rate_limit_seed_defaults_match_expected_system_policies() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();

        let default_indexer: (i32, i32, i32, bool) = sqlx::query_as(
            "SELECT
                requests_per_minute,
                burst,
                concurrent_requests,
                is_system
             FROM rate_limit_policy
             WHERE display_name = 'default_indexer'
               AND deleted_at IS NULL",
        )
        .fetch_one(pool)
        .await?;
        assert_eq!(default_indexer, (60, 30, 2, true));

        let default_routing: (i32, i32, i32, bool) = sqlx::query_as(
            "SELECT
                requests_per_minute,
                burst,
                concurrent_requests,
                is_system
             FROM rate_limit_policy
             WHERE display_name = 'default_routing'
               AND deleted_at IS NULL",
        )
        .fetch_one(pool)
        .await?;
        assert_eq!(default_routing, (120, 60, 4, true));
        Ok(())
    }

    #[tokio::test]
    async fn rate_limit_try_consume_rejects_invalid_inputs() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };
        let pool = test_db.pool();

        let err = rate_limit_try_consume(pool, "indexer_instance", 1, 0, 1)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("capacity_invalid"));

        let err = rate_limit_try_consume(pool, "indexer_instance", 1, 1, 0)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("tokens_invalid"));
        Ok(())
    }
}
