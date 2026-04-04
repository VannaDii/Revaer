//! Stored-procedure access for tracker and media-domain category mappings.
//!
//! # Design
//! - Encapsulates mapping procedures behind typed wrappers.
//! - Keeps SQL confined to stored-procedure calls with named binds.
//! - Uses constant error messages for mapping database failures.

use crate::error::{Result, try_op};
use sqlx::PgPool;
use uuid::Uuid;

const TRACKER_CATEGORY_MAPPING_UPSERT_CALL: &str = r"
    SELECT tracker_category_mapping_upsert(
        actor_user_public_id => $1,
        torznab_instance_public_id_input => $2,
        indexer_definition_upstream_slug_input => $3,
        indexer_instance_public_id_input => $4,
        tracker_category_input => $5,
        tracker_subcategory_input => $6,
        torznab_cat_id_input => $7,
        media_domain_key_input => $8
    )
";

const TRACKER_CATEGORY_MAPPING_DELETE_CALL: &str = r"
    SELECT tracker_category_mapping_delete(
        actor_user_public_id => $1,
        torznab_instance_public_id_input => $2,
        indexer_definition_upstream_slug_input => $3,
        indexer_instance_public_id_input => $4,
        tracker_category_input => $5,
        tracker_subcategory_input => $6
    )
";

const TRACKER_CATEGORY_MAPPING_RESOLVE_FEED_CALL: &str = r"
    SELECT torznab_cat_id
    FROM tracker_category_mapping_resolve_feed(
        torznab_instance_public_id_input => $1,
        indexer_instance_public_id_input => $2,
        tracker_category_input => $3,
        tracker_subcategory_input => $4
    )
";

const MEDIA_DOMAIN_MAPPING_UPSERT_CALL: &str = r"
    SELECT media_domain_to_torznab_category_upsert(
        actor_user_public_id => $1,
        media_domain_key_input => $2,
        torznab_cat_id_input => $3,
        is_primary_input => $4
    )
";

const MEDIA_DOMAIN_MAPPING_DELETE_CALL: &str = r"
    SELECT media_domain_to_torznab_category_delete(
        actor_user_public_id => $1,
        media_domain_key_input => $2,
        torznab_cat_id_input => $3
    )
";

/// Tracker category mapping upsert arguments.
#[derive(Clone, Copy, Debug)]
pub struct TrackerCategoryMappingUpsertInput<'a> {
    /// Optional Torznab instance scope.
    pub torznab_instance_public_id: Option<Uuid>,
    /// Optional definition scope.
    pub indexer_definition_upstream_slug: Option<&'a str>,
    /// Optional instance scope.
    pub indexer_instance_public_id: Option<Uuid>,
    /// Tracker category identifier.
    pub tracker_category: i32,
    /// Optional tracker subcategory identifier.
    pub tracker_subcategory: Option<i32>,
    /// Torznab category identifier.
    pub torznab_cat_id: i32,
    /// Optional media-domain filter.
    pub media_domain_key: Option<&'a str>,
}

/// Tracker category mapping delete arguments.
#[derive(Clone, Copy, Debug)]
pub struct TrackerCategoryMappingDeleteInput<'a> {
    /// Optional Torznab instance scope.
    pub torznab_instance_public_id: Option<Uuid>,
    /// Optional definition scope.
    pub indexer_definition_upstream_slug: Option<&'a str>,
    /// Optional instance scope.
    pub indexer_instance_public_id: Option<Uuid>,
    /// Tracker category identifier.
    pub tracker_category: i32,
    /// Optional tracker subcategory identifier.
    pub tracker_subcategory: Option<i32>,
}

/// Upsert a tracker category mapping.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn tracker_category_mapping_upsert(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    input: TrackerCategoryMappingUpsertInput<'_>,
) -> Result<()> {
    sqlx::query(TRACKER_CATEGORY_MAPPING_UPSERT_CALL)
        .bind(actor_user_public_id)
        .bind(input.torznab_instance_public_id)
        .bind(input.indexer_definition_upstream_slug)
        .bind(input.indexer_instance_public_id)
        .bind(input.tracker_category)
        .bind(input.tracker_subcategory)
        .bind(input.torznab_cat_id)
        .bind(input.media_domain_key)
        .execute(pool)
        .await
        .map_err(try_op("tracker category mapping upsert"))?;
    Ok(())
}

/// Delete a tracker category mapping.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn tracker_category_mapping_delete(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    input: TrackerCategoryMappingDeleteInput<'_>,
) -> Result<()> {
    sqlx::query(TRACKER_CATEGORY_MAPPING_DELETE_CALL)
        .bind(actor_user_public_id)
        .bind(input.torznab_instance_public_id)
        .bind(input.indexer_definition_upstream_slug)
        .bind(input.indexer_instance_public_id)
        .bind(input.tracker_category)
        .bind(input.tracker_subcategory)
        .execute(pool)
        .await
        .map_err(try_op("tracker category mapping delete"))?;
    Ok(())
}

/// Resolve feed category ids for a Torznab instance and source context.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn tracker_category_mapping_resolve_feed(
    pool: &PgPool,
    torznab_instance_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    tracker_category: Option<i32>,
    tracker_subcategory: Option<i32>,
) -> Result<Vec<i32>> {
    sqlx::query_scalar(TRACKER_CATEGORY_MAPPING_RESOLVE_FEED_CALL)
        .bind(torznab_instance_public_id)
        .bind(indexer_instance_public_id)
        .bind(tracker_category)
        .bind(tracker_subcategory)
        .fetch_all(pool)
        .await
        .map_err(try_op("tracker category mapping resolve feed"))
}

/// Upsert a media-domain-to-torznab-category mapping.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn media_domain_mapping_upsert(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    media_domain_key: &str,
    torznab_cat_id: i32,
    is_primary: Option<bool>,
) -> Result<()> {
    sqlx::query(MEDIA_DOMAIN_MAPPING_UPSERT_CALL)
        .bind(actor_user_public_id)
        .bind(media_domain_key)
        .bind(torznab_cat_id)
        .bind(is_primary)
        .execute(pool)
        .await
        .map_err(try_op("media domain mapping upsert"))?;
    Ok(())
}

/// Delete a media-domain-to-torznab-category mapping.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn media_domain_mapping_delete(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    media_domain_key: &str,
    torznab_cat_id: i32,
) -> Result<()> {
    sqlx::query(MEDIA_DOMAIN_MAPPING_DELETE_CALL)
        .bind(actor_user_public_id)
        .bind(media_domain_key)
        .bind(torznab_cat_id)
        .execute(pool)
        .await
        .map_err(try_op("media domain mapping delete"))?;
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
    async fn tracker_category_mapping_upsert_requires_torznab_category() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = tracker_category_mapping_upsert(
            pool,
            actor,
            TrackerCategoryMappingUpsertInput {
                torznab_instance_public_id: None,
                indexer_definition_upstream_slug: None,
                indexer_instance_public_id: None,
                tracker_category: 9000,
                tracker_subcategory: Some(0),
                torznab_cat_id: 9999,
                media_domain_key: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("torznab_category_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn tracker_category_mapping_delete_requires_mapping() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = tracker_category_mapping_delete(
            pool,
            actor,
            TrackerCategoryMappingDeleteInput {
                torznab_instance_public_id: None,
                indexer_definition_upstream_slug: None,
                indexer_instance_public_id: None,
                tracker_category: 9000,
                tracker_subcategory: Some(0),
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("mapping_not_found"));
        Ok(())
    }

    #[tokio::test]
    async fn tracker_category_mapping_upsert_rejects_missing_instance() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;

        let err = tracker_category_mapping_upsert(
            pool,
            actor,
            TrackerCategoryMappingUpsertInput {
                torznab_instance_public_id: None,
                indexer_definition_upstream_slug: None,
                indexer_instance_public_id: Some(Uuid::nil()),
                tracker_category: 9000,
                tracker_subcategory: Some(0),
                torznab_cat_id: 2000,
                media_domain_key: None,
            },
        )
        .await
        .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("indexer_instance_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn media_domain_mapping_upsert_requires_category() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = media_domain_mapping_upsert(pool, actor, "movies", 9999, None)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("torznab_category_not_found"));
        Ok(())
    }
    #[tokio::test]
    async fn media_domain_mapping_delete_requires_mapping() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();

        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let err = media_domain_mapping_delete(pool, actor, "movies", 9999)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("torznab_category_not_found"));
        Ok(())
    }

    #[tokio::test]
    async fn media_domain_mapping_upsert_rejects_invalid_key() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;

        let err = media_domain_mapping_upsert(pool, actor, "Movies", 2000, None)
            .await
            .unwrap_err();
        assert!(matches!(err, DataError::QueryFailed { .. }));
        assert_eq!(err.database_detail(), Some("media_domain_key_invalid"));
        Ok(())
    }

    #[tokio::test]
    async fn media_domain_mapping_upsert_switches_primary_mapping() -> anyhow::Result<()> {
        let Ok(test_db) = setup_db().await else {
            return Ok(());
        };

        let pool = test_db.pool();
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;

        media_domain_mapping_upsert(pool, actor, "movies", 8000, Some(true)).await?;

        let primary_count: i64 = sqlx::query_scalar(
            "SELECT count(*)\n             FROM media_domain_to_torznab_category mdtc\n             JOIN media_domain md ON md.media_domain_id = mdtc.media_domain_id\n             WHERE md.media_domain_key::TEXT = $1 AND mdtc.is_primary = TRUE",
        )
        .bind("movies")
        .fetch_one(pool)
        .await?;
        assert_eq!(primary_count, 1);

        let is_primary: bool = sqlx::query_scalar(
            "SELECT mdtc.is_primary\n             FROM media_domain_to_torznab_category mdtc\n             JOIN media_domain md ON md.media_domain_id = mdtc.media_domain_id\n             JOIN torznab_category tc ON tc.torznab_category_id = mdtc.torznab_category_id\n             WHERE md.media_domain_key::TEXT = $1 AND tc.torznab_cat_id = $2",
        )
        .bind("movies")
        .bind(8000_i32)
        .fetch_one(pool)
        .await?;
        assert!(is_primary);

        Ok(())
    }
}
