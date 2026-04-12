use super::*;
use crate::DataError;

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}

async fn create_policy_set_and_rule(pool: &PgPool, actor: Uuid) -> anyhow::Result<(Uuid, Uuid)> {
    let policy_set_public_id =
        policy_set_create(pool, actor, "Test Policy Set", "global", Some(true)).await?;
    let input = PolicyRuleCreateInput {
        actor_user_public_id: actor,
        policy_set_public_id,
        rule_type: "block_title_regex",
        match_field: "title",
        match_operator: "regex",
        sort_order: None,
        match_value_text: Some("sample"),
        match_value_int: None,
        match_value_uuid: None,
        value_set_items: None,
        action: "drop_canonical",
        severity: "hard",
        is_case_insensitive: Some(true),
        rationale: None,
        expires_at: None,
    };
    let policy_rule_public_id = policy_rule_create(pool, &input).await?;
    Ok((policy_set_public_id, policy_rule_public_id))
}

#[tokio::test]
async fn policy_set_create_rejects_duplicate_global() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let first = policy_set_create(pool, actor, "Global", "global", Some(true)).await?;
    assert_ne!(first, Uuid::nil());

    let err = policy_set_create(pool, actor, "Global", "global", Some(true))
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("global_policy_set_exists"));
    Ok(())
}

#[tokio::test]
async fn policy_rule_create_requires_policy_set() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let input = PolicyRuleCreateInput {
        actor_user_public_id: actor,
        policy_set_public_id: Uuid::new_v4(),
        rule_type: "block_title_regex",
        match_field: "title",
        match_operator: "regex",
        sort_order: None,
        match_value_text: Some("sample"),
        match_value_int: None,
        match_value_uuid: None,
        value_set_items: None,
        action: "drop_canonical",
        severity: "hard",
        is_case_insensitive: Some(true),
        rationale: None,
        expires_at: None,
    };
    let err = policy_rule_create(pool, &input).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("policy_set_not_found"));
    Ok(())
}

#[tokio::test]
async fn policy_set_reorder_requires_ids() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let err = policy_set_reorder(pool, actor, &[]).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("policy_set_ids_empty"));
    Ok(())
}

#[tokio::test]
async fn policy_rule_disable_enable_toggles_disabled_state() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let (_, policy_rule_public_id) = create_policy_set_and_rule(pool, actor).await?;

    policy_rule_disable(pool, actor, policy_rule_public_id).await?;
    let disabled_after_disable: bool = sqlx::query_scalar(
        "SELECT is_disabled
         FROM policy_rule
         WHERE policy_rule_public_id = $1",
    )
    .bind(policy_rule_public_id)
    .fetch_one(pool)
    .await?;
    assert!(disabled_after_disable);

    policy_rule_enable(pool, actor, policy_rule_public_id).await?;
    let disabled_after_enable: bool = sqlx::query_scalar(
        "SELECT is_disabled
         FROM policy_rule
         WHERE policy_rule_public_id = $1",
    )
    .bind(policy_rule_public_id)
    .fetch_one(pool)
    .await?;
    assert!(!disabled_after_enable);
    Ok(())
}

#[tokio::test]
async fn policy_rule_reorder_requires_ids() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let (policy_set_public_id, _) = create_policy_set_and_rule(pool, actor).await?;

    let err = policy_rule_reorder(pool, actor, policy_set_public_id, &[])
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("policy_rule_ids_empty"));
    Ok(())
}
