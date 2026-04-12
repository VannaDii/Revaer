use super::*;
use crate::DataError;

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("health notification hook tests").await
}

#[tokio::test]
async fn health_notification_hook_crud_roundtrip() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;

    let webhook_public_id = indexer_health_notification_hook_create(
        pool,
        actor,
        "webhook",
        "Pager webhook",
        "failing",
        Some("https://hooks.example.test/indexers"),
        None,
    )
    .await?;
    let email_public_id = indexer_health_notification_hook_create(
        pool,
        actor,
        "email",
        "Ops inbox",
        "degraded",
        None,
        Some("Ops@example.test"),
    )
    .await?;

    let list = indexer_health_notification_hook_list(pool, actor).await?;
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].channel, "email");
    assert_eq!(list[0].email.as_deref(), Some("ops@example.test"));
    assert_eq!(
        list[1].indexer_health_notification_hook_public_id,
        webhook_public_id
    );

    let updated_public_id = indexer_health_notification_hook_update(
        pool,
        &IndexerHealthNotificationHookUpdateInput {
            actor_user_public_id: actor,
            hook_public_id: webhook_public_id,
            display_name: Some("Pager escalation"),
            status_threshold: Some("quarantined"),
            webhook_url: Some("https://hooks.example.test/escalation"),
            email: None,
            is_enabled: Some(false),
        },
    )
    .await?;
    assert_eq!(updated_public_id, webhook_public_id);

    let webhook = indexer_health_notification_hook_get(pool, actor, webhook_public_id).await?;
    assert_eq!(webhook.display_name, "Pager escalation");
    assert_eq!(webhook.status_threshold, "quarantined");
    assert_eq!(
        webhook.webhook_url.as_deref(),
        Some("https://hooks.example.test/escalation")
    );
    assert!(!webhook.is_enabled);

    indexer_health_notification_hook_delete(pool, actor, email_public_id).await?;
    let list = indexer_health_notification_hook_list(pool, actor).await?;
    assert_eq!(list.len(), 1);

    Ok(())
}

#[tokio::test]
async fn health_notification_hook_rejects_channel_payload_mismatch() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;

    let hook_public_id = indexer_health_notification_hook_create(
        pool,
        actor,
        "email",
        "Ops inbox",
        "failing",
        None,
        Some("ops@example.test"),
    )
    .await?;

    let err = indexer_health_notification_hook_update(
        pool,
        &IndexerHealthNotificationHookUpdateInput {
            actor_user_public_id: actor,
            hook_public_id,
            display_name: None,
            status_threshold: None,
            webhook_url: Some("https://hooks.example.test/wrong"),
            email: None,
            is_enabled: None,
        },
    )
    .await
    .expect_err("email hook update should reject webhook payload");

    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("channel_payload_mismatch"));

    Ok(())
}

#[tokio::test]
async fn health_notification_hook_get_requires_reference() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let err = indexer_health_notification_hook_get(pool, actor, Uuid::new_v4())
        .await
        .expect_err("missing hook should fail");

    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("hook_not_found"));

    Ok(())
}
