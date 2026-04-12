use super::*;
use crate::DataError;

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}
#[tokio::test]
async fn tag_create_update_delete_roundtrip() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let tag_id = tag_create(pool, actor, "favorites", "Favorites").await?;

    let updated = tag_update(
        pool,
        actor,
        Some(tag_id),
        Some("favorites"),
        "Favorites Updated",
    )
    .await?;
    assert_eq!(tag_id, updated);

    tag_soft_delete(pool, actor, Some(tag_id), None).await?;
    Ok(())
}

#[tokio::test]
async fn tag_list_returns_active_tags_only() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let keep_id = tag_create(pool, actor, "keep", "Keep").await?;
    let drop_id = tag_create(pool, actor, "drop", "Drop").await?;
    tag_soft_delete(pool, actor, Some(drop_id), None).await?;

    let tags = tag_list(pool, actor).await?;
    assert!(tags.iter().any(|row| row.tag_public_id == keep_id));
    assert!(tags.iter().all(|row| row.tag_public_id != drop_id));
    Ok(())
}

#[tokio::test]
async fn tag_update_requires_reference() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let err = tag_update(pool, actor, None, None, "Name")
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("tag_reference_missing"));
    Ok(())
}
#[tokio::test]
async fn tag_soft_delete_requires_reference() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let err = tag_soft_delete(pool, actor, None, None).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("tag_reference_missing"));
    Ok(())
}
