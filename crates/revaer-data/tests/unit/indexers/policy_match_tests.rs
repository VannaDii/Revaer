use super::*;
async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}
#[tokio::test]
async fn policy_match_helpers_work() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let text_match =
        policy_text_match(pool, Some("Alpha"), "eq", Some("alpha"), None, true).await?;
    assert!(text_match);

    let value = Uuid::new_v4();
    let uuid_match = policy_uuid_match(pool, Some(value), "eq", Some(value), None).await?;
    assert!(uuid_match);

    let int_match = policy_int_match(pool, Some(5), "eq", Some(5), None).await?;
    assert!(int_match);

    let group_match =
        policy_release_group_match(pool, 0, Some("Group"), "eq", Some("group"), None, true).await?;
    assert!(group_match);

    Ok(())
}
