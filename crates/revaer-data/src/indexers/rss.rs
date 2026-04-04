//! Stored-procedure access for RSS management.
//!
//! # Design
//! - Encapsulates operator-facing RSS status and seen-item helpers behind stored procedures.
//! - Keeps SQL confined to stored-procedure calls with explicit parameter binding.
//! - Returns typed rows so API/UI layers can stay free of database details.

use crate::error::{Result, try_op};
use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

const RSS_SUBSCRIPTION_GET_CALL: &str = r"
    SELECT
        indexer_instance_public_id,
        CASE
            WHEN instance_is_enabled THEN 'enabled'
            ELSE 'disabled'
        END AS instance_status,
        CASE
            WHEN instance_enable_rss THEN 'enabled'
            ELSE 'disabled'
        END AS rss_status,
        subscription_exists,
        subscription_is_enabled,
        interval_seconds,
        last_polled_at,
        next_poll_at,
        backoff_seconds,
        last_error_class
    FROM indexer_rss_subscription_get(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2
    )
";

const RSS_ITEM_SEEN_LIST_CALL: &str = r"
    SELECT
        item_guid,
        infohash_v1,
        infohash_v2,
        magnet_hash,
        first_seen_at
    FROM indexer_rss_item_seen_list(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        limit_input => $3
    )
";

const RSS_ITEM_SEEN_MARK_CALL: &str = r"
    SELECT
        item_guid,
        infohash_v1,
        infohash_v2,
        magnet_hash,
        first_seen_at,
        inserted
    FROM indexer_rss_item_seen_mark(
        actor_user_public_id => $1,
        indexer_instance_public_id_input => $2,
        item_guid_input => $3,
        infohash_v1_input => $4,
        infohash_v2_input => $5,
        magnet_hash_input => $6
    )
";

/// RSS subscription snapshot for one indexer instance.
#[derive(Debug, Clone, FromRow)]
pub struct RssSubscriptionRow {
    /// Indexer instance public identifier.
    pub indexer_instance_public_id: Uuid,
    /// Whether the instance itself is enabled.
    pub instance_status: String,
    /// Whether RSS is enabled on the instance configuration.
    pub rss_status: String,
    /// Whether a subscription row exists.
    pub subscription_exists: bool,
    /// Whether the subscription is enabled.
    pub subscription_is_enabled: bool,
    /// Poll interval in seconds.
    pub interval_seconds: i32,
    /// Last successful poll timestamp.
    pub last_polled_at: Option<DateTime<Utc>>,
    /// Next scheduled poll timestamp.
    pub next_poll_at: Option<DateTime<Utc>>,
    /// Backoff seconds currently applied.
    pub backoff_seconds: Option<i32>,
    /// Last RSS error class.
    pub last_error_class: Option<String>,
}

/// RSS item seen row for operator review.
#[derive(Debug, Clone, FromRow)]
pub struct RssSeenItemRow {
    /// Normalized item GUID, when present.
    pub item_guid: Option<String>,
    /// Infohash v1, when present.
    pub infohash_v1: Option<String>,
    /// Infohash v2, when present.
    pub infohash_v2: Option<String>,
    /// Magnet hash, when present.
    pub magnet_hash: Option<String>,
    /// First seen timestamp.
    pub first_seen_at: DateTime<Utc>,
}

/// Result row for a manual mark-seen action.
#[derive(Debug, Clone, FromRow)]
pub struct RssSeenMarkRow {
    /// Normalized item GUID, when present.
    pub item_guid: Option<String>,
    /// Infohash v1, when present.
    pub infohash_v1: Option<String>,
    /// Infohash v2, when present.
    pub infohash_v2: Option<String>,
    /// Magnet hash, when present.
    pub magnet_hash: Option<String>,
    /// First seen timestamp for the dedupe row.
    pub first_seen_at: DateTime<Utc>,
    /// Whether a new row was inserted.
    pub inserted: bool,
}

/// Manual mark-seen payload.
#[derive(Debug, Clone, Copy)]
pub struct RssSeenMarkInput<'a> {
    /// Actor user public id for authorization and audit.
    pub actor_user_public_id: Uuid,
    /// Indexer instance public id.
    pub indexer_instance_public_id: Uuid,
    /// Optional feed GUID or stable item identifier.
    pub item_guid: Option<&'a str>,
    /// Optional v1 infohash.
    pub infohash_v1: Option<&'a str>,
    /// Optional v2 infohash.
    pub infohash_v2: Option<&'a str>,
    /// Optional magnet hash.
    pub magnet_hash: Option<&'a str>,
}

/// Fetch RSS subscription status for an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn rss_subscription_get(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
) -> Result<RssSubscriptionRow> {
    sqlx::query_as(RSS_SUBSCRIPTION_GET_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .fetch_one(pool)
        .await
        .map_err(try_op("rss subscription get"))
}

/// List recent RSS items seen for an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn rss_item_seen_list(
    pool: &PgPool,
    actor_user_public_id: Uuid,
    indexer_instance_public_id: Uuid,
    limit: Option<i32>,
) -> Result<Vec<RssSeenItemRow>> {
    sqlx::query_as(RSS_ITEM_SEEN_LIST_CALL)
        .bind(actor_user_public_id)
        .bind(indexer_instance_public_id)
        .bind(limit)
        .fetch_all(pool)
        .await
        .map_err(try_op("rss item seen list"))
}

/// Mark an RSS item as seen for an indexer instance.
///
/// # Errors
///
/// Returns an error if the stored procedure rejects the input.
pub async fn rss_item_seen_mark(
    pool: &PgPool,
    input: &RssSeenMarkInput<'_>,
) -> Result<RssSeenMarkRow> {
    sqlx::query_as(RSS_ITEM_SEEN_MARK_CALL)
        .bind(input.actor_user_public_id)
        .bind(input.indexer_instance_public_id)
        .bind(input.item_guid)
        .bind(input.infohash_v1)
        .bind(input.infohash_v2)
        .bind(input.magnet_hash)
        .fetch_one(pool)
        .await
        .map_err(try_op("rss item seen mark"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataError;

    const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";

    async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
        crate::indexers::setup_indexer_db("rss tests").await
    }

    async fn create_indexer(pool: &PgPool, actor: Uuid) -> anyhow::Result<Uuid> {
        let upstream_slug = format!("rss-test-{}", Uuid::new_v4().simple());
        sqlx::query(
            "INSERT INTO indexer_definition (
                upstream_source,
                upstream_slug,
                display_name,
                protocol,
                engine,
                schema_version,
                definition_hash,
                is_deprecated
            )
            VALUES ($1::upstream_source, $2, $3, $4::protocol, $5::engine, $6, $7, $8)",
        )
        .bind("prowlarr_indexers")
        .bind(&upstream_slug)
        .bind("RSS Test Definition")
        .bind("torrent")
        .bind("torznab")
        .bind(1_i32)
        .bind("a".repeat(64))
        .bind(false)
        .execute(pool)
        .await?;

        crate::indexers::instances::indexer_instance_create(
            pool,
            actor,
            &upstream_slug,
            &format!("RSS Test {}", Uuid::new_v4().simple()),
            Some(50),
            Some("public"),
            None,
        )
        .await
        .map_err(anyhow::Error::from)
    }

    #[tokio::test]
    async fn rss_subscription_get_returns_status_for_instance() -> anyhow::Result<()> {
        let Ok(db) = setup_db().await else {
            return Ok(());
        };
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let indexer_instance_public_id = create_indexer(db.pool(), actor).await?;

        let row = rss_subscription_get(db.pool(), actor, indexer_instance_public_id).await?;

        assert_eq!(row.indexer_instance_public_id, indexer_instance_public_id);
        assert_eq!(row.instance_status, "enabled");
        assert_eq!(row.rss_status, "enabled");
        assert!(row.subscription_exists);
        assert!(row.subscription_is_enabled);
        assert_eq!(row.interval_seconds, 900);
        assert!(row.next_poll_at.is_some());
        Ok(())
    }

    #[tokio::test]
    async fn rss_item_seen_list_returns_recent_rows() -> anyhow::Result<()> {
        let Ok(db) = setup_db().await else {
            return Ok(());
        };
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let indexer_instance_public_id = create_indexer(db.pool(), actor).await?;

        let marked = rss_item_seen_mark(
            db.pool(),
            &RssSeenMarkInput {
                actor_user_public_id: actor,
                indexer_instance_public_id,
                item_guid: Some("GUID-123"),
                infohash_v1: None,
                infohash_v2: None,
                magnet_hash: None,
            },
        )
        .await?;

        let items =
            rss_item_seen_list(db.pool(), actor, indexer_instance_public_id, Some(10)).await?;

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item_guid.as_deref(), Some("guid-123"));
        assert_eq!(items[0].first_seen_at, marked.first_seen_at);
        Ok(())
    }

    #[tokio::test]
    async fn rss_subscription_get_requires_instance() -> anyhow::Result<()> {
        let Ok(db) = setup_db().await else {
            return Ok(());
        };
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;

        let err = rss_subscription_get(db.pool(), actor, Uuid::new_v4())
            .await
            .expect_err("missing instance");

        assert_eq!(err.database_detail(), Some("indexer_not_found"));
        Ok(())
    }

    #[tokio::test]
    async fn rss_item_seen_mark_is_idempotent() -> anyhow::Result<()> {
        let Ok(db) = setup_db().await else {
            return Ok(());
        };
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let indexer_instance_public_id = create_indexer(db.pool(), actor).await?;

        let first = rss_item_seen_mark(
            db.pool(),
            &RssSeenMarkInput {
                actor_user_public_id: actor,
                indexer_instance_public_id,
                item_guid: None,
                infohash_v1: Some("0123456789abcdef0123456789abcdef01234567"),
                infohash_v2: None,
                magnet_hash: None,
            },
        )
        .await?;
        let second = rss_item_seen_mark(
            db.pool(),
            &RssSeenMarkInput {
                actor_user_public_id: actor,
                indexer_instance_public_id,
                item_guid: None,
                infohash_v1: Some("0123456789ABCDEF0123456789ABCDEF01234567"),
                infohash_v2: None,
                magnet_hash: None,
            },
        )
        .await?;

        assert!(first.inserted);
        assert!(!second.inserted);
        assert_eq!(first.first_seen_at, second.first_seen_at);
        Ok(())
    }

    #[tokio::test]
    async fn rss_item_seen_mark_requires_identifier() -> anyhow::Result<()> {
        let Ok(db) = setup_db().await else {
            return Ok(());
        };
        let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
        let indexer_instance_public_id = create_indexer(db.pool(), actor).await?;

        let err = rss_item_seen_mark(
            db.pool(),
            &RssSeenMarkInput {
                actor_user_public_id: actor,
                indexer_instance_public_id,
                item_guid: None,
                infohash_v1: None,
                infohash_v2: None,
                magnet_hash: None,
            },
        )
        .await
        .expect_err("missing identifiers");

        assert_eq!(
            match err {
                DataError::QueryFailed { .. } => err.database_detail(),
                _ => None,
            },
            Some("rss_item_identifier_missing")
        );
        Ok(())
    }
}
