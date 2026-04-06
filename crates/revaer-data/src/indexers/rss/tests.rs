use super::*;
use crate::DataError;

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("rss tests").await
}

async fn create_indexer(pool: &PgPool, actor: Uuid) -> anyhow::Result<Uuid> {
    let upstream_slug = format!("rss-test-{}", Uuid::new_v4().simple());
    sqlx::query(include_str!("sql/insert_indexer_definition.sql"))
        .bind("prowlarr_indexers")
        .bind(&upstream_slug)
        .bind("RSS Test Definition")
        .bind("torrent")
        .bind("torznab")
        .bind(1_i32)
        .bind("a".repeat(64))
        .bind(false)
        .execute(pool)
        .await?;

    crate::indexers::instances::indexer_instance_create(
        pool,
        actor,
        &upstream_slug,
        &format!("RSS Test {}", Uuid::new_v4().simple()),
        Some(50),
        Some("public"),
        None,
    )
    .await
    .map_err(anyhow::Error::from)
}

#[tokio::test]
async fn rss_subscription_get_returns_status_for_instance() -> anyhow::Result<()> {
    let Ok(db) = setup_db().await else {
        return Ok(());
    };
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let indexer_instance_public_id = create_indexer(db.pool(), actor).await?;

    let row = rss_subscription_get(db.pool(), actor, indexer_instance_public_id).await?;

    assert_eq!(row.indexer_instance_public_id, indexer_instance_public_id);
    assert_eq!(row.instance_status, "enabled");
    assert_eq!(row.rss_status, "enabled");
    assert!(row.subscription_exists);
    assert!(row.subscription_is_enabled);
    assert_eq!(row.interval_seconds, 900);
    assert!(row.next_poll_at.is_some());
    Ok(())
}

#[tokio::test]
async fn rss_item_seen_list_returns_recent_rows() -> anyhow::Result<()> {
    let Ok(db) = setup_db().await else {
        return Ok(());
    };
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let indexer_instance_public_id = create_indexer(db.pool(), actor).await?;

    let marked = rss_item_seen_mark(
        db.pool(),
        &RssSeenMarkInput {
            actor_user_public_id: actor,
            indexer_instance_public_id,
            item_guid: Some("GUID-123"),
            infohash_v1: None,
            infohash_v2: None,
            magnet_hash: None,
        },
    )
    .await?;

    let items = rss_item_seen_list(db.pool(), actor, indexer_instance_public_id, Some(10)).await?;

    assert_eq!(items.len(), 1);
    assert_eq!(items[0].item_guid.as_deref(), Some("guid-123"));
    assert_eq!(items[0].first_seen_at, marked.first_seen_at);
    Ok(())
}

#[tokio::test]
async fn rss_subscription_get_requires_instance() -> anyhow::Result<()> {
    let Ok(db) = setup_db().await else {
        return Ok(());
    };
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;

    let err = rss_subscription_get(db.pool(), actor, Uuid::new_v4())
        .await
        .expect_err("missing instance");

    assert_eq!(err.database_detail(), Some("indexer_not_found"));
    Ok(())
}

#[tokio::test]
async fn rss_item_seen_mark_is_idempotent() -> anyhow::Result<()> {
    let Ok(db) = setup_db().await else {
        return Ok(());
    };
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let indexer_instance_public_id = create_indexer(db.pool(), actor).await?;

    let first = rss_item_seen_mark(
        db.pool(),
        &RssSeenMarkInput {
            actor_user_public_id: actor,
            indexer_instance_public_id,
            item_guid: None,
            infohash_v1: Some("0123456789abcdef0123456789abcdef01234567"),
            infohash_v2: None,
            magnet_hash: None,
        },
    )
    .await?;
    let second = rss_item_seen_mark(
        db.pool(),
        &RssSeenMarkInput {
            actor_user_public_id: actor,
            indexer_instance_public_id,
            item_guid: None,
            infohash_v1: Some("0123456789ABCDEF0123456789ABCDEF01234567"),
            infohash_v2: None,
            magnet_hash: None,
        },
    )
    .await?;

    assert!(first.inserted);
    assert!(!second.inserted);
    assert_eq!(first.first_seen_at, second.first_seen_at);
    Ok(())
}

#[tokio::test]
async fn rss_item_seen_mark_requires_identifier() -> anyhow::Result<()> {
    let Ok(db) = setup_db().await else {
        return Ok(());
    };
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let indexer_instance_public_id = create_indexer(db.pool(), actor).await?;

    let err = rss_item_seen_mark(
        db.pool(),
        &RssSeenMarkInput {
            actor_user_public_id: actor,
            indexer_instance_public_id,
            item_guid: None,
            infohash_v1: None,
            infohash_v2: None,
            magnet_hash: None,
        },
    )
    .await
    .expect_err("missing identifiers");

    assert_eq!(
        match err {
            DataError::QueryFailed { .. } => err.database_detail(),
            _ => None,
        },
        Some("rss_item_identifier_missing")
    );
    Ok(())
}
