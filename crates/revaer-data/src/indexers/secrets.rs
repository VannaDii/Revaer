//! Stored-procedure access for secret management.
//!
//! # Design
//! - Encapsulates secret lifecycle procedures behind typed Rust functions.
//! - Delegates validation to stored procedures to keep logic centralized.
//! - Uses constant operation labels for error mapping.

use crate::error::{Result, try_op};
use sqlx::{PgPool, Row};
use uuid::Uuid;

const SECRET_CREATE_CALL: &str = r"
    SELECT secret_create(
        actor_user_public_id => $1,
        secret_type_input => $2::secret_type,
        plaintext_value_input => $3
    )
";

const SECRET_ROTATE_CALL: &str = r"
    SELECT secret_rotate(
        actor_user_public_id => $1,
        secret_public_id_input => $2,
        plaintext_value_input => $3
    )
";

const SECRET_REVOKE_CALL: &str = r"
    SELECT secret_revoke(
        actor_user_public_id => $1,
        secret_public_id_input => $2
    )
";

const SECRET_READ_CALL: &str = r"
    SELECT
        secret_type::text AS secret_type,
        cipher_text,
        key_id
    FROM secret_read(
        actor_user_public_id => $1,
        secret_public_id_input => $2
    )
";

/// Encrypted secret payload returned by the database.
#[derive(Debug, Clone)]
pub struct SecretCipherRow {
    /// Secret type label.
    pub secret_type: String,
    /// Ciphertext bytes stored in the database.
    pub cipher_text: Vec<u8>,
    /// Key identifier associated with the cipher.
    pub key_id: String,
}

/// Create a new encrypted secret.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn secret_create(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    secret_type: &str,
    plaintext_value: &str,
) -> Result<Uuid> {
    sqlx::query_scalar(SECRET_CREATE_CALL)
        .bind(actor_user_public_id)
        .bind(secret_type)
        .bind(plaintext_value)
        .fetch_one(pool)
        .await
        .map_err(try_op("secret create"))
}

/// Rotate an existing secret.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn secret_rotate(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    secret_public_id: Uuid,
    plaintext_value: &str,
) -> Result<Uuid> {
    sqlx::query_scalar(SECRET_ROTATE_CALL)
        .bind(actor_user_public_id)
        .bind(secret_public_id)
        .bind(plaintext_value)
        .fetch_one(pool)
        .await
        .map_err(try_op("secret rotate"))
}

/// Revoke an existing secret.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn secret_revoke(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    secret_public_id: Uuid,
) -> Result<()> {
    sqlx::query(SECRET_REVOKE_CALL)
        .bind(actor_user_public_id)
        .bind(secret_public_id)
        .execute(pool)
        .await
        .map_err(try_op("secret revoke"))?;
    Ok(())
}

/// Read encrypted secret payload data.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn secret_read(
    pool: &PgPool,
    actor_user_public_id: Option<Uuid>,
    secret_public_id: Uuid,
) -> Result<SecretCipherRow> {
    let row = sqlx::query(SECRET_READ_CALL)
        .bind(actor_user_public_id)
        .bind(secret_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("secret read"))?;

    Ok(SecretCipherRow {
        secret_type: row.try_get("secret_type").map_err(try_op("secret read"))?,
        cipher_text: row.try_get("cipher_text").map_err(try_op("secret read"))?,
        key_id: row.try_get("key_id").map_err(try_op("secret read"))?,
    })
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
    async fn secret_create_rotate_revoke_roundtrip() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let secret_id = secret_create(pool, actor, "api_key", "first-value").await?;

        let read_row = secret_read(pool, Some(actor), secret_id).await?;
        assert_eq!(read_row.secret_type, "api_key");
        assert_eq!(read_row.key_id, "test-key");
        assert!(!read_row.cipher_text.is_empty());

        let rotated = secret_rotate(pool, actor, secret_id, "second-value").await?;
        assert_eq!(rotated, secret_id);

        let read_again = secret_read(pool, Some(actor), secret_id).await?;
        assert_eq!(read_again.secret_type, "api_key");
        assert_eq!(read_again.key_id, "test-key");
        assert!(!read_again.cipher_text.is_empty());

        secret_revoke(pool, actor, secret_id).await?;

        let revoked = secret_read(pool, Some(actor), secret_id).await;
        let err = revoked.unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("secret_revoked"));
        Ok(())
    }
    #[tokio::test]
    async fn secret_read_allows_system_actor_none() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let secret_id = secret_create(pool, actor, "api_key", "value").await?;

        let read_row = secret_read(pool, None, secret_id).await?;
        assert_eq!(read_row.secret_type, "api_key");
        Ok(())
    }
}
