use super::*;
use crate::DataError;
use crate::indexers::instances::indexer_instance_field_bind_secret;
use crate::indexers::secrets::secret_create;
use chrono::Duration;
use sqlx::PgPool;

async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}

async fn insert_indexer_instance(
    pool: &PgPool,
    is_enabled: bool,
    enable_rss: bool,
) -> anyhow::Result<(i64, Uuid)> {
    let definition_id: i64 = sqlx::query_scalar(
        "INSERT INTO indexer_definition (
                upstream_source,
                upstream_slug,
                display_name,
                protocol,
                engine,
                schema_version,
                definition_hash,
                is_deprecated
            )
            VALUES ($1::upstream_source, $2, $3, $4::protocol, $5::engine, $6, $7, $8)
            RETURNING indexer_definition_id",
    )
    .bind("prowlarr_indexers")
    .bind(format!("executor-{}", Uuid::new_v4().simple()))
    .bind("Executor Definition")
    .bind("torrent")
    .bind("torznab")
    .bind(1_i32)
    .bind("d".repeat(64))
    .bind(false)
    .fetch_one(pool)
    .await?;

    let public_id = Uuid::new_v4();
    let instance_id: i64 = sqlx::query_scalar(
        "INSERT INTO indexer_instance (
                indexer_instance_public_id,
                indexer_definition_id,
                display_name,
                is_enabled,
                migration_state,
                enable_rss,
                enable_automatic_search,
                enable_interactive_search,
                priority,
                trust_tier_key,
                created_by_user_id,
                updated_by_user_id
            )
            VALUES (
                $1,
                $2,
                $3,
                $4,
                $5::indexer_instance_migration_state,
                $6,
                TRUE,
                TRUE,
                100,
                $7::trust_tier_key,
                0,
                0
            )
            RETURNING indexer_instance_id",
    )
    .bind(public_id)
    .bind(definition_id)
    .bind(format!("Executor Instance {}", public_id.simple()))
    .bind(is_enabled)
    .bind("ready")
    .bind(enable_rss)
    .bind("public")
    .fetch_one(pool)
    .await?;

    Ok((instance_id, public_id))
}

async fn insert_definition_field(
    pool: &PgPool,
    instance_id: i64,
    field_name: &str,
    field_type: &str,
    is_required: bool,
) -> anyhow::Result<()> {
    let definition_id: i64 = sqlx::query_scalar(
        "SELECT indexer_definition_id
             FROM indexer_instance
             WHERE indexer_instance_id = $1",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "INSERT INTO indexer_definition_field (
                indexer_definition_id,
                name,
                label,
                field_type,
                is_required,
                is_advanced,
                display_order
            )
            VALUES ($1, $2, $3, $4::field_type, $5, FALSE, 1)",
    )
    .bind(definition_id)
    .bind(field_name)
    .bind(format!("Field {field_name}"))
    .bind(field_type)
    .bind(is_required)
    .execute(pool)
    .await?;

    Ok(())
}

async fn insert_subscription(
    pool: &PgPool,
    instance_id: i64,
    is_enabled: bool,
    next_poll_at: chrono::DateTime<Utc>,
) -> anyhow::Result<i64> {
    let effective_next_poll_at = if is_enabled { Some(next_poll_at) } else { None };
    sqlx::query_scalar(
        "INSERT INTO indexer_rss_subscription (
                indexer_instance_id,
                is_enabled,
                interval_seconds,
                last_polled_at,
                next_poll_at,
                backoff_seconds,
                last_error_class
            )
            VALUES ($1, $2, 900, NULL, $3, NULL, NULL)
            RETURNING indexer_rss_subscription_id",
    )
    .bind(instance_id)
    .bind(is_enabled)
    .bind(effective_next_poll_at)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}
#[tokio::test]
async fn rss_poll_claim_rejects_invalid_limit() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let err = rss_poll_claim(pool, 0).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("limit_invalid"));
    Ok(())
}
#[tokio::test]
async fn rss_poll_apply_requires_subscription() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let now = Utc::now();
    let input = RssPollApplyInput {
        rss_subscription_id: 1,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now,
        finished_at: now,
        outcome: "failure",
        error_class: Some("timeout"),
        http_status: None,
        latency_ms: None,
        parse_ok: false,
        result_count: None,
        via_mitigation: "none",
        rate_limit_denied_scope: None,
        cf_detected: false,
        cf_retryable: false,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    let err = rss_poll_apply(pool, &input).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("rss_subscription_not_found"));
    Ok(())
}

#[tokio::test]
async fn rss_poll_claim_returns_due_enabled_subscriptions_only() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (due_instance_id, due_public_id) = insert_indexer_instance(pool, true, true).await?;
    let (not_due_instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let (subscription_disabled_instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let (instance_disabled_id, _) = insert_indexer_instance(pool, false, true).await?;
    let (rss_disabled_id, _) = insert_indexer_instance(pool, true, false).await?;

    let due_subscription_id =
        insert_subscription(pool, due_instance_id, true, now - Duration::minutes(1)).await?;
    let _ =
        insert_subscription(pool, not_due_instance_id, true, now + Duration::minutes(30)).await?;
    let _ = insert_subscription(
        pool,
        subscription_disabled_instance_id,
        false,
        now - Duration::minutes(1),
    )
    .await?;
    let _ =
        insert_subscription(pool, instance_disabled_id, true, now - Duration::minutes(1)).await?;
    let _ = insert_subscription(pool, rss_disabled_id, true, now - Duration::minutes(1)).await?;

    let claims = rss_poll_claim(pool, 25).await?;
    assert_eq!(claims.len(), 1);
    let claim = &claims[0];
    assert_eq!(claim.rss_subscription_id, due_subscription_id);
    assert_eq!(claim.indexer_instance_public_id, due_public_id);
    assert_eq!(claim.retry_seq, 0);
    assert_ne!(claim.correlation_id, Uuid::nil());

    let next_poll_at: chrono::DateTime<Utc> = sqlx::query_scalar(
        "SELECT next_poll_at
             FROM indexer_rss_subscription
             WHERE indexer_rss_subscription_id = $1",
    )
    .bind(due_subscription_id)
    .fetch_one(pool)
    .await?;
    assert!(next_poll_at > now);
    Ok(())
}

#[tokio::test]
async fn rss_poll_apply_success_updates_subscription_and_dedupes_items() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let subscription_id =
        insert_subscription(pool, instance_id, true, now - Duration::minutes(1)).await?;

    let item_guid = vec![
        format!("guid-{}", Uuid::new_v4().simple()),
        format!("guid-{}", Uuid::new_v4().simple()),
    ];
    let infohash_v1 = vec![
        format!("{:040x}", Uuid::new_v4().as_u128()),
        format!("{:040x}", Uuid::new_v4().as_u128()),
    ];
    let input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now - Duration::seconds(2),
        finished_at: now,
        outcome: "success",
        error_class: None,
        http_status: Some(200),
        latency_ms: Some(120),
        parse_ok: true,
        result_count: Some(2),
        via_mitigation: "none",
        rate_limit_denied_scope: None,
        cf_detected: false,
        cf_retryable: false,
        item_guid: Some(&item_guid),
        infohash_v1: Some(&infohash_v1),
        infohash_v2: None,
        magnet_hash: None,
    };

    let first = rss_poll_apply(pool, &input).await?;
    assert_eq!(first.items_parsed, 2);
    assert_eq!(first.items_eligible, 2);
    assert_eq!(first.items_inserted, 2);
    assert!(first.subscription_succeeded);

    let second = rss_poll_apply(pool, &input).await?;
    assert_eq!(second.items_parsed, 2);
    assert_eq!(second.items_eligible, 2);
    assert_eq!(second.items_inserted, 0);
    assert!(second.subscription_succeeded);

    let row: (
        bool,
        Option<chrono::DateTime<Utc>>,
        Option<i32>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT is_enabled, next_poll_at, backoff_seconds, last_error_class::text
                 FROM indexer_rss_subscription
                 WHERE indexer_rss_subscription_id = $1",
    )
    .bind(subscription_id)
    .fetch_one(pool)
    .await?;
    assert!(row.0);
    assert!(row.1.is_some());
    assert!(row.2.is_none());
    assert!(row.3.is_none());
    Ok(())
}

#[tokio::test]
async fn rss_poll_apply_non_retryable_disables_subscription_and_audits() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (instance_id, instance_public_id) = insert_indexer_instance(pool, true, true).await?;
    let subscription_id =
        insert_subscription(pool, instance_id, true, now - Duration::minutes(1)).await?;

    let input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now - Duration::seconds(2),
        finished_at: now,
        outcome: "failure",
        error_class: Some("auth_error"),
        http_status: Some(401),
        latency_ms: Some(150),
        parse_ok: false,
        result_count: None,
        via_mitigation: "none",
        rate_limit_denied_scope: None,
        cf_detected: false,
        cf_retryable: false,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    let result = rss_poll_apply(pool, &input).await?;
    assert!(!result.subscription_succeeded);
    assert_eq!(result.items_inserted, 0);

    let row: (
        bool,
        Option<chrono::DateTime<Utc>>,
        Option<i32>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT is_enabled, next_poll_at, backoff_seconds, last_error_class::text
                 FROM indexer_rss_subscription
                 WHERE indexer_rss_subscription_id = $1",
    )
    .bind(subscription_id)
    .fetch_one(pool)
    .await?;
    assert!(!row.0);
    assert!(row.1.is_none());
    assert!(row.2.is_none());
    assert_eq!(row.3.as_deref(), Some("auth_error"));

    let disabled_audit_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM config_audit_log
             WHERE entity_type = 'indexer_instance'
               AND entity_pk_bigint = $1
               AND entity_public_id = $2
               AND action = 'update'
               AND change_summary = $3",
    )
    .bind(instance_id)
    .bind(instance_public_id)
    .bind("RSS subscription auto-disabled: auth_error")
    .fetch_one(pool)
    .await?;
    assert_eq!(disabled_audit_count, 1);
    Ok(())
}

#[tokio::test]
async fn rss_poll_apply_retryable_failure_applies_exponential_backoff() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let subscription_id =
        insert_subscription(pool, instance_id, true, now - Duration::minutes(1)).await?;

    let input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now - Duration::seconds(2),
        finished_at: now,
        outcome: "failure",
        error_class: Some("timeout"),
        http_status: Some(504),
        latency_ms: Some(800),
        parse_ok: false,
        result_count: None,
        via_mitigation: "none",
        rate_limit_denied_scope: None,
        cf_detected: false,
        cf_retryable: false,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    let first = rss_poll_apply(pool, &input).await?;
    assert!(!first.subscription_succeeded);
    assert_eq!(first.items_inserted, 0);

    let first_state: (bool, Option<i32>, Option<String>, i32) = sqlx::query_as(
        "SELECT
                is_enabled,
                backoff_seconds,
                last_error_class::text,
                GREATEST(0, EXTRACT(EPOCH FROM (next_poll_at - now()))::int) AS seconds_until_next
             FROM indexer_rss_subscription
             WHERE indexer_rss_subscription_id = $1",
    )
    .bind(subscription_id)
    .fetch_one(pool)
    .await?;
    assert!(first_state.0);
    assert_eq!(first_state.1, Some(60));
    assert_eq!(first_state.2.as_deref(), Some("timeout"));
    assert!((60..=75).contains(&first_state.3));

    let second = rss_poll_apply(pool, &input).await?;
    assert!(!second.subscription_succeeded);
    assert_eq!(second.items_inserted, 0);

    let second_state: (bool, Option<i32>, Option<String>, i32) = sqlx::query_as(
        "SELECT
                is_enabled,
                backoff_seconds,
                last_error_class::text,
                GREATEST(0, EXTRACT(EPOCH FROM (next_poll_at - now()))::int) AS seconds_until_next
             FROM indexer_rss_subscription
             WHERE indexer_rss_subscription_id = $1",
    )
    .bind(subscription_id)
    .fetch_one(pool)
    .await?;
    assert!(second_state.0);
    assert_eq!(second_state.1, Some(120));
    assert_eq!(second_state.2.as_deref(), Some("timeout"));
    assert!((120..=150).contains(&second_state.3));
    Ok(())
}

#[tokio::test]
async fn rss_poll_apply_success_schedules_next_poll_with_interval_jitter() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let subscription_id =
        insert_subscription(pool, instance_id, true, now - Duration::minutes(1)).await?;

    let input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now - Duration::seconds(2),
        finished_at: now,
        outcome: "success",
        error_class: None,
        http_status: Some(200),
        latency_ms: Some(110),
        parse_ok: true,
        result_count: Some(0),
        via_mitigation: "none",
        rate_limit_denied_scope: None,
        cf_detected: false,
        cf_retryable: false,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    let result = rss_poll_apply(pool, &input).await?;
    assert!(result.subscription_succeeded);

    let row: (Option<i32>, Option<String>, i32) = sqlx::query_as(
        "SELECT
                backoff_seconds,
                last_error_class::text,
                GREATEST(0, EXTRACT(EPOCH FROM (next_poll_at - last_polled_at))::int)
             FROM indexer_rss_subscription
             WHERE indexer_rss_subscription_id = $1",
    )
    .bind(subscription_id)
    .fetch_one(pool)
    .await?;
    assert!(row.0.is_none());
    assert!(row.1.is_none());
    assert!((900..=960).contains(&row.2));
    Ok(())
}

#[tokio::test]
async fn rss_poll_apply_rate_limited_requires_scope() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let subscription_id =
        insert_subscription(pool, instance_id, true, now - Duration::minutes(1)).await?;

    let input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now - Duration::seconds(1),
        finished_at: now,
        outcome: "failure",
        error_class: Some("rate_limited"),
        http_status: Some(429),
        latency_ms: Some(25),
        parse_ok: false,
        result_count: None,
        via_mitigation: "none",
        rate_limit_denied_scope: None,
        cf_detected: false,
        cf_retryable: false,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    let err = rss_poll_apply(pool, &input).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("rate_limit_scope_missing"));
    Ok(())
}

#[tokio::test]
async fn rss_poll_apply_rate_limited_uses_retry_semantics_and_zeroes_counts() -> anyhow::Result<()>
{
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let subscription_id =
        insert_subscription(pool, instance_id, true, now - Duration::minutes(1)).await?;

    let input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now - Duration::seconds(2),
        finished_at: now,
        outcome: "failure",
        error_class: Some("rate_limited"),
        http_status: Some(429),
        latency_ms: Some(120),
        parse_ok: false,
        result_count: None,
        via_mitigation: "none",
        rate_limit_denied_scope: Some("indexer_instance"),
        cf_detected: false,
        cf_retryable: false,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    let result = rss_poll_apply(pool, &input).await?;
    assert!(!result.subscription_succeeded);
    assert_eq!(result.items_parsed, 0);
    assert_eq!(result.items_eligible, 0);
    assert_eq!(result.items_inserted, 0);

    let sub_state: (bool, Option<i32>, Option<String>, i32) = sqlx::query_as(
        "SELECT
                is_enabled,
                backoff_seconds,
                last_error_class::text,
                GREATEST(0, EXTRACT(EPOCH FROM (next_poll_at - now()))::int) AS seconds_until_next
             FROM indexer_rss_subscription
             WHERE indexer_rss_subscription_id = $1",
    )
    .bind(subscription_id)
    .fetch_one(pool)
    .await?;
    assert!(sub_state.0);
    assert_eq!(sub_state.1, Some(60));
    assert_eq!(sub_state.2.as_deref(), Some("rate_limited"));
    assert!((60..=75).contains(&sub_state.3));

    let outbound: (String, String, bool, i32, i32, Option<String>) = sqlx::query_as(
        "SELECT
                outcome::text,
                error_class::text,
                parse_ok,
                latency_ms,
                result_count,
                rate_limit_denied_scope::text
             FROM outbound_request_log
             WHERE correlation_id = $1
               AND request_type = 'rss'
             ORDER BY outbound_request_log_id DESC
             LIMIT 1",
    )
    .bind(input.correlation_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(outbound.0.as_str(), "failure");
    assert_eq!(outbound.1.as_str(), "rate_limited");
    assert!(!outbound.2);
    assert_eq!(outbound.3, 0);
    assert_eq!(outbound.4, 0);
    assert_eq!(outbound.5.as_deref(), Some("indexer_instance"));
    Ok(())
}

#[tokio::test]
async fn rss_poll_apply_cf_challenge_non_retryable_sets_challenged_state() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let subscription_id =
        insert_subscription(pool, instance_id, true, now - Duration::minutes(1)).await?;

    let input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now - Duration::seconds(1),
        finished_at: now,
        outcome: "failure",
        error_class: Some("cf_challenge"),
        http_status: Some(403),
        latency_ms: Some(100),
        parse_ok: false,
        result_count: None,
        via_mitigation: "none",
        rate_limit_denied_scope: None,
        cf_detected: true,
        cf_retryable: false,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    let result = rss_poll_apply(pool, &input).await?;
    assert!(!result.subscription_succeeded);

    let cf_state: (
        String,
        i32,
        Option<i32>,
        Option<chrono::DateTime<Utc>>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT
                    state::text,
                    consecutive_failures,
                    backoff_seconds,
                    cooldown_until,
                    last_error_class::text
                 FROM indexer_cf_state
                 WHERE indexer_instance_id = $1",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(cf_state.0.as_str(), "challenged");
    assert_eq!(cf_state.1, 1);
    assert!(cf_state.2.is_none());
    assert!(cf_state.3.is_none());
    assert_eq!(cf_state.4.as_deref(), Some("cf_challenge"));
    Ok(())
}

#[tokio::test]
async fn rss_poll_apply_cf_challenge_non_retryable_reaches_cooldown_after_five_failures()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let subscription_id =
        insert_subscription(pool, instance_id, true, now - Duration::minutes(1)).await?;

    let input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now - Duration::seconds(1),
        finished_at: now,
        outcome: "failure",
        error_class: Some("cf_challenge"),
        http_status: Some(403),
        latency_ms: Some(100),
        parse_ok: false,
        result_count: None,
        via_mitigation: "none",
        rate_limit_denied_scope: None,
        cf_detected: true,
        cf_retryable: false,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    for _ in 0..5 {
        let _ = rss_poll_apply(pool, &input).await?;
    }

    let cf_state: (
        String,
        i32,
        Option<i32>,
        Option<chrono::DateTime<Utc>>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT
                    state::text,
                    consecutive_failures,
                    backoff_seconds,
                    cooldown_until,
                    last_error_class::text
                 FROM indexer_cf_state
                 WHERE indexer_instance_id = $1",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(cf_state.0.as_str(), "cooldown");
    assert_eq!(cf_state.1, 5);
    assert_eq!(cf_state.2, Some(60));
    assert!(cf_state.3.is_some());
    assert_eq!(cf_state.4.as_deref(), Some("cf_challenge"));
    Ok(())
}

#[tokio::test]
async fn rss_poll_apply_cf_challenge_retryable_sets_challenged_state() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let subscription_id =
        insert_subscription(pool, instance_id, true, now - Duration::minutes(1)).await?;

    let input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now - Duration::seconds(1),
        finished_at: now,
        outcome: "failure",
        error_class: Some("cf_challenge"),
        http_status: Some(403),
        latency_ms: Some(100),
        parse_ok: false,
        result_count: None,
        via_mitigation: "flaresolverr",
        rate_limit_denied_scope: None,
        cf_detected: true,
        cf_retryable: true,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    let result = rss_poll_apply(pool, &input).await?;
    assert!(!result.subscription_succeeded);

    let cf_state: (
        String,
        i32,
        Option<i32>,
        Option<chrono::DateTime<Utc>>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT
                    state::text,
                    consecutive_failures,
                    backoff_seconds,
                    cooldown_until,
                    last_error_class::text
                 FROM indexer_cf_state
                 WHERE indexer_instance_id = $1",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(cf_state.0.as_str(), "challenged");
    assert_eq!(cf_state.1, 1);
    assert!(cf_state.2.is_none());
    assert!(cf_state.3.is_none());
    assert_eq!(cf_state.4.as_deref(), Some("cf_challenge"));
    Ok(())
}

#[tokio::test]
async fn rss_poll_apply_flaresolverr_success_promotes_challenged_to_solved() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    let now = test_db.now();
    let (instance_id, _) = insert_indexer_instance(pool, true, true).await?;
    let subscription_id =
        insert_subscription(pool, instance_id, true, now - Duration::minutes(1)).await?;

    let challenge_input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 0,
        started_at: now - Duration::seconds(2),
        finished_at: now - Duration::seconds(1),
        outcome: "failure",
        error_class: Some("cf_challenge"),
        http_status: Some(403),
        latency_ms: Some(100),
        parse_ok: false,
        result_count: None,
        via_mitigation: "flaresolverr",
        rate_limit_denied_scope: None,
        cf_detected: true,
        cf_retryable: true,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    let challenge_result = rss_poll_apply(pool, &challenge_input).await?;
    assert!(!challenge_result.subscription_succeeded);

    let success_input = RssPollApplyInput {
        rss_subscription_id: subscription_id,
        correlation_id: Uuid::new_v4(),
        retry_seq: 1,
        started_at: now - Duration::seconds(1),
        finished_at: now,
        outcome: "success",
        error_class: None,
        http_status: Some(200),
        latency_ms: Some(80),
        parse_ok: true,
        result_count: Some(1),
        via_mitigation: "flaresolverr",
        rate_limit_denied_scope: None,
        cf_detected: false,
        cf_retryable: true,
        item_guid: None,
        infohash_v1: None,
        infohash_v2: None,
        magnet_hash: None,
    };

    let success_result = rss_poll_apply(pool, &success_input).await?;
    assert!(success_result.subscription_succeeded);

    let cf_state: (
        String,
        i32,
        Option<i32>,
        Option<chrono::DateTime<Utc>>,
        Option<String>,
    ) = sqlx::query_as(
        "SELECT
                    state::text,
                    consecutive_failures,
                    backoff_seconds,
                    cooldown_until,
                    last_error_class::text
                 FROM indexer_cf_state
                 WHERE indexer_instance_id = $1",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(cf_state.0.as_str(), "solved");
    assert_eq!(cf_state.1, 0);
    assert_eq!(cf_state.2, Some(60));
    assert!(cf_state.3.is_none());
    assert!(cf_state.4.is_none());
    Ok(())
}
#[tokio::test]
async fn indexer_test_prepare_requires_instance() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let err = indexer_instance_test_prepare(pool, None, Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("indexer_not_found"));
    Ok(())
}

#[tokio::test]
async fn indexer_test_prepare_reports_missing_required_secret() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let (instance_id, instance_public_id) = insert_indexer_instance(pool, true, true).await?;
    insert_definition_field(pool, instance_id, "api_key", "api_key", true).await?;

    let prepare = indexer_instance_test_prepare(pool, None, instance_public_id).await?;
    assert!(!prepare.can_execute);
    assert_eq!(prepare.error_class.as_deref(), Some("auth_error"));
    assert_eq!(prepare.error_code.as_deref(), Some("missing_secret"));
    assert_eq!(prepare.detail.as_deref(), Some("api_key"));
    assert!(prepare.field_names.is_none());
    assert!(prepare.secret_public_ids.is_none());

    let state: (String, bool) = sqlx::query_as(
        "SELECT migration_state::text, is_enabled
             FROM indexer_instance
             WHERE indexer_instance_id = $1",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(state.0, "needs_secret");
    assert!(!state.1);
    Ok(())
}

#[tokio::test]
async fn indexer_test_prepare_returns_bound_secret_configuration() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str("00000000-0000-0000-0000-000000000000")?;
    let (instance_id, instance_public_id) = insert_indexer_instance(pool, true, true).await?;
    insert_definition_field(pool, instance_id, "api_key", "api_key", true).await?;

    let secret_public_id = secret_create(pool, actor, "api_key", "executor-secret").await?;
    indexer_instance_field_bind_secret(
        pool,
        actor,
        instance_public_id,
        "api_key",
        secret_public_id,
    )
    .await?;

    let prepare = indexer_instance_test_prepare(pool, None, instance_public_id).await?;
    assert!(prepare.can_execute);
    assert!(prepare.error_class.is_none());
    assert!(prepare.error_code.is_none());
    assert!(prepare.detail.is_none());
    assert_eq!(prepare.field_names, Some(vec!["api_key".to_string()]));
    assert_eq!(prepare.field_types, Some(vec!["api_key".to_string()]));
    assert_eq!(
        prepare.secret_public_ids,
        Some(vec![Some(secret_public_id)])
    );
    Ok(())
}
#[tokio::test]
async fn indexer_test_finalize_requires_instance() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let input = IndexerTestFinalizeInput {
        actor_user_public_id: None,
        indexer_instance_public_id: Uuid::new_v4(),
        ok: false,
        error_class: Some("timeout"),
        error_code: Some("probe_failed"),
        detail: Some("timeout"),
        result_count: None,
    };
    let err = indexer_instance_test_finalize(pool, &input)
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("indexer_not_found"));
    Ok(())
}

#[tokio::test]
async fn indexer_test_finalize_success_clears_migration_error_state() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str("00000000-0000-0000-0000-000000000000")?;
    let (instance_id, instance_public_id) = insert_indexer_instance(pool, true, true).await?;
    insert_definition_field(pool, instance_id, "api_key", "api_key", true).await?;

    let _ = indexer_instance_test_prepare(pool, None, instance_public_id).await?;

    let secret_public_id = secret_create(pool, actor, "api_key", "executor-secret").await?;
    indexer_instance_field_bind_secret(
        pool,
        actor,
        instance_public_id,
        "api_key",
        secret_public_id,
    )
    .await?;

    let finalized = indexer_instance_test_finalize(
        pool,
        &IndexerTestFinalizeInput {
            actor_user_public_id: None,
            indexer_instance_public_id: instance_public_id,
            ok: true,
            error_class: None,
            error_code: None,
            detail: None,
            result_count: Some(3),
        },
    )
    .await?;
    assert!(finalized.ok);
    assert!(finalized.error_class.is_none());
    assert!(finalized.error_code.is_none());
    assert!(finalized.detail.is_none());
    assert_eq!(finalized.result_count, Some(3));

    let state: (String, Option<String>, bool) = sqlx::query_as(
        "SELECT migration_state::text, migration_detail, is_enabled
             FROM indexer_instance
             WHERE indexer_instance_id = $1",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(state.0, "ready");
    assert!(state.1.is_none());
    assert!(state.2);
    Ok(())
}
#[tokio::test]
async fn secret_read_requires_secret() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let err = secret_read(pool, None, Uuid::new_v4()).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("secret_not_found"));
    Ok(())
}
