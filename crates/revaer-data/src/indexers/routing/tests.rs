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

    let rate_limit_policy_public_id: Uuid =
        sqlx::query_scalar(include_str!("sql/insert_rate_limit_policy.sql"))
            .bind(Uuid::new_v4())
            .bind("Proxy Rate Limit")
            .bind(90_i32)
            .bind(15_i32)
            .bind(3_i32)
            .fetch_one(pool)
            .await?;

    sqlx::query(include_str!("sql/insert_routing_policy_rate_limit.sql"))
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
    assert!(
        rows.iter().any(
            |row| row.param_key.as_deref() == Some("proxy_port") && row.value_int == Some(8443)
        )
    );
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
