//! Stored-procedure access for `app_user` management.
//!
//! # Design
//! - Encapsulates `app_user` procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::PgPool;
use uuid::Uuid;

const APP_USER_CREATE_CALL: &str = r"
    SELECT app_user_create(
        email_input => $1,
        display_name_input => $2
    )
";

const APP_USER_UPDATE_CALL: &str = r"
    SELECT app_user_update(
        user_public_id_input => $1,
        display_name_input => $2
    )
";

const APP_USER_VERIFY_CALL: &str = r"
    SELECT app_user_verify_email(
        user_public_id_input => $1
    )
";

/// Create a new app user.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn app_user_create(pool: &PgPool, email: &str, display_name: &str) -> Result<Uuid> {
    sqlx::query_scalar(APP_USER_CREATE_CALL)
        .bind(email)
        .bind(display_name)
        .fetch_one(pool)
        .await
        .map_err(try_op("app user create"))
}

/// Update an app user's display name.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn app_user_update(
    pool: &PgPool,
    user_public_id: Uuid,
    display_name: &str,
) -> Result<()> {
    sqlx::query(APP_USER_UPDATE_CALL)
        .bind(user_public_id)
        .bind(display_name)
        .execute(pool)
        .await
        .map_err(try_op("app user update"))?;
    Ok(())
}

/// Mark an app user as verified.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn app_user_verify_email(pool: &PgPool, user_public_id: Uuid) -> Result<()> {
    sqlx::query(APP_USER_VERIFY_CALL)
        .bind(user_public_id)
        .execute(pool)
        .await
        .map_err(try_op("app user verify email"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }
    #[tokio::test]
    async fn app_user_create_update_verify_roundtrip() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let email = format!("user-{}@example.test", Uuid::new_v4());
        let user_public_id = app_user_create(pool, &email, "User One").await?;

        app_user_update(pool, user_public_id, "User Updated").await?;
        app_user_verify_email(pool, user_public_id).await?;

        Ok(())
    }
}
