use super::*;

use crate::DataError;
use chrono::{DateTime, Duration, Utc};
use sqlx::PgPool;
use uuid::Uuid;
type JobScheduleRow = (
    Option<DateTime<Utc>>,
    DateTime<Utc>,
    Option<DateTime<Utc>>,
    Option<String>,
);
type ClaimedRow = (Option<DateTime<Utc>>, Option<String>, i64);

const JOB_LEASE_EXPECTATIONS: [(&str, i64); 13] = [
    ("connectivity_profile_refresh", 30),
    ("reputation_rollup_1h", 60),
    ("reputation_rollup_24h", 300),
    ("reputation_rollup_7d", 600),
    ("retention_purge", 300),
    ("canonical_backfill_best_source", 900),
    ("base_score_refresh_recent", 900),
    ("canonical_prune_low_confidence", 900),
    ("policy_snapshot_gc", 900),
    ("policy_snapshot_refcount_repair", 900),
    ("rate_limit_state_purge", 300),
    ("rss_poll", 60),
    ("rss_subscription_backfill", 300),
];
const JOB_CADENCE_EXPECTATIONS: [(&str, i32); 13] = [
    ("connectivity_profile_refresh", 300),
    ("reputation_rollup_1h", 300),
    ("reputation_rollup_24h", 3600),
    ("reputation_rollup_7d", 21600),
    ("retention_purge", 3600),
    ("canonical_backfill_best_source", 86400),
    ("base_score_refresh_recent", 3600),
    ("canonical_prune_low_confidence", 86400),
    ("policy_snapshot_gc", 86400),
    ("policy_snapshot_refcount_repair", 86400),
    ("rate_limit_state_purge", 3600),
    ("rss_poll", 60),
    ("rss_subscription_backfill", 300),
];

async fn setup_db() -> anyhow::Result<crate::indexers::IndexerTestDb> {
    crate::indexers::setup_indexer_db("indexer tests").await
}

async fn mark_job_due(pool: &PgPool, job_key: &str) -> anyhow::Result<()> {
    sqlx::query(
        "UPDATE job_schedule
             SET next_run_at = now() - make_interval(secs => 1),
                 locked_until = NULL,
                 lock_owner = NULL
             WHERE job_key = $1::job_key",
    )
    .bind(job_key)
    .execute(pool)
    .await?;
    Ok(())
}

async fn ensure_deployment_config(pool: &PgPool) -> anyhow::Result<()> {
    let config_count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM deployment_config")
        .fetch_one(pool)
        .await?;

    if config_count == 0 {
        sqlx::query("INSERT INTO deployment_config DEFAULT VALUES")
            .execute(pool)
            .await?;
    }

    Ok(())
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
    .bind(format!("retention-{}", Uuid::new_v4().simple()))
    .bind("Retention Definition")
    .bind("torrent")
    .bind("torznab")
    .bind(1_i32)
    .bind("d".repeat(64))
    .bind(false)
    .fetch_one(pool)
    .await?;

    let indexer_instance_id: i64 = sqlx::query_scalar(
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
                $5,
                $6::trust_tier_key,
                $7,
                $8
            )
            RETURNING indexer_instance_id",
    )
    .bind(Uuid::new_v4())
    .bind(definition_id)
    .bind(format!("Retention Instance {}", Uuid::new_v4().simple()))
    .bind("ready")
    .bind(50_i32)
    .bind("public")
    .bind(0_i64)
    .bind(0_i64)
    .fetch_one(pool)
    .await?;

    Ok(indexer_instance_id)
}

async fn insert_canonical_records(
    pool: &PgPool,
    indexer_instance_id: i64,
) -> anyhow::Result<(i64, i64)> {
    let canonical_torrent_id: i64 = sqlx::query_scalar(
        "INSERT INTO canonical_torrent (
                canonical_torrent_public_id,
                identity_confidence,
                identity_strategy,
                infohash_v1,
                title_display,
                title_normalized,
                size_bytes
            )
            VALUES ($1, $2, $3::identity_strategy, $4, $5, $6, $7)
            RETURNING canonical_torrent_id",
    )
    .bind(Uuid::new_v4())
    .bind(1.0_f64)
    .bind("infohash_v1")
    .bind("a".repeat(40))
    .bind("Retention Canonical")
    .bind("retention canonical")
    .bind(1024_i64)
    .fetch_one(pool)
    .await?;

    let canonical_torrent_source_id: i64 = sqlx::query_scalar(
        "INSERT INTO canonical_torrent_source (
                indexer_instance_id,
                canonical_torrent_source_public_id,
                source_guid,
                infohash_v1,
                title_normalized,
                size_bytes
            )
            VALUES ($1, $2, $3, $4, $5, $6)
            RETURNING canonical_torrent_source_id",
    )
    .bind(indexer_instance_id)
    .bind(Uuid::new_v4())
    .bind(format!("retention-source-{}", Uuid::new_v4().simple()))
    .bind("b".repeat(40))
    .bind("retention canonical")
    .bind(1024_i64)
    .fetch_one(pool)
    .await?;

    Ok((canonical_torrent_id, canonical_torrent_source_id))
}

async fn insert_search_request_for_retention(
    pool: &PgPool,
    policy_snapshot_id: i64,
    query_text: &str,
    status: &str,
    finished_at: Option<DateTime<Utc>>,
) -> anyhow::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO search_request (
                search_request_public_id,
                policy_snapshot_id,
                query_text,
                query_type,
                status,
                finished_at
            )
            VALUES ($1, $2, $3, $4::query_type, $5::search_status, $6)
            RETURNING search_request_id",
    )
    .bind(Uuid::new_v4())
    .bind(policy_snapshot_id)
    .bind(query_text)
    .bind("free_text")
    .bind(status)
    .bind(finished_at)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

#[derive(Debug)]
struct RetentionRows {
    old_outbound_request_log: i64,
    recent_outbound_request_log: i64,
    old_rss_item_seen: i64,
    recent_rss_item_seen: i64,
    old_conflict: i64,
    recent_conflict: i64,
    old_conflict_audit_log: i64,
    recent_conflict_audit_log: i64,
    old_health_event: i64,
    recent_health_event: i64,
    old_reputation: i64,
    recent_reputation: i64,
}

#[derive(Debug)]
struct IdPair {
    old_id: i64,
    recent_id: i64,
}

#[derive(Debug)]
struct ConflictRows {
    old_conflict: i64,
    recent_conflict: i64,
    old_audit_log: i64,
    recent_audit_log: i64,
}

async fn insert_outbound_rows(
    pool: &PgPool,
    indexer_instance_id: i64,
    old_ts: DateTime<Utc>,
    recent_ts: DateTime<Utc>,
) -> anyhow::Result<IdPair> {
    let old_id: i64 = sqlx::query_scalar(
            "INSERT INTO outbound_request_log (
                indexer_instance_id,
                request_type,
                correlation_id,
                retry_seq,
                started_at,
                finished_at,
                outcome,
                via_mitigation,
                parse_ok
            )
            VALUES ($1, $2::outbound_request_type, $3, $4, $5, $6, $7::outbound_request_outcome, $8::outbound_via_mitigation, $9)
            RETURNING outbound_request_log_id",
        )
        .bind(indexer_instance_id)
        .bind("search")
        .bind(Uuid::new_v4())
        .bind(0_i16)
        .bind(old_ts)
        .bind(old_ts + Duration::seconds(5))
        .bind("success")
        .bind("none")
        .bind(true)
        .fetch_one(pool)
        .await?;

    let recent_id: i64 = sqlx::query_scalar(
            "INSERT INTO outbound_request_log (
                indexer_instance_id,
                request_type,
                correlation_id,
                retry_seq,
                started_at,
                finished_at,
                outcome,
                via_mitigation,
                parse_ok
            )
            VALUES ($1, $2::outbound_request_type, $3, $4, $5, $6, $7::outbound_request_outcome, $8::outbound_via_mitigation, $9)
            RETURNING outbound_request_log_id",
        )
        .bind(indexer_instance_id)
        .bind("search")
        .bind(Uuid::new_v4())
        .bind(0_i16)
        .bind(recent_ts)
        .bind(recent_ts + Duration::seconds(5))
        .bind("success")
        .bind("none")
        .bind(true)
        .fetch_one(pool)
        .await?;

    Ok(IdPair { old_id, recent_id })
}

async fn insert_rss_rows(
    pool: &PgPool,
    indexer_instance_id: i64,
    old_ts: DateTime<Utc>,
    recent_ts: DateTime<Utc>,
) -> anyhow::Result<IdPair> {
    let old_id: i64 = sqlx::query_scalar(
        "INSERT INTO indexer_rss_item_seen (
                indexer_instance_id,
                item_guid,
                first_seen_at
            )
            VALUES ($1, $2, $3)
            RETURNING rss_item_seen_id",
    )
    .bind(indexer_instance_id)
    .bind(format!("old-rss-{}", Uuid::new_v4().simple()))
    .bind(old_ts)
    .fetch_one(pool)
    .await?;

    let recent_id: i64 = sqlx::query_scalar(
        "INSERT INTO indexer_rss_item_seen (
                indexer_instance_id,
                item_guid,
                first_seen_at
            )
            VALUES ($1, $2, $3)
            RETURNING rss_item_seen_id",
    )
    .bind(indexer_instance_id)
    .bind(format!("recent-rss-{}", Uuid::new_v4().simple()))
    .bind(recent_ts)
    .fetch_one(pool)
    .await?;

    Ok(IdPair { old_id, recent_id })
}

async fn insert_conflict_rows(
    pool: &PgPool,
    canonical_torrent_source_id: i64,
    old_ts: DateTime<Utc>,
    recent_ts: DateTime<Utc>,
) -> anyhow::Result<ConflictRows> {
    let old_conflict_id: i64 = sqlx::query_scalar(
        "INSERT INTO source_metadata_conflict (
                canonical_torrent_source_id,
                conflict_type,
                existing_value,
                incoming_value,
                observed_at
            )
            VALUES ($1, $2::conflict_type, $3, $4, $5)
            RETURNING source_metadata_conflict_id",
    )
    .bind(canonical_torrent_source_id)
    .bind("tracker_name")
    .bind("old-existing")
    .bind("old-incoming")
    .bind(old_ts)
    .fetch_one(pool)
    .await?;

    let recent_conflict_id: i64 = sqlx::query_scalar(
        "INSERT INTO source_metadata_conflict (
                canonical_torrent_source_id,
                conflict_type,
                existing_value,
                incoming_value,
                observed_at
            )
            VALUES ($1, $2::conflict_type, $3, $4, $5)
            RETURNING source_metadata_conflict_id",
    )
    .bind(canonical_torrent_source_id)
    .bind("tracker_name")
    .bind("recent-existing")
    .bind("recent-incoming")
    .bind(recent_ts)
    .fetch_one(pool)
    .await?;

    let old_conflict_audit_log_id: i64 = sqlx::query_scalar(
        "INSERT INTO source_metadata_conflict_audit_log (
                conflict_id,
                action,
                actor_user_id,
                occurred_at
            )
            VALUES ($1, $2::source_metadata_conflict_action, $3, $4)
            RETURNING source_metadata_conflict_audit_log_id",
    )
    .bind(old_conflict_id)
    .bind("created")
    .bind(0_i64)
    .bind(old_ts)
    .fetch_one(pool)
    .await?;

    let recent_conflict_audit_log_id: i64 = sqlx::query_scalar(
        "INSERT INTO source_metadata_conflict_audit_log (
                conflict_id,
                action,
                actor_user_id,
                occurred_at
            )
            VALUES ($1, $2::source_metadata_conflict_action, $3, $4)
            RETURNING source_metadata_conflict_audit_log_id",
    )
    .bind(recent_conflict_id)
    .bind("created")
    .bind(0_i64)
    .bind(recent_ts)
    .fetch_one(pool)
    .await?;

    Ok(ConflictRows {
        old_conflict: old_conflict_id,
        recent_conflict: recent_conflict_id,
        old_audit_log: old_conflict_audit_log_id,
        recent_audit_log: recent_conflict_audit_log_id,
    })
}

async fn insert_health_rows(
    pool: &PgPool,
    indexer_instance_id: i64,
    old_ts: DateTime<Utc>,
    recent_ts: DateTime<Utc>,
) -> anyhow::Result<IdPair> {
    let old_id: i64 = sqlx::query_scalar(
        "INSERT INTO indexer_health_event (
                indexer_instance_id,
                occurred_at,
                event_type
            )
            VALUES ($1, $2, $3::health_event_type)
            RETURNING indexer_health_event_id",
    )
    .bind(indexer_instance_id)
    .bind(old_ts)
    .bind("identity_conflict")
    .fetch_one(pool)
    .await?;

    let recent_id: i64 = sqlx::query_scalar(
        "INSERT INTO indexer_health_event (
                indexer_instance_id,
                occurred_at,
                event_type
            )
            VALUES ($1, $2, $3::health_event_type)
            RETURNING indexer_health_event_id",
    )
    .bind(indexer_instance_id)
    .bind(recent_ts)
    .bind("identity_conflict")
    .fetch_one(pool)
    .await?;

    Ok(IdPair { old_id, recent_id })
}

async fn insert_reputation_rows(
    pool: &PgPool,
    indexer_instance_id: i64,
    old_ts: DateTime<Utc>,
    recent_ts: DateTime<Utc>,
) -> anyhow::Result<IdPair> {
    let old_id: i64 = sqlx::query_scalar(
        "INSERT INTO source_reputation (
                indexer_instance_id,
                window_key,
                window_start,
                request_success_rate,
                acquisition_success_rate,
                fake_rate,
                dmca_rate,
                request_count,
                request_success_count,
                acquisition_count,
                acquisition_success_count,
                min_samples,
                computed_at
            )
            VALUES ($1, $2::reputation_window, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING source_reputation_id",
    )
    .bind(indexer_instance_id)
    .bind("1h")
    .bind(old_ts)
    .bind(0.5_f64)
    .bind(0.5_f64)
    .bind(0.0_f64)
    .bind(0.0_f64)
    .bind(10_i32)
    .bind(5_i32)
    .bind(10_i32)
    .bind(5_i32)
    .bind(10_i32)
    .bind(old_ts)
    .fetch_one(pool)
    .await?;

    let recent_id: i64 = sqlx::query_scalar(
        "INSERT INTO source_reputation (
                indexer_instance_id,
                window_key,
                window_start,
                request_success_rate,
                acquisition_success_rate,
                fake_rate,
                dmca_rate,
                request_count,
                request_success_count,
                acquisition_count,
                acquisition_success_count,
                min_samples,
                computed_at
            )
            VALUES ($1, $2::reputation_window, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            RETURNING source_reputation_id",
    )
    .bind(indexer_instance_id)
    .bind("24h")
    .bind(recent_ts)
    .bind(0.5_f64)
    .bind(0.5_f64)
    .bind(0.0_f64)
    .bind(0.0_f64)
    .bind(10_i32)
    .bind(5_i32)
    .bind(10_i32)
    .bind(5_i32)
    .bind(10_i32)
    .bind(recent_ts)
    .fetch_one(pool)
    .await?;

    Ok(IdPair { old_id, recent_id })
}

async fn insert_retention_rows(
    pool: &PgPool,
    indexer_instance_id: i64,
    canonical_torrent_source_id: i64,
    old_ts: DateTime<Utc>,
    recent_ts: DateTime<Utc>,
) -> anyhow::Result<RetentionRows> {
    let outbound = insert_outbound_rows(pool, indexer_instance_id, old_ts, recent_ts).await?;
    let rss = insert_rss_rows(pool, indexer_instance_id, old_ts, recent_ts).await?;
    let conflict =
        insert_conflict_rows(pool, canonical_torrent_source_id, old_ts, recent_ts).await?;
    let health = insert_health_rows(pool, indexer_instance_id, old_ts, recent_ts).await?;
    let reputation = insert_reputation_rows(pool, indexer_instance_id, old_ts, recent_ts).await?;

    Ok(RetentionRows {
        old_outbound_request_log: outbound.old_id,
        recent_outbound_request_log: outbound.recent_id,
        old_rss_item_seen: rss.old_id,
        recent_rss_item_seen: rss.recent_id,
        old_conflict: conflict.old_conflict,
        recent_conflict: conflict.recent_conflict,
        old_conflict_audit_log: conflict.old_audit_log,
        recent_conflict_audit_log: conflict.recent_audit_log,
        old_health_event: health.old_id,
        recent_health_event: health.recent_id,
        old_reputation: reputation.old_id,
        recent_reputation: reputation.recent_id,
    })
}

async fn insert_outbound_log_row(
    pool: &PgPool,
    indexer_instance_id: i64,
    finished_at: DateTime<Utc>,
    outcome: &str,
    error_class: Option<&str>,
    latency_ms: i32,
) -> anyhow::Result<()> {
    let rate_limit_scope = match error_class {
        Some("rate_limited") => Some("indexer_instance"),
        _ => None,
    };
    let parse_ok = outcome == "success";
    let result_count = if parse_ok { Some(1_i32) } else { None };

    sqlx::query(
        "INSERT INTO outbound_request_log (
                indexer_instance_id,
                request_type,
                correlation_id,
                retry_seq,
                started_at,
                finished_at,
                outcome,
                via_mitigation,
                rate_limit_denied_scope,
                error_class,
                latency_ms,
                parse_ok,
                result_count
            )
            VALUES (
                $1,
                $2::outbound_request_type,
                $3,
                $4,
                $5,
                $6,
                $7::outbound_request_outcome,
                $8::outbound_via_mitigation,
                $9::rate_limit_scope,
                $10::error_class,
                $11,
                $12,
                $13
            )",
    )
    .bind(indexer_instance_id)
    .bind("search")
    .bind(Uuid::new_v4())
    .bind(0_i16)
    .bind(finished_at - Duration::seconds(1))
    .bind(finished_at)
    .bind(outcome)
    .bind("none")
    .bind(rate_limit_scope)
    .bind(error_class)
    .bind(latency_ms)
    .bind(parse_ok)
    .bind(result_count)
    .execute(pool)
    .await?;

    Ok(())
}

#[derive(Debug, sqlx::FromRow)]
struct ConnectivityProfileRow {
    status: String,
    error_class: Option<String>,
    success_rate_1h: Option<f64>,
}

async fn fetch_connectivity_profile(
    pool: &PgPool,
    indexer_instance_id: i64,
) -> anyhow::Result<ConnectivityProfileRow> {
    sqlx::query_as(
        "SELECT
                status::TEXT AS status,
                error_class::TEXT AS error_class,
                success_rate_1h::DOUBLE PRECISION AS success_rate_1h
             FROM indexer_connectivity_profile
             WHERE indexer_instance_id = $1",
    )
    .bind(indexer_instance_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

#[derive(Debug, sqlx::FromRow)]
struct SourceReputationRow {
    request_success_rate: f64,
    acquisition_success_rate: f64,
    fake_rate: f64,
    dmca_rate: f64,
    request_count: i32,
    request_success_count: i32,
    acquisition_count: i32,
    acquisition_success_count: i32,
    min_samples: i32,
}

async fn fetch_source_reputation(
    pool: &PgPool,
    indexer_instance_id: i64,
    window_key: &str,
) -> anyhow::Result<SourceReputationRow> {
    sqlx::query_as(
        "SELECT
                request_success_rate::DOUBLE PRECISION AS request_success_rate,
                acquisition_success_rate::DOUBLE PRECISION AS acquisition_success_rate,
                fake_rate::DOUBLE PRECISION AS fake_rate,
                dmca_rate::DOUBLE PRECISION AS dmca_rate,
                request_count,
                request_success_count,
                acquisition_count,
                acquisition_success_count,
                min_samples
             FROM source_reputation
             WHERE indexer_instance_id = $1
               AND window_key = $2::reputation_window
             ORDER BY window_start DESC
             LIMIT 1",
    )
    .bind(indexer_instance_id)
    .bind(window_key)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn fetch_source_public_id(
    pool: &PgPool,
    canonical_torrent_source_id: i64,
) -> anyhow::Result<String> {
    sqlx::query_scalar(
        "SELECT canonical_torrent_source_public_id::TEXT
             FROM canonical_torrent_source
             WHERE canonical_torrent_source_id = $1",
    )
    .bind(canonical_torrent_source_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn insert_acquisition_attempt_row(
    pool: &PgPool,
    canonical_torrent_id: i64,
    canonical_torrent_source_id: i64,
    search_request_id: i64,
    started_at: DateTime<Utc>,
    status: &str,
    failure_class: Option<&str>,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO acquisition_attempt (
                origin,
                canonical_torrent_id,
                canonical_torrent_source_id,
                search_request_id,
                user_id,
                infohash_v1,
                torrent_client_name,
                started_at,
                finished_at,
                status,
                failure_class
            )
            VALUES (
                $1::acquisition_origin,
                $2,
                $3,
                $4,
                $5,
                $6,
                $7::torrent_client_name,
                $8,
                $9,
                $10::acquisition_status,
                $11::acquisition_failure_class
            )",
    )
    .bind("api")
    .bind(canonical_torrent_id)
    .bind(canonical_torrent_source_id)
    .bind(search_request_id)
    .bind(0_i64)
    .bind("c".repeat(40))
    .bind("unknown")
    .bind(started_at)
    .bind(started_at + Duration::seconds(1))
    .bind(status)
    .bind(failure_class)
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_reported_fake_action(
    pool: &PgPool,
    search_request_id: i64,
    canonical_torrent_id: i64,
    source_public_id: &str,
    created_at: DateTime<Utc>,
) -> anyhow::Result<()> {
    let action_id: i64 = sqlx::query_scalar(
        "INSERT INTO user_result_action (
                user_id,
                search_request_id,
                canonical_torrent_id,
                action,
                reason_code,
                created_at
            )
            VALUES ($1, $2, $3, $4::user_action, $5::user_reason_code, $6)
            RETURNING user_result_action_id",
    )
    .bind(0_i64)
    .bind(search_request_id)
    .bind(canonical_torrent_id)
    .bind("reported_fake")
    .bind("suspicious")
    .bind(created_at)
    .fetch_one(pool)
    .await?;

    sqlx::query(
        "INSERT INTO user_result_action_kv (
                user_result_action_id,
                key,
                value
            )
            VALUES ($1, $2::user_action_kv_key, $3)",
    )
    .bind(action_id)
    .bind("chosen_source_public_id")
    .bind(source_public_id)
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_canonical_torrent_row(
    pool: &PgPool,
    created_at: DateTime<Utc>,
) -> anyhow::Result<(i64, Uuid)> {
    let canonical_public_id = Uuid::new_v4();
    let canonical_torrent_id: i64 = sqlx::query_scalar(
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
    .bind(canonical_public_id)
    .bind(1.0_f64)
    .bind("infohash_v1")
    .bind(format!("{:040x}", Uuid::new_v4().as_u128()))
    .bind("Jobs Canonical")
    .bind("jobs canonical")
    .bind(4096_i64)
    .bind(created_at)
    .bind(created_at)
    .fetch_one(pool)
    .await?;
    Ok((canonical_torrent_id, canonical_public_id))
}

async fn insert_canonical_torrent_source_row(
    pool: &PgPool,
    indexer_instance_id: i64,
    source_guid: &str,
    last_seen_at: DateTime<Utc>,
    seeders: i32,
    leechers: i32,
) -> anyhow::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO canonical_torrent_source (
                indexer_instance_id,
                canonical_torrent_source_public_id,
                source_guid,
                infohash_v1,
                title_normalized,
                size_bytes,
                last_seen_at,
                last_seen_seeders,
                last_seen_leechers,
                last_seen_published_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING canonical_torrent_source_id",
    )
    .bind(indexer_instance_id)
    .bind(Uuid::new_v4())
    .bind(source_guid)
    .bind(format!("{:040x}", Uuid::new_v4().as_u128()))
    .bind("jobs canonical")
    .bind(4096_i64)
    .bind(last_seen_at)
    .bind(seeders)
    .bind(leechers)
    .bind(last_seen_at - Duration::hours(1))
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

async fn insert_base_score_row(
    pool: &PgPool,
    canonical_torrent_id: i64,
    canonical_torrent_source_id: i64,
    score_total_base: f64,
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
            VALUES ($1, $2, $3, 0, 0, 0, 0, 0, 0)
            ON CONFLICT (canonical_torrent_id, canonical_torrent_source_id)
            DO UPDATE SET
                score_total_base = EXCLUDED.score_total_base,
                computed_at = now()",
    )
    .bind(canonical_torrent_id)
    .bind(canonical_torrent_source_id)
    .bind(score_total_base)
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_context_score_row(
    pool: &PgPool,
    canonical_torrent_id: i64,
    canonical_torrent_source_id: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO canonical_torrent_source_context_score (
                context_key_type,
                context_key_id,
                canonical_torrent_id,
                canonical_torrent_source_id,
                score_total_context,
                score_policy_adjust,
                score_tag_adjust,
                is_dropped,
                computed_at
            )
            VALUES (
                $1::context_key_type,
                $2,
                $3,
                $4,
                $5,
                0,
                0,
                FALSE,
                now()
            )",
    )
    .bind("search_request")
    .bind(1_i64)
    .bind(canonical_torrent_id)
    .bind(canonical_torrent_source_id)
    .bind(0.0_f64)
    .execute(pool)
    .await?;
    Ok(())
}

async fn insert_policy_sets_for_retention(
    pool: &PgPool,
    old_search_request_id: i64,
) -> anyhow::Result<(i64, i64)> {
    let old_auto_policy_set_id: i64 = sqlx::query_scalar(
        "INSERT INTO policy_set (
                policy_set_public_id,
                display_name,
                scope,
                is_enabled,
                is_auto_created,
                created_for_search_request_id,
                created_by_user_id,
                updated_by_user_id
            )
            VALUES ($1, $2, $3::policy_scope, TRUE, TRUE, $4, $5, $6)
            RETURNING policy_set_id",
    )
    .bind(Uuid::new_v4())
    .bind("Old Auto Policy Set")
    .bind("request")
    .bind(old_search_request_id)
    .bind(0_i64)
    .bind(0_i64)
    .fetch_one(pool)
    .await?;

    let retained_manual_policy_set_id: i64 = sqlx::query_scalar(
        "INSERT INTO policy_set (
                policy_set_public_id,
                display_name,
                scope,
                is_enabled,
                is_auto_created,
                created_by_user_id,
                updated_by_user_id
            )
            VALUES ($1, $2, $3::policy_scope, TRUE, FALSE, $4, $5)
            RETURNING policy_set_id",
    )
    .bind(Uuid::new_v4())
    .bind("Manual Policy Set")
    .bind("global")
    .bind(0_i64)
    .bind(0_i64)
    .fetch_one(pool)
    .await?;

    Ok((old_auto_policy_set_id, retained_manual_policy_set_id))
}

async fn insert_search_request_context_rows(
    pool: &PgPool,
    old_search_request_id: i64,
    running_search_request_id: i64,
    canonical_torrent_id: i64,
    canonical_torrent_source_id: i64,
) -> anyhow::Result<()> {
    for search_request_id in [old_search_request_id, running_search_request_id] {
        sqlx::query(
            "INSERT INTO canonical_torrent_source_context_score (
                    context_key_type,
                    context_key_id,
                    canonical_torrent_id,
                    canonical_torrent_source_id,
                    score_total_context,
                    score_policy_adjust,
                    score_tag_adjust,
                    is_dropped
                )
                VALUES ($1::context_key_type, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind("search_request")
        .bind(search_request_id)
        .bind(canonical_torrent_id)
        .bind(canonical_torrent_source_id)
        .bind(1.0_f64)
        .bind(0.0_f64)
        .bind(0.0_f64)
        .bind(false)
        .execute(pool)
        .await?;

        sqlx::query(
            "INSERT INTO canonical_torrent_best_source_context (
                    context_key_type,
                    context_key_id,
                    canonical_torrent_id,
                    canonical_torrent_source_id
                )
                VALUES ($1::context_key_type, $2, $3, $4)",
        )
        .bind("search_request")
        .bind(search_request_id)
        .bind(canonical_torrent_id)
        .bind(canonical_torrent_source_id)
        .execute(pool)
        .await?;
    }

    Ok(())
}

async fn assert_row_count(
    pool: &PgPool,
    query: &'static str,
    row_id: i64,
    expected_count: i64,
) -> anyhow::Result<()> {
    let row_count: i64 = sqlx::query_scalar(query)
        .bind(row_id)
        .fetch_one(pool)
        .await?;
    assert_eq!(row_count, expected_count);
    Ok(())
}

async fn assert_retention_operational_rows(
    pool: &PgPool,
    retention_rows: &RetentionRows,
) -> anyhow::Result<()> {
    for (row_id, expected_count, query) in [
        (
            retention_rows.old_outbound_request_log,
            0_i64,
            "SELECT COUNT(*) FROM outbound_request_log WHERE outbound_request_log_id = $1",
        ),
        (
            retention_rows.recent_outbound_request_log,
            1_i64,
            "SELECT COUNT(*) FROM outbound_request_log WHERE outbound_request_log_id = $1",
        ),
        (
            retention_rows.old_rss_item_seen,
            0_i64,
            "SELECT COUNT(*) FROM indexer_rss_item_seen WHERE rss_item_seen_id = $1",
        ),
        (
            retention_rows.recent_rss_item_seen,
            1_i64,
            "SELECT COUNT(*) FROM indexer_rss_item_seen WHERE rss_item_seen_id = $1",
        ),
        (
            retention_rows.old_conflict,
            0_i64,
            "SELECT COUNT(*) FROM source_metadata_conflict WHERE source_metadata_conflict_id = $1",
        ),
        (
            retention_rows.recent_conflict,
            1_i64,
            "SELECT COUNT(*) FROM source_metadata_conflict WHERE source_metadata_conflict_id = $1",
        ),
        (
            retention_rows.old_conflict_audit_log,
            0_i64,
            "SELECT COUNT(*) FROM source_metadata_conflict_audit_log WHERE source_metadata_conflict_audit_log_id = $1",
        ),
        (
            retention_rows.recent_conflict_audit_log,
            1_i64,
            "SELECT COUNT(*) FROM source_metadata_conflict_audit_log WHERE source_metadata_conflict_audit_log_id = $1",
        ),
        (
            retention_rows.old_health_event,
            0_i64,
            "SELECT COUNT(*) FROM indexer_health_event WHERE indexer_health_event_id = $1",
        ),
        (
            retention_rows.recent_health_event,
            1_i64,
            "SELECT COUNT(*) FROM indexer_health_event WHERE indexer_health_event_id = $1",
        ),
        (
            retention_rows.old_reputation,
            0_i64,
            "SELECT COUNT(*) FROM source_reputation WHERE source_reputation_id = $1",
        ),
        (
            retention_rows.recent_reputation,
            1_i64,
            "SELECT COUNT(*) FROM source_reputation WHERE source_reputation_id = $1",
        ),
    ] {
        assert_row_count(pool, query, row_id, expected_count).await?;
    }

    Ok(())
}

#[derive(Debug)]
struct RetentionFixture {
    policy_snapshot_id: i64,
    old_search_request_id: i64,
    running_search_request_id: i64,
    old_auto_policy_set_id: i64,
    retained_manual_policy_set_id: i64,
    canonical_torrent_source_id: i64,
    retention_rows: RetentionRows,
}

async fn seed_retention_fixture(
    pool: &PgPool,
    now: DateTime<Utc>,
) -> anyhow::Result<RetentionFixture> {
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let (canonical_torrent_id, canonical_torrent_source_id) =
        insert_canonical_records(pool, indexer_instance_id).await?;

    let policy_snapshot_id: i64 = sqlx::query_scalar(
        "INSERT INTO policy_snapshot (snapshot_hash, ref_count)
             VALUES ($1, $2)
             RETURNING policy_snapshot_id",
    )
    .bind("e".repeat(64))
    .bind(2_i32)
    .fetch_one(pool)
    .await?;

    let old_search_request_id = insert_search_request_for_retention(
        pool,
        policy_snapshot_id,
        "old-search",
        "finished",
        Some(now - Duration::days(8)),
    )
    .await?;
    let running_search_request_id = insert_search_request_for_retention(
        pool,
        policy_snapshot_id,
        "running-search",
        "running",
        None,
    )
    .await?;

    let (old_auto_policy_set_id, retained_manual_policy_set_id) =
        insert_policy_sets_for_retention(pool, old_search_request_id).await?;
    insert_search_request_context_rows(
        pool,
        old_search_request_id,
        running_search_request_id,
        canonical_torrent_id,
        canonical_torrent_source_id,
    )
    .await?;

    let retention_rows = insert_retention_rows(
        pool,
        indexer_instance_id,
        canonical_torrent_source_id,
        now - Duration::days(181),
        now - Duration::days(1),
    )
    .await?;

    Ok(RetentionFixture {
        policy_snapshot_id,
        old_search_request_id,
        running_search_request_id,
        old_auto_policy_set_id,
        retained_manual_policy_set_id,
        canonical_torrent_source_id,
        retention_rows,
    })
}

async fn assert_retention_search_rows(
    pool: &PgPool,
    fixture: &RetentionFixture,
) -> anyhow::Result<()> {
    assert_row_count(
        pool,
        "SELECT COUNT(*) FROM search_request WHERE search_request_id = $1",
        fixture.old_search_request_id,
        0,
    )
    .await?;
    assert_row_count(
        pool,
        "SELECT COUNT(*) FROM search_request WHERE search_request_id = $1",
        fixture.running_search_request_id,
        1,
    )
    .await?;
    assert_row_count(
        pool,
        "SELECT COUNT(*)
             FROM canonical_torrent_source_context_score
             WHERE context_key_type = 'search_request'
               AND context_key_id = $1",
        fixture.old_search_request_id,
        0,
    )
    .await?;
    assert_row_count(
        pool,
        "SELECT COUNT(*)
             FROM canonical_torrent_source_context_score
             WHERE context_key_type = 'search_request'
               AND context_key_id = $1",
        fixture.running_search_request_id,
        1,
    )
    .await?;
    assert_row_count(
        pool,
        "SELECT COUNT(*)
             FROM canonical_torrent_best_source_context
             WHERE context_key_type = 'search_request'
               AND context_key_id = $1",
        fixture.old_search_request_id,
        0,
    )
    .await?;
    assert_row_count(
        pool,
        "SELECT COUNT(*)
             FROM canonical_torrent_best_source_context
             WHERE context_key_type = 'search_request'
               AND context_key_id = $1",
        fixture.running_search_request_id,
        1,
    )
    .await?;
    Ok(())
}

async fn assert_retention_policy_rows(
    pool: &PgPool,
    fixture: &RetentionFixture,
) -> anyhow::Result<()> {
    assert_row_count(
        pool,
        "SELECT COUNT(*) FROM policy_set WHERE policy_set_id = $1",
        fixture.old_auto_policy_set_id,
        0,
    )
    .await?;
    assert_row_count(
        pool,
        "SELECT COUNT(*) FROM policy_set WHERE policy_set_id = $1",
        fixture.retained_manual_policy_set_id,
        1,
    )
    .await?;
    assert_row_count(
        pool,
        "SELECT COUNT(*) FROM canonical_torrent_source WHERE canonical_torrent_source_id = $1",
        fixture.canonical_torrent_source_id,
        1,
    )
    .await?;

    let ref_count: i32 =
        sqlx::query_scalar("SELECT ref_count FROM policy_snapshot WHERE policy_snapshot_id = $1")
            .bind(fixture.policy_snapshot_id)
            .fetch_one(pool)
            .await?;
    assert_eq!(ref_count, 1);
    Ok(())
}

#[tokio::test]
async fn job_claim_next_rejects_missing_schedule() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    let job_key = "rss_poll";
    sqlx::query("DELETE FROM job_schedule WHERE job_key = $1::job_key")
        .bind(job_key)
        .execute(pool)
        .await?;
    let err = job_claim_next(pool, job_key).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("job_not_found"));
    Ok(())
}

#[tokio::test]
async fn job_claim_next_rejects_not_due_schedule() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let job_key = "rss_poll";
    sqlx::query(
        "UPDATE job_schedule
             SET next_run_at = now() + make_interval(secs => 60),
                 locked_until = NULL,
                 lock_owner = NULL
             WHERE job_key = $1::job_key",
    )
    .bind(job_key)
    .execute(pool)
    .await?;

    let err = job_claim_next(pool, job_key).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("job_not_due"));
    Ok(())
}

#[tokio::test]
async fn job_claim_next_rejects_locked_schedule() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let job_key = "rss_poll";
    sqlx::query(
        "UPDATE job_schedule
             SET next_run_at = now() - make_interval(secs => 1),
                 locked_until = now() + make_interval(secs => 120),
                 lock_owner = 'worker'
             WHERE job_key = $1::job_key",
    )
    .bind(job_key)
    .execute(pool)
    .await?;

    let err = job_claim_next(pool, job_key).await.unwrap_err();
    assert!(matches!(err, DataError::QueryFailed { .. }));
    assert_eq!(err.database_detail(), Some("job_locked"));
    Ok(())
}

#[tokio::test]
async fn job_claim_next_applies_job_key_lease_duration() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    for (job_key, expected_lease_seconds) in JOB_LEASE_EXPECTATIONS {
        mark_job_due(pool, job_key).await?;
        job_claim_next(pool, job_key).await?;

        let (locked_until, lock_owner, lease_seconds_remaining): ClaimedRow = sqlx::query_as(
            "SELECT
                    locked_until,
                    lock_owner,
                    GREATEST(
                        0,
                        FLOOR(EXTRACT(EPOCH FROM (locked_until - now())))::BIGINT
                    )
                 FROM job_schedule
                 WHERE job_key = $1::job_key",
        )
        .bind(job_key)
        .fetch_one(pool)
        .await?;

        assert!(locked_until.is_some());
        assert!(lock_owner.is_some());
        let minimum_lease = expected_lease_seconds.saturating_sub(10);
        assert!(
            (minimum_lease..=expected_lease_seconds).contains(&lease_seconds_remaining),
            "lease duration mismatch for {job_key}"
        );
    }

    Ok(())
}

#[tokio::test]
async fn job_schedule_cadence_matches_erd_refresh_timing() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = test_db.pool();

    for (job_key, expected_cadence_seconds) in JOB_CADENCE_EXPECTATIONS {
        let cadence_seconds: i32 = sqlx::query_scalar(
            "SELECT cadence_seconds
                 FROM job_schedule
                 WHERE job_key = $1::job_key",
        )
        .bind(job_key)
        .fetch_one(pool)
        .await?;
        assert_eq!(cadence_seconds, expected_cadence_seconds);
    }

    Ok(())
}

#[tokio::test]
async fn job_run_smoke_paths() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    match job_run_retention_purge(pool).await {
        Ok(()) => {}
        Err(err) => {
            assert!(matches!(
                err,
                DataError::JobFailed { .. } | DataError::QueryFailed { .. }
            ));
        }
    }
    job_run_connectivity_profile_refresh(pool).await?;
    job_run_reputation_rollup(pool, "1h").await?;
    job_run_canonical_backfill_best_source(pool).await?;
    job_run_base_score_refresh_recent(pool).await?;
    job_run_rss_subscription_backfill(pool).await?;
    job_run_policy_snapshot_gc(pool).await?;
    job_run_policy_snapshot_refcount_repair(pool).await?;
    job_run_rate_limit_state_purge(pool).await?;
    Ok(())
}

#[tokio::test]
async fn job_run_rss_subscription_backfill_populates_missing_rows_and_disables_schedule()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let enabled_instance = insert_indexer_instance(pool).await?;
    let disabled_instance = insert_indexer_instance(pool).await?;
    sqlx::query(
        "UPDATE indexer_instance
             SET is_enabled = FALSE
             WHERE indexer_instance_id = $1",
    )
    .bind(disabled_instance)
    .execute(pool)
    .await?;

    job_run_rss_subscription_backfill(pool).await?;

    let enabled_subscription: (bool, i32, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT is_enabled, interval_seconds, next_poll_at
             FROM indexer_rss_subscription
             WHERE indexer_instance_id = $1",
    )
    .bind(enabled_instance)
    .fetch_one(pool)
    .await?;
    assert!(enabled_subscription.0);
    assert_eq!(enabled_subscription.1, 900);
    assert!(enabled_subscription.2.is_some());

    let disabled_subscription: (bool, i32, Option<DateTime<Utc>>) = sqlx::query_as(
        "SELECT is_enabled, interval_seconds, next_poll_at
             FROM indexer_rss_subscription
             WHERE indexer_instance_id = $1",
    )
    .bind(disabled_instance)
    .fetch_one(pool)
    .await?;
    assert!(!disabled_subscription.0);
    assert_eq!(disabled_subscription.1, 900);
    assert!(disabled_subscription.2.is_none());

    let maintenance_completed_at: Option<DateTime<Utc>> = sqlx::query_scalar(
        "SELECT rss_subscription_backfill_completed_at
             FROM deployment_maintenance_state
             ORDER BY deployment_maintenance_state_id
             LIMIT 1",
    )
    .fetch_one(pool)
    .await?;
    assert!(maintenance_completed_at.is_some());

    let schedule_enabled: bool = sqlx::query_scalar(
        "SELECT enabled
             FROM job_schedule
             WHERE job_key = 'rss_subscription_backfill'",
    )
    .fetch_one(pool)
    .await?;
    assert!(!schedule_enabled);
    Ok(())
}

#[tokio::test]
async fn job_run_rss_subscription_backfill_skips_when_maintenance_completed() -> anyhow::Result<()>
{
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let instance_id = insert_indexer_instance(pool).await?;
    sqlx::query(
        "INSERT INTO deployment_maintenance_state (
                rss_subscription_backfill_completed_at,
                last_updated_at
            )
            VALUES (now(), now())",
    )
    .execute(pool)
    .await?;

    job_run_rss_subscription_backfill(pool).await?;

    let subscription_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM indexer_rss_subscription
             WHERE indexer_instance_id = $1",
    )
    .bind(instance_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(subscription_count, 0);

    let schedule_enabled: bool = sqlx::query_scalar(
        "SELECT enabled
             FROM job_schedule
             WHERE job_key = 'rss_subscription_backfill'",
    )
    .fetch_one(pool)
    .await?;
    assert!(!schedule_enabled);
    Ok(())
}

#[tokio::test]
async fn job_run_updates_schedule_on_completion() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();

    sqlx::query(
        "UPDATE job_schedule
             SET locked_until = now() + make_interval(secs => 3600),
                 lock_owner = 'test'
             WHERE job_key = 'retention_purge'",
    )
    .execute(pool)
    .await?;

    match job_run_retention_purge(pool).await {
        Ok(()) => {}
        Err(err) => {
            assert!(matches!(
                err,
                DataError::JobFailed { .. } | DataError::QueryFailed { .. }
            ));
        }
    }

    let (last_run_at, next_run_at, locked_until, lock_owner): JobScheduleRow = sqlx::query_as(
        "SELECT last_run_at, next_run_at, locked_until, lock_owner
             FROM job_schedule
             WHERE job_key = 'retention_purge'",
    )
    .fetch_one(pool)
    .await?;

    assert!(last_run_at.is_some());
    if let Some(last_run_at) = last_run_at {
        assert!(next_run_at > last_run_at);
    }
    assert!(locked_until.is_none());
    assert!(lock_owner.is_none());
    Ok(())
}

#[tokio::test]
async fn job_run_retention_purge_applies_table_windows() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    ensure_deployment_config(pool).await?;
    let fixture = seed_retention_fixture(pool, test_db.now()).await?;
    job_run_retention_purge(pool).await?;
    assert_retention_search_rows(pool, &fixture).await?;
    assert_retention_operational_rows(pool, &fixture.retention_rows).await?;
    assert_retention_policy_rows(pool, &fixture).await?;
    Ok(())
}

#[tokio::test]
async fn job_run_connectivity_profile_refresh_upserts_degraded_without_samples()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let indexer_instance_id = insert_indexer_instance(pool).await?;

    job_run_connectivity_profile_refresh(pool).await?;

    let profile = fetch_connectivity_profile(pool, indexer_instance_id).await?;
    assert_eq!(profile.status, "degraded");
    assert_eq!(profile.error_class.as_deref(), Some("unknown"));
    assert_eq!(profile.success_rate_1h, None);
    Ok(())
}

#[tokio::test]
async fn job_run_connectivity_profile_refresh_marks_low_success_as_failing() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let sample_time = test_db.now() - Duration::minutes(5);

    for _ in 0..8 {
        insert_outbound_log_row(
            pool,
            indexer_instance_id,
            sample_time,
            "success",
            None,
            1200,
        )
        .await?;
    }

    for _ in 0..2 {
        insert_outbound_log_row(
            pool,
            indexer_instance_id,
            sample_time,
            "failure",
            Some("parse_error"),
            1200,
        )
        .await?;
    }

    job_run_connectivity_profile_refresh(pool).await?;

    let profile = fetch_connectivity_profile(pool, indexer_instance_id).await?;
    let success_rate = profile
        .success_rate_1h
        .ok_or_else(|| anyhow::anyhow!("expected success rate"))?;
    assert!((success_rate - 0.8_f64).abs() < 0.0001_f64);
    assert_eq!(profile.status, "failing");
    assert_eq!(profile.error_class.as_deref(), Some("unknown"));
    Ok(())
}

#[tokio::test]
async fn connectivity_and_reputation_exclude_rate_limited_samples() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let sample_time = test_db.now() - Duration::minutes(5);

    for _ in 0..30 {
        insert_outbound_log_row(pool, indexer_instance_id, sample_time, "success", None, 900)
            .await?;
    }
    for _ in 0..5 {
        insert_outbound_log_row(
            pool,
            indexer_instance_id,
            sample_time,
            "failure",
            Some("parse_error"),
            1200,
        )
        .await?;
    }
    for _ in 0..10 {
        insert_outbound_log_row(
            pool,
            indexer_instance_id,
            sample_time,
            "failure",
            Some("rate_limited"),
            1500,
        )
        .await?;
    }

    job_run_connectivity_profile_refresh(pool).await?;
    let profile = fetch_connectivity_profile(pool, indexer_instance_id).await?;
    let success_rate = profile
        .success_rate_1h
        .ok_or_else(|| anyhow::anyhow!("expected success rate"))?;
    let expected_rate = 30_f64 / 35_f64;
    assert!((success_rate - expected_rate).abs() < 0.0001_f64);

    job_run_reputation_rollup(pool, "1h").await?;
    let reputation = fetch_source_reputation(pool, indexer_instance_id, "1h").await?;
    assert_eq!(reputation.request_count, 35);
    assert_eq!(reputation.request_success_count, 30);
    assert!((reputation.request_success_rate - expected_rate).abs() < 0.0001_f64);
    Ok(())
}

#[tokio::test]
async fn job_run_connectivity_profile_refresh_quarantines_persistent_auth_failures()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let sample_time = test_db.now() - Duration::minutes(5);

    sqlx::query(
            "INSERT INTO indexer_connectivity_profile (
                indexer_instance_id,
                status,
                error_class,
                last_checked_at
            )
            VALUES ($1, $2::connectivity_status, $3::error_class, now() - make_interval(mins => 31))",
        )
        .bind(indexer_instance_id)
        .bind("failing")
        .bind("auth_error")
        .execute(pool)
        .await?;

    for _ in 0..25 {
        insert_outbound_log_row(
            pool,
            indexer_instance_id,
            sample_time,
            "failure",
            Some("auth_error"),
            2500,
        )
        .await?;
    }

    job_run_connectivity_profile_refresh(pool).await?;

    let profile = fetch_connectivity_profile(pool, indexer_instance_id).await?;
    assert_eq!(profile.status, "quarantined");
    assert_eq!(profile.error_class.as_deref(), Some("auth_error"));
    Ok(())
}

#[tokio::test]
async fn job_run_connectivity_profile_refresh_recovers_quarantine_to_degraded_after_cooldown()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let sample_time = test_db.now() - Duration::minutes(5);

    sqlx::query(
            "INSERT INTO indexer_connectivity_profile (
                indexer_instance_id,
                status,
                error_class,
                last_checked_at
            )
            VALUES ($1, $2::connectivity_status, $3::error_class, now() - make_interval(mins => 40))",
        )
        .bind(indexer_instance_id)
        .bind("quarantined")
        .bind("cf_challenge")
        .execute(pool)
        .await?;

    for _ in 0..25 {
        insert_outbound_log_row(pool, indexer_instance_id, sample_time, "success", None, 200)
            .await?;
    }

    job_run_connectivity_profile_refresh(pool).await?;

    let profile = fetch_connectivity_profile(pool, indexer_instance_id).await?;
    assert_eq!(profile.status, "degraded");
    assert_eq!(profile.error_class.as_deref(), Some("cf_challenge"));
    Ok(())
}

#[tokio::test]
async fn job_run_reputation_rollup_skips_insufficient_samples() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let sample_time = test_db.now() - Duration::minutes(5);

    for _ in 0..29 {
        insert_outbound_log_row(pool, indexer_instance_id, sample_time, "success", None, 700)
            .await?;
    }

    job_run_reputation_rollup(pool, "1h").await?;

    let row_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM source_reputation
             WHERE indexer_instance_id = $1
               AND window_key = '1h'::reputation_window",
    )
    .bind(indexer_instance_id)
    .fetch_one(pool)
    .await?;

    assert_eq!(row_count, 0);
    Ok(())
}

#[tokio::test]
async fn job_run_reputation_rollup_writes_rates_for_eligible_samples() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let (canonical_torrent_id, canonical_torrent_source_id) =
        insert_canonical_records(pool, indexer_instance_id).await?;
    let source_public_id = fetch_source_public_id(pool, canonical_torrent_source_id).await?;
    let sample_time = test_db.now() - Duration::minutes(5);

    let policy_snapshot_id: i64 = sqlx::query_scalar(
        "INSERT INTO policy_snapshot (snapshot_hash, ref_count)
             VALUES ($1, $2)
             RETURNING policy_snapshot_id",
    )
    .bind("f".repeat(64))
    .bind(1_i32)
    .fetch_one(pool)
    .await?;
    let search_request_id = insert_search_request_for_retention(
        pool,
        policy_snapshot_id,
        "reputation-rollup",
        "finished",
        Some(sample_time),
    )
    .await?;

    for _ in 0..30 {
        insert_outbound_log_row(pool, indexer_instance_id, sample_time, "success", None, 900)
            .await?;
    }
    for _ in 0..10 {
        insert_outbound_log_row(
            pool,
            indexer_instance_id,
            sample_time,
            "failure",
            Some("parse_error"),
            1100,
        )
        .await?;
    }

    for _ in 0..8 {
        insert_acquisition_attempt_row(
            pool,
            canonical_torrent_id,
            canonical_torrent_source_id,
            search_request_id,
            sample_time,
            "succeeded",
            None,
        )
        .await?;
    }
    for _ in 0..2 {
        insert_acquisition_attempt_row(
            pool,
            canonical_torrent_id,
            canonical_torrent_source_id,
            search_request_id,
            sample_time,
            "failed",
            Some("dmca"),
        )
        .await?;
    }
    for _ in 0..2 {
        insert_acquisition_attempt_row(
            pool,
            canonical_torrent_id,
            canonical_torrent_source_id,
            search_request_id,
            sample_time,
            "failed",
            Some("corrupted"),
        )
        .await?;
    }

    insert_reported_fake_action(
        pool,
        search_request_id,
        canonical_torrent_id,
        &source_public_id,
        sample_time,
    )
    .await?;

    job_run_reputation_rollup(pool, "1h").await?;

    let reputation = fetch_source_reputation(pool, indexer_instance_id, "1h").await?;
    assert_eq!(reputation.request_count, 40);
    assert_eq!(reputation.request_success_count, 30);
    assert_eq!(reputation.acquisition_count, 12);
    assert_eq!(reputation.acquisition_success_count, 8);
    assert_eq!(reputation.min_samples, 10);
    assert!((reputation.request_success_rate - 0.75_f64).abs() < 0.0001_f64);
    assert!((reputation.acquisition_success_rate - (8_f64 / 12_f64)).abs() < 0.0001_f64);
    assert!((reputation.dmca_rate - (2_f64 / 12_f64)).abs() < 0.0001_f64);
    assert!((reputation.fake_rate - 0.25_f64).abs() < 0.0001_f64);
    Ok(())
}

async fn insert_rollup_window_outbound_rows(
    pool: &PgPool,
    indexer_instance_id: i64,
    within_24h: DateTime<Utc>,
    too_old_for_24h: DateTime<Utc>,
    too_old_for_7d: DateTime<Utc>,
) -> anyhow::Result<()> {
    for _ in 0..30 {
        insert_outbound_log_row(pool, indexer_instance_id, within_24h, "success", None, 850)
            .await?;
    }
    for _ in 0..10 {
        insert_outbound_log_row(
            pool,
            indexer_instance_id,
            within_24h,
            "failure",
            Some("parse_error"),
            1050,
        )
        .await?;
    }
    for _ in 0..5 {
        insert_outbound_log_row(
            pool,
            indexer_instance_id,
            too_old_for_24h,
            "failure",
            Some("timeout"),
            1200,
        )
        .await?;
    }
    for _ in 0..6 {
        insert_outbound_log_row(
            pool,
            indexer_instance_id,
            too_old_for_7d,
            "failure",
            Some("timeout"),
            1400,
        )
        .await?;
    }
    Ok(())
}

async fn insert_rollup_window_acquisition_rows(
    pool: &PgPool,
    canonical_torrent_id: i64,
    canonical_torrent_source_id: i64,
    search_request_id: i64,
    source_public_id: &str,
    within_24h: DateTime<Utc>,
) -> anyhow::Result<()> {
    for _ in 0..8 {
        insert_acquisition_attempt_row(
            pool,
            canonical_torrent_id,
            canonical_torrent_source_id,
            search_request_id,
            within_24h,
            "succeeded",
            None,
        )
        .await?;
    }
    for _ in 0..2 {
        insert_acquisition_attempt_row(
            pool,
            canonical_torrent_id,
            canonical_torrent_source_id,
            search_request_id,
            within_24h,
            "failed",
            Some("dmca"),
        )
        .await?;
    }
    insert_reported_fake_action(
        pool,
        search_request_id,
        canonical_torrent_id,
        source_public_id,
        within_24h,
    )
    .await?;
    Ok(())
}

#[tokio::test]
async fn job_run_reputation_rollup_respects_window_boundaries() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let (canonical_torrent_id, canonical_torrent_source_id) =
        insert_canonical_records(pool, indexer_instance_id).await?;
    let source_public_id = fetch_source_public_id(pool, canonical_torrent_source_id).await?;
    let within_24h = test_db.now() - Duration::hours(2);
    let too_old_for_24h = test_db.now() - Duration::hours(30);
    let too_old_for_7d = test_db.now() - Duration::days(8);

    let policy_snapshot_id: i64 = sqlx::query_scalar(
        "INSERT INTO policy_snapshot (snapshot_hash, ref_count)
             VALUES ($1, $2)
             RETURNING policy_snapshot_id",
    )
    .bind("e".repeat(64))
    .bind(1_i32)
    .fetch_one(pool)
    .await?;
    let search_request_id = insert_search_request_for_retention(
        pool,
        policy_snapshot_id,
        "window-rollup",
        "finished",
        Some(within_24h),
    )
    .await?;

    insert_rollup_window_outbound_rows(
        pool,
        indexer_instance_id,
        within_24h,
        too_old_for_24h,
        too_old_for_7d,
    )
    .await?;
    insert_rollup_window_acquisition_rows(
        pool,
        canonical_torrent_id,
        canonical_torrent_source_id,
        search_request_id,
        &source_public_id,
        within_24h,
    )
    .await?;

    job_run_reputation_rollup(pool, "24h").await?;
    let reputation_24h = fetch_source_reputation(pool, indexer_instance_id, "24h").await?;
    assert_eq!(reputation_24h.request_count, 40);
    assert_eq!(reputation_24h.request_success_count, 30);
    assert!((reputation_24h.request_success_rate - 0.75_f64).abs() < 0.0001_f64);

    job_run_reputation_rollup(pool, "7d").await?;
    let reputation_7d = fetch_source_reputation(pool, indexer_instance_id, "7d").await?;
    assert_eq!(reputation_7d.request_count, 45);
    assert_eq!(reputation_7d.request_success_count, 30);
    assert!((reputation_7d.request_success_rate - (30_f64 / 45_f64)).abs() < 0.0001_f64);
    assert_eq!(reputation_7d.acquisition_count, 10);
    assert_eq!(reputation_7d.acquisition_success_count, 8);
    assert!((reputation_7d.dmca_rate - 0.2_f64).abs() < 0.0001_f64);

    Ok(())
}

#[tokio::test]
async fn job_run_base_score_refresh_recent_uses_durable_source_activity() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let created_at = test_db.now() - Duration::days(45);
    let last_seen_at = test_db.now() - Duration::days(1);
    let (canonical_torrent_id, _) = insert_canonical_torrent_row(pool, created_at).await?;
    let source_id = insert_canonical_torrent_source_row(
        pool,
        indexer_instance_id,
        &format!("base-refresh-{}", Uuid::new_v4().simple()),
        last_seen_at,
        100,
        10,
    )
    .await?;

    insert_context_score_row(pool, canonical_torrent_id, source_id).await?;

    job_run_base_score_refresh_recent(pool).await?;

    let base_score_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM canonical_torrent_source_base_score
             WHERE canonical_torrent_id = $1
               AND canonical_torrent_source_id = $2",
    )
    .bind(canonical_torrent_id)
    .bind(source_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(base_score_count, 1);

    let best_source_id: Option<i64> = sqlx::query_scalar(
        "SELECT canonical_torrent_source_id
             FROM canonical_torrent_best_source_global
             WHERE canonical_torrent_id = $1",
    )
    .bind(canonical_torrent_id)
    .fetch_optional(pool)
    .await?;
    assert_eq!(best_source_id, Some(source_id));
    Ok(())
}

#[tokio::test]
async fn job_run_canonical_backfill_best_source_recomputes_recent_durable_sources()
-> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let indexer_instance_id = insert_indexer_instance(pool).await?;
    let created_at = test_db.now() - Duration::days(45);
    let last_seen_at = test_db.now() - Duration::days(1);
    let (canonical_torrent_id, canonical_public_id) =
        insert_canonical_torrent_row(pool, created_at).await?;
    let first_source_id = insert_canonical_torrent_source_row(
        pool,
        indexer_instance_id,
        &format!("backfill-first-{}", Uuid::new_v4().simple()),
        last_seen_at,
        40,
        5,
    )
    .await?;
    let second_source_id = insert_canonical_torrent_source_row(
        pool,
        indexer_instance_id,
        &format!("backfill-second-{}", Uuid::new_v4().simple()),
        last_seen_at,
        120,
        20,
    )
    .await?;

    insert_base_score_row(pool, canonical_torrent_id, first_source_id, 20.0_f64).await?;
    insert_base_score_row(pool, canonical_torrent_id, second_source_id, 10.0_f64).await?;
    sqlx::query(
        "INSERT INTO canonical_torrent_best_source_global (
                canonical_torrent_id,
                canonical_torrent_source_id,
                computed_at
            )
            VALUES ($1, $2, now())",
    )
    .bind(canonical_torrent_id)
    .bind(first_source_id)
    .execute(pool)
    .await?;

    insert_base_score_row(pool, canonical_torrent_id, second_source_id, 50.0_f64).await?;

    job_run_canonical_backfill_best_source(pool).await?;

    let best_source_id: Option<i64> = sqlx::query_scalar(
        "SELECT canonical_torrent_source_id
             FROM canonical_torrent_best_source_global
             WHERE canonical_torrent_id = $1",
    )
    .bind(canonical_torrent_id)
    .fetch_optional(pool)
    .await?;
    assert_eq!(best_source_id, Some(second_source_id));

    let resolved_source_public_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT canonical_recompute_best_source_v1($1, 'global_current'::scoring_context)",
    )
    .bind(canonical_public_id)
    .fetch_optional(pool)
    .await?;
    assert!(resolved_source_public_id.is_some());
    Ok(())
}

#[tokio::test]
async fn job_run_policy_snapshot_gc_repairs_ref_count_before_delete() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let old_hash = "a".repeat(64);
    let recent_hash = "b".repeat(64);

    let old_snapshot_id: i64 = sqlx::query_scalar(
        "INSERT INTO policy_snapshot (snapshot_hash, ref_count, created_at)
             VALUES ($1, $2, now() - make_interval(days => $3))
             RETURNING policy_snapshot_id",
    )
    .bind(&old_hash)
    .bind(1_i32)
    .bind(35_i32)
    .fetch_one(pool)
    .await?;

    let recent_snapshot_id: i64 = sqlx::query_scalar(
        "INSERT INTO policy_snapshot (snapshot_hash, ref_count, created_at)
             VALUES ($1, $2, now() - make_interval(days => $3))
             RETURNING policy_snapshot_id",
    )
    .bind(&recent_hash)
    .bind(1_i32)
    .bind(5_i32)
    .fetch_one(pool)
    .await?;

    job_run_policy_snapshot_gc(pool).await?;

    let old_snapshot_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM policy_snapshot
             WHERE policy_snapshot_id = $1",
    )
    .bind(old_snapshot_id)
    .fetch_one(pool)
    .await?;
    assert_eq!(old_snapshot_count, 0);

    let recent_ref_count: Option<i32> = sqlx::query_scalar(
        "SELECT ref_count
             FROM policy_snapshot
             WHERE policy_snapshot_id = $1",
    )
    .bind(recent_snapshot_id)
    .fetch_optional(pool)
    .await?;
    assert_eq!(recent_ref_count, Some(0));
    Ok(())
}

#[tokio::test]
async fn job_run_rate_limit_state_purge_removes_old_windows() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };

    let pool = test_db.pool();
    let old_window = test_db.now() - Duration::hours(7);
    let recent_window = test_db.now() - Duration::hours(1);

    sqlx::query(
        "INSERT INTO rate_limit_state (scope_type, scope_id, window_start, tokens_used)
             VALUES ($1::rate_limit_scope, $2, $3, $4)",
    )
    .bind("indexer_instance")
    .bind(101_i64)
    .bind(old_window)
    .bind(1_i32)
    .execute(pool)
    .await?;

    sqlx::query(
        "INSERT INTO rate_limit_state (scope_type, scope_id, window_start, tokens_used)
             VALUES ($1::rate_limit_scope, $2, $3, $4)",
    )
    .bind("indexer_instance")
    .bind(102_i64)
    .bind(recent_window)
    .bind(1_i32)
    .execute(pool)
    .await?;

    job_run_rate_limit_state_purge(pool).await?;

    let old_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM rate_limit_state
             WHERE scope_id = $1",
    )
    .bind(101_i64)
    .fetch_one(pool)
    .await?;

    let recent_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)
             FROM rate_limit_state
             WHERE scope_id = $1",
    )
    .bind(102_i64)
    .fetch_one(pool)
    .await?;

    assert_eq!(old_count, 0);
    assert_eq!(recent_count, 1);
    Ok(())
}
