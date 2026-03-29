//! Stored-procedure access for indexer instance management.
//!
//! # Design
//! - Exposes typed wrappers around indexer instance stored procedures.
//! - Keeps SQL confined to stored-procedure invocations with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::PgPool;
use uuid::Uuid;

const INDEXER_INSTANCE_CREATE_CALL: &str = r"
    SELECT indexer_instance_create(
        actor_user_public_id => $1,
        indexer_definition_upstream_slug_input => $2,
        display_name_input => $3,
        priority_input => $4,
        trust_tier_key_input => $5::trust_tier_key,
        routing_policy_public_id_input => $6
    )
";

const INDEXER_INSTANCE_UPDATE_CALL: &str = r"
    SELECT indexer_instance_update(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        display_name_input => $3,
        priority_input => $4,
        trust_tier_key_input => $5::trust_tier_key,
        routing_policy_public_id_input => $6,
        is_enabled_input => $7,
        enable_rss_input => $8,
        enable_automatic_search_input => $9,
        enable_interactive_search_input => $10
    )
";

const RSS_SUBSCRIPTION_SET_CALL: &str = r"
    SELECT indexer_rss_subscription_set(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        is_enabled_input => $3,
        interval_seconds_input => $4
    )
";

const RSS_SUBSCRIPTION_DISABLE_CALL: &str = r"
    SELECT indexer_rss_subscription_disable(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2
    )
";

const INDEXER_INSTANCE_MEDIA_DOMAINS_CALL: &str = r"
    SELECT indexer_instance_set_media_domains(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        media_domain_keys_input => $3
    )
";

const INDEXER_INSTANCE_TAGS_CALL: &str = r"
    SELECT indexer_instance_set_tags(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        tag_public_ids_input => $3,
        tag_keys_input => $4
    )
";

const INDEXER_INSTANCE_FIELD_SET_CALL: &str = r"
    SELECT indexer_instance_field_set_value(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        field_name_input => $3,
        value_plain_input => $4,
        value_int_input => $5,
        value_decimal_input => $6::numeric,
        value_bool_input => $7
    )
";

const INDEXER_INSTANCE_FIELD_BIND_SECRET_CALL: &str = r"
    SELECT indexer_instance_field_bind_secret(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        field_name_input => $3,
        secret_public_id_input => $4
    )
";

/// Create a new indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_instance_create(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_definition_upstream_slug: &str,
    display_name: &str,
    priority: Option<i32>,
    trust_tier_key: Option<&str>,
    routing_policy_public_id: Option<Uuid>,
) -> Result<Uuid> {
    sqlx::query_scalar(INDEXER_INSTANCE_CREATE_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_definition_upstream_slug)
        .bind(display_name)
        .bind(priority)
        .bind(trust_tier_key)
        .bind(routing_policy_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("indexer instance create"))
}

/// Input payload for updating an indexer instance.
#[derive(Debug, Clone, Copy)]
pub struct IndexerInstanceUpdateInput<'a> {
    /// Actor user public id for audit.
    pub actor_user_public_id: Uuid,
    /// Indexer instance public id to update.
    pub indexer_instance_public_id: Uuid,
    /// Updated display name.
    pub display_name: Option<&'a str>,
    /// Updated priority.
    pub priority: Option<i32>,
    /// Updated trust tier key.
    pub trust_tier_key: Option<&'a str>,
    /// Updated routing policy public id.
    pub routing_policy_public_id: Option<Uuid>,
    /// Enable or disable the instance.
    pub is_enabled: Option<bool>,
    /// Enable or disable RSS.
    pub enable_rss: Option<bool>,
    /// Enable automatic search.
    pub enable_automatic_search: Option<bool>,
    /// Enable interactive search.
    pub enable_interactive_search: Option<bool>,
}

/// Update an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_instance_update(
    pool: &PgPool,
    input: &IndexerInstanceUpdateInput<'_>,
) -> Result<Uuid> {
    sqlx::query_scalar(INDEXER_INSTANCE_UPDATE_CALL)
        .bind(input.actor_user_public_id)
        .bind(input.indexer_instance_public_id)
        .bind(input.display_name)
        .bind(input.priority)
        .bind(input.trust_tier_key)
        .bind(input.routing_policy_public_id)
        .bind(input.is_enabled)
        .bind(input.enable_rss)
        .bind(input.enable_automatic_search)
        .bind(input.enable_interactive_search)
        .fetch_one(pool)
        .await
        .map_err(try_op("indexer instance update"))
}

/// Enable or update an RSS subscription for an indexer.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn rss_subscription_set(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    is_enabled: bool,
    interval_seconds: Option<i32>,
) -> Result<()> {
    sqlx::query(RSS_SUBSCRIPTION_SET_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .bind(is_enabled)
        .bind(interval_seconds)
        .execute(pool)
        .await
        .map_err(try_op("rss subscription set"))?;
    Ok(())
}

/// Disable an RSS subscription for an indexer.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn rss_subscription_disable(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
) -> Result<()> {
    sqlx::query(RSS_SUBSCRIPTION_DISABLE_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .execute(pool)
        .await
        .map_err(try_op("rss subscription disable"))?;
    Ok(())
}

/// Replace the media domain assignments for an indexer.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_instance_set_media_domains(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    media_domain_keys: &[String],
) -> Result<()> {
    sqlx::query(INDEXER_INSTANCE_MEDIA_DOMAINS_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .bind(media_domain_keys)
        .execute(pool)
        .await
        .map_err(try_op("indexer instance set media domains"))?;
    Ok(())
}

/// Replace the tag assignments for an indexer.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_instance_set_tags(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    tag_public_ids: Option<&[Uuid]>,
    tag_keys: Option<&[String]>,
) -> Result<()> {
    sqlx::query(INDEXER_INSTANCE_TAGS_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .bind(tag_public_ids)
        .bind(tag_keys)
        .execute(pool)
        .await
        .map_err(try_op("indexer instance set tags"))?;
    Ok(())
}

/// Input payload for setting an indexer field value.
#[derive(Debug, Clone, Copy)]
pub struct IndexerInstanceFieldValueInput<'a> {
    /// Actor user public id for audit.
    pub actor_user_public_id: Uuid,
    /// Indexer instance public id.
    pub indexer_instance_public_id: Uuid,
    /// Field name to update.
    pub field_name: &'a str,
    /// Optional plain text value.
    pub value_plain: Option<&'a str>,
    /// Optional integer value.
    pub value_int: Option<i32>,
    /// Optional decimal value stored as text.
    pub value_decimal: Option<&'a str>,
    /// Optional boolean value.
    pub value_bool: Option<bool>,
}

/// Set a field value on an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_instance_field_set_value(
    pool: &PgPool,
    input: &IndexerInstanceFieldValueInput<'_>,
) -> Result<()> {
    sqlx::query(INDEXER_INSTANCE_FIELD_SET_CALL)
        .bind(input.actor_user_public_id)
        .bind(input.indexer_instance_public_id)
        .bind(input.field_name)
        .bind(input.value_plain)
        .bind(input.value_int)
        .bind(input.value_decimal)
        .bind(input.value_bool)
        .execute(pool)
        .await
        .map_err(try_op("indexer instance field set"))?;
    Ok(())
}

/// Bind a secret to an indexer field.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn indexer_instance_field_bind_secret(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    field_name: &str,
    secret_public_id: Uuid,
) -> Result<()> {
    sqlx::query(INDEXER_INSTANCE_FIELD_BIND_SECRET_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .bind(field_name)
        .bind(secret_public_id)
        .execute(pool)
        .await
        .map_err(try_op("indexer instance field bind secret"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataError;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }
    #[tokio::test]
    async fn indexer_instance_create_requires_definition() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = indexer_instance_create(
            pool,
            actor,
            "missing-definition",
            "Example",
            Some(50),
            None,
            None,
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("definition_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn indexer_instance_update_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let input = IndexerInstanceUpdateInput {
            actor_user_public_id: actor,
            indexer_instance_public_id: Uuid::new_v4(),
            display_name: Some("Example"),
            priority: None,
            trust_tier_key: None,
            routing_policy_public_id: None,
            is_enabled: None,
            enable_rss: None,
            enable_automatic_search: None,
            enable_interactive_search: None,
        };
        let err = indexer_instance_update(pool, &input).await.unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn rss_subscription_set_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = rss_subscription_set(pool, actor, Uuid::new_v4(), true, Some(900))
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn rss_subscription_disable_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = rss_subscription_disable(pool, actor, Uuid::new_v4())
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn indexer_instance_field_set_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let input = IndexerInstanceFieldValueInput {
            actor_user_public_id: actor,
            indexer_instance_public_id: Uuid::new_v4(),
            field_name: "api_key",
            value_plain: Some("value"),
            value_int: None,
            value_decimal: None,
            value_bool: None,
        };
        let err = indexer_instance_field_set_value(pool, &input)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn indexer_instance_field_bind_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = indexer_instance_field_bind_secret(
            pool,
            actor,
            Uuid::new_v4(),
            "api_key",
            Uuid::new_v4(),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn indexer_instance_set_media_domains_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = indexer_instance_set_media_domains(
            pool,
            actor,
            Uuid::new_v4(),
            &["movies".to_string()],
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn indexer_instance_set_tags_requires_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = indexer_instance_set_tags(
            pool,
            actor,
            Uuid::new_v4(),
            None,
            Some(&["tag".to_string()]),
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }
}
