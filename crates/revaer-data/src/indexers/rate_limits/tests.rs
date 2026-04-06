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

    let err = indexer_instance_set_rate_limit_policy(pool, actor, Uuid::new_v4(), Some(policy_id))
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("indexer_not_found"));

    let err = routing_policy_set_rate_limit_policy(pool, actor, Uuid::new_v4(), Some(policy_id))
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
async fn rate_limit_try_consume_enforces_bucket_capacity_for_routing_scope() -> anyhow::Result<()> {
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
