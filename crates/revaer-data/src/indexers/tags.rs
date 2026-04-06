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
#[path = "../../tests/unit/indexers/tags_tests.rs"]
mod tests;
