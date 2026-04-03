use std::collections::{BTreeMap, BTreeSet};

use revaer_data::config::run_migrations;
use revaer_test_support::postgres::start_postgres;
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};

const SYSTEM_USER_PUBLIC_ID: &str = "00000000-0000-0000-0000-000000000000";
const INDEXER_TABLES: &[&str] = &[
    "app_user",
    "deployment_config",
    "deployment_maintenance_state",
    "indexer_definition",
    "indexer_definition_field",
    "indexer_definition_field_validation",
    "indexer_definition_field_value_set",
    "indexer_definition_field_value_set_item",
    "indexer_definition_field_option",
    "trust_tier",
    "media_domain",
    "tag",
    "indexer_instance",
    "indexer_instance_media_domain",
    "indexer_instance_tag",
    "indexer_rss_subscription",
    "indexer_rss_item_seen",
    "indexer_instance_field_value",
    "indexer_instance_import_blob",
    "import_job",
    "import_indexer_result",
    "import_indexer_result_media_domain",
    "import_indexer_result_tag",
    "secret",
    "secret_binding",
    "secret_audit_log",
    "routing_policy",
    "routing_policy_parameter",
    "rate_limit_policy",
    "indexer_instance_rate_limit",
    "routing_policy_rate_limit",
    "rate_limit_state",
    "indexer_cf_state",
    "indexer_connectivity_profile",
    "indexer_health_event",
    "indexer_health_notification_hook",
    "config_audit_log",
    "search_profile",
    "search_profile_media_domain",
    "search_profile_trust_tier",
    "search_profile_indexer_allow",
    "search_profile_indexer_block",
    "search_profile_tag_allow",
    "search_profile_tag_block",
    "search_profile_tag_prefer",
    "search_profile_policy_set",
    "torznab_instance",
    "torznab_category",
    "media_domain_to_torznab_category",
    "tracker_category_mapping",
    "policy_set",
    "policy_rule",
    "policy_rule_value_set",
    "policy_rule_value_set_item",
    "policy_snapshot",
    "policy_snapshot_rule",
    "search_request",
    "search_request_identifier",
    "search_request_torznab_category_requested",
    "search_request_torznab_category_effective",
    "search_request_indexer_run",
    "search_request_indexer_run_correlation",
    "indexer_run_cursor",
    "search_request_canonical",
    "search_page",
    "search_page_item",
    "search_request_source_observation",
    "search_request_source_observation_attr",
    "canonical_torrent",
    "canonical_size_rollup",
    "canonical_size_sample",
    "canonical_external_id",
    "canonical_torrent_source",
    "canonical_torrent_source_attr",
    "canonical_torrent_signal",
    "canonical_disambiguation_rule",
    "canonical_torrent_source_base_score",
    "canonical_torrent_source_context_score",
    "canonical_torrent_best_source_global",
    "canonical_torrent_best_source_context",
    "source_metadata_conflict",
    "source_metadata_conflict_audit_log",
    "search_filter_decision",
    "user_result_action",
    "user_result_action_kv",
    "acquisition_attempt",
    "outbound_request_log",
    "source_reputation",
    "job_schedule",
];

const SOFT_DELETE_TABLES: &[&str] = &[
    "indexer_instance",
    "routing_policy",
    "policy_set",
    "search_profile",
    "tag",
    "torznab_instance",
    "rate_limit_policy",
];

const MANDATORY_PUBLIC_ID_TABLES: &[&str] = &[
    "app_user",
    "indexer_instance",
    "routing_policy",
    "policy_set",
    "policy_rule",
    "search_profile",
    "search_request",
    "canonical_torrent",
    "canonical_torrent_source",
    "torznab_instance",
    "rate_limit_policy",
    "secret",
    "indexer_health_notification_hook",
];

const OPTIONAL_PUBLIC_ID_COLUMNS: &[(&str, &str)] = &[("tag", "tag_public_id")];

const EXPECTED_ENUMS: &[(&str, &[&str])] = &[
    ("upstream_source", &["prowlarr_indexers", "cardigann"]),
    ("protocol", &["torrent", "usenet"]),
    ("engine", &["torznab", "cardigann"]),
    (
        "field_type",
        &[
            "string",
            "password",
            "api_key",
            "cookie",
            "token",
            "header_value",
            "number_int",
            "number_decimal",
            "bool",
            "select_single",
        ],
    ),
    (
        "validation_type",
        &[
            "min_length",
            "max_length",
            "min_value",
            "max_value",
            "regex",
            "allowed_value",
            "required_if_field_equals",
        ],
    ),
    ("depends_on_operator", &["eq", "neq", "in_set"]),
    ("value_set_type", &["text", "int", "bigint", "uuid"]),
    (
        "trust_tier_key",
        &["public", "semi_private", "private", "invite_only"],
    ),
    (
        "media_domain_key",
        &[
            "movies",
            "tv",
            "audiobooks",
            "ebooks",
            "software",
            "adult_movies",
            "adult_scenes",
        ],
    ),
    (
        "secret_type",
        &["api_key", "password", "cookie", "token", "header_value"],
    ),
    (
        "secret_bound_table",
        &["indexer_instance_field_value", "routing_policy_parameter"],
    ),
    (
        "secret_binding_name",
        &[
            "api_key",
            "password",
            "cookie",
            "token",
            "header_value",
            "proxy_password",
            "socks_password",
        ],
    ),
    (
        "routing_policy_mode",
        &[
            "direct",
            "http_proxy",
            "socks_proxy",
            "flaresolverr",
            "vpn_route",
            "tor",
        ],
    ),
    (
        "routing_param_key",
        &[
            "verify_tls",
            "proxy_host",
            "proxy_port",
            "proxy_username",
            "proxy_use_tls",
            "http_proxy_auth",
            "socks_host",
            "socks_port",
            "socks_username",
            "socks_proxy_auth",
            "fs_url",
            "fs_timeout_ms",
            "fs_session_ttl_seconds",
            "fs_user_agent",
        ],
    ),
    ("import_source_system", &["prowlarr"]),
    ("import_payload_format", &["prowlarr_indexer_json_v1"]),
    (
        "audit_entity_type",
        &[
            "indexer_instance",
            "indexer_instance_field_value",
            "routing_policy",
            "routing_policy_parameter",
            "policy_set",
            "policy_rule",
            "search_profile",
            "search_profile_rule",
            "tag",
            "canonical_disambiguation_rule",
            "torznab_instance",
            "rate_limit_policy",
            "tracker_category_mapping",
            "media_domain_to_torznab_category",
        ],
    ),
    (
        "audit_action",
        &[
            "create",
            "update",
            "enable",
            "disable",
            "soft_delete",
            "restore",
            "delete",
        ],
    ),
    (
        "secret_audit_action",
        &["create", "rotate", "revoke", "bind", "unbind"],
    ),
    ("policy_scope", &["global", "user", "profile", "request"]),
    (
        "policy_rule_type",
        &[
            "block_infohash_v1",
            "block_infohash_v2",
            "block_magnet",
            "block_title_regex",
            "block_release_group",
            "block_uploader",
            "block_tracker",
            "block_indexer_instance",
            "allow_release_group",
            "allow_title_regex",
            "allow_indexer_instance",
            "downrank_title_regex",
            "require_trust_tier_min",
            "require_media_domain",
            "prefer_indexer_instance",
            "prefer_trust_tier",
        ],
    ),
    (
        "policy_match_field",
        &[
            "infohash_v1",
            "infohash_v2",
            "magnet_hash",
            "title",
            "release_group",
            "uploader",
            "tracker",
            "indexer_instance_public_id",
            "media_domain_key",
            "trust_tier_key",
            "trust_tier_rank",
        ],
    ),
    (
        "policy_match_operator",
        &[
            "eq",
            "contains",
            "regex",
            "starts_with",
            "ends_with",
            "in_set",
        ],
    ),
    (
        "policy_action",
        &[
            "drop_canonical",
            "drop_source",
            "downrank",
            "require",
            "prefer",
            "flag",
        ],
    ),
    ("policy_severity", &["hard", "soft"]),
    ("deployment_role", &["owner", "admin", "user"]),
    ("import_source", &["prowlarr_api", "prowlarr_backup"]),
    (
        "import_job_status",
        &["pending", "running", "completed", "failed", "canceled"],
    ),
    (
        "import_indexer_result_status",
        &[
            "imported_ready",
            "imported_needs_secret",
            "imported_test_failed",
            "unmapped_definition",
            "skipped_duplicate",
        ],
    ),
    (
        "indexer_instance_migration_state",
        &[
            "ready",
            "needs_secret",
            "test_failed",
            "unmapped_definition",
            "duplicate_suspected",
        ],
    ),
    ("identifier_type", &["imdb", "tmdb", "tvdb"]),
    (
        "query_type",
        &["free_text", "imdb", "tmdb", "tvdb", "season_episode"],
    ),
    ("torznab_mode", &["generic", "tv", "movie"]),
    (
        "search_status",
        &["running", "canceled", "finished", "failed"],
    ),
    (
        "failure_class",
        &[
            "coordinator_error",
            "db_error",
            "auth_error",
            "invalid_request",
            "timeout",
            "canceled_by_system",
        ],
    ),
    (
        "run_status",
        &["queued", "running", "finished", "failed", "canceled"],
    ),
    (
        "error_class",
        &[
            "dns",
            "tls",
            "timeout",
            "connection_refused",
            "http_403",
            "http_429",
            "http_5xx",
            "parse_error",
            "auth_error",
            "cf_challenge",
            "rate_limited",
            "unknown",
        ],
    ),
    (
        "outbound_request_type",
        &["caps", "search", "tvsearch", "moviesearch", "rss", "probe"],
    ),
    ("outbound_request_outcome", &["success", "failure"]),
    (
        "outbound_via_mitigation",
        &["none", "proxy", "flaresolverr"],
    ),
    ("rate_limit_scope", &["indexer_instance", "routing_policy"]),
    (
        "cf_state",
        &["clear", "challenged", "solved", "banned", "cooldown"],
    ),
    (
        "cursor_type",
        &["offset_limit", "page_number", "since_time", "opaque_token"],
    ),
    (
        "identity_strategy",
        &[
            "infohash_v1",
            "infohash_v2",
            "magnet_hash",
            "title_size_fallback",
        ],
    ),
    (
        "durable_source_attr_key",
        &[
            "tracker_name",
            "tracker_category",
            "tracker_subcategory",
            "size_bytes_reported",
            "files_count",
            "imdb_id",
            "tmdb_id",
            "tvdb_id",
            "season",
            "episode",
            "year",
        ],
    ),
    (
        "observation_attr_key",
        &[
            "tracker_name",
            "tracker_category",
            "tracker_subcategory",
            "size_bytes_reported",
            "files_count",
            "imdb_id",
            "tmdb_id",
            "tvdb_id",
            "season",
            "episode",
            "year",
            "release_group",
            "freeleech",
            "internal_flag",
            "scene_flag",
            "minimum_ratio",
            "minimum_seed_time_hours",
            "language_primary",
            "subtitles_primary",
        ],
    ),
    (
        "attr_value_type",
        &["text", "int", "bigint", "numeric", "bool", "uuid"],
    ),
    (
        "signal_key",
        &[
            "release_group",
            "resolution",
            "source_type",
            "codec",
            "audio_codec",
            "container",
            "language",
            "subtitles",
            "edition",
            "year",
            "season",
            "episode",
        ],
    ),
    (
        "decision_type",
        &["drop_canonical", "drop_source", "downrank", "flag"],
    ),
    (
        "user_action",
        &[
            "viewed",
            "selected",
            "deselected",
            "downloaded",
            "blocked",
            "reported_fake",
            "preferred_source",
            "separated_canonical",
            "feedback_positive",
            "feedback_negative",
        ],
    ),
    (
        "user_reason_code",
        &[
            "none",
            "wrong_title",
            "wrong_language",
            "wrong_quality",
            "suspicious",
            "known_bad_group",
            "dmca_risk",
            "dead_torrent",
            "duplicate",
            "personal_preference",
            "other",
        ],
    ),
    (
        "user_action_kv_key",
        &[
            "ui_surface",
            "device",
            "chosen_indexer_instance_public_id",
            "chosen_source_public_id",
            "note_short",
        ],
    ),
    (
        "acquisition_status",
        &["started", "succeeded", "failed", "canceled"],
    ),
    (
        "acquisition_origin",
        &["torznab", "ui", "api", "automation"],
    ),
    (
        "acquisition_failure_class",
        &[
            "dead",
            "dmca",
            "passworded",
            "corrupted",
            "stalled",
            "not_enough_space",
            "auth_error",
            "network_error",
            "client_error",
            "user_canceled",
            "unknown",
        ],
    ),
    (
        "torrent_client_name",
        &[
            "revaer_internal",
            "transmission",
            "qbittorrent",
            "deluge",
            "rtorrent",
            "aria2",
            "unknown",
        ],
    ),
    ("health_event_type", &["identity_conflict"]),
    ("indexer_health_notification_channel", &["email", "webhook"]),
    (
        "indexer_health_notification_threshold",
        &["degraded", "failing", "quarantined"],
    ),
    (
        "connectivity_status",
        &["healthy", "degraded", "failing", "quarantined"],
    ),
    ("reputation_window", &["1h", "24h", "7d"]),
    (
        "context_key_type",
        &["policy_snapshot", "search_profile", "search_request"],
    ),
    ("scoring_context", &["global_current"]),
    (
        "job_key",
        &[
            "retention_purge",
            "reputation_rollup_1h",
            "reputation_rollup_24h",
            "reputation_rollup_7d",
            "connectivity_profile_refresh",
            "canonical_backfill_best_source",
            "base_score_refresh_recent",
            "canonical_prune_low_confidence",
            "policy_snapshot_gc",
            "policy_snapshot_refcount_repair",
            "rate_limit_state_purge",
            "rss_poll",
            "rss_subscription_backfill",
        ],
    ),
    ("disambiguation_rule_type", &["prevent_merge"]),
    (
        "disambiguation_identity_type",
        &[
            "infohash_v1",
            "infohash_v2",
            "magnet_hash",
            "canonical_public_id",
        ],
    ),
    (
        "conflict_type",
        &[
            "tracker_name",
            "tracker_category",
            "external_id",
            "hash",
            "source_guid",
        ],
    ),
    (
        "conflict_resolution",
        &["accepted_incoming", "kept_existing", "merged", "ignored"],
    ),
    (
        "source_metadata_conflict_action",
        &["created", "resolved", "reopened", "ignored"],
    ),
];

const VERSIONED_PROCS: &[&str] = &[
    "deployment_init_v1",
    "app_user_create_v1",
    "app_user_update_v1",
    "app_user_verify_email_v1",
    "import_job_create_v1",
    "import_job_run_prowlarr_api_v1",
    "import_job_run_prowlarr_backup_v1",
    "import_job_get_status_v1",
    "import_job_list_results_v1",
    "indexer_instance_create_v1",
    "indexer_instance_update_v1",
    "indexer_rss_subscription_set_v1",
    "indexer_rss_subscription_disable_v1",
    "indexer_rss_subscription_get_v1",
    "indexer_rss_item_seen_list_v1",
    "indexer_rss_item_seen_mark_v1",
    "indexer_source_reputation_list_v1",
    "indexer_health_notification_hook_create_v1",
    "indexer_health_notification_hook_update_v1",
    "indexer_health_notification_hook_delete_v1",
    "indexer_health_notification_hook_get_v1",
    "indexer_health_notification_hook_list_v1",
    "indexer_instance_set_media_domains_v1",
    "indexer_instance_set_tags_v1",
    "indexer_instance_field_set_value_v1",
    "indexer_instance_field_bind_secret_v1",
    "indexer_instance_test_prepare_v1",
    "indexer_instance_test_finalize_v1",
    "tag_create_v1",
    "tag_update_v1",
    "tag_soft_delete_v1",
    "routing_policy_create_v1",
    "routing_policy_set_param_v1",
    "routing_policy_bind_secret_v1",
    "indexer_cf_state_reset_v1",
    "indexer_connectivity_profile_get_v1",
    "secret_create_v1",
    "secret_rotate_v1",
    "secret_revoke_v1",
    "secret_read_v1",
    "policy_set_create_v1",
    "indexer_policy_set_rule_list_v1",
    "policy_set_update_v1",
    "policy_set_enable_v1",
    "policy_set_disable_v1",
    "policy_set_reorder_v1",
    "policy_rule_create_v1",
    "policy_rule_disable_v1",
    "policy_rule_enable_v1",
    "policy_rule_reorder_v1",
    "search_profile_create_v1",
    "indexer_search_profile_list_v1",
    "search_profile_update_v1",
    "search_profile_set_default_v1",
    "search_profile_set_default_domain_v1",
    "search_profile_set_domain_allowlist_v1",
    "search_profile_add_policy_set_v1",
    "search_profile_remove_policy_set_v1",
    "search_profile_indexer_allow_v1",
    "search_profile_indexer_block_v1",
    "search_profile_tag_allow_v1",
    "search_profile_tag_block_v1",
    "search_profile_tag_prefer_v1",
    "torznab_instance_create_v1",
    "indexer_torznab_instance_list_v1",
    "torznab_instance_rotate_key_v1",
    "torznab_instance_enable_disable_v1",
    "torznab_instance_soft_delete_v1",
    "tracker_category_mapping_upsert_v1",
    "tracker_category_mapping_delete_v1",
    "tracker_category_mapping_resolve_feed_v1",
    "media_domain_to_torznab_category_upsert_v1",
    "media_domain_to_torznab_category_delete_v1",
    "rate_limit_policy_create_v1",
    "rate_limit_policy_update_v1",
    "rate_limit_policy_soft_delete_v1",
    "indexer_instance_set_rate_limit_policy_v1",
    "routing_policy_set_rate_limit_policy_v1",
    "rate_limit_try_consume_v1",
    "search_request_create_v1",
    "search_request_cancel_v1",
    "search_indexer_run_enqueue_v1",
    "search_indexer_run_mark_started_v1",
    "search_indexer_run_mark_finished_v1",
    "search_indexer_run_mark_failed_v1",
    "search_indexer_run_mark_canceled_v1",
    "outbound_request_log_write_v1",
    "search_result_ingest_v1",
    "canonical_merge_by_infohash_v1",
    "canonical_recompute_best_source_v1",
    "canonical_prune_low_confidence_v1",
    "canonical_disambiguation_rule_create_v1",
    "source_metadata_conflict_resolve_v1",
    "source_metadata_conflict_reopen_v1",
    "job_claim_next_v1",
    "job_run_retention_purge_v1",
    "job_run_connectivity_profile_refresh_v1",
    "job_run_reputation_rollup_v1",
    "job_run_canonical_backfill_best_source_v1",
    "job_run_base_score_refresh_recent_v1",
    "job_run_rss_subscription_backfill_v1",
    "rss_poll_claim_v1",
    "rss_poll_apply_v1",
    "job_run_policy_snapshot_gc_v1",
    "job_run_policy_snapshot_refcount_repair_v1",
    "job_run_rate_limit_state_purge_v1",
];

fn stable_proc_name(versioned: &str) -> &str {
    versioned.strip_suffix("_v1").unwrap_or(versioned)
}

struct TestDb {
    _db: revaer_test_support::postgres::TestDatabase,
    pool: PgPool,
}

async fn setup_db() -> anyhow::Result<TestDb> {
    let postgres = match start_postgres() {
        Ok(db) => db,
        Err(err) => {
            eprintln!("skipping indexer schema tests: {err}");
            return Err(anyhow::anyhow!("postgres unavailable"));
        }
    };

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .after_connect(|conn, _meta| {
            Box::pin(async move {
                sqlx::query("SET TIME ZONE 'UTC'")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("SELECT set_config('revaer.secret_key_id', 'test-key', false)")
                    .execute(&mut *conn)
                    .await?;
                sqlx::query("SELECT set_config('revaer.secret_key', 'test-secret', false)")
                    .execute(&mut *conn)
                    .await?;
                Ok(())
            })
        })
        .connect(postgres.connection_string())
        .await?;

    run_migrations(&pool).await?;
    Ok(TestDb {
        _db: postgres,
        pool,
    })
}

async fn fetch_public_tables(pool: &PgPool) -> anyhow::Result<BTreeSet<String>> {
    let rows = sqlx::query(
        r"
        SELECT table_name
        FROM information_schema.tables
        WHERE table_schema = 'public'
          AND table_type = 'BASE TABLE'
        ORDER BY table_name
        ",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("table_name"))
        .collect())
}

async fn fetch_enum_map(pool: &PgPool) -> anyhow::Result<BTreeMap<String, Vec<String>>> {
    let rows = sqlx::query(
        r"
        SELECT t.typname AS enum_name, e.enumlabel AS enum_value
        FROM pg_type t
        JOIN pg_enum e ON e.enumtypid = t.oid
        JOIN pg_namespace n ON n.oid = t.typnamespace
        WHERE n.nspname = 'public'
        ORDER BY t.typname, e.enumsortorder
        ",
    )
    .fetch_all(pool)
    .await?;

    let mut enums = BTreeMap::<String, Vec<String>>::new();
    for row in rows {
        enums
            .entry(row.get("enum_name"))
            .or_default()
            .push(row.get("enum_value"));
    }
    Ok(enums)
}

async fn fetch_proc_names(pool: &PgPool) -> anyhow::Result<BTreeSet<String>> {
    let rows = sqlx::query(
        r"
        SELECT DISTINCT proname
        FROM pg_proc
        WHERE pronamespace = 'public'::regnamespace
        ORDER BY proname
        ",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("proname"))
        .collect())
}

async fn table_has_column(pool: &PgPool, table: &str, column: &str) -> anyhow::Result<bool> {
    let exists = sqlx::query_scalar(
        r"
        SELECT EXISTS (
            SELECT 1
            FROM information_schema.columns
            WHERE table_schema = 'public'
              AND table_name = $1
              AND column_name = $2
        )
        ",
    )
    .bind(table)
    .bind(column)
    .fetch_one(pool)
    .await?;
    Ok(exists)
}

async fn fetch_check_constraints(pool: &PgPool, table: &str) -> anyhow::Result<Vec<String>> {
    let rows = sqlx::query(
        r"
        SELECT pg_get_constraintdef(c.oid) AS definition
        FROM pg_constraint c
        JOIN pg_class rel ON rel.oid = c.conrelid
        JOIN pg_namespace n ON n.oid = rel.relnamespace
        WHERE n.nspname = 'public'
          AND rel.relname = $1
          AND c.contype = 'c'
        ORDER BY c.conname
        ",
    )
    .bind(table)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| row.get::<String, _>("definition"))
        .collect())
}

#[tokio::test]
async fn indexer_schema_contains_all_expected_tables() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = &test_db.pool;

    let tables = fetch_public_tables(pool).await?;
    for table in INDEXER_TABLES {
        assert!(tables.contains(*table), "missing table {table}");
    }

    Ok(())
}

#[tokio::test]
async fn indexer_schema_registers_expected_enums() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = &test_db.pool;

    let enums = fetch_enum_map(pool).await?;
    for (enum_name, expected_values) in EXPECTED_ENUMS {
        let actual = enums
            .get(*enum_name)
            .ok_or_else(|| anyhow::anyhow!("missing enum {enum_name}"))?;
        let expected = expected_values
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>();
        assert_eq!(*actual, expected, "enum {enum_name} does not match ERD");
    }

    Ok(())
}

#[tokio::test]
async fn indexer_schema_registers_versioned_and_stable_procedures() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = &test_db.pool;

    let procedures = fetch_proc_names(pool).await?;
    for versioned in VERSIONED_PROCS {
        assert!(
            procedures.contains(*versioned),
            "missing versioned proc {versioned}"
        );
        let stable = stable_proc_name(versioned);
        assert!(procedures.contains(stable), "missing stable proc {stable}");
    }

    Ok(())
}

#[tokio::test]
async fn indexer_schema_enforces_public_id_soft_delete_and_no_json_columns() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = &test_db.pool;

    for table in MANDATORY_PUBLIC_ID_TABLES {
        assert!(
            table_has_column(pool, table, &format!("{table}_public_id"))
                .await
                .unwrap_or(false)
                || table_has_column(pool, table, "user_public_id").await?
                || table_has_column(pool, table, "tag_public_id").await?,
            "missing public id column on {table}"
        );
    }

    for (table, column) in OPTIONAL_PUBLIC_ID_COLUMNS {
        assert!(
            table_has_column(pool, table, column).await?,
            "missing optional public id column on {table}"
        );
    }

    assert!(
        !table_has_column(pool, "indexer_definition", "indexer_definition_public_id").await?,
        "indexer_definition must not expose a public id in v1"
    );

    for table in SOFT_DELETE_TABLES {
        assert!(
            table_has_column(pool, table, "deleted_at").await?,
            "missing deleted_at on {table}"
        );
    }

    let json_columns: i64 = sqlx::query_scalar(
        r"
        SELECT COUNT(*)
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = ANY($1)
          AND udt_name IN ('json', 'jsonb')
        ",
    )
    .bind(INDEXER_TABLES)
    .fetch_one(pool)
    .await?;
    assert_eq!(json_columns, 0, "indexer schema must not use JSON or JSONB");

    Ok(())
}

#[tokio::test]
async fn indexer_schema_seeds_core_catalog_rows() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = &test_db.pool;

    let trust_tiers = sqlx::query_scalar::<_, String>(
        "SELECT trust_tier_key::text FROM trust_tier ORDER BY rank",
    )
    .fetch_all(pool)
    .await?;
    assert_eq!(
        trust_tiers,
        vec![
            "public".to_string(),
            "semi_private".to_string(),
            "private".to_string(),
            "invite_only".to_string(),
        ]
    );

    let media_domains = sqlx::query_scalar::<_, String>(
        "SELECT media_domain_key::text FROM media_domain ORDER BY media_domain_key::text",
    )
    .fetch_all(pool)
    .await?;
    assert_eq!(
        media_domains,
        vec![
            "adult_movies".to_string(),
            "adult_scenes".to_string(),
            "audiobooks".to_string(),
            "ebooks".to_string(),
            "movies".to_string(),
            "software".to_string(),
            "tv".to_string(),
        ]
    );

    let torznab_categories: BTreeSet<i32> =
        sqlx::query_scalar("SELECT torznab_cat_id FROM torznab_category")
            .fetch_all(pool)
            .await?
            .into_iter()
            .collect();
    for category in [2000, 5000, 5070, 8000] {
        assert!(
            torznab_categories.contains(&category),
            "missing seeded torznab category {category}"
        );
    }

    let default_rate_limits = sqlx::query_scalar::<_, String>(
        "SELECT display_name FROM rate_limit_policy ORDER BY display_name",
    )
    .fetch_all(pool)
    .await?;
    assert!(
        default_rate_limits.contains(&"default_indexer".to_string()),
        "missing default_indexer rate limit policy"
    );
    assert!(
        default_rate_limits.contains(&"default_routing".to_string()),
        "missing default_routing rate limit policy"
    );

    let job_keys = sqlx::query_scalar::<_, String>("SELECT job_key::text FROM job_schedule")
        .fetch_all(pool)
        .await?
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected_job_keys = EXPECTED_ENUMS
        .iter()
        .find_map(|(name, values)| (*name == "job_key").then_some(*values))
        .ok_or_else(|| anyhow::anyhow!("missing job_key enum spec"))?;
    assert_eq!(
        job_keys,
        expected_job_keys
            .iter()
            .map(|value| (*value).to_string())
            .collect::<BTreeSet<_>>()
    );

    let system_user_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM app_user WHERE user_id = 0 AND user_public_id = $1::uuid",
    )
    .bind(SYSTEM_USER_PUBLIC_ID)
    .fetch_one(pool)
    .await?;
    assert_eq!(system_user_count, 1, "missing system sentinel user");

    Ok(())
}

#[tokio::test]
async fn indexer_schema_keeps_key_constraints_and_caps_in_catalog() -> anyhow::Result<()> {
    let Ok(test_db) = setup_db().await else {
        return Ok(());
    };
    let pool = &test_db.pool;

    let tag_constraints = fetch_check_constraints(pool, "tag").await?;
    assert!(
        tag_constraints
            .iter()
            .any(|definition| definition.contains("tag_key") && definition.contains("lower")),
        "tag lower-case check is missing"
    );

    let definition_constraints = fetch_check_constraints(pool, "indexer_definition").await?;
    assert!(
        definition_constraints
            .iter()
            .any(|definition| definition.contains("upstream_slug") && definition.contains("lower")),
        "indexer_definition lower-case slug check is missing"
    );

    let tag_key_length: Option<i32> = sqlx::query_scalar(
        r"
        SELECT character_maximum_length
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'tag'
          AND column_name = 'tag_key'
        ",
    )
    .fetch_one(pool)
    .await?;
    assert_eq!(tag_key_length, Some(128));

    let display_name_length: Option<i32> = sqlx::query_scalar(
        r"
        SELECT character_maximum_length
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'indexer_instance'
          AND column_name = 'display_name'
        ",
    )
    .fetch_one(pool)
    .await?;
    assert_eq!(display_name_length, Some(256));

    let match_value_text_length: Option<i32> = sqlx::query_scalar(
        r"
        SELECT character_maximum_length
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'policy_rule'
          AND column_name = 'match_value_text'
        ",
    )
    .fetch_one(pool)
    .await?;
    assert_eq!(match_value_text_length, Some(512));

    let rationale_length: Option<i32> = sqlx::query_scalar(
        r"
        SELECT character_maximum_length
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'policy_rule'
          AND column_name = 'rationale'
        ",
    )
    .fetch_one(pool)
    .await?;
    assert_eq!(rationale_length, Some(1024));

    Ok(())
}
