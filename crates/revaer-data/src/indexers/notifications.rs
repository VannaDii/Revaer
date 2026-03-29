//! Stored-procedure access for indexer health notification hooks.
//!
//! # Design
//! - Keeps health notification hook CRUD behind stored-procedure calls.
//! - Returns typed rows so API and UI layers stay free of database details.
//! - Uses constant operation names when mapping data-layer failures.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const HEALTH_NOTIFICATION_HOOK_CREATE_CALL: &str = r"
    SELECT indexer_health_notification_hook_create(
        actor_user_public_id => $1,
        channel_input => $2::indexer_health_notification_channel,
        display_name_input => $3,
        status_threshold_input => $4::indexer_health_notification_threshold,
        webhook_url_input => $5,
        email_input => $6
    )
";

const HEALTH_NOTIFICATION_HOOK_UPDATE_CALL: &str = r"
    SELECT indexer_health_notification_hook_update(
        actor_user_public_id => $1,
        indexer_health_notification_hook_public_id_input => $2,
        display_name_input => $3,
        status_threshold_input => $4::indexer_health_notification_threshold,
        webhook_url_input => $5,
        email_input => $6,
        is_enabled_input => $7
    )
";

const HEALTH_NOTIFICATION_HOOK_DELETE_CALL: &str = r"
    SELECT indexer_health_notification_hook_delete(
        actor_user_public_id => $1,
        indexer_health_notification_hook_public_id_input => $2
    )
";

const HEALTH_NOTIFICATION_HOOK_LIST_CALL: &str = r"
    SELECT
        indexer_health_notification_hook_public_id,
        channel::text,
        display_name,
        status_threshold::text,
        webhook_url,
        email,
        is_enabled,
        updated_at
    FROM indexer_health_notification_hook_list(
        actor_user_public_id => $1
    )
";

const HEALTH_NOTIFICATION_HOOK_GET_CALL: &str = r"
    SELECT
        indexer_health_notification_hook_public_id,
        channel::text,
        display_name,
        status_threshold::text,
        webhook_url,
        email,
        is_enabled,
        updated_at
    FROM indexer_health_notification_hook_get(
        actor_user_public_id => $1,
        indexer_health_notification_hook_public_id_input => $2
    )
";

/// Row returned for an indexer health notification hook.
#[derive(Debug, Clone, FromRow)]
pub struct IndexerHealthNotificationHookRow {
    /// Public identifier for the hook.
    pub indexer_health_notification_hook_public_id: Uuid,
    /// Channel type (`email` or `webhook`).
    pub channel: String,
    /// Operator-facing name.
    pub display_name: String,
    /// Lowest connectivity status that should trigger delivery.
    pub status_threshold: String,
    /// Webhook target URL when `channel=webhook`.
    pub webhook_url: Option<String>,
    /// Email target address when `channel=email`.
    pub email: Option<String>,
    /// Whether the hook is active.
    pub is_enabled: bool,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Input bundle for updating a health notification hook.
#[derive(Debug, Clone, Copy)]
pub struct IndexerHealthNotificationHookUpdateInput<'a> {
    /// Actor performing the update.
    pub actor_user_public_id: Uuid,
    /// Target hook public identifier.
    pub hook_public_id: Uuid,
    /// Updated display name when present.
    pub display_name: Option<&'a str>,
    /// Updated threshold when present.
    pub status_threshold: Option<&'a str>,
    /// Updated webhook URL when present.
    pub webhook_url: Option<&'a str>,
    /// Updated email when present.
    pub email: Option<&'a str>,
    /// Updated enabled state when present.
    pub is_enabled: Option<bool>,
}

/// Create a new health notification hook.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_health_notification_hook_create(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    channel: &str,
    display_name: &str,
    status_threshold: &str,
    webhook_url: Option<&str>,
    email: Option<&str>,
) -> Result<Uuid> {
    sqlx::query_scalar(HEALTH_NOTIFICATION_HOOK_CREATE_CALL)
        .bind(actor_user_public_id)
        .bind(channel)
        .bind(display_name)
        .bind(status_threshold)
        .bind(webhook_url)
        .bind(email)
        .fetch_one(pool)
        .await
        .map_err(try_op("indexer health notification hook create"))
}

/// Update an existing health notification hook.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_health_notification_hook_update(
    pool: &PgPool,
    input: &IndexerHealthNotificationHookUpdateInput<'_>,
) -> Result<Uuid> {
    sqlx::query_scalar(HEALTH_NOTIFICATION_HOOK_UPDATE_CALL)
        .bind(input.actor_user_public_id)
        .bind(input.hook_public_id)
        .bind(input.display_name)
        .bind(input.status_threshold)
        .bind(input.webhook_url)
        .bind(input.email)
        .bind(input.is_enabled)
        .fetch_one(pool)
        .await
        .map_err(try_op("indexer health notification hook update"))
}

/// Delete a health notification hook.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_health_notification_hook_delete(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    hook_public_id: Uuid,
) -> Result<()> {
    sqlx::query(HEALTH_NOTIFICATION_HOOK_DELETE_CALL)
        .bind(actor_user_public_id)
        .bind(hook_public_id)
        .execute(pool)
        .await
        .map_err(try_op("indexer health notification hook delete"))?;
    Ok(())
}

/// List configured health notification hooks.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_health_notification_hook_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
) -> Result<Vec<IndexerHealthNotificationHookRow>> {
    sqlx::query_as(HEALTH_NOTIFICATION_HOOK_LIST_CALL)
        .bind(actor_user_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("indexer health notification hook list"))
}

/// Fetch a single configured health notification hook.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_health_notification_hook_get(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    hook_public_id: Uuid,
) -> Result<IndexerHealthNotificationHookRow> {
    sqlx::query_as(HEALTH_NOTIFICATION_HOOK_GET_CALL)
        .bind(actor_user_public_id)
        .bind(hook_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("indexer health notification hook get"))
}

#[cfg(test)]
mod tests {
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
}
