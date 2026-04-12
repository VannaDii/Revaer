use super::*;
use crate::DataError;
use crate::indexers::search_requests::{SearchRequestCreateInput, search_request_create};
use chrono::{Duration, Utc};

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}

async fn create_search_request(pool: &PgPool) -> anyhow::Result<Uuid> {
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let input = SearchRequestCreateInput {
        actor_user_public_id: Some(actor),
        query_text: "dune",
        query_type: "free_text",
        torznab_mode: None,
        requested_media_domain_key: None,
        page_size: None,
        search_profile_public_id: None,
        request_policy_set_public_id: None,
        season_number: None,
        episode_number: None,
        identifier_types: None,
        identifier_values: None,
        torznab_cat_ids: None,
    };

    let row = search_request_create(pool, &input).await?;
    Ok(row.search_request_public_id)
}

async fn fetch_search_request_id(
    pool: &PgPool,
    search_request_public_id: Uuid,
) -> anyhow::Result<i64> {
    sqlx::query_scalar(
        "SELECT search_request_id
         FROM search_request
         WHERE search_request_public_id = $1",
    )
    .bind(search_request_public_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn create_indexer_instance(pool: &PgPool) -> anyhow::Result<i64> {
    let definition_id: i64 = sqlx::query_scalar(include_str!("sql/insert_indexer_definition.sql"))
        .bind("prowlarr_indexers")
        .bind(format!("search-page-{}", Uuid::new_v4().simple()))
        .bind("Search Page Definition")
        .bind("torrent")
        .bind("torznab")
        .bind(1_i32)
        .bind("a".repeat(64))
        .bind(false)
        .fetch_one(pool)
        .await?;

    sqlx::query_scalar(include_str!("sql/insert_indexer_instance.sql"))
        .bind(Uuid::new_v4())
        .bind(definition_id)
        .bind(format!("Search Page Instance {}", Uuid::new_v4().simple()))
        .bind("ready")
        .bind(50_i32)
        .bind("public")
        .bind(0_i64)
        .bind(0_i64)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

async fn create_canonical_source(
    pool: &PgPool,
    indexer_instance_id: i64,
) -> anyhow::Result<(i64, i64)> {
    let canonical_torrent_id: i64 =
        sqlx::query_scalar(include_str!("sql/insert_canonical_torrent.sql"))
            .bind(Uuid::new_v4())
            .bind(1.0_f64)
            .bind("infohash_v1")
            .bind("b".repeat(40))
            .bind("Blocked Item")
            .bind("blocked item")
            .bind(1234_i64)
            .fetch_one(pool)
            .await?;

    let canonical_torrent_source_id: i64 =
        sqlx::query_scalar(include_str!("sql/insert_canonical_torrent_source.sql"))
            .bind(indexer_instance_id)
            .bind(Uuid::new_v4())
            .bind(format!("source-{}", Uuid::new_v4().simple()))
            .bind("c".repeat(40))
            .bind("blocked item")
            .bind(1234_i64)
            .fetch_one(pool)
            .await?;

    Ok((canonical_torrent_id, canonical_torrent_source_id))
}

async fn insert_policy_snapshot(pool: &PgPool) -> anyhow::Result<i64> {
    sqlx::query_scalar(include_str!("sql/insert_policy_snapshot.sql"))
        .bind("d".repeat(64))
        .bind(1_i32)
        .fetch_one(pool)
        .await
        .map_err(Into::into)
}

async fn insert_rate_limited_retrying_run(
    pool: &PgPool,
    search_request_id: i64,
    indexer_instance_id: i64,
    retry_at: chrono::DateTime<Utc>,
) -> anyhow::Result<()> {
    sqlx::query(include_str!("sql/insert_retrying_run.sql"))
        .bind(search_request_id)
        .bind(indexer_instance_id)
        .bind("queued")
        .bind(retry_at)
        .bind("rate_limited")
        .bind("indexer_instance")
        .bind(None::<String>)
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_terminal_run(
    pool: &PgPool,
    search_request_id: i64,
    status: &str,
    error_class: Option<&str>,
) -> anyhow::Result<()> {
    let indexer_instance_id = create_indexer_instance(pool).await?;
    sqlx::query(include_str!("sql/insert_terminal_run.sql"))
        .bind(search_request_id)
        .bind(indexer_instance_id)
        .bind(status)
        .bind(error_class)
        .execute(pool)
        .await?;
    Ok(())
}

async fn insert_blocked_decision(
    pool: &PgPool,
    search_request_id: i64,
    policy_snapshot_id: i64,
    canonical_torrent_id: i64,
    canonical_torrent_source_id: i64,
    block_rule_id: Uuid,
) -> anyhow::Result<()> {
    sqlx::query(include_str!("sql/insert_search_filter_decision.sql"))
        .bind(search_request_id)
        .bind(block_rule_id)
        .bind(policy_snapshot_id)
        .bind(canonical_torrent_id)
        .bind(canonical_torrent_source_id)
        .bind("drop_source")
        .execute(pool)
        .await?;
    Ok(())
}

#[tokio::test]
async fn search_page_list_requires_request() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;

    let err = search_page_list(pool, actor, Uuid::new_v4())
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("search_request_not_found"));
    Ok(())
}

#[tokio::test]
async fn search_page_list_returns_initial_page() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let request_id = create_search_request(pool).await?;

    let pages = search_page_list(pool, actor, request_id).await?;
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].page_number, 1);
    assert_eq!(pages[0].item_count, 0);
    Ok(())
}

#[tokio::test]
async fn search_page_fetch_rejects_invalid_page_number() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let request_id = create_search_request(pool).await?;

    let err = search_page_fetch(pool, actor, request_id, 0)
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("page_number_invalid"));
    Ok(())
}

#[tokio::test]
async fn search_page_fetch_returns_empty_page() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let request_id = create_search_request(pool).await?;

    let rows = search_page_fetch(pool, actor, request_id, 1).await?;
    let first = rows.first().expect("expected page row");
    assert_eq!(first.page_number, 1);
    assert_eq!(first.item_count, 0);
    assert!(first.canonical_torrent_public_id.is_none());
    Ok(())
}

#[tokio::test]
async fn search_request_explainability_defaults_with_zero_runnable_indexers() -> anyhow::Result<()>
{
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let request_id = create_search_request(pool).await?;

    let row = search_request_explainability(pool, actor, request_id).await?;
    assert!(row.zero_runnable_indexers);
    assert_eq!(row.skipped_canceled_indexers, 0);
    assert_eq!(row.skipped_failed_indexers, 0);
    assert_eq!(row.blocked_results, 0);
    assert!(row.blocked_rule_public_ids.is_empty());
    assert_eq!(row.rate_limited_indexers, 0);
    assert_eq!(row.retrying_indexers, 0);
    Ok(())
}

#[tokio::test]
async fn search_request_explainability_surfaces_blocked_and_retrying_state() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let search_request_public_id = create_search_request(pool).await?;
    let search_request_id = fetch_search_request_id(pool, search_request_public_id).await?;
    let indexer_instance_id = create_indexer_instance(pool).await?;
    let (canonical_torrent_id, canonical_torrent_source_id) =
        create_canonical_source(pool, indexer_instance_id).await?;
    let block_rule_id = Uuid::new_v4();
    let retry_at = Utc::now() + Duration::minutes(5);
    let policy_snapshot_id = insert_policy_snapshot(pool).await?;

    insert_rate_limited_retrying_run(pool, search_request_id, indexer_instance_id, retry_at)
        .await?;
    insert_terminal_run(pool, search_request_id, "failed", Some("timeout")).await?;
    insert_terminal_run(pool, search_request_id, "canceled", None).await?;
    insert_blocked_decision(
        pool,
        search_request_id,
        policy_snapshot_id,
        canonical_torrent_id,
        canonical_torrent_source_id,
        block_rule_id,
    )
    .await?;

    let row = search_request_explainability(pool, actor, search_request_public_id).await?;
    assert!(!row.zero_runnable_indexers);
    assert_eq!(row.skipped_canceled_indexers, 1);
    assert_eq!(row.skipped_failed_indexers, 1);
    assert_eq!(row.blocked_results, 1);
    assert_eq!(row.blocked_rule_public_ids, vec![block_rule_id]);
    assert_eq!(row.rate_limited_indexers, 1);
    assert_eq!(row.retrying_indexers, 1);
    Ok(())
}
