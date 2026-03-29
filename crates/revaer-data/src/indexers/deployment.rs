//! Stored-procedure access for deployment initialization.
//!
//! # Design
//! - Encapsulates deployment bootstrap procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::PgPool;
use uuid::Uuid;

const DEPLOYMENT_INIT_CALL: &str = r"
    SELECT deployment_init(
        actor_user_public_id => $1
    )
";

/// Initialize deployment configuration and schedules.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn deployment_init(pool: &PgPool, actor_user_public_id: Uuid) -> Result<()> {
    sqlx::query(DEPLOYMENT_INIT_CALL)
        .bind(actor_user_public_id)
        .execute(pool)
        .await
        .map_err(try_op("deployment init"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }
    #[tokio::test]
    async fn deployment_init_accepts_system_user() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        deployment_init(pool, actor).await?;
        Ok(())
    }
}
