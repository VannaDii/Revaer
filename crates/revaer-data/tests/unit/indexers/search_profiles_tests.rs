use super::*;
use crate::DataError;

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}
#[tokio::test]
async fn search_profile_create_update_roundtrip() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let profile_id =
        search_profile_create(pool, actor, "Default", Some(true), None, None, None).await?;

    let updated =
        search_profile_update(pool, actor, profile_id, Some("Default Updated"), Some(20)).await?;
    assert_eq!(profile_id, updated);
    Ok(())
}
#[tokio::test]
async fn search_profile_set_default_requires_profile() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let err = search_profile_set_default(pool, actor, Uuid::new_v4(), None)
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("search_profile_not_found"));
    Ok(())
}
#[tokio::test]
async fn search_profile_set_default_domain_requires_profile() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let err = search_profile_set_default_domain(pool, actor, Uuid::new_v4(), Some("movies"))
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("search_profile_not_found"));
    Ok(())
}
#[tokio::test]
async fn search_profile_tag_allow_requires_profile() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let err = search_profile_tag_allow(pool, actor, Uuid::new_v4(), None, None)
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("search_profile_not_found"));
    Ok(())
}
