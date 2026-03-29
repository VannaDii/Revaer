//! Stored-procedure access for normalization helpers.
//!
//! # Design
//! - Encapsulates normalization helpers behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::PgPool;

const NORMALIZE_TITLE_CALL: &str = r"
    SELECT normalize_title(
        title_raw => $1
    )
";

const NORMALIZE_MAGNET_CALL: &str = r"
    SELECT normalize_magnet_uri(
        raw_uri => $1
    )
";

const DERIVE_MAGNET_HASH_CALL: &str = r"
    SELECT derive_magnet_hash(
        infohash_v2_input => $1,
        infohash_v1_input => $2,
        magnet_uri_input => $3
    )
";

const COMPUTE_TITLE_SIZE_HASH_CALL: &str = r"
    SELECT compute_title_size_hash(
        title_normalized_input => $1,
        size_bytes_input => $2
    )
";

/// Normalize a raw title into its canonical match string.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn normalize_title(pool: &PgPool, title_raw: &str) -> Result<String> {
    sqlx::query_scalar(NORMALIZE_TITLE_CALL)
        .bind(title_raw)
        .fetch_one(pool)
        .await
        .map_err(try_op("normalize title"))
}

/// Normalize a magnet URI into its canonical representation.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn normalize_magnet_uri(pool: &PgPool, raw_uri: &str) -> Result<String> {
    sqlx::query_scalar(NORMALIZE_MAGNET_CALL)
        .bind(raw_uri)
        .fetch_one(pool)
        .await
        .map_err(try_op("normalize magnet uri"))
}

/// Derive a magnet hash from available inputs.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn derive_magnet_hash(
    pool: &PgPool,
    infohash_v2: Option<&str>,
    infohash_v1: Option<&str>,
    magnet_uri: Option<&str>,
) -> Result<Option<String>> {
    sqlx::query_scalar(DERIVE_MAGNET_HASH_CALL)
        .bind(infohash_v2)
        .bind(infohash_v1)
        .bind(magnet_uri)
        .fetch_one(pool)
        .await
        .map_err(try_op("derive magnet hash"))
}

/// Compute a title/size hash used for fallback identity matching.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn compute_title_size_hash(
    pool: &PgPool,
    title_normalized: Option<&str>,
    size_bytes: Option<i64>,
) -> Result<Option<String>> {
    sqlx::query_scalar(COMPUTE_TITLE_SIZE_HASH_CALL)
        .bind(title_normalized)
        .bind(size_bytes)
        .fetch_one(pool)
        .await
        .map_err(try_op("compute title size hash"))
}

#[cfg(test)]
mod tests {
    use super::*;
    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("indexer tests").await
    }
    #[tokio::test]
    async fn normalization_helpers_roundtrip() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let normalized = normalize_title(pool, "Hello.World.1080p").await?;
        assert_eq!(normalized, "hello world");

        let magnet = normalize_magnet_uri(pool, "magnet:?xt=urn:btih:abcdef&dn=Test").await?;
        assert!(magnet.starts_with("magnet:?"));

        let infohash = "a".repeat(64);
        let hash = derive_magnet_hash(pool, Some(infohash.as_str()), None, None).await?;
        let hash = hash.expect("expected hash");
        assert_eq!(hash.len(), 64);

        let title_hash = compute_title_size_hash(pool, Some("hello"), Some(123)).await?;
        let title_hash = title_hash.expect("expected title hash");
        assert_eq!(title_hash.len(), 64);

        Ok(())
    }

    #[tokio::test]
    async fn derive_magnet_hash_prefers_infohash_v2() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let infohash_v2 = "a".repeat(64);
        let infohash_v1 = "b".repeat(40);

        let hash_v2 = derive_magnet_hash(pool, Some(infohash_v2.as_str()), None, None).await?;
        let hash_v1 = derive_magnet_hash(pool, None, Some(infohash_v1.as_str()), None).await?;
        let hash_both = derive_magnet_hash(
            pool,
            Some(infohash_v2.as_str()),
            Some(infohash_v1.as_str()),
            Some("magnet:?dn=Test&xt=urn:btih:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        )
        .await?;

        assert_eq!(hash_both, hash_v2);
        assert_ne!(hash_v2, hash_v1);
        Ok(())
    }

    #[tokio::test]
    async fn derive_magnet_hash_normalizes_magnet_uri() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let raw = "magnet:?dn=Test&tr=udp://tracker.invalid/announce&xt=urn:btih:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
        let normalized = normalize_magnet_uri(pool, raw).await?;

        let hash_raw = derive_magnet_hash(pool, None, None, Some(raw)).await?;
        let hash_normalized =
            derive_magnet_hash(pool, None, None, Some(normalized.as_str())).await?;

        assert_eq!(hash_raw, hash_normalized);
        Ok(())
    }

    #[tokio::test]
    async fn derive_magnet_hash_returns_none_without_inputs() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let hash = derive_magnet_hash(pool, None, None, None).await?;
        assert!(hash.is_none());
        Ok(())
    }
}
