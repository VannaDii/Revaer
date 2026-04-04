//! Stored-procedure access for tag management.
//!
//! # Design
//! - Exposes typed wrappers around tag stored procedures.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const TAG_CREATE_CALL: &str = r"
    SELECT tag_create(
        actor_user_public_id => $1,
        tag_key_input => $2,
        display_name_input => $3
    )
";

const TAG_UPDATE_CALL: &str = r"
    SELECT tag_update(
        actor_user_public_id => $1,
        tag_public_id_input => $2,
        tag_key_input => $3,
        display_name_input => $4
    )
";

const TAG_SOFT_DELETE_CALL: &str = r"
    SELECT tag_soft_delete(
        actor_user_public_id => $1,
        tag_public_id_input => $2,
        tag_key_input => $3
    )
";

const TAG_LIST_CALL: &str = r"
    SELECT
        tag_public_id,
        tag_key,
        display_name,
        updated_at
    FROM tag_list(
        actor_user_public_id => $1
    )
";

/// Operator-visible tag row.
#[derive(Debug, Clone, FromRow)]
pub struct TagListRow {
    /// Stable public identifier for the tag.
    pub tag_public_id: Uuid,
    /// Tag key.
    pub tag_key: String,
    /// Human-readable label.
    pub display_name: String,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Create a new tag.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn tag_create(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    tag_key: &str,
    display_name: &str,
) -> Result<Uuid> {
    sqlx::query_scalar(TAG_CREATE_CALL)
        .bind(actor_user_public_id)
        .bind(tag_key)
        .bind(display_name)
        .fetch_one(pool)
        .await
        .map_err(try_op("tag create"))
}

/// Update an existing tag.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn tag_update(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    tag_public_id: Option<Uuid>,
    tag_key: Option<&str>,
    display_name: &str,
) -> Result<Uuid> {
    sqlx::query_scalar(TAG_UPDATE_CALL)
        .bind(actor_user_public_id)
        .bind(tag_public_id)
        .bind(tag_key)
        .bind(display_name)
        .fetch_one(pool)
        .await
        .map_err(try_op("tag update"))
}

/// Soft delete a tag.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn tag_soft_delete(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    tag_public_id: Option<Uuid>,
    tag_key: Option<&str>,
) -> Result<()> {
    sqlx::query(TAG_SOFT_DELETE_CALL)
        .bind(actor_user_public_id)
        .bind(tag_public_id)
        .bind(tag_key)
        .execute(pool)
        .await
        .map_err(try_op("tag soft delete"))?;
    Ok(())
}

/// List active tags for operator workflows.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn tag_list(pool: &PgPool, actor_user_public_id: Uuid) -> Result<Vec<TagListRow>> {
    sqlx::query_as(TAG_LIST_CALL)
        .bind(actor_user_public_id)
        .fetch_all(pool)
        .await
        .map_err(try_op("tag list"))
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
}
