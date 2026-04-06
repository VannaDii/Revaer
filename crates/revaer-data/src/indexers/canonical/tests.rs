use super::*;
use crate::DataError;
use chrono::{Duration, Utc};
use sqlx::PgPool;

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}

async fn insert_indexer_instance(pool: &PgPool) -> anyhow::Result<i64> {
    let definition_id: i64 = sqlx::query_scalar(
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
            VALUES ($1::upstream_source, $2, $3, $4::protocol, $5::engine, $6, $7, $8)
            RETURNING indexer_definition_id",
    )
    .bind("prowlarr_indexers")
    .bind(format!("canonical-prune-{}", Uuid::new_v4().simple()))
    .bind("Canonical Prune Definition")
    .bind("torrent")
    .bind("torznab")
    .bind(1_i32)
    .bind("d".repeat(64))
    .bind(false)
    .fetch_one(pool)
    .await?;

    sqlx::query_scalar(
        "INSERT INTO indexer_instance (
                indexer_instance_public_id,
                indexer_definition_id,
                display_name,
                is_enabled,
                migration_state,
                enable_rss,
                enable_automatic_search,
                enable_interactive_search,
                priority,
                trust_tier_key,
                created_by_user_id,
                updated_by_user_id
            )
            VALUES (
                $1,
                $2,
                $3,
                TRUE,
                $4::indexer_instance_migration_state,
                TRUE,
                TRUE,
                TRUE,
                100,
                $5::trust_tier_key,
                0,
                0
            )
            RETURNING indexer_instance_id",
    )
    .bind(Uuid::new_v4())
    .bind(definition_id)
    .bind("Canonical Prune Instance")
    .bind("ready")
    .bind("public")
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn insert_canonical_fallback_candidate(
    pool: &PgPool,
    created_at: chrono::DateTime<Utc>,
) -> anyhow::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO canonical_torrent (
                canonical_torrent_public_id,
                identity_confidence,
                identity_strategy,
                title_size_hash,
                title_display,
                title_normalized,
                size_bytes,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3::identity_strategy, $4, $5, $6, $7, $8, $9)
            RETURNING canonical_torrent_id",
    )
    .bind(Uuid::new_v4())
    .bind(0.6_f64)
    .bind("title_size_fallback")
    .bind(format!("{:064x}", Uuid::new_v4().as_u128()))
    .bind("Prune Candidate")
    .bind("prune candidate")
    .bind(2_048_i64)
    .bind(created_at)
    .bind(created_at)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn insert_canonical_non_candidate(
    pool: &PgPool,
    created_at: chrono::DateTime<Utc>,
    infohash_v1: &str,
) -> anyhow::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO canonical_torrent (
                canonical_torrent_public_id,
                identity_confidence,
                identity_strategy,
                infohash_v1,
                title_display,
                title_normalized,
                size_bytes,
                created_at,
                updated_at
            )
            VALUES ($1, $2, $3::identity_strategy, $4, $5, $6, $7, $8, $9)
            RETURNING canonical_torrent_id",
    )
    .bind(Uuid::new_v4())
    .bind(1.0_f64)
    .bind("infohash_v1")
    .bind(infohash_v1)
    .bind("Non Candidate")
    .bind("non candidate")
    .bind(2_048_i64)
    .bind(created_at)
    .bind(created_at)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn insert_source(
    pool: &PgPool,
    indexer_instance_id: i64,
    infohash_v1: &str,
) -> anyhow::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO canonical_torrent_source (
                indexer_instance_id,
                canonical_torrent_source_public_id,
                source_guid,
                infohash_v1,
                title_normalized,
                size_bytes,
                last_seen_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, now())
            RETURNING canonical_torrent_source_id",
    )
    .bind(indexer_instance_id)
    .bind(Uuid::new_v4())
    .bind(format!("guid-{}", Uuid::new_v4().simple()))
    .bind(infohash_v1)
    .bind("prune candidate")
    .bind(2_048_i64)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn insert_base_score_link(
    pool: &PgPool,
    canonical_torrent_id: i64,
    source_id: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO canonical_torrent_source_base_score (
                canonical_torrent_id,
                canonical_torrent_source_id,
                score_total_base,
                score_seed,
                score_leech,
                score_age,
                score_trust,
                score_health,
                score_reputation
            )
            VALUES ($1, $2, 0, 0, 0, 0, 0, 0, 0)",
    )
    .bind(canonical_torrent_id)
    .bind(source_id)
    .execute(pool)
    .await?;
    Ok(())
}
#[tokio::test]
async fn canonical_merge_recompute_prune_smoke() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let err = canonical_merge_by_infohash(pool, Some("not-a-hash"), None)
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("hash_invalid"));

    let err = canonical_recompute_best_source(pool, Uuid::new_v4(), None)
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("canonical_not_found"));

    canonical_prune_low_confidence(pool).await?;
    Ok(())
}
#[tokio::test]
async fn canonical_disambiguation_requires_values() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let actor = Uuid::parse_str(SYSTEM_USER_PUBLIC_ID)?;
    let input = CanonicalDisambiguationRuleCreateInput {
        actor_user_public_id: actor,
        rule_type: "prevent_merge",
        identity_left_type: "infohash_v1",
        identity_left_value_text: None,
        identity_left_value_uuid: None,
        identity_right_type: "infohash_v2",
        identity_right_value_text: None,
        identity_right_value_uuid: None,
        reason: None,
    };
    let err = canonical_disambiguation_rule_create(pool, &input)
        .await
        .unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("left_value_missing"));
    Ok(())
}

#[tokio::test]
async fn canonical_prune_low_confidence_deletes_eligible_fallback_candidates() -> anyhow::Result<()>
{
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let candidate_id =
        insert_canonical_fallback_candidate(pool, test_db.now() - Duration::days(45)).await?;

    canonical_prune_low_confidence(pool).await?;

    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM canonical_torrent WHERE canonical_torrent_id = $1",
    )
    .bind(candidate_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(remaining, 0);
    Ok(())
}

#[tokio::test]
async fn canonical_prune_low_confidence_keeps_candidates_with_non_candidate_source_links()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let created_at = test_db.now() - Duration::days(45);
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let source_hash = format!("{:040x}", Uuid::new_v4().as_u128());
    let candidate_id = insert_canonical_fallback_candidate(pool, created_at).await?;
    let non_candidate_id = insert_canonical_non_candidate(pool, created_at, &source_hash).await?;
    let source_id = insert_source(pool, indexer_instance_id, &source_hash).await?;

    insert_base_score_link(pool, candidate_id, source_id).await?;
    insert_base_score_link(pool, non_candidate_id, source_id).await?;

    canonical_prune_low_confidence(pool).await?;

    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM canonical_torrent WHERE canonical_torrent_id = $1",
    )
    .bind(candidate_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(remaining, 1);
    Ok(())
}

#[tokio::test]
async fn canonical_prune_low_confidence_removes_candidate_groups_with_only_candidate_links()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let created_at = test_db.now() - Duration::days(45);
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let source_hash = format!("{:040x}", Uuid::new_v4().as_u128());
    let first_candidate_id = insert_canonical_fallback_candidate(pool, created_at).await?;
    let second_candidate_id = insert_canonical_fallback_candidate(pool, created_at).await?;
    let source_id = insert_source(pool, indexer_instance_id, &source_hash).await?;

    insert_base_score_link(pool, first_candidate_id, source_id).await?;
    insert_base_score_link(pool, second_candidate_id, source_id).await?;

    canonical_prune_low_confidence(pool).await?;

    let remaining: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM canonical_torrent
             WHERE canonical_torrent_id IN ($1, $2)",
    )
    .bind(first_candidate_id)
    .bind(second_candidate_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(remaining, 0);
    Ok(())
}
