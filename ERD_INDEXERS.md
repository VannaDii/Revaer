# Revaer Indexers and Search ERD

## Legend

- PK = Primary Key
- FK = Foreign Key
- UQ = Unique
- NN = Not Null
- Derived = computed or stored snapshot, or materialized rollup

## Global conventions

### Scope and non-goals (v1)

- Extends indexers/search/policy to support CF/proxy/rate limiting reliability and telemetry.
- Provides Torznab serving and category mapping for partial migration.
- Adds first-class stats/insights (health, quality, guidance).
- Does not introduce media management in v1; Revaer stays Prowlarr replacement + torrent client
  first, media later.

### Core data types and ID exposure

- Internal PKs: bigint identity (generated always).
- Public IDs: uuid stored as \*\_public_id with UQ.
- API accepts only public UUIDs; internal bigint never crosses the API boundary.
- Mandatory public IDs: app_user, indexer_instance, routing_policy, policy_set,
  policy_rule, search_profile, search_request, canonical_torrent, canonical_torrent_source,
  torznab_instance, rate_limit_policy, secret.
- Optional public IDs (v1): trust_tier, media_domain.
- indexer_definition has no public_id in v1.
- trust_tier and media_domain are referenced by key in APIs; internal storage uses bigint IDs.
- media_domain_id is stored internally on search_profile and search_request
  (requested_media_domain_id and effective_media_domain_id).
- tag is referenced by key in APIs; public_id is included in v1.
- search_request and canonical_torrent public_ids require auth in v1.
- All FK joins inside DB use bigint PKs.
- External references (policies, disambiguation) store UUIDs or keys, never internal PKs.

### Timestamp conventions

- created_at and updated_at: timestamptz NN DEFAULT now() when present.
- deleted_at: timestamptz NULL for soft-deleted rows.
- finished_at and canceled_at: nullable timestamptz.

### Text column caps

- Keys or slugs (machine identifiers): varchar(128) NN, lowercase enforced by app and CHECK.
- Display names: varchar(256) NN.
- URLs: varchar(2048) nullable.
- Regex patterns: varchar(512) nullable.
- Freeform notes or reason text: varchar(1024) nullable unless tighter limits are specified.

### Hash identity rules

- infohash_v1: SHA-1, 20 bytes, stored as lowercase hex char(40).
- infohash_v2: SHA-256, 32 bytes, stored as lowercase hex char(64).
- magnet_hash: SHA-256, stored as lowercase hex char(64).
- magnet_hash derivation:
    - If infohash_v2 exists: SHA-256(infohash_v2 bytes).
    - Else if infohash_v1 exists: SHA-256(infohash_v1 bytes).
    - Else: SHA-256(normalized magnet URI string).
- magnet URI normalization before hashing (only when no infohash_v1/v2 exists):
    - parse as URI; if parse fails, trim the raw string and hash as-is.
    - lowercase scheme and host.
    - preserve percent-encoding (no decode/re-encode).
    - split query into key/value pairs (value may be empty).
    - lowercase keys only; values are case-sensitive and preserved.
    - drop params with empty key.
    - sort by (key, value) lexicographically; preserve duplicate keys.
    - rebuild: "magnet:?" + join params with "&" using lowercased keys and original values.

### Tenancy and scope

- Single-tenant deployment; no multi-tenant scoping tables.
- indexer_definition, trust_tier, and media_domain are global; seeded by migrations.
- app_user is global; roles are stored on app_user.
- All config, indexers, and search state are deployment-global; no tenant-scoped columns.

### Roles and permissions (v1)

- Roles: owner, admin, user (deployment_role enum), stored on app_user.
- Global policy_set creation/changes: owner/admin only.
- Secret CRUD/rotation/revoke/bind: owner/admin only.
- User-scope policy_set: user can manage their own.
- Profile-scope policy_sets: admin only.
- Enforcement is proc-level validation based on actor role lookup.

### Seed data ownership

- trust_tier, media_domain, and torznab_category are seeded by migrations.
- tag is user-created; no starter set is seeded in v1.

### Soft delete boundaries

- Soft delete via deleted_at on: indexer_instance, routing_policy, policy_set,
  search_profile, tag, torznab_instance, rate_limit_policy.
- High-volume operational tables are hard-deleted by retention jobs.

### FK on-delete rules (high level)

- indexer_definition -> indexer_instance: RESTRICT.
- indexer_instance -> instance children: CASCADE.
- search_request -> search children: CASCADE.
- canonical_torrent -> canonical children: CASCADE.
- policy_set -> policy_rule: CASCADE.
- secret -> secret_binding: RESTRICT (revoke and detach first).

### Audit fields

- created_by_user_id, updated_by_user_id, and changed_by_user_id are NN.
- System actions use user_id=0 (or 00000000-0000-0000-0000-000000000000 for UUID-typed
  user IDs) instead of NULL.

### JSON or JSONB prohibition

- No JSON or JSONB columns.

### Secrets linkage

- secret_binding is the only authoritative linkage.
- No inline secret_id columns on other tables.

## Enum catalog and usage

### upstream_source

- Values: prowlarr_indexers.
- Used in: indexer_definition.upstream_source.

### protocol

- Values: torrent, usenet (reserved).
- Used in: indexer_definition.protocol.

### engine

- Values: torznab, cardigann.
- Used in: indexer_definition.engine.

### field_type

- Values: string, password, api_key, cookie, token, header_value, number_int,
  number_decimal, bool, select_single.
- Used in: indexer_definition_field.field_type, indexer_instance_field_value.field_type.

### validation_type

- Values: min_length, max_length, min_value, max_value, regex, allowed_value,
  required_if_field_equals.
- Used in: indexer_definition_field_validation.validation_type.

### depends_on_operator

- Values: eq, neq, in_set.
- Used in: indexer_definition_field_validation.depends_on_operator.

### value_set_type

- Values: text, int, bigint, uuid.
- Used in: indexer_definition_field_value_set.value_set_type (text/int/bigint only in v1),
  policy_rule_value_set.value_set_type.

### trust_tier_key (seeded values)

- Values: public, semi_private, private, invite_only.
- Used as: trust_tier.trust_tier_key and references by key.

### media_domain_key (seeded values)

- Values: movies, tv, audiobooks, ebooks, software, adult_movies, adult_scenes (extensible).
- Used as: media_domain.media_domain_key and references by key.

### secret_type

- Values: api_key, password, cookie, token, header_value.
- Used in: secret.secret_type.

### secret_bound_table

- Values: indexer_instance_field_value, routing_policy_parameter.
- Used in: secret_binding.bound_table.

### secret_binding_name

- Values: api_key, password, cookie, token, header_value, proxy_password, socks_password.
- Used in: secret_binding.binding_name.

### routing_policy_mode

- Values: direct, http_proxy, socks_proxy, flaresolverr, vpn_route (reserved), tor (reserved).
- Used in: routing_policy.mode.

### routing_param_key

- Values: verify_tls, proxy_host, proxy_port, proxy_username, proxy_use_tls,
  http_proxy_auth, socks_host, socks_port, socks_username, socks_proxy_auth,
  fs_url, fs_timeout_ms, fs_session_ttl_seconds, fs_user_agent.
- Used in: routing_policy_parameter.param_key.

### import_source_system

- Values: prowlarr.
- Used in: indexer_instance_import_blob.source_system.

### import_payload_format

- Values: prowlarr_indexer_json_v1.
- Used in: indexer_instance_import_blob.import_payload_format.

### audit_entity_type

- Values: indexer_instance, indexer_instance_field_value, routing_policy,
  routing_policy_parameter, policy_set, policy_rule, search_profile, search_profile_rule,
  tag, canonical_disambiguation_rule, torznab_instance, rate_limit_policy,
  tracker_category_mapping, media_domain_to_torznab_category.
- Used in: config_audit_log.entity_type.

### audit_action

- Values: create, update, enable, disable, soft_delete, restore.
- Used in: config_audit_log.action.

### secret_audit_action

- Values: create, rotate, revoke, bind, unbind.
- Used in: secret_audit_log.action.

### policy_scope

- Values: global, user, profile, request.
- Used in: policy_set.scope.

### policy_rule_type

- Values: block_infohash_v1, block_infohash_v2, block_magnet, block_title_regex,
  block_release_group, block_uploader, block_tracker, block_indexer_instance,
  allow_release_group, allow_title_regex, allow_indexer_instance,
  downrank_title_regex, require_trust_tier_min, require_media_domain,
  prefer_indexer_instance, prefer_trust_tier.
- Used in: policy_rule.rule_type.

### policy_match_field

- Values: infohash_v1, infohash_v2, magnet_hash, title, release_group, uploader,
  tracker, indexer_instance_public_id, media_domain_key, trust_tier_key,
  trust_tier_rank.
- Used in: policy_rule.match_field.

### policy_match_operator

- Values: eq, contains, regex, starts_with, ends_with, in_set.
- Used in: policy_rule.match_operator.

### policy_action

- Values: drop_canonical, drop_source, downrank, require, prefer, flag.
- Used in: policy_rule.action.

### policy_severity

- Values: hard, soft.
- Used in: policy_rule.severity.

### deployment_role

- Values: owner, admin, user.
- Used in: app_user.role.

### import_source

- Values: prowlarr_api, prowlarr_backup.
- Used in: import_job.source.

### import_job_status

- Values: pending, running, completed, failed, canceled.
- Used in: import_job.status.

### import_indexer_result_status

- Values: imported_ready, imported_needs_secret, imported_test_failed,
  unmapped_definition, skipped_duplicate.
- Used in: import_indexer_result.status.

### indexer_instance_migration_state

- Values: ready, needs_secret, test_failed, unmapped_definition, duplicate_suspected.
- Used in: indexer_instance.migration_state.

### identifier_type

- Values: imdb, tmdb, tvdb.
- Used in: search_request_identifier.id_type.

### query_type

- Values: free_text, imdb, tmdb, tvdb, season_episode.
- Used in: search_request.query_type.

### torznab_mode

- Values: generic, tv, movie.
- Used in: search_request.torznab_mode.

### search_status

- Values: running, canceled, finished, failed.
- Used in: search_request.status.

### failure_class

- Values: coordinator_error, db_error, auth_error, invalid_request, timeout,
  canceled_by_system.
- Used in: search_request.failure_class.

### run_status

- Values: queued, running, finished, failed, canceled.
- Used in: search_request_indexer_run.status.

### error_class

- Values: dns, tls, timeout, connection_refused, http_403, http_429, http_5xx,
  parse_error, auth_error, cf_challenge, rate_limited, unknown.
- Used in: search_request_indexer_run.error_class, indexer_connectivity_profile.error_class.

### outbound_request_type

- Values: caps, search, tvsearch, moviesearch, rss, probe.
- Used in: outbound_request_log.request_type.

### outbound_request_outcome

- Values: success, failure.
- Used in: outbound_request_log.outcome.

### outbound_via_mitigation

- Values: none, proxy, flaresolverr.
- Used in: outbound_request_log.via_mitigation.

### rate_limit_scope

- Values: indexer_instance, routing_policy.
- Used in: rate_limit_state.scope_type.

### cf_state

- Values: clear, challenged, solved, banned, cooldown.
- Used in: indexer_cf_state.state.

### cursor_type

- Values: offset_limit, page_number, since_time, opaque_token.
- Used in: indexer_run_cursor.cursor_type.

### identity_strategy

- Values: infohash_v1, infohash_v2, magnet_hash, title_size_fallback.
- Used in: canonical_torrent.identity_strategy.

### durable_source_attr_key

- Values: tracker_name, tracker_category, tracker_subcategory, size_bytes_reported,
  files_count, imdb_id, tmdb_id, tvdb_id, season, episode, year.
- Used in: canonical_torrent_source_attr.attr_key.

### observation_attr_key

- Values: all durable_source_attr_key values plus release_group, freeleech, internal_flag,
  scene_flag, minimum_ratio, minimum_seed_time_hours, language_primary, subtitles_primary.
- Used in: search_result_ingest_v1 attr_keys and
  search_request_source_observation_attr.attr_key.

### attr_value_type

- Values: text, int, bigint, numeric, bool, uuid.
- Used in: search_result_ingest_v1 attr_types arrays.

### signal_key

- Values: release_group, resolution, source_type, codec, audio_codec, container,
  language, subtitles, edition, year, season, episode.
- Used in: canonical_torrent_signal.signal_key.

### decision_type

- Values: drop_canonical, drop_source, downrank, flag.
- Used in: search_filter_decision.decision.

### user_action

- Values: viewed, selected, deselected, downloaded, blocked, reported_fake,
  preferred_source, separated_canonical, feedback_positive, feedback_negative.
- Used in: user_result_action.action.

### user_reason_code

- Values: none, wrong_title, wrong_language, wrong_quality, suspicious,
  known_bad_group, dmca_risk, dead_torrent, duplicate, personal_preference, other.
- Used in: user_result_action.reason_code.

### user_action_kv_key

- Values: ui_surface, device, chosen_indexer_instance_public_id,
  chosen_source_public_id, note_short.
- Used in: user_result_action_kv.key.

### acquisition_status

- Values: started, succeeded, failed, canceled.
- Used in: acquisition_attempt.status.

### acquisition_origin

- Values: torznab, ui, api, automation.
- Used in: acquisition_attempt.origin.

### acquisition_failure_class

- Values: dead, dmca, passworded, corrupted, stalled, not_enough_space,
  auth_error, network_error, client_error, user_canceled, unknown.
- Used in: acquisition_attempt.failure_class.

### torrent_client_name

- Values: revaer_internal, transmission, qbittorrent, deluge, rtorrent, aria2, unknown.
- Used in: acquisition_attempt.torrent_client_name.

### health_event_type

- Values: identity_conflict.
- Used in: indexer_health_event.event_type.

### connectivity_status

- Values: healthy, degraded, failing, quarantined.
- Used in: indexer_connectivity_profile.status.

### reputation_window

- Values: 1h, 24h, 7d.
- Used in: source_reputation.window_key.

### context_key_type

- Values: policy_snapshot, search_profile, search_request.
- Used in: canonical_torrent_source_context_score.context_key_type,
  canonical_torrent_best_source_context.context_key_type.

### job_key

- Values: retention_purge, reputation_rollup_1h, reputation_rollup_24h,
  reputation_rollup_7d, connectivity_profile_refresh, canonical_backfill_best_source,
  base_score_refresh_recent, canonical_prune_low_confidence, policy_snapshot_gc,
  policy_snapshot_refcount_repair, rate_limit_state_purge, rss_poll,
  rss_subscription_backfill.
- Used in: job_schedule.job_key.

### disambiguation_rule_type

- Values: prevent_merge.
- Used in: canonical_disambiguation_rule.rule_type.

### disambiguation_identity_type

- Values: infohash_v1, infohash_v2, magnet_hash, canonical_public_id.
- Used in: canonical*disambiguation_rule.identity*\*\_type.

### conflict_type

- Values: tracker_name, tracker_category, external_id, hash, source_guid.
- Used in: source_metadata_conflict.conflict_type.

### conflict_resolution

- Values: accepted_incoming, kept_existing, merged, ignored.
- Used in: source_metadata_conflict.resolution.

### source_metadata_conflict_action

- Values: created, resolved, reopened, ignored.
- Used in: source_metadata_conflict_audit_log.action.

## 1. Deployment, users, and global config

### app_user

- PK: user_id
- NN: user_public_id (uuid)
- NN: email (varchar(320))
- NN: email_normalized (varchar(320))
- NN: is_email_verified (bool, default false)
- NN: display_name
- NN: role (deployment_role)
- NN: created_at
- UQ: (user_public_id)
- UQ: (email)
- UQ: (email_normalized)

#### Notes

- app_user is global; roles are deployment-wide.
- user_id=0 is reserved for system actions; user_public_id is all-zero UUID.
- email_normalized is trimmed and lowercased; no provider-specific normalization.
- display_name is cosmetic and not unique.

### deployment_config

- PK: deployment_config_id
- NN: default_page_size (int, default 50, range 10..200)
- NN: retention_search_days (int, default 7, range 1..90)
- NN: retention_health_events_days (int, default 14, range 1..90)
- NN: retention_reputation_days (int, default 180, range 30..3650)
- NN: retention_outbound_request_log_days (int, default 14, range 1..90)
- NN: retention_source_metadata_conflict_days (int, default 30, range 1..365)
- NN: retention_source_metadata_conflict_audit_days (int, default 90, range 7..3650)
- NN: retention_rss_item_seen_days (int, default 30, range 1..365)
- connectivity_refresh_seconds (nullable, range 30..3600)
- NN: created_at
- NN: updated_at

#### Notes

- deployment_config is a singleton row per deployment.
- job_schedule cadence_seconds is the runtime source of truth; deployment_config does not override.

### deployment_maintenance_state

Tracks completion of one-time maintenance jobs.

- PK: deployment_maintenance_state_id
- rss_subscription_backfill_completed_at (timestamptz, nullable)
- NN: last_updated_at

## 2. Indexer definitions (global catalog)

### indexer_definition

Represents what an indexer is, sourced from Prowlarr Indexers or Cardigann.

- PK: indexer_definition_id
- NN: upstream_source (enum)
- NN: upstream_slug
- NN: display_name
- NN: protocol (enum)
- NN: engine (enum)
- NN: schema_version (int)
- NN: definition_hash (char(64) lowercase hex)
- NN: is_deprecated (bool)
- NN: created_at
- NN: updated_at
- UQ: (upstream_source, upstream_slug)

#### Notes

- definition_hash is SHA-256 over canonicalized definition content.
- definition_hash excludes decorative metadata (icons, display-only descriptions, non-behavioral tags).
- definition_hash includes request construction, auth, selectors/parsers, category mapping hints,
  and capabilities.
- indexer_definition has no public_id in v1.

### indexer_definition_field

Normalized field metadata for configuration UIs and validation.

- PK: indexer_definition_field_id
- FK: indexer_definition_id -> indexer_definition.indexer_definition_id
- NN: name
- NN: label
- NN: field_type (enum)
- NN: is_required (bool)
- NN: is_advanced (bool)
- NN: display_order (int, default 1000)
- default_value_plain (nullable)
- default_value_int (nullable)
- default_value_decimal (numeric(12,4), nullable)
- default_value_bool (nullable)
- UQ: (indexer_definition_id, name)

#### Notes

- At most one default*value*\* column set.
- Secret-backed field types cannot have default values.
- display_order is persisted during sync:
    - upstream order if present.
    - else: required first, non-advanced before advanced, then alphabetical by name.

### indexer_definition_field_validation

Normalized validation rules for indexer definition fields.

- PK: indexer_definition_field_validation_id
- FK: indexer_definition_field_id -> indexer_definition_field.indexer_definition_field_id
- NN: validation_type (enum)
- int_value (nullable)
- numeric_value (numeric(12,4), nullable)
- text_value (varchar(512), nullable)
- text_value_norm (stored generated, nullable)
- value_set_id (nullable) -> indexer_definition_field_value_set.value_set_id
- depends_on_field_name (nullable)
- depends_on_operator (nullable)
- depends_on_value_plain (nullable)
- depends_on_value_plain_norm (stored generated, nullable)
- depends_on_value_int (nullable)
- depends_on_value_bool (nullable)
- depends_on_value_set_id (nullable) -> indexer_definition_field_value_set.value_set_id
- UQ: (indexer_definition_field_id, validation_type, coalesce(depends_on_field_name,''),
  coalesce(depends_on_operator,''), coalesce(text_value_norm,''), coalesce(int_value,-1),
  coalesce(numeric_value,-1), coalesce(value_set_id,0), coalesce(depends_on_value_set_id,0),
  coalesce(depends_on_value_plain_norm,''), coalesce(depends_on_value_int,-1),
  coalesce(depends_on_value_bool,false))

#### Notes

- min_length: int_value required (>= 0).
- max_length: int_value required (>= 0).
- min_value: numeric_value required.
- max_value: numeric_value required.
- regex: text_value required (length <= 512).
- allowed_value: exactly one of text_value or value_set_id.
- required_if_field_equals:
    - depends_on_field_name and depends_on_operator required.
    - exactly one of depends_on_value_plain/int/bool or depends_on_value_set_id.
- text_value_norm and depends_on_value_plain_norm are generated and used for UQ enforcement:
    - text_value_norm = trim(text_value) when validation_type=regex.
    - text_value_norm = lower(trim(text_value)) for all other validation types.
    - depends_on_value_plain_norm = lower(trim(depends_on_value_plain)).

### indexer_definition_field_value_set

Value set for allowed_value or required_if_field_equals.

- PK: value_set_id
- FK: indexer_definition_field_validation_id -> indexer_definition_field_validation.indexer_definition_field_validation_id
- NN: value_set_type (enum: text, int, bigint)
- name (nullable)
- UQ: (indexer_definition_field_validation_id)

### indexer_definition_field_value_set_item

- PK: value_set_item_id
- FK: value_set_id
- value_text (varchar(256), nullable)
- value_int (nullable)
- value_bigint (nullable)

#### Notes

- Exactly one of value_text, value_int, value_bigint must be set.
- text values are stored lowercase.

### indexer_definition_field_option

Select options for select fields.

- PK: indexer_definition_field_option_id
- FK: indexer_definition_field_id -> indexer_definition_field.indexer_definition_field_id
- NN: option_value
- NN: option_label
- NN: sort_order
- UQ: (indexer_definition_field_id, option_value)

## 3. Indexer instances (deployment-scoped)

### trust_tier

- PK: trust_tier_id
- NN: trust_tier_key
- NN: display_name
- NN: default_weight (numeric(12,4))
- NN: rank (smallint)
- NN: created_at
- UQ: (trust_tier_key)

#### Notes

- Seeded ranks: public=10, semi_private=20, private=30, invite_only=40.
- Seeded default_weight values: public=0, semi_private=5, private=10, invite_only=15.
- default_weight range: -50..50.

### media_domain

- PK: media_domain_id
- NN: media_domain_key
- NN: display_name
- NN: created_at
- UQ: (media_domain_key)

### tag

- PK: tag_id
- NN: tag_public_id (uuid)
- NN: tag_key
- NN: display_name
- NN: created_by_user_id -> app_user.user_id (0=system)
- NN: updated_by_user_id -> app_user.user_id (0=system)
- NN: created_at
- NN: updated_at
- deleted_at (nullable)
- UQ: (tag_key)
- UQ: (tag_public_id)

#### Notes

- tag_key is immutable once created.
- REST accepts tag_public_id or tag_key for tag references; if both provided and conflict,
  return invalid_tag_reference.

### indexer_instance

Configured copy of an indexer definition.

- PK: indexer_instance_id
- NN: indexer_instance_public_id (uuid)
- FK: indexer_definition_id -> indexer_definition.indexer_definition_id
- NN: display_name
- NN: is_enabled (bool)
- migration_state (indexer_instance_migration_state, nullable)
- migration_detail (varchar(256), nullable)
- NN: enable_rss (bool, default true)
- NN: enable_automatic_search (bool, default true)
- NN: enable_interactive_search (bool, default true)
- NN: priority (int, default 50, range 0..100)
- trust_tier_key (nullable)
- FK: routing_policy_id -> routing_policy.routing_policy_id (nullable)
- NN: connect_timeout_ms (int, default 5000, range 500..60000)
- NN: read_timeout_ms (int, default 15000, range 500..120000)
- NN: max_parallel_requests (int, default 2, range 1..16)
- NN: created_by_user_id -> app_user.user_id (0=system)
- NN: updated_by_user_id -> app_user.user_id (0=system)
- NN: created_at
- NN: updated_at
- deleted_at (nullable)
- UQ: (indexer_instance_public_id)
- UQ: (display_name)

#### Notes

- migration_state is populated by import/test flows; ready means no blockers.
- needs_secret/test_failed/duplicate_suspected imply is_enabled=false and require explicit
  enable after remediation.
- migration_detail stores a short reason string when migration_state is not NULL.
- unmapped_definition is not set on indexer_instance in v1 (unmapped imports do not
  create instances).
- If migration_state transitions from duplicate_suspected to ready, is_enabled remains
  false until the user explicitly enables the indexer.
- enable_automatic_search is reserved for future automation (no automatic search in v1).
- enable_interactive_search is enforced for Torznab and REST interactive searches.

### indexer_instance_media_domain

- PK: indexer_instance_media_domain_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- FK: media_domain_id -> media_domain.media_domain_id
- UQ: (indexer_instance_id, media_domain_id)

### indexer_instance_tag

- PK: indexer_instance_tag_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- FK: tag_id -> tag.tag_id
- UQ: (indexer_instance_id, tag_id)

### indexer_rss_subscription

Per-indexer RSS polling schedule.

- PK: indexer_rss_subscription_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- NN: is_enabled (bool, default true)
- NN: interval_seconds (int, default 900, min 300, max 86400)
- last_polled_at (nullable)
- next_poll_at (nullable)
- backoff_seconds (int, nullable)
- last_error_class (error_class, nullable)
- NN: created_at
- UQ: (indexer_instance_id)
- CHECK: (is_enabled=true AND next_poll_at IS NOT NULL) OR
  (is_enabled=false AND next_poll_at IS NULL)

#### Notes

- RSS polling is per indexer_instance (not per torznab_instance).
- Effective RSS enablement requires indexer_instance.is_enabled=true,
  indexer_instance.enable_rss=true, and indexer_rss_subscription.is_enabled=true.
- next_poll_at is NULL when is_enabled=false; when enabled, next_poll_at is set to
  now()+random_jitter(0..60s).
- last_error_class/backoff_seconds track RSS polling failures; retryable failures set
  backoff_seconds and next_poll_at, non-retryable failures auto-disable the subscription.

### indexer_rss_item_seen

Deduplication of RSS items per indexer.

- PK: rss_item_seen_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- item_guid (varchar(256), nullable)
- infohash_v1 (char(40), nullable)
- infohash_v2 (char(64), nullable)
- magnet_hash (char(64), nullable)
- NN: first_seen_at
- At least one of item_guid, infohash_v1, infohash_v2, magnet_hash must be present.
- UQ: (indexer_instance_id, item_guid) WHERE item_guid IS NOT NULL
- UQ: (indexer_instance_id, infohash_v2) WHERE infohash_v2 IS NOT NULL
- UQ: (indexer_instance_id, infohash_v1) WHERE infohash_v1 IS NOT NULL
- UQ: (indexer_instance_id, magnet_hash) WHERE magnet_hash IS NOT NULL

#### Notes

- first_seen_at is the poll time (now() at insertion).
- item_guid normalization:
    - <guid>/<id>: trim whitespace, lowercase; if empty string, store NULL.
    - stable link fallback: lowercase scheme and host, drop default ports, trim trailing "/"
      from path, preserve path case; if empty, store NULL.
- item_guid length > 256 is discarded and treated as NULL.
- magnet_hash derivation: if magnet URI contains xt=urn:btih or xt=urn:btmh, parse hashes
  and derive magnet_hash per global rules; otherwise magnet_hash = SHA-256(normalized
  magnet URI string per the global magnet URI normalization rules.
- Items with no identifiers after normalization are skipped; this does not fail the poll.
- On UQ conflicts, do nothing; item_seen rows are immutable in v1.
- RSS download_url is not persisted in v1; item_seen stores identifiers only.

### indexer_instance_field_value

Configured values, preserving upstream names.

- PK: indexer_instance_field_value_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- NN: field_name
- NN: field_type (enum)
- value_plain (nullable)
- value_int (nullable)
- value_decimal (numeric(12,4), nullable)
- value_bool (nullable)
- NN: created_at
- NN: updated_at
- NN: updated_by_user_id -> app_user.user_id (0=system)
- UQ: (indexer_instance_id, field_name)

#### Notes

- field_type must match the indexer_definition_field.field_type.
- Non-secret types require exactly one value\_\* column set.
- Secret-backed types require all value\_\* columns NULL and a secret_binding row.

### indexer_instance_import_blob

Lossless compatibility layer for upstream JSON.

- PK: indexer_instance_import_blob_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- NN: source_system (enum: prowlarr)
- NN: import_payload_text (text)
- NN: import_payload_format (enum: prowlarr_indexer_json_v1)
- NN: imported_at
- UQ: (indexer_instance_id, source_system)

### import_job

Tracks Prowlarr import execution and status.

- PK: import_job_id
- NN: import_job_public_id (uuid)
- FK: target_search_profile_id -> search_profile.search_profile_id (nullable)
- FK: target_torznab_instance_id -> torznab_instance.torznab_instance_id (nullable)
- NN: created_by_user_id -> app_user.user_id (0=system)
- NN: source (import_source)
- NN: is_dry_run (bool, default false)
- NN: status (import_job_status)
- started_at (nullable)
- finished_at (nullable)
- error_detail (varchar(1024), nullable)
- NN: created_at
- UQ: (import_job_public_id)

### import_indexer_result

Per-indexer outcome for an import job.

- PK: import_indexer_result_id
- FK: import_job_id -> import_job.import_job_id
- NN: prowlarr_identifier (varchar(256))
- upstream_slug (varchar(128), nullable)
- indexer_instance_id (bigint, nullable)
- NN: status (import_indexer_result_status)
- detail (varchar(512), nullable)
- NN: created_at
- UQ: (import_job_id, prowlarr_identifier)

#### Notes (import mapping, v1)

- Categories: map Prowlarr/Torznab cats to torznab_category IDs, then to media_domain via
  seeded media_domain_to_torznab_category.
- Tags: import as tag_key (lowercase normalized); create missing tags.
- Priority: map directly to indexer_instance.priority (clamp 0..100).
- Capability toggles: map Prowlarr enableRss/enableAutomaticSearch/enableInteractiveSearch
  when present; default all to true when missing.
- Dry-run imports persist results but do not write indexer_instance/routing_policy/tag rows;
  import_indexer_result.indexer_instance_id remains NULL.
- Each import run targets exactly one search_profile and one torznab_instance.
    - If import*job.is_dry_run=true and target*\_ are NULL: do not auto-create; leave
      target\_\_ NULL and report "would create profile/torznab instance" in status detail.
    - If target\_\* are provided on a dry-run: validate ownership but do not mutate them.
    - If not a dry-run and target\_\* are NULL: auto-create both and store IDs on import_job.
- Default multi-instance behavior (non-dry-run): create one search_profile and one
  torznab_instance per imported Prowlarr instance.
- Deterministic naming: search_profile.display_name = "Imported: {prowlarr_instance_name}";
  torznab_instance.display_name = "Torznab: {prowlarr_instance_name}".
- skipped_duplicate criteria: same upstream_slug and same resolved endpoint identity
  (normalized torznab base URL or tracker host signature, if available) and same
  credentials binding presence. If uncertain, do not skip; use imported_test_failed
  with detail="duplicate_suspected" in v1.
    - Torznab base URL normalization:
        - parse URL; lowercase scheme and host.
        - treat http and https as distinct (no scheme coercion).
        - drop default port (80 for http, 443 for https); keep non-default port.
        - trim trailing "/" from path but keep the path.
        - drop query and fragment entirely.
        - normalized form: scheme://host[:port]/path
    - Tracker host signature:
        - derive from configured endpoint host (or torznab base URL host).
        - lowercase and remove leading "www." only.
        - do not apply PSL/eTLD stripping in v1.
        - signature form: host_without_www.
    - Credentials binding presence:
        - presence=true only if all required secret fields are bound.
        - required secret fields come from indexer_definition_field where
          field_type is secret-backed and is_required=true.
        - if no required secret fields exist, presence=true.
- imported_needs_secret and imported_test_failed force indexer_instance.is_enabled=false.
- import populates indexer_instance.migration_state and migration_detail; needs_secret,
  test_failed, and duplicate_suspected require is_enabled=false.
- Clearing migration_state: when secrets are bound and a test passes, set migration_state=ready.
- unmapped_definition does not create an indexer_instance; import_indexer_result.indexer_instance_id
  remains NULL.
- Imported indexers auto-create indexer_rss_subscription with
  is_enabled = (indexer_instance.is_enabled AND enable_rss),
  interval_seconds=900, next_poll_at=now()+random_jitter(0..60s) when enabled,
  otherwise NULL, last_polled_at=NULL.

#### Routing behavior (v1)

- media_domain links act as a hard filter when a search is scoped to a domain.
- tags can be used as a hard filter in search profiles; otherwise they influence ranking.

## 4. Secrets, credentials, and encryption boundaries

### secret

- PK: secret_id
- NN: secret_public_id (uuid)
- NN: secret_type (enum)
- NN: cipher_text (bytea)
- NN: key_id (varchar(128))
- NN: created_at
- rotated_at (nullable)
- NN: is_revoked (bool)
- UQ: (secret_public_id)

### secret_binding

Single authoritative secret linkage.

- PK: secret_binding_id
- FK: secret_id -> secret.secret_id
- NN: bound_table (enum)
- NN: bound_id
- NN: binding_name (enum)
- NN: created_at
- UQ: (bound_table, bound_id, binding_name)

#### binding_name allowlist

- indexer_instance_field_value: api_key, password, cookie, token, header_value.
- routing_policy_parameter: proxy_password (binds to param_key http_proxy_auth),
  socks_password (binds to param_key socks_proxy_auth).

### secret_audit_log

- PK: secret_audit_log_id
- FK: secret_id -> secret.secret_id
- NN: action (enum)
- actor_user_id -> app_user.user_id (nullable)
- NN: occurred_at
- NN: detail (varchar(256))

#### Encryption and rotation (v1)

- AES-GCM envelope encryption.
- key_id references the active KEK (local or external KMS).
- rotation metadata stored; background job rotates secrets on demand when implemented.
- log binding changes and rotations, not every decrypt.

## 5. Routing, proxying, and connectivity policy

### routing_policy

- PK: routing_policy_id
- NN: routing_policy_public_id (uuid)
- NN: display_name
- NN: mode (enum)
- NN: created_by_user_id -> app_user.user_id (0=system)
- NN: updated_by_user_id -> app_user.user_id (0=system)
- NN: created_at
- NN: updated_at
- deleted_at (nullable)
- UQ: (routing_policy_public_id)
- UQ: (display_name)

#### Notes

- If indexer_instance.routing_policy_id is NULL, treat as direct with verify_tls=true.

### routing_policy_parameter

Typed parameters; secrets bound via secret_binding.

- PK: routing_policy_parameter_id
- FK: routing_policy_id -> routing_policy.routing_policy_id
- NN: param_key (enum, per mode)
- value_plain (nullable)
- value_int (nullable)
- value_bool (nullable)
- NN: created_at
- UQ: (routing_policy_id, param_key)

#### Parameter requirements by mode (v1)

- direct: verify_tls (bool, default true).
- http*proxy: proxy_host (text), proxy_port (int), proxy_use_tls (bool default false),
  verify_tls (bool default true), proxy_username (text optional),
  http_proxy_auth (for password binding; value*\* columns NULL; secret optional).
- socks*proxy: socks_host (text), socks_port (int), verify_tls (bool default true),
  socks_username (text optional), socks_proxy_auth (for password binding; value*\* NULL;
  secret optional).
- flaresolverr: fs_url (text), fs_timeout_ms (int default 60000),
  fs_session_ttl_seconds (int default 600), verify_tls (bool default true),
  fs_user_agent (text optional).

#### TLS settings (v1)

- verify_tls is stored only as routing_policy_parameter.
- http_proxy_auth and socks_proxy_auth rows exist for their modes even when no secret is bound.

### rate_limit_policy

Named rate limit settings.

- PK: rate_limit_policy_id
- NN: rate_limit_policy_public_id (uuid)
- NN: display_name (varchar(256))
- NN: requests_per_minute (int, 1..6000)
- NN: burst (int, 0..6000)
- NN: concurrent_requests (int, 1..64)
- NN: is_system (bool, default false)
- NN: created_at
- NN: updated_at
- deleted_at (nullable)
- UQ: (rate_limit_policy_public_id)
- UQ: (display_name)

#### Notes

- is_system=true policies are fixed in v1:
    - cannot be soft-deleted.
    - display_name cannot be changed.
    - updates are rejected.

### indexer_instance_rate_limit

Optional per-indexer override.

- PK: indexer_instance_rate_limit_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- FK: rate_limit_policy_id -> rate_limit_policy.rate_limit_policy_id
- UQ: (indexer_instance_id)

### routing_policy_rate_limit

Optional per-routing policy budget (proxy/flaresolverr).

- PK: routing_policy_rate_limit_id
- FK: routing_policy_id -> routing_policy.routing_policy_id
- FK: rate_limit_policy_id -> rate_limit_policy.rate_limit_policy_id
- UQ: (routing_policy_id)

### rate_limit_state

Shared token bucket state for multi-node readiness.

- PK: rate_limit_state_id
- NN: scope_type (rate_limit_scope)
- NN: scope_id (bigint)
- NN: window_start (timestamptz)
- NN: tokens_used (int)
- NN: updated_at
- UQ: (scope_type, scope_id, window_start)

#### Notes

- Defaults are seeded:
    - default_indexer (rpm=60, burst=30, concurrent=2).
    - default_routing (rpm=120, burst=60, concurrent=4).
- indexer_instance uses default_indexer unless indexer_instance_rate_limit is set.
- routing_policy uses default_routing unless routing_policy_rate_limit is set.
- effective_concurrency = min(indexer_instance.max_parallel_requests,
  rate_limit_policy.concurrent_requests).
- Enforcement uses token bucket (rpm/burst) plus concurrency semaphore.
- Both indexer_instance and routing_policy budgets must pass when both are present.
- Direct routing (routing_policy_id NULL) uses the default routing budget with a hard
  invariant: scope_type=routing_policy and scope_id=0.
- Order: acquire concurrency permits, consume routing tokens, consume indexer tokens,
  execute request, release permits.
- If indexer scope denies after routing tokens were consumed, do not roll back routing tokens.
- FIFO per-indexer queue is best-effort (avoid starvation).
- Token buckets are tracked in rate_limit_state; concurrency is per-process in v1.
- Rate-limited attempts are logged to outbound_request_log with outcome=failure,
  error_class=rate_limited, parse_ok=false, latency_ms=0.
- All outbound request types consume tokens: caps, search, tvsearch, moviesearch, rss, probe.
- rate limiting scope is per indexer_instance and routing_policy only (no request_type
  partitioning in v1).
- Token consumption uses rate_limit_try_consume_v1 with row-level locking.
- window_start is the minute bucket for requests_per_minute accounting and is computed
  inside rate_limit_try_consume_v1.
- Bucket capacity is rpm + burst per minute (no sliding window in v1).
- rate_limit_state rows older than 6 hours are purged by a scheduled job.

### indexer_cf_state

Per-indexer Cloudflare challenge state.

- PK: indexer_cf_state_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- NN: state (cf_state)
- NN: last_changed_at
- cf_session_id (varchar(256), nullable)
- cf_session_expires_at (nullable)
- cooldown_until (nullable)
- backoff_seconds (nullable)
- NN: consecutive_failures (int, default 0)
- last_error_class (error_class, nullable)
- UQ: (indexer_instance_id)

#### Notes

- Used by scheduling and routing to decide whether to route via FlareSolverr
  and whether to quarantine.
- Initialized on indexer_instance_create_v1 with state=clear and consecutive_failures=0.
- cf_state influences routing choice and quarantine decisions but does not override
  indexer_instance.is_enabled.
- In v1, routing uses only indexer_instance.routing_policy_id.
  A flaresolverr route exists iff indexer_instance.routing_policy_id is set and
  routing_policy.mode = flaresolverr.
  If cf_state is challenged or solved and a flaresolverr route exists, prefer that route.
  Otherwise use routing_policy.mode (proxy or flaresolverr) or direct when
  routing_policy_id is NULL.
- Transitions (v1):
    - clear -> challenged on cf_detected=true or error_class=cf_challenge.
    - challenged -> solved when a FlareSolverr request succeeds with parse_ok=true.
    - challenged -> cooldown after >= 5 failures in 10 minutes.
    - solved -> challenged when session expires or cf_detected=true again.
    - solved -> banned after >= 5 auth failures (401/403) in 10 minutes.
    - cooldown -> clear/solved on probe success; failure extends cooldown with backoff.
    - cooldown -> banned after sustained failure > 30 minutes with CF indicators.
    - banned persists until manual reset (preferred in v1).
- Backoff schedule (v1):
    - initial backoff_seconds = 60.
    - multiplier = 2x; max = 6 hours (21600s); jitter = 0–25%.
    - on CF-triggered failure burst: cooldown_until = now() + backoff_seconds + jitter.
    - on successful probe: backoff_seconds = 60 and cooldown_until = NULL.
- Manual reset: indexer_cf_state_reset_v1(actor_user_public_id,
  indexer_instance_public_id, reason).
- Dominant error counters are derived from outbound_request_log rollups and are not
  persisted.

### indexer_connectivity_profile (derived)

Latest-only snapshot per indexer in v1.

- PK: indexer_instance_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- NN: status (enum)
- error_class (nullable when status is healthy)
- latency_p50_ms (nullable)
- latency_p95_ms (nullable)
- success_rate_1h (numeric(5,4), nullable)
- success_rate_24h (numeric(5,4), nullable)
- NN: last_checked_at

#### Notes

- error_class is NULL when status is healthy and NOT NULL otherwise.
- success_rate fields are constrained between 0 and 1.
- Connectivity rollups are derived from outbound_request_log, not indexer_health_event.
- last_checked_at is updated when the profile snapshot is refreshed.

#### Connectivity thresholds and quarantine (v1)

- Window: rolling 1h per indexer_instance.
- Dominant error_class is the highest-count failure class in the last 1h, only if
  failure_count >= 5 and failure_count >= 0.2 \* total_samples; otherwise none.
- http_429 burst: 10 occurrences in 10 minutes or http_429 rate >= 0.3 of samples
  in 10 minutes (min 20 samples).
- healthy: success_rate_1h >= 0.98 and p95_latency <= 1500ms.
- degraded: success_rate_1h >= 0.90 or p95_latency <= 4000ms.
- failing: success_rate_1h < 0.90 or dominant error_class in (auth_error, cf_challenge,
  tls, dns) with >= 5 occurrences.
- quarantined: failing persists for 30 minutes and error_class in (cf_challenge, auth_error)
  or repeated http_429 bursts.
- quarantine recovery: 30 minute cooldown, then 1 probe request; success moves to degraded,
  failure re-quarantines with exponential backoff capped at 6 hours.
- p95 latency handling:
    - if sample_count < 20, use p50 for latency penalty.
    - if sample_count < 5, latency penalty is 0 (unknown).

### config_audit_log

- PK: audit_log_id
- NN: entity_type (enum)
- entity_pk_bigint (nullable)
- entity_public_id (nullable)
- NN: action (enum)
- NN: changed_by_user_id -> app_user.user_id (0=system)
- NN: changed_at
- NN: change_summary (varchar(1024))

#### Notes

- At least one of entity_pk_bigint or entity_public_id must be present.
- Covers all config mutations: indexer_instance (+ fields, domains, tags),
  routing_policy (+ parameters), policy_set and policy_rule, search_profile, tag,
  canonical_disambiguation_rule, search_profile_rule (allow/block/prefer changes),
  torznab_instance, rate_limit_policy, tracker_category_mapping,
  media_domain_to_torznab_category.
- search_profile_rule entries use entity_public_id = search_profile_public_id.

## 6. Search profiles (user intent presets)

### search_profile

- PK: search_profile_id
- NN: search_profile_public_id (uuid)
- FK: user_id -> app_user.user_id (nullable; null = deployment default)
- NN: display_name
- NN: is_default (bool)
- NN: page_size (int, default 50, range 10..200)
- FK: default_media_domain_id -> media_domain.media_domain_id (nullable)
- NN: created_by_user_id -> app_user.user_id (0=system)
- NN: updated_by_user_id -> app_user.user_id (0=system)
- NN: created_at
- NN: updated_at
- deleted_at (nullable)
- UQ: (search_profile_public_id)

### search_profile_media_domain

Optional allowlist of domains for profile filtering.

- PK: search_profile_media_domain_id
- FK: search_profile_id -> search_profile.search_profile_id
- FK: media_domain_id -> media_domain.media_domain_id
- UQ: (search_profile_id, media_domain_id)

#### Notes

- If rows exist, the profile media_domain allowlist participates in effective_media_domain_id
  intersection and runnable indexer filtering.

### search_profile_trust_tier

- PK: search_profile_trust_tier_id
- FK: search_profile_id -> search_profile.search_profile_id
- FK: trust_tier_id -> trust_tier.trust_tier_id
- weight_override (nullable)
- UQ: (search_profile_id, trust_tier_id)

#### Notes

- weight_override range: -50..50.

### search_profile_indexer_allow

- PK: search_profile_indexer_allow_id
- FK: search_profile_id -> search_profile.search_profile_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- UQ: (search_profile_id, indexer_instance_id)

### search_profile_indexer_block

- PK: search_profile_indexer_block_id
- FK: search_profile_id -> search_profile.search_profile_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- UQ: (search_profile_id, indexer_instance_id)

### search_profile_tag_allow

- PK: search_profile_tag_allow_id
- FK: search_profile_id -> search_profile.search_profile_id
- FK: tag_id -> tag.tag_id
- UQ: (search_profile_id, tag_id)

### search_profile_tag_block

- PK: search_profile_tag_block_id
- FK: search_profile_id -> search_profile.search_profile_id
- FK: tag_id -> tag.tag_id
- UQ: (search_profile_id, tag_id)

### search_profile_tag_prefer

Optional preferred tags for scoring.

- PK: search_profile_tag_prefer_id
- FK: search_profile_id -> search_profile.search_profile_id
- FK: tag_id -> tag.tag_id
- weight_override (int, default 5)
- UQ: (search_profile_id, tag_id)

#### Notes

- Dual allow and block rows for the same target are rejected.
- tag blocks win over allows within a profile.
- Tag hard filters are applied before policy evaluation.
- Tag scoring: preferred tags add +5 per tag; sum of tag weights is clamped to [-15, +15]
  (override allowed per tag).
- weight_override is bounded to -50..50.
- If search_profile_media_domain has rows, default_media_domain_id must be one of them.

### search_profile_policy_set

Links profile-scoped policy sets to a profile.

- PK: search_profile_policy_set_id
- FK: search_profile_id -> search_profile.search_profile_id
- FK: policy_set_id -> policy_set.policy_set_id
- UQ: (search_profile_id, policy_set_id)

#### Notes

- Profile policy_sets are applied in policy_set.sort_order ASC, then created_at ASC,
  then policy_set_public_id ASC.

- Profile scope ordering uses policy_set.sort_order ASC, then created_at ASC,
  then policy_set_public_id ASC.

### torznab_instance

Arr-facing Torznab endpoint mapped to a search_profile.

- PK: torznab_instance_id
- FK: search_profile_id -> search_profile.search_profile_id
- NN: torznab_instance_public_id (uuid)
- NN: display_name (varchar(256))
- NN: api_key_hash (text, Argon2id PHC string)
- NN: is_enabled (bool, default true)
- NN: created_at
- NN: updated_at
- deleted_at (nullable)
- UQ: (torznab_instance_public_id)
- UQ: (display_name)

#### Notes

- Torznab routing path: /torznab/{torznab_instance_public_id}/api.
- API keys are hashed with Argon2id and stored as PHC strings; plaintext is never persisted.
- Verification uses Argon2id verify against the stored PHC string.
- API keys are generated by Revaer only (no user-supplied keys).
- Key generation: 32 random bytes, base64url without padding (43 chars, 256-bit entropy).
- Argon2id params (v1): time_cost=3, memory_cost=64 MiB, parallelism=1,
  hash_length=32 bytes, salt_length=16 bytes.
- Authentication accepts ?apikey=... query param only (no headers in v1).
- torznab_instance_create_v1 and torznab_instance_rotate_key_v1 return the plaintext
  API key once; after response, it is not retrievable. UI must prompt copy.
- Torznab response mapping:
    - guid = canonical_torrent_source_public_id.
    - link = Revaer details endpoint for the source.
- download = Revaer redirect URL; validates API key, records acquisition_attempt,
  then redirects to magnet_uri if present, else download_url.
- Download endpoint hit always creates acquisition_attempt with status=started; if a
  redirect target exists, status remains started until the torrent client updates it.
- If neither magnet_uri nor download_url exists, return 404 and create
  acquisition_attempt with status=failed, failure_class=client_error,
  failure_detail=no_download_target.
- Details endpoint: /torznab/{torznab_instance_public_id}/details/{canonical_torrent_source_public_id}?apikey=...
- Download endpoint: /torznab/{torznab_instance_public_id}/download/{canonical_torrent_source_public_id}?apikey=...
- Details endpoint returns JSON only (Content-Type: application/json).
- Details/download endpoints require a valid apikey and an enabled torznab_instance;
  return 404 if disabled/soft-deleted and 401 if apikey invalid.
- Details/download endpoints return 404 if canonical_torrent_source_public_id is not found
  or does not belong to the same torznab_instance.
- Torznab /api returns 404 if the torznab_instance is disabled/soft-deleted; invalid apikey returns 401.
- is_enabled is for operational toggling; deleted_at is for removal with audit history.

#### Torznab response mapping (v1, XML)

- title: observation.title_raw (as-seen).
- size: observation.size_bytes; else canonical_torrent.size_bytes; else omitted.
- guid: canonical_torrent_source_public_id.
- link: /torznab/{instance}/details/{source}?apikey=...
- comments: same as link.
- pubDate: observation.published_at if present; otherwise omit.
- categories: map observation_attr tracker_category/subcategory via tracker_category_mapping;
  treat NULL tracker_subcategory as 0; if no mapping, use 8000 (Other).
- torznab:attr:
    - seeders: observation.seeders (else 0).
    - leechers: observation.leechers (else 0).
    - peers: seeders + leechers if both known; else 0.
    - downloadvolumefactor: 0 if freeleech true, else 1 (if known).
    - uploadvolumefactor: 1.
    - infohash: canonical.infohash_v2 else canonical.infohash_v1 if present.
- Source-of-truth: prefer observation/observation*attr; fall back to durable last_seen*\*
  only when no observation exists for the source (e.g., details endpoint).
- When multiple observations exist for a source in a search, use the latest by observed_at.

#### Torznab caps response (v1, XML)

- searching: search, tv-search, movie-search supported.
- categories: full seeded torznab_category list.
- limits: default=50, max=200.
- supported identifiers: imdbid, tmdbid, tvdbid.
- season/ep supported for tv-search only.

#### Details endpoint response (v1, JSON)

Required top-level keys:

- source
    - source_public_id (uuid)
    - indexer_instance_public_id (uuid)
    - indexer_display_name (string)
    - trust_tier_key (string|null)
- last_seen
    - last_seen_at (timestamptz)
    - seeders (int|null)
    - leechers (int|null)
    - published_at (timestamptz|null)
    - download_url (string|null)
    - magnet_uri (string|null)
    - details_url (string|null)
    - uploader (string|null)
- canonical
    - canonical_public_id (uuid)
    - title_display (string)
    - size_bytes (bigint|null)
    - infohash_v2 (char64|null)
    - infohash_v1 (char40|null)
    - magnet_hash (char64|null)
- external_ids
    - imdb_id (string|null)
    - tmdb_id (int|null)
    - tvdb_id (int|null)
- scores
    - base
        - score_total_base (numeric)
        - components: seed, leech, age, trust, health, reputation (numeric)
    - context (object|null)
        - present only if search_request_public_id is provided
        - score_total_context (numeric)
        - score_policy_adjust (numeric)
        - score_tag_adjust (numeric)
        - is_dropped (bool)
- signals (array)
    - items: { key, value_text|null, value_int|null, confidence }

Optional top-level keys:

- observation (object|null)
    - present only if search_request_public_id is provided; null when no observation exists
    - observed_at (timestamptz)
    - title_raw (string)
    - size_bytes (bigint|null)
    - seeders (int|null)
    - leechers (int|null)
    - published_at (timestamptz|null)
    - download_url (string|null)
    - magnet_uri (string|null)
    - details_url (string|null)
    - uploader (string|null)

Notes:

- Optional query param search_request_public_id includes context scores for that search
  and the latest observation in that search if available; last_seen remains the durable
  snapshot.
- If search_request_public_id is not found in the same deployment as the torznab_instance,
  return 404.
- Base scores are always included; context scores are included only when requested.

#### Torznab request mapping (v1)

- t -> torznab_mode (generic, tv, movie).
- q -> query_text.
- imdbid -> search_request_identifier(imdb).
- tmdbid -> search_request_identifier(tmdb).
- tvdbid -> search_request_identifier(tvdb).
- season -> season_number.
- ep -> episode_number.
- cat -> requested_torznab_category_ids (list).
- offset -> internal paging cursor (not persisted in search_request).
- limit -> page_size (clamped 10..200).
- Torznab interactive searches schedule only indexers with is_enabled=true and
  enable_interactive_search=true.
- Identifiers may be parsed from q only for imdb IDs and prefixed tmdb/tvdb IDs
  (no infohash/magnet parsing from q in v1). Parsed identifiers are stored with
  id_value_raw including any prefix (e.g., "tmdb:12345").
- If both cat and identifiers are provided, both apply.
- Invalid combinations return an empty result set (not an error).
- Invalid Torznab requests return empty results with no committed DB writes. Some invalids
  are rejected by the handler before any proc call; others are rejected inside the proc
  and roll back.
- If cat is omitted, requested category list is empty and no category restriction applies.
- No committed writes means no search_request row, no runs, and no DB log rows.
- Short-circuited Torznab requests do not consume rate-limit tokens.
- Unknown cat IDs are dropped; if cat is provided and the sanitized list is empty while
  the original list length > 0, emit invalid_category_filter and return empty.
- invalid_category_filter:
    - Torznab: when sanitized cat IDs are empty after dropping unknown IDs, or when
      explicit cat filters are reduced to an empty effective set by profile domain
      allowlist filtering (handler short-circuit, no DB writes).
    - REST: when sanitized cat IDs are empty, or when explicit cat filters are reduced
      to an empty effective set by profile domain allowlist filtering.
- Torznab pagination uses append-order:
    - offset skips N items in the flattened stream (page 1 positions, then page 2, etc.).
    - limit applies after offset; ordering is append-order, not score order.
    - if offset exceeds currently available items (search still running), return empty.

#### Torznab invalid combinations (v1)

- More than one explicit identifier param (imdbid/tmdbid/tvdbid) present -> empty.
- Any provided identifier malformed after normalization -> empty.
- q contains more than one distinct identifier type and no explicit id params -> empty.
- q contains multiple matches of the same identifier type and no explicit id params -> empty.
- request_policy_set_public_id invalid or unauthorized -> empty.
- cat param provided and sanitized list is empty -> empty (invalid_category_filter).
- cat param provided and sanitized list is non-empty but effective categories become empty
  after profile domain allowlist filtering:
    - Torznab returns empty results (no DB writes) and emits invalid_category_filter.
    - REST returns invalid_category_filter when cats were explicitly provided.
- season/ep provided when t=generic -> empty (use tvsearch instead).
- t=movie:
    - season or ep provided -> empty.
    - tvdbid present -> empty.
- t=tv:
    - ep without season -> empty.
    - season provided without query anchor (empty q and no identifier) -> empty.
- No external metadata lookup in v1 (tmdbid tv/movie type is not validated).
- cat filters trigger invalid_category_filter when the sanitized list is empty, and
  for REST when explicit cat filters are reduced to an empty effective set by profile
  domain allowlist filtering; otherwise they restrict runs and results.
- If explicit imdbid/tmdbid/tvdbid is provided, it wins over any parsed identifier.
  Parsed identifiers from q are ignored when any explicit identifier param is present.
  Parsed identifiers from q are ignored when any explicit identifier param is present.

#### RSS polling (v1)

- RSS polling is scheduled per indexer_instance using indexer_rss_subscription.
- RSS polling ignores torznab_instance/search_profile; it runs only when
  indexer_instance.is_enabled=true, indexer_instance.enable_rss=true, and
  indexer_rss_subscription.is_enabled=true.
- RSS parsing reuses the same indexer adapter pipeline as interactive search
  (Torznab/Cardigann) but uses RSS feed inputs.
- RSS polling uses the indexer RSS endpoint (or caps-defined RSS URL when provided).
- RSS item field mapping:
    - item_guid: prefer <guid> (or Atom <id>); normalize trim+lowercase, empty->NULL,
      length>256->NULL.
      If <guid>/<id> is absent, fallback to <link> only if stable.
      Stable link criteria (all required):
        - scheme http/https
        - host present
        - no query string
        - no fragment
        - path not empty and not "/"
        - path does not contain (case-insensitive): /download, /dl, /get, /api, /rss
          If stable, item_guid = normalized link:
        - lowercase scheme and host
        - drop default port (80/443)
        - trim trailing "/" from path
        - keep path as-is
    - magnet URI: use <link> or <enclosure url> if it contains magnet:?, otherwise
      use download_url transiently when provided (not persisted in v1).
    - infohash: parse from magnet xt=urn:btih (v1) or xt=urn:btmh (v2) when present.
    - magnet_hash: derive per global rules (hashes preferred, else normalized magnet string).
- random_jitter(0..60s) uses a uniform integer in [0, 60] seconds from an OS CSPRNG.
- rss_poll is deployment-scoped and claimed via job_claim_next_v1(job_key='rss_poll');
  a single global rss_poll job row exists.
- RSS polling is executed by the app job runner and uses stored procedures:
    - rss_poll_claim_v1(limit) to claim due subscriptions.
    - rss_poll_apply_v1(...) to record outcomes, insert items, and update scheduling.
- Polling writes outbound_request_log with request_type=rss.
- RSS items are recorded in indexer_rss_item_seen; no auto-grab in v1.
- RSS polling failures:
    - Retryable: dns, tls, timeout, connection_refused, http_5xx, http_429, rate_limited,
      cf_challenge
      only when a flaresolverr route exists; otherwise cf_challenge is non-retryable and
      triggers CF quarantine.
    - Non-retryable: auth_error, http_403, parse_error, unknown.
    - Non-retryable failures auto-disable the subscription (is_enabled=false,
      next_poll_at=NULL).
- RSS success logging:
    - outbound_request_log.result_count = items_parsed (total items parsed from the feed).
    - each scheduled poll attempt is a new correlation_id with retry_seq=0.
- RSS item counts:
    - items_parsed includes all parsed items before identifier checks.
    - items_eligible includes items with at least one identifier after normalization.
- parse_ok is true when the feed parses successfully and is recognized as RSS/Atom,
  even if some items are skipped due to missing identifiers.

### torznab_category

Seeded canonical Torznab categories.

- PK: torznab_category_id
- NN: torznab_cat_id (int)
- NN: name (varchar(128))
- NN: created_at
- UQ: (torznab_cat_id)

#### Notes

- Seed standard Torznab categories in v1: Movies, TV (including TV/Anime family),
  Music, Books, Software, Adult, Other.
- "Anime" in v1 refers to the TV/Anime family; no separate Anime root is seeded.
- Torznab "Other" category uses torznab_cat_id = 8000.
- Seeded category IDs (v1):
    - Movies: 2000, 2010, 2020, 2030, 2040, 2045, 2050, 2060.
    - TV: 5000, 5010, 5020, 5030, 5040, 5045, 5050, 5060, 5070, 5075, 5080.
    - Audio/Music: 3000, 3010, 3020 (3020 maps to media_domain=audiobooks; 3000/3010
      are seeded but unmapped in v1).
    - Books: 7000, 7010, 7020 (all map to media_domain=ebooks).
    - Software: 4000, 4050.
    - Adult: 6000, 6010, 6020, 6030, 6040.
    - Other: 8000.

### media_domain_to_torznab_category

Maps Revaer media domains to Torznab categories.

- PK: media_domain_to_torznab_category_id
- FK: media_domain_id -> media_domain.media_domain_id
- FK: torznab_category_id -> torznab_category.torznab_category_id
- NN: is_primary (bool, default false)
- UQ: (media_domain_id, torznab_category_id)
- UQ: (media_domain_id) WHERE is_primary = true

#### Notes

- Seeded primaries (v1):
    - movies -> 2000
    - tv -> 5000
    - audiobooks -> 3020
    - ebooks -> 7010
    - software -> 4000
    - adult_movies -> 6000
    - adult_scenes -> 6000
- other (8000) is a special-case fallback and has no primary flag; no media_domain
  row is seeded for "other".
- Seeded full mapping sets (v1):
    - movies -> 2000, 2010, 2020, 2030, 2040, 2045, 2050, 2060
    - tv -> 5000, 5010, 5020, 5030, 5040, 5045, 5050, 5060, 5070, 5075, 5080
    - audiobooks -> 3020
    - ebooks -> 7000, 7010, 7020
    - software -> 4000, 4050
    - adult_movies -> 6000, 6010, 6020, 6030, 6040
    - adult_scenes -> 6000, 6010, 6020, 6030, 6040
    - other (implicit) -> 8000
- Music categories 3000/3010 are seeded but intentionally unmapped in v1
  (music domain unsupported).

### tracker_category_mapping

Maps tracker category/subcategory to Torznab category and media domain.

- PK: tracker_category_mapping_id
- FK: indexer_definition_id -> indexer_definition.indexer_definition_id (nullable)
- NN: tracker_category (int)
- NN: tracker_subcategory (int, default 0)
- FK: torznab_category_id -> torznab_category.torznab_category_id
- FK: media_domain_id -> media_domain.media_domain_id
- NN: confidence (numeric(4,3), default 1.0)
- UQ: (indexer_definition_id, tracker_category, tracker_subcategory)

#### Notes

- NULL indexer_definition_id means a global default mapping.
- Definition-specific mappings override global defaults.
- API identifies indexer definitions by upstream_slug; stored procs resolve to id.
- Seed global defaults in v1 (indexer_definition_id NULL), using full supported families:
    - movies: 2000, 2010, 2020, 2030, 2040, 2045, 2050, 2060 -> media_domain=movies
    - tv: 5000, 5010, 5020, 5030, 5040, 5045, 5050, 5060, 5070, 5075, 5080 -> media_domain=tv
    - ebooks: 7000, 7010, 7020 -> media_domain=ebooks
    - audiobooks: 3020 -> media_domain=audiobooks
    - software: 4000, 4050 -> media_domain=software
    - adult (6000-series): 6000, 6010, 6020, 6030, 6040 -> media_domain=adult_movies
    - music: 3000, 3010 -> media_domain_id NULL (unsupported in v1)
    - anything else -> Other (8000) with media_domain_id NULL
- adult_scenes is only assigned via indexer_definition-specific mappings in v1.

#### Mapping fallback (v1)

- Mapping lookup key uses (tracker_category, coalesce(tracker_subcategory, 0)).
- If a mapping exists for (indexer_definition_id, category, subcategory), use it.
- Else if a global mapping exists for (category, subcategory), use it.
- Else map to Torznab "Other" (torznab_cat_id=8000) and set media_domain_id = NULL
  (unknown domain).
- Strict domain filtering:
    - If effective_media_domain_id is non-null OR explicit category filters are provided
      (and do not include 8000), unmapped categories are treated as non-matching and dropped.
    - If requested categories include 8000 (Other), treat as catch-all and do not drop
      unmapped categories.
    - If cat filters are omitted (requested list empty), there is no category restriction.
    - Categories with no media_domain mapping (e.g., 3000/3010) do not constrain
      effective_media_domain_id.
- If a Torznab category maps to multiple media domains, allow all mapped domains; in that
  case effective_media_domain_id remains NULL and filtering uses the category list.

## 7. Policies and snapshots

### policy_set

Named collection of rules.

- PK: policy_set_id
- NN: policy_set_public_id (uuid)
- FK: user_id -> app_user.user_id (nullable; null = global)
- NN: display_name
- NN: scope (enum)
- NN: is_enabled (bool)
- NN: sort_order (int, default 1000)
- NN: is_auto_created (bool, default false)
- created_for_search_request_id -> search_request.search_request_id (nullable)
- NN: created_by_user_id -> app_user.user_id (0=system)
- NN: updated_by_user_id -> app_user.user_id (0=system)
- NN: created_at
- NN: updated_at
- deleted_at (nullable)
- UQ: (policy_set_public_id)

#### Notes

- scope=profile policy_sets must be linked via search_profile_policy_set.
- Policy set ordering uses sort_order ASC, then created_at ASC, then policy_set_public_id ASC.
- Reorder operations rewrite sort_order as 10,20,30... (gap of 10).
- Cardinality enforcement (v1): at most one enabled global policy_set per deployment and
  at most one enabled user policy_set per user.
- Auto-created request policy_sets set is_auto_created=true and created_for_search_request_id;
  retention hard-deletes only those tied to a purged search (rules cascade).
- Auto-created request policy_sets set user_id = NULL; user-supplied request policy_sets
  require user_id = actor_user_id.

### policy_rule

Atomic rule records.

- PK: policy_rule_id
- FK: policy_set_id -> policy_set.policy_set_id
- NN: policy_rule_public_id (uuid)
- NN: rule_type (enum)
- NN: match_field (enum)
- NN: match_operator (enum)
- NN: sort_order (int, default 1000)
- match_value_text (varchar(512), nullable)
- match_value_int (int, nullable)
- match_value_uuid (uuid, nullable)
- value_set_id (nullable) -> policy_rule_value_set.value_set_id
- NN: action (enum)
- NN: severity (enum)
- NN: is_case_insensitive (bool, default true)
- NN: is_disabled (bool, default false)
- rationale (varchar(1024), nullable)
- expires_at (nullable)
- NN: immutable_flag (bool, default false)
- NN: created_by_user_id -> app_user.user_id (0=system)
- NN: updated_by_user_id -> app_user.user_id (0=system)
- NN: created_at
- NN: updated_at
- UQ: (policy_rule_public_id)

#### Notes

- match*operator = in_set requires value_set_id and all match_value*\* NULL.
- match_field = indexer_instance_public_id uses match_value_uuid or uuid value set.
- match_field = trust_tier_key or media_domain_key uses match_value_text (lowercase).
- media_domain_key matching maps keys to media_domain_id via cached lookup.
- match_field in (title, release_group, uploader, tracker) uses match_value_text.
- match_field in (infohash_v1, infohash_v2, magnet_hash) only allows eq or in_set and
  requires strict hash validation (40 or 64 lowercase hex).
- match_field = indexer_instance_public_id only allows eq or in_set.
- require_trust_tier_min uses match_value_int as the minimum rank; match_operator must be eq.
- match_field for require_trust_tier_min must be trust_tier_rank (ignored by evaluation).
- allow_indexer_instance requires match_field = indexer_instance_public_id.
- sort_order controls rule ordering within a policy_set (ascending, tie-break by
  policy_rule_public_id). Precedence grouping and policy_set ordering are applied first.
- Reorder operations rewrite sort_order as 10,20,30... (gap of 10).
- Disabled rules are excluded from policy snapshots and may be re-enabled.
- is_disabled rules are excluded from snapshots and evaluation.
- immutable_flag is set once a rule is referenced by any policy_snapshot.
- Rules are immutable in v1; updates are modeled as disable + create.
- Rules are never hard-deleted in v1; disable is the only removal mechanism.
- Exception: rules in auto-created request policy_sets may be hard-deleted via retention.
- regex match_value_text max 512; non-regex max 256 (proc enforcement).
- eq/contains/starts_with/ends_with comparisons are case-insensitive by normalization.
- regex uses is_case_insensitive to decide compilation mode (default true).
- in_set comparisons are case-insensitive; stored text is normalized lowercase.

### policy_rule_value_set

- PK: value_set_id
- FK: policy_rule_id -> policy_rule.policy_rule_id
- NN: value_set_type (enum: text, int, bigint, uuid)
- UQ: (policy_rule_id)

### policy_rule_value_set_item

- PK: value_set_item_id
- FK: value_set_id
- value_text (varchar(256), nullable)
- value_bigint (nullable)
- value_int (nullable)
- value_uuid (uuid, nullable)
- UQ: (value_set_id, value_text) WHERE value_text IS NOT NULL
- UQ: (value_set_id, value_bigint) WHERE value_bigint IS NOT NULL
- UQ: (value_set_id, value_int) WHERE value_int IS NOT NULL
- UQ: (value_set_id, value_uuid) WHERE value_uuid IS NOT NULL

#### Notes

- Exactly one typed value is set; text is normalized lowercase.

### policy_snapshot

- PK: policy_snapshot_id
- NN: created_at
- NN: snapshot_hash (char(64))
- NN: ref_count (int, default 0)
- excluded_disabled_count (int, default 0)
- excluded_expired_count (int, default 0)
- UQ: (snapshot_hash)

#### Notes

- policy_snapshot is reusable across searches; ref_count tracks active references.
- policy_snapshot_gc deletes rows where ref_count=0 and created_at older than 30 days.
- policy_snapshot_refcount_repair recomputes ref_count daily.

### policy_snapshot_rule

- PK: policy_snapshot_rule_id
- FK: policy_snapshot_id -> policy_snapshot.policy_snapshot_id (ON DELETE CASCADE)
- NN: policy_rule_public_id (uuid)
- NN: rule_order (int)
- UQ: (policy_snapshot_id, rule_order)
- UQ: (policy_snapshot_id, policy_rule_public_id)

#### Notes

- rule_order is derived by precedence group, then policy_set ordering, then rule ordering:
    - precedence groups: request, profile, user, global.
    - policy_sets ordered by policy_set.sort_order ASC, then created_at ASC,
      then policy_set_public_id ASC.
- policy_rules ordered by policy_rule.sort_order ASC, then policy_rule_public_id ASC.
- Snapshots store only enabled, non-expired rules; excluded_disabled_count and
  excluded_expired_count record omissions.
- policy_snapshot rows are reusable; search_request references do not delete snapshots.
  ref_count is incremented on search_request_create_v1 and decremented when a search is purged.

#### Snapshot hash computation (v1)

- scope_bitmap is a 4-bit integer (0..15):
    - bit0 = global
    - bit1 = user
    - bit2 = profile
    - bit3 = request
- Canonical string (UTF-8, pipe-delimited):
    - {scope_bitmap}
    - g={global*policy_set_public_id_or*-}
    - u={user*policy_set_public_id_or*-}
    - p={profile*policy_set_public_ids_csv_or*-}
    - r={request*policy_set_public_id_or*-}
    - rules={rule_public_ids_csv}
- Empty scope representation is "-" (single dash).
- Profile policy_set list is ordered by sort_order, created_at, policy_set_public_id ASC.
- rule_public_ids_csv is ordered by precedence grouping, then policy_set ordering, then
  policy_rule.sort_order ASC, tie-break by policy_rule_public_id.
- Hash = SHA-256 of the canonical string, stored as 64 lowercase hex.
- Disabled rules are excluded from the snapshot and from the hash entirely.

#### Precedence and evaluation (v1)

- Effective policy stack order: request, profile, user, global.
- Policy_set ordering within each scope: sort_order ASC, then created_at ASC,
  then policy_set_public_id ASC.
- v1 scope cardinality: request 0/1, profile 0..N, user 0/1, global 0/1.
- Blocks win over allows at the same precedence level.
- Hard blocks that are not overridable: block_infohash_v1, block_infohash_v2, block_magnet.
- Higher-precedence allow(require) acts as an allowlist and drops non-matches regardless
  of lower-precedence non-hard blocks; prefer does not override blocks.
- allow\_\* with action=require is strict allowlist behavior (drop non-matches).
- allow\_\* with action=require combines by OR within the same rule_type and AND across types.
- Higher-precedence allowlists replace lower-precedence allowlists of the same type.
- allow*indexer_instance(require) gates scheduling; allow*\* require rules apply at the
  canonical level by default and only apply at the source level when the rule targets
  source fields (tracker, uploader).
- require_media_domain filters sources even when search_request.effective_media_domain_id is NULL
  (all-domain searches can still be narrowed by policy).
- require_trust_tier_min uses trust_tier.rank ordering (match_value_int threshold).
- Downrank values: soft -10, medium -25, hard -50 score delta.
- expires_at is evaluated only at snapshot creation; evaluation uses the snapshot and
  does not re-check wall-clock expiry during a search.
- Allowlist changes mid-search do not cancel existing runs and affect only future searches.
- allow\_\* with action=require is filter-only and does not adjust scores.
- prefer_indexer_instance +15; prefer_trust_tier +10; allow_title_regex(prefer) +8;
  allow_release_group(prefer) +8.
- prefer and downrank adjustments are additive; hard drop overrides everything.
- Regex evaluation uses Rust regex engine; max pattern length 512.
- eq/contains/starts_with/ends_with comparisons are case-insensitive by normalization.
- Regex case sensitivity uses policy_rule.is_case_insensitive.
- Max rules evaluated per result: 200 (reject oversized rules at write time).
- Rule_type and action combinations are validated at write time.
- policy rules are immutable in v1; updates are modeled as disable + create.
- Deletion is not allowed in v1; disable via is_disabled instead.
- expired and disabled rules are excluded at snapshot creation; evaluation uses the snapshot
  only and does not re-check wall-clock expiry during a search.

#### Allowed rule_type and action pairs (v1)

- block_infohash_v1 -> drop_canonical (hard only)
- block_infohash_v2 -> drop_canonical (hard only)
- block_magnet -> drop_canonical (hard only)
- block_title_regex -> drop_canonical, downrank, or flag
- block_release_group -> drop_source, downrank, or flag
- block_uploader -> drop_source, downrank, or flag
- block_tracker -> drop_source, downrank, or flag
- block_indexer_instance -> drop_source (v1)
- allow_release_group -> prefer or require
- allow_title_regex -> prefer or require
- allow_indexer_instance -> require (strict allowlist)
- downrank_title_regex -> downrank only
- require_trust_tier_min -> require (uses trust_tier.rank ordering)
- require_media_domain -> require
- prefer_indexer_instance -> prefer
- prefer_trust_tier -> prefer

#### Authoritative match sources (v1)

- title: canonical_torrent.title_normalized.
- release_group: canonical_torrent_signal(release_group), fallback to observation_attr.release_group
  for the current observation if present.
- tracker: canonical_torrent_source_attr.tracker_name.
- uploader: search_request_source_observation.uploader.
- media_domain: indexer_instance media_domain links.
- trust_tier: indexer_instance.trust_tier_key and trust_tier.rank.
- hashes: canonical_torrent hashes for canonical-level rules; observation hashes for source-level rules.

## 8. Search request, streaming pages, and cancellation

### search_request

One user query execution context (async and paged).

- PK: search_request_id
- NN: search_request_public_id (uuid)
- FK: user_id -> app_user.user_id (nullable)
- FK: search_profile_id -> search_profile.search_profile_id (nullable)
- FK: policy_set_id -> policy_set.policy_set_id (nullable)
- FK: policy_snapshot_id -> policy_snapshot.policy_snapshot_id
- FK: requested_media_domain_id -> media_domain.media_domain_id (nullable)
- FK: effective_media_domain_id -> media_domain.media_domain_id (nullable)
- NN: query_text (varchar(512), allow empty)
- NN: query_type (enum)
- torznab_mode (torznab_mode, nullable)
- NN: page_size (int, default 50, range 10..200)
- season_number (nullable)
- episode_number (nullable)
- NN: created_at
- canceled_at (nullable)
- finished_at (nullable)
- NN: status (enum)
- failure_class (nullable)
- error_detail (varchar(1024), nullable)
- UQ: (search_request_public_id)

#### Notes

- requested_media_domain_id stores caller intent:
    - explicit request provides requested_media_domain_key -> requested_media_domain_id
      (explicit request wins; ignore profile default).
    - otherwise copy search_profile.default_media_domain_id if provided.
- effective_media_domain_id is computed by intersecting:
    - requested_media_domain_id (from the rules above, if set),
    - Torznab category mapping for categories that map to a media_domain
      (ignore 8000 and unmapped categories such as 3000/3010),
    - policy require_media_domain rules (if present),
    - search_profile media_domain allowlist (if present).
      If intersection size is 0: finish search immediately. If size is 1: set that domain.
      If size > 1: set effective_media_domain_id=NULL to indicate multi-domain.
      If the request includes only categories with no media_domain mapping (e.g., 8000 or
      3000/3010), category filtering does not narrow domains; effective_media_domain_id is
      computed only from requested_media_domain_id/policy/allowlist constraints (if none,
      it remains NULL).
      Special case: if the profile media_domain allowlist has exactly one domain and no other
      domain constraints exist (no requested domain, no mapped cats, no require_media_domain),
      set effective_media_domain_id to that single allowed domain.
- Torznab category filters are stored as lists in join tables and contribute to the
  effective media domain intersection; they do not set requested_media_domain_id.
- If requested category list is empty, there is no category restriction.
- If requested categories include torznab_cat_id=8000 (Other), the category filter
  becomes a catch-all: results match if they map to any requested category OR do not
  map to any requested category.
- query_text may be empty for identifiers-only searches; store ''.
- user_id is NULL for Torznab API-key requests.
- REST search requests require authenticated user_id (actor_user_public_id non-NULL).
- Profile/policy changes apply to future searches only; in-flight searches use the
  derived values stored on search_request and policy_snapshot (no separate snapshot table).
- API surfaces any coercions applied during scheduling.
- v1 does not support requested indexer/tag subsets beyond media_domain, torznab category
  list, and search_profile selection.
- Page rendering uses canonical_torrent_best_source_context with
  context_key_type=search_request.
- search_request_create_v1 uses request_policy_set_public_id if provided; otherwise it
  auto-creates a request-scoped policy_set (may be empty). The policy_set_public_id is
  returned to callers, sets is_auto_created=true and created_for_search_request_id, and
  is cleaned up when the search is purged.
- policy_snapshot rows are reusable and are not deleted with their search_request.
- season_number and episode_number are >= 0; season=0/episode=0 represent specials.

### search_request_identifier

Normalized structured IDs used by a search request.

- PK: search_request_identifier_id
- FK: search_request_id -> search_request.search_request_id
- NN: id_type (enum)
- NN: id_value_normalized (varchar(32))
- NN: id_value_raw (varchar(64))
- UQ: (search_request_id, id_type)

#### Notes

- query_text is always stored as entered.
- imdb normalized format: tt + 7..9 digits (lowercase).
- tmdb/tvdb normalized format: 1..10 digits (digits only).
- id_value_raw stores the matched substring, not the full query.
- For explicit identifier params, id_value_raw stores the trimmed raw input (digits only);
  prefixed values like "tmdb:12345" are invalid in params.
- Identifier parsing from query_text (q) in v1:
    - Parse only imdb IDs and prefixed tmdb/tvdb IDs.
    - Do not parse plain digits without a prefix (ambiguous tmdb vs tvdb).
    - Do not parse infohashes or magnet URIs from q.
    - If any explicit identifier param is provided, ignore all identifier tokens in q.
    - Regexes require token boundaries (start/end or non-alphanumeric on both sides):
        - imdb: (?i)(?<![a-z0-9])tt([0-9]{7,9})(?![a-z0-9])
        - tmdb: (?i)(?<![a-z0-9])tmdb[:\\s]\*([0-9]{1,10})(?![a-z0-9])
        - tvdb: (?i)(?<![a-z0-9])tvdb[:\\s]\*([0-9]{1,10})(?![a-z0-9])
    - Accepted prefix separator for tmdb/tvdb: ":" or whitespace.
- If query_type is imdb/tmdb/tvdb, parse identifier from query_text; if parse fails and
  no explicit identifier provided, reject invalid_request (REST) or return empty results
  (Torznab).
- If an explicit identifier is provided, it wins over any parsed identifier from query_text.
- Multiple explicit identifiers (more than one of imdb/tmdb/tvdb) are invalid.
- If query_text contains multiple identifier types and no explicit id params are provided,
  or multiple matches of the same identifier type are found:
  REST returns invalid_identifier_combo; Torznab returns empty results.
- REST invalid_identifier_mismatch: query_type explicitly set to imdb/tmdb/tvdb but the
  provided identifier type does not match.
- If identifiers are present and exactly one type is provided, query_type is coerced to
  that identifier type; free_text indicates no explicit identifier type.
- media_domain override order:
    - requested_media_domain_key from request input if non-null; resolve to id in-proc.
    - else copy search_profile.default_media_domain_id into requested_media_domain_id.
- effective_media_domain_id computed via intersection (request/profile, categories
  that map to a media_domain; ignore 8000 and unmapped categories such as 3000/3010,
  policy require_media_domain, and profile media_domain allowlist). NULL means multi-domain
  (not necessarily all domains). If the profile allowlist has exactly one domain and no
  other domain constraints exist, set effective_media_domain_id to that single domain.
- API accepts media_domain_key; stored procedures resolve it to media_domain_id.
- If torznab_mode is NULL and query_type is season_episode, season_number and
  episode_number are required.
- If torznab_mode is NULL and query_type is not season_episode, season_number and
  episode_number must be NULL.
- If torznab_mode is tv, season_number is optional; episode_number requires season_number.
- If torznab_mode is generic or movie, season_number and episode_number must be NULL.
- query_text may be empty; validation requires either non-empty query_text or at least
  one identifier. For season_episode, require season/episode plus non-empty query_text
  or at least one identifier.
- page_size precedence: request input, search_profile.page_size, deployment_config.default_page_size.
- page_size is clamped to [10, 200].
- Terminal states require finished_at; canceled also sets canceled_at.
- Coordinator deterministically sets status=finished when all associated
  search_request_indexer_run rows are terminal, or immediately if no runnable indexers
  exist at creation time.
- status=failed is reserved for coordinator/system failure only (db_error or
  coordinator_error). Indexer failures never fail the search.
- REST invalid_request errors do not create a search_request row.
- search_profile allowlists filter runs; requests are accepted even if filters remove all runs,
  except when explicit category filters are reduced to an empty effective set by profile
  domain allowlist filtering (REST invalid_category_filter).
- search_request.policy_set_id is the explicit request-scope policy set; policy_snapshot_id is authoritative.
- Torznab uses torznab_mode for search semantics; query_type is set to the single
  identifier type when exactly one is present, otherwise free_text. Invalid Torznab
  combinations return empty results while native REST returns invalid_request errors.
- Torznab season-only searches (torznab_mode=tv) are allowed when season_number is
  present and either query_text or an identifier is present; episode_number requires
  season_number.

### search_request_torznab_category_requested

Torznab category filters as requested by the client.

- PK: search_request_torznab_category_requested_id
- FK: search_request_id -> search_request.search_request_id
- FK: torznab_category_id -> torznab_category.torznab_category_id
- UQ: (search_request_id, torznab_category_id)

### search_request_torznab_category_effective

Torznab category filters after mapping and allowlist intersection.

- PK: search_request_torznab_category_effective_id
- FK: search_request_id -> search_request.search_request_id
- FK: torznab_category_id -> torznab_category.torznab_category_id
- UQ: (search_request_id, torznab_category_id)

#### Notes

- Effective categories derivation (v1):
    1. Start from requested categories after sanitizing to known torznab_category IDs.
    2. If requested includes 8000:
        - effective = requested (keep 8000 and all others).
        - skip domain narrowing from categories.
    3. Else effective = requested.
    4. If search_profile domain allowlist exists:
        - keep only categories that map to at least one allowed domain.
        - categories with no domain mapping are dropped from effective (unless 8000 was requested).
    5. Do not auto-convert unmapped categories into 8000; 8000 must be explicitly requested.
    6. If explicit cat filters are provided and the effective set becomes empty after
       allowlist filtering:
        - Torznab returns empty results (no DB writes).
        - REST returns invalid_category_filter.

### search_request_indexer_run

Tracks per-indexer execution state within a search.

- PK: search_request_indexer_run_id
- FK: search_request_id -> search_request.search_request_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- started_at (nullable)
- finished_at (nullable)
- next_attempt_at (nullable)
- NN: attempt_count (int, default 0)
- NN: rate_limited_attempt_count (int, default 0)
- last_error_class (error_class, nullable)
- last_rate_limit_scope (rate_limit_scope, nullable)
- last_correlation_id (uuid, nullable)
- NN: status (enum)
- error_class (enum, nullable)
- error_detail (varchar(1024), nullable)
- NN: items_seen_count
- NN: items_emitted_count
- NN: canonical_added_count
- UQ: (search_request_id, indexer_instance_id)

#### Status timestamp rules

- started_at required when status in (running, finished, failed, canceled).
- finished_at required when status in (finished, failed, canceled).
- started_at and finished_at NULL for queued.
- Rate-limited runs remain queued; next_attempt_at is set with backoff and
  last_error_class=rate_limited and last_rate_limit_scope set.
- Rate-limited deferrals still create a correlation_id and outbound_request_log row,
  and insert a search_request_indexer_run_correlation entry.
- error_class is set only when status=failed; last_error_class tracks the most recent
  failed attempt (including transient failures).
- next_attempt_at NULL means runnable now; a non-NULL value defers execution until
  that timestamp.
- attempt_count increments on every attempt (rate-limited, failed, and successful).
- attempt_count increments per outbound request attempt, including each page fetch.
- rate_limited_attempt_count increments only on rate-limited deferrals.
- items_seen_count increments per result parsed from upstream (pre-filter, pre-dedupe).
- items_emitted_count increments per result that survives filtering and is eligible for
  canonicalization (post-filter, pre-dedupe).
- canonical_added_count increments per canonical actually inserted into
  search_request_canonical (post-dedupe).
- Retries that return the same page still increment items_seen_count and items_emitted_count;
  canonical_added_count increments only when a canonical is newly added.
- Backoff formula for rate-limited attempts:
    - base = 5s
    - increment rate_limited_attempt_count first; let n = rate_limited_attempt_count - 1.
    - delay_no_jitter = min(base \* 2^n, 5 minutes).
    - jitter_pct = uniform integer in [0, 25] from OS CSPRNG.
    - jitter_seconds = floor(delay_no_jitter \* jitter_pct / 100).
    - next_attempt_at = now() + delay_no_jitter + jitter_seconds.
- If rate_limited_attempt_count >= 10, mark run failed with error_class=rate_limited
  and emit a final outbound_request_log entry.
- last_correlation_id stores the most recent outbound_request_log.correlation_id for
  the run.

#### Retry policy (v1, non-rate-limited)

- Retry budget is per logical page fetch (correlation_id group).
- Retryable error_class: timeout, http_5xx, dns, connection_refused.
- tls: max 1 retry (retry_seq 0..1).
- parse_error: max 1 retry (retry_seq 0..1).
- Not retryable: auth_error, http_403 (unless cf_detected -> cf_challenge), cf_challenge.
- http_429 uses a separate backoff and consumes retry budget:
    - delay_no_jitter = min(30s \* 2^retry_seq, 600s).
    - If Retry-After is present and parseable, delay_no_jitter =
      max(delay_no_jitter, retry_after_seconds).
    - jitter_pct = uniform integer in [0, 25] from OS CSPRNG.
    - jitter_seconds = floor(delay_no_jitter \* jitter_pct / 100).
    - delay = delay_no_jitter + jitter_seconds.
- Backoff for retryable failures: base 2s, multiplier 2x, cap 120s, jitter 0–25% of delay.
  delay_no_jitter = min(2s \* 2^retry_seq, 120s);
  jitter_seconds = floor(delay_no_jitter \* jitter_pct / 100);
  delay = delay_no_jitter + jitter_seconds.
- Max retries per page request: retry_seq allowed 0..3 (4 total attempts).
- Retryable failures keep status=queued, set last_error_class, and set next_attempt_at
  using the backoff; no retry sub-state.
- http_429 retries increment attempt_count and consume retry budget; they do not
  increment rate_limited_attempt_count.

### search_request_indexer_run_correlation

Run-to-request correlation mapping for outbound logs.

- PK: search_request_indexer_run_correlation_id
- FK: search_request_indexer_run_id -> search_request_indexer_run.search_request_indexer_run_id
- NN: correlation_id (uuid)
- page_number (nullable)
- NN: created_at
- UQ: (search_request_indexer_run_id, correlation_id)

### indexer_run_cursor

Typed pagination cursor state for an indexer run.

- PK: indexer_run_cursor_id
- FK: search_request_indexer_run_id -> search_request_indexer_run.search_request_indexer_run_id
- NN: cursor_type (enum)
- offset (nullable)
- limit (nullable)
- page (nullable)
- since (nullable)
- opaque_token (varchar(1024), nullable)
- UQ: (search_request_indexer_run_id)

### search_request_canonical

Ensures a canonical appears only once per search.

- PK: search_request_canonical_id
- FK: search_request_id -> search_request.search_request_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- NN: first_seen_at
- UQ: (search_request_id, canonical_torrent_id)

### search_page

Represents page-ready boundaries (stabilizes UI).

- PK: search_page_id
- FK: search_request_id -> search_request.search_request_id
- NN: page_number
- sealed_at (nullable)
- UQ: (search_request_id, page_number)

### search_page_item

Stable ordering of canonical results within a page.

- PK: search_page_item_id
- FK: search_page_id -> search_page.search_page_id
- FK: search_request_canonical_id -> search_request_canonical.search_request_canonical_id
- NN: position
- UQ: (search_page_id, position)
- UQ: (search_request_canonical_id)

### search_request_source_observation

Search-scoped snapshot of a durable source.

- PK: observation_id
- FK: search_request_id -> search_request.search_request_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- FK: canonical_torrent_source_id -> canonical_torrent_source.canonical_torrent_source_id
- NN: observed_at
- seeders (nullable)
- leechers (nullable)
- published_at (nullable)
- uploader (varchar(256), nullable)
- source_guid (varchar(256), nullable)
- details_url (varchar(2048), nullable)
- download_url (varchar(2048), nullable)
- magnet_uri (varchar(2048), nullable)
- NN: title_raw (varchar(512))
- size_bytes (nullable)
- infohash_v1 (char(40), nullable)
- infohash_v2 (char(64), nullable)
- magnet_hash (char(64), nullable)
- NN: guid_conflict (bool, default false)
- NN: was_downranked (bool, default false)
- NN: was_flagged (bool, default false)

### search_request_source_observation_attr

Typed observation-only attributes for Torznab fields and diagnostics.

- PK: observation_attr_id
- FK: observation_id -> search_request_source_observation.observation_id
- NN: attr_key (observation_attr_key)
- value_text (varchar(512), nullable)
- value_int (nullable)
- value_bigint (nullable)
- value_numeric (numeric(12,4), nullable)
- value_bool (bool, nullable)
- value_uuid (uuid, nullable)
- NN: created_at
- UQ: (observation_id, attr_key)

#### Notes

- Observations are deleted by retention; durable sources remain.
- Idempotency keys are defined in section 15.
- observation_public_id is omitted in v1.
- observed_at defaults to now() when not provided.
- Exactly one value\_\* is set on observation_attr.
- Observation attrs include tracker_name, tracker_category, tracker_subcategory,
  size_bytes_reported, files_count, and release_group as-seen.
- Optional language_primary and subtitles_primary store a single preferred value for UI;
  multi-valued language/subtitles live in canonical_torrent_signal.
- release_group should be stored only when parser confidence >= 0.8 and the group token
  is a terminal suffix; otherwise omit.
- Observation-only keys include freeleech, internal_flag, scene_flag, minimum_ratio,
  minimum_seed_time_hours.
- observation*attr_key values map to search_request_source_observation_attr; durable_source_attr_key
  values are written to canonical_torrent_source_attr, and tracker*\* plus size_bytes_reported
  and files_count are mirrored to observation_attr when present.
- guid_conflict marks source_guid backfill conflicts.
- was_downranked is true if any downrank rule matched for the observation; was_flagged is true
  if any flag rule matched. These may still be true even if the observation is dropped.

#### Type map (v1)

- durable_source_attr_key types follow canonical_torrent_source_attr.
- observation-only keys:
    - freeleech, internal_flag, scene_flag -> value_bool.
    - minimum_ratio -> value_numeric.
    - minimum_seed_time_hours -> value_int.
    - language_primary, subtitles_primary -> value_text.
    - tracker_name, release_group -> value_text.
    - tracker_category, tracker_subcategory, files_count -> value_int.
    - size_bytes_reported -> value_bigint.

#### State machines (v1)

- search_request: running -> finished, canceled, or failed.
- search_request_indexer_run: queued -> running -> finished or failed; queued or running -> canceled.
- Rate-limited runs remain queued with next_attempt_at set.
- Indexer run failure does not fail the entire search; the search only fails if
  the coordinator fails.

#### Page sizing and sealing (v1)

- page_number starts at 1.
- A page is sealed when it reaches page_size.
- Once sealed, ordering is fixed; only inline fields update (seed counts, best_source_context).
- Insertion is append-only; late higher-score items do not reorder existing pages.
- Each canonical appears at most once per search_request via search_request_canonical.

#### Cancellation (v1)

- Coordinator sets canceled_at and finished_at and emits an SSE cancel event.
- Runners stop after current request completes; late results are discarded.

## 9. Canonical torrents (dedupe and collapse)

### canonical_torrent

One deduplicated content item.

- PK: canonical_torrent_id
- NN: canonical_torrent_public_id (uuid)
- NN: identity_confidence (numeric(4,3))
- NN: identity_strategy (enum)
- infohash_v1 (char(40), nullable)
- infohash_v2 (char(64), nullable)
- magnet_hash (char(64), nullable)
- title_size_hash (char(64), nullable)
- imdb_id (varchar(16), nullable)
- tmdb_id (int, nullable)
- tvdb_id (int, nullable)
- ids_confidence (numeric(4,3), nullable)
- NN: title_display
- NN: title_normalized
- size_bytes (nullable)
- NN: created_at
- NN: updated_at
- UQ: (canonical_torrent_public_id)
- UQ: (infohash_v2) WHERE infohash_v2 IS NOT NULL
- UQ: (infohash_v1) WHERE infohash_v1 IS NOT NULL
- UQ: (magnet_hash) WHERE magnet_hash IS NOT NULL
- UQ: (title_size_hash) WHERE title_size_hash IS NOT NULL

#### Identity strategy (v1)

- priority: infohash_v2 > infohash_v1 > magnet_hash > title_size_fallback.
- identity_confidence:
    - infohash_v2: 1.0
    - infohash_v1: 1.0
    - magnet_hash: 0.85
    - title_size_fallback: 0.60 (mark low confidence in UI)
- infohash parsed from magnet xt=urn:btih or xt=urn:btmh is treated as confidence 1.0.
- For title_size_fallback, title_size_hash = sha256_hex(title_normalized || '|' || size_bytes).

#### Normalization rules (v1)

- lowercase, unicode NFKD, strip diacritics.
- replace separators with spaces, collapse whitespace.
- remove common release tokens for normalization only (see token list below).

#### External IDs (v1)

- imdb_id stored as lowercase tt + 7-9 digits.
- tmdb_id and tvdb_id stored as positive ints.
- canonical_torrent holds the best-known IDs; conflicts are resolved by highest
  trust tier source and alternates are preserved in canonical_external_id.
- canonical_torrent.imdb_id/tmdb_id/tvdb_id update when a higher trust-tier source
  introduces a different ID.

#### title_display selection (v1)

- title_display is selected from the highest-trust-tier observation for the canonical.
- Tie-breakers: higher base score source, then most recent observed_at.
- title_display may change over time; title_normalized remains immutable where applicable.
- After observations are purged by retention, title_display remains unchanged until a
  new observation arrives.
- size_bytes derives from canonical_size_rollup.size_median for hash-based identities.
- For title_size_fallback, size_bytes is set from the first non-null sample and is
  immutable once set.
- When fewer than 3 size samples exist for hash-based canonicals, size_bytes uses the
  first non-null sample.

### canonical_size_rollup

Median size rollup per canonical (robust to outliers).

- PK: canonical_size_rollup_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- NN: sample_count (int)
- NN: size_median (bigint)
- NN: size_min (bigint)
- NN: size_max (bigint)
- NN: updated_at
- UQ: (canonical_torrent_id)

#### Notes

- size_median updates when at least 3 samples exist.
- For title_size_fallback canonicals, rollups may update but do not update
  canonical_torrent.size_bytes.

### canonical_size_sample

Bounded size samples used to compute the median.

- PK: canonical_size_sample_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- NN: observed_at
- NN: size_bytes (bigint)
- UQ: (canonical_torrent_id, observed_at, size_bytes)

#### Notes

- Ignore size_bytes <= 0 when sampling.
- Ignore size_bytes > 10 TiB unless media_domain is one of ebooks, audiobooks, or software.
  Media_domain selection priority: effective_media_domain_id if set; if NULL (including
  multi-domain requests), fall back to a single indexer_instance domain if exactly one,
  else treat as unknown and apply the cutoff.
- Keep the newest N=25 samples per canonical by observed_at (v1).
- Samples are retained with the canonical; no purge unless the canonical is pruned.

### canonical_external_id

Alternate or conflicting external IDs for a canonical.

- PK: canonical_external_id_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- NN: id_type (enum: imdb, tmdb, tvdb)
- id_value_text (varchar(16), nullable)
- id_value_int (int, nullable)
- FK: source_canonical_torrent_source_id -> canonical_torrent_source.canonical_torrent_source_id
  (nullable)
- NN: trust_tier_rank (smallint)
- NN: first_seen_at
- NN: last_seen_at
- UQ: (canonical_torrent_id, id_type, id_value_text) WHERE id_value_text IS NOT NULL
- UQ: (canonical_torrent_id, id_type, id_value_int) WHERE id_value_int IS NOT NULL

#### Notes

- id_type uses identifier_type enum.
- Exactly one of id_value_text or id_value_int is set.
- imdb id_value_text must match tt[0-9]{7,9} (lowercase).
- tmdb and tvdb ids use id_value_int > 0.
- Insert a row for every observed ID value and update last_seen_at on repeat observations.
- Repeated observations from the same source update last_seen_at; duplicates are allowed
  as long as the (canonical, id_type, id_value) pair is the same.
- trust_tier_rank is stored at ingest time; use rank=0 when the indexer has no trust tier.
- canonical_torrent.imdb_id/tmdb_id/tvdb_id selects the highest trust_tier_rank; ties
  break by highest base score source, then most recent observation.

### canonical_torrent_source

Durable source identity per indexer instance.

- PK: canonical_torrent_source_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- NN: canonical_torrent_source_public_id (uuid)
- source_guid (varchar(256), nullable)
- infohash_v1 (char(40), nullable)
- infohash_v2 (char(64), nullable)
- magnet_hash (char(64), nullable)
- NN: title_normalized (varchar(512))
- size_bytes (nullable)
- NN: last_seen_at (timestamptz, default now())
- last_seen_seeders (nullable)
- last_seen_leechers (nullable)
- last_seen_published_at (nullable)
- last_seen_download_url (varchar(2048), nullable)
- last_seen_magnet_uri (varchar(2048), nullable)
- last_seen_details_url (varchar(2048), nullable)
- last_seen_uploader (varchar(256), nullable)
- NN: created_at
- NN: updated_at
- UQ: (canonical_torrent_source_public_id)
- UQ: (indexer_instance_id, source_guid)
  WHERE source_guid IS NOT NULL

#### Notes

- Durable sources are not deleted by search retention.
- URLs and per-search snapshots live on search_request_source_observation.
- last*seen*\* fields are updated on idempotent observation ingest and drive base scoring
  after observations are purged.
- last*seen*\* updates only apply when observed_at is newer than last_seen_at.
- title_normalized is required for all durable sources and is immutable once set.

### canonical_torrent_source_attr

Whitelisted extra fields for a source, typed.

- PK: canonical_torrent_source_attr_id
- FK: canonical_torrent_source_id -> canonical_torrent_source.canonical_torrent_source_id
- NN: attr_key (durable_source_attr_key)
- value_text (varchar(512), nullable)
- value_int (nullable)
- value_bigint (nullable)
- value_numeric (numeric(12,4), nullable)
- value_bool (bool, nullable)
- UQ: (canonical_torrent_source_id, attr_key)

#### Type map (v1)

- value_text:
    - tracker_name, imdb_id.
- value_bigint:
    - size_bytes_reported.
- value_int:
    - tracker_category, tracker_subcategory, files_count, season, episode, year, tmdb_id,
      tvdb_id.
- value_numeric:
    - reserved (no durable numeric keys in v1).
- value_bool:
    - reserved (no durable boolean keys in v1).

#### Notes

- Exactly one value\_\* is set.
- imdb_id must match tt[0-9]{7,9} (lowercase).
- tmdb_id and tvdb_id must be > 0.
- tracker_category and tracker_subcategory must be >= 0.
- language and subtitles are stored in canonical_torrent_signal (v1).
- release_group is stored in canonical_torrent_signal and observation attrs, not durable.
- freeleech, internal_flag, scene_flag, minimum_ratio, and minimum_seed_time_hours are
  observation-only and are stored in search_request_source_observation_attr.
- stored procedures reject durable writes for observation-only keys.

### source_metadata_conflict

Structured record of durable metadata conflicts for operator review.

- PK: source_metadata_conflict_id
- FK: canonical_torrent_source_id -> canonical_torrent_source.canonical_torrent_source_id
- NN: conflict_type (conflict_type)
- NN: existing_value (varchar(256))
- NN: incoming_value (varchar(256))
- NN: observed_at
- resolved_at (nullable)
- resolved_by_user_id -> app_user.user_id (nullable)
- resolution (conflict_resolution, nullable)
- resolution_note (varchar(256), nullable)

#### Notes

- Inserted when durable tracker metadata, hashes, external IDs, or source_guid values
  conflict with incoming observations.
- source_guid conflicts store conflict_type=source_guid.
- For source_guid conflicts, existing_value stores canonical_torrent_source_public_id and
  incoming_value stores the conflicting GUID string.
- identity_conflict indexer_health_event is also emitted.
- Retention uses deployment_config.retention_source_metadata_conflict_days (default 30).
- Resolution actions are recorded in source_metadata_conflict_audit_log (not config_audit_log).
- A source_metadata_conflict_audit_log row with action=created is auto-inserted when a
  conflict is created (actor_user_id NULL when system-generated).

### source_metadata_conflict_audit_log

Operational audit log for conflict resolution actions.

- PK: source_metadata_conflict_audit_log_id
- FK: conflict_id -> source_metadata_conflict.source_metadata_conflict_id
- NN: action (source_metadata_conflict_action)
- FK: actor_user_id -> app_user.user_id (nullable)
- NN: occurred_at
- note (varchar(256), nullable)

#### Notes

- Retention uses deployment_config.retention_source_metadata_conflict_audit_days (default 90).

### canonical_torrent_source_base_score

Global, policy-agnostic base score.

- PK: canonical_torrent_source_base_score_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- FK: canonical_torrent_source_id -> canonical_torrent_source.canonical_torrent_source_id
- NN: score_total_base (numeric(12,4))
- NN: score_seed (numeric(12,4))
- NN: score_leech (numeric(12,4))
- NN: score_age (numeric(12,4))
- NN: score_trust (numeric(12,4))
- NN: score_health (numeric(12,4))
- NN: score_reputation (numeric(12,4))
- NN: computed_at
- UQ: (canonical_torrent_id, canonical_torrent_source_id)

#### Notes

- score_total_base is clamped to [-10000, 10000] and rounded to 4 decimals at write time.

### canonical_torrent_source_context_score

Context-specific adjustments for ordering within a policy snapshot or profile/search view.

- PK: canonical_torrent_source_context_score_id
- NN: context_key_type (enum)
- NN: context_key_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- FK: canonical_torrent_source_id -> canonical_torrent_source.canonical_torrent_source_id
- NN: score_total_context (numeric(12,4))
- NN: score_policy_adjust (numeric(12,4))
- NN: score_tag_adjust (numeric(12,4))
- NN: is_dropped (bool, default false)
- NN: computed_at
- UQ: (context_key_type, context_key_id, canonical_torrent_id, canonical_torrent_source_id)

#### Notes

- context_key_id refers to policy_snapshot, search_profile, or search_request by type.
- Context scores are required for streaming searches; only search_request contexts are
  persisted in v1. Profile contexts compute on read and cache in app memory for 1 hour.
- Hard-dropped sources still get a context score row with is_dropped=true and
  score_total_context set to -10000.
- score_total_context is clamped to [-10000, 10000] and rounded to 4 decimals at write time.

### canonical_torrent_best_source_global (derived)

Global default best source using base score only.

- PK: canonical_torrent_best_source_global_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- FK: canonical_torrent_source_id -> canonical_torrent_source.canonical_torrent_source_id
- NN: computed_at
- UQ: (canonical_torrent_id)

### canonical_torrent_best_source_context (derived)

Best source for a specific policy or search context.

- PK: canonical_torrent_best_source_context_id
- NN: context_key_type (enum)
- NN: context_key_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- FK: canonical_torrent_source_id -> canonical_torrent_source.canonical_torrent_source_id
- NN: computed_at
- UQ: (context_key_type, context_key_id, canonical_torrent_id)

#### Notes

- best*source*\* always references a durable source, not observations.
- best_source_global tie-break order:
    - score_total_base DESC,
    - canonical_torrent_source.last_seen_at DESC,
    - canonical_torrent_source_public_id ASC.
- best_source_context uses context_key_type=search_request for streaming search pages.

### canonical_torrent_signal

Structured extraction signals from title or metadata parsing.

- PK: canonical_torrent_signal_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- NN: signal_key (enum)
- value_text (varchar(128), nullable)
- value_int (nullable)
- NN: confidence (numeric(4,3))
- NN: parser_version (smallint, default 1)
- UQ: (canonical_torrent_id, signal_key, value_text, value_int)

#### Type map (v1)

- value_int:
    - year, season, episode.
- value_text:
    - release_group, resolution, source_type, codec, audio_codec, container,
      language, subtitles, edition.

#### Notes

- Exactly one of value_text or value_int is set.
- Multiple rows per signal_key are allowed (value is part of the uniqueness).
- confidence base = 0.5 + 0.1 \* trust_tier_rank_bucket (public=0, semi_private=1,
  private=2, invite_only=3).
- Each additional supporting source adds +0.05; cap at 1.0.
- confidence never decreases in v1.
- release_group is only stored when parser confidence >= 0.8 and the group token is a
  terminal suffix (e.g., "-GROUP"); otherwise omit.

### canonical_disambiguation_rule

User-owned rule that prevents merging of canonicals.

- PK: canonical_disambiguation_rule_id
- NN: created_by_user_id -> app_user.user_id (0=system)
- NN: created_at
- NN: rule_type (enum: prevent_merge)
- NN: identity_left_type (enum)
- identity_left_value_text (varchar(64), nullable)
- identity_left_value_uuid (uuid, nullable)
- NN: identity_right_type (enum)
- identity_right_value_text (varchar(64), nullable)
- identity_right_value_uuid (uuid, nullable)
- reason (varchar(256), nullable)
- UQ: (identity_left_type, identity_left_value_text, identity_left_value_uuid,
  identity_right_type, identity_right_value_text, identity_right_value_uuid)

#### Notes

- identity values use public UUIDs or hashes, never internal PKs.
- identity_left and identity_right cannot be identical.
- identity pairs are stored in canonical order to enforce symmetric uniqueness.
- canonical ordering comparator:
    - identity_type order: canonical_public_id, infohash_v2, infohash_v1, magnet_hash.
    - within type: lexicographic on normalized value (UUID bytes or lowercase hex).

#### Canonical conflict resolution (v1)

- If two canonicals share the same infohash, merge via stored procedure unless a
  prevent_merge rule exists.
- Fallback dedupe collisions can be separated by a prevent_merge rule.

#### Title normalization token list (v1)

- Resolutions: 2160p, 1080p, 720p, 480p, 4320p, 4k, 8k.
- Sources: web, webrip, web-dl, webdl, bluray, blu-ray, bdrip, brip, dvdrip,
  hdrip, hdtv, tvrip, cam, ts, tc, scr, screener.
- Codecs and video: x264, x265, h264, h265, hevc, avc, xvid, divx, vp9, av1.
- HDR or color: hdr, hdr10, hdr10plus, dv, dolbyvision.
- Audio: aac, ac3, eac3, ddp, dts, dtshd, truehd, atmos, flac, mp3, opus.
- Channels: 2.0, 5.1, 7.1.
- Containers: mkv, mp4, avi.
- Common flags: repack, proper, rerip, extended, uncut, remux.
- Language tokens: multi, dual, eng, en, ita, it, spa, es, fre, fr, ger, de, jpn,
  jp, kor, kr.
- Group suffix handling: if title contains " - GROUP" or ends with "-GROUP", remove the
  group token from normalized title and store as release_group signal.

#### release_group confidence (v1)

- Confidence starts at 0.0.
- +0.6 if token is a terminal group suffix "-GROUP" and GROUP matches [A-Za-z0-9]{2,20}.
- +0.2 if preceding tokens include a standard release pattern (resolution/source/codec).
- +0.1 if no whitespace inside the group token.
- -0.2 if group token matches common false positives (REPACK, PROPER, WEB).
- Store release_group only if confidence >= 0.8.

#### Best source scoring model (v1)

Two-layer scoring: base (global) + context (policy/profile/search adjustments).

Base score components (canonical_torrent_source_base_score):

- score_seed = ln(1 + seeders) \* w_seed
- score_leech = ln(1 + leechers) \* w_leech
- score_age = age bucket score \* w_age
- score_trust = trust_tier.default_weight (no profile override in base)
- score_health = latency/error penalties (plus NULL seed/leech penalty)
- score_reputation:
    - acquisition_success_rate if acquisition_count >= 10,
    - else request_success_rate if request_count >= 30,
    - else 0.5 (neutral)
- score_reputation = (rep_rate - 0.5) \* 10, capped to [-5, +5]
- score_total_base = clamp(
  score_seed + score_leech + score_age + score_trust + score_health + score_reputation,
  -10000,
  10000
  ), rounded to 4 decimal places at write time
- No normalization beyond clamping; components are already bounded.

Context score components (canonical_torrent_source_context_score):

- score*policy_adjust = prefer/downrank adjustments; allow*\* with action=require adds no score
  (allow_title_regex(prefer) +8; allow_release_group(prefer) +8)
- score_tag_adjust = tag preference bonus (clamp to [-15, +15])
- score_total_context = clamp(
  score_total_base + score_policy_adjust + score_tag_adjust,
  -10000,
  10000
  ), rounded to 4 decimals at write time
- Ranking within a context uses score_total_context DESC with deterministic tie-breaks:
  score_total_base DESC, observation.observed_at DESC (fallback to
  canonical_torrent_source.last_seen_at),
  append_order ASC (search_page_item page_number ASC, position ASC),
  canonical_torrent_source_public_id ASC.

Weights:

- w_seed = 10.0
- w_leech = 2.0
- w_age by domain: movies 1.0, tv 3.0, audiobooks 0.5, ebooks 0.5, software 0.75,
  adult_movies 1.5, adult_scenes 2.0

Age bucket scoring:

- < 6h: +6 \* w_age
- < 24h: +4 \* w_age
- < 72h: +2 \* w_age
- < 14d: +1 \* w_age
- else: +0

Health penalty (score_health is the sum of latency, dominant error, NULL penalties, and quarantine):

- latency p95 <= 500ms: 0
- latency p95 <= 1500ms: -2
- latency p95 <= 4000ms: -5
- latency p95 > 4000ms: -10
- error_class none: 0
- http_429: -8
- timeout: -10
- cf_challenge: -12
- auth_error: -15
- http_403: -10
- tls, dns, or connection_refused: -12
- parse_error: -8
- http_5xx: -6
- unknown: -5
- quarantined: treat as excluded; if used, apply -1000

#### Scoring notes (v1)

- NULL seeders or leechers: seed_component = 0, leech_component = 0, and apply -0.5
  per missing value to score_health (max -1).
- NULL trust_tier: rank 0 and weight 0.
- If published*at is NULL, recency uses observed_at with w_age * 0.5; base score refresh
  uses last*seen_at when last_seen_published_at is NULL, also with w_age * 0.5.
- If effective_media_domain_id is NULL/unknown, use w_age = 1.0 (no torznab_mode override in v1).
- trust_tier overrides apply only in context scoring; base scores always use
  trust_tier.default_weight.
- For search_request context scoring, apply linked search_profile preferences; if no
  profile is attached, do not apply tag or trust overrides.
- trust_tier overrides adjust score_policy_adjust by (override - default_weight) for the
  context key.
- Tag preference bonus is applied in score_tag_adjust; sum per tag weight_override and
  clamp to [-15, +15].
- allow\_\* with action=require never adds to score_policy_adjust.
- Component scores are stored as numeric(12,4) without clamping; only score_total_base
  and score_total_context are clamped to [-10000, 10000] and rounded at write time.
- Context scores are materialized only for search_request contexts; search_profile contexts
  compute on read and cache in app memory for 1 hour in v1 (not persisted).
- Profile context cache is keyed by (search_profile_id).
- A cached profile context score is stale if computed_at is older than 1 hour or if the
  base score computed_at is newer than the context score.
- Base scores are refreshed by the hourly base_score_refresh_recent job; no immediate
  recompute on bucket changes in v1.
- Tie-break order applies to scoring-ranked lists (best source selection, internal pick
  logic, future UI sorted views), not Torznab append-order responses.
- best_source_context may update mid-search when a new source exceeds the current best
  by a material margin: score delta >= 2.0 or a seed bucket jump from 20 to 100+.
  Page order remains stable.

#### Material change buckets (reference only)

- seeders bucket changes: 0, 1-4, 5-19, 20-99, 100-499, 500+.
- leechers bucket changes: 0, 1-9, 10-49, 50+.
- published_at crosses recency buckets (6h, 24h, 72h, 14d).
- connectivity_status changes (healthy, degraded, failing, quarantined).
- policy_snapshot changes for a search_request (future searches only).
- tag preference changes apply to future searches only (v1).

## 10. Filtering outcomes: what got dropped and why

### search_filter_decision

Captures policy actions for transparency and debugging.

- PK: search_filter_decision_id
- FK: search_request_id -> search_request.search_request_id
- NN: policy_rule_public_id (uuid)
- NN: policy_snapshot_id -> policy_snapshot.policy_snapshot_id
- FK: observation_id -> search_request_source_observation.observation_id (nullable)
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id (nullable)
- FK: canonical_torrent_source_id -> canonical_torrent_source.canonical_torrent_source_id
  (nullable)
- NN: decision (enum)
- decision_detail (varchar(512), nullable)
- NN: decided_at

#### Notes

- Must reference at least one of canonical_torrent_id or canonical_torrent_source_id.
- When known, populate both canonical_torrent_id and canonical_torrent_source_id.
- observation_id is set when the decision is tied to a specific observation.

## 11. Feedback loop: user actions and acquisition outcomes

### user_result_action

Tracks user choices on results to improve ranking and block suggestions.

- PK: user_result_action_id
- FK: user_id -> app_user.user_id
- FK: search_request_id -> search_request.search_request_id
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- NN: action (enum)
- NN: reason_code (enum)
- reason_text (varchar(512), nullable)
- NN: created_at

### user_result_action_kv

Optional structured metadata for user actions.

- PK: user_result_action_kv_id
- FK: user_result_action_id -> user_result_action.user_result_action_id
- NN: key (enum)
- NN: value (varchar(512))
- UQ: (user_result_action_id, key)

### acquisition_attempt

When the torrent is actually used in the torrent client.

- PK: acquisition_attempt_id
- FK: torznab_instance_id -> torznab_instance.torznab_instance_id (nullable)
- NN: origin (acquisition_origin)
- FK: canonical_torrent_id -> canonical_torrent.canonical_torrent_id
- FK: canonical_torrent_source_id -> canonical_torrent_source.canonical_torrent_source_id
- FK: search_request_id -> search_request.search_request_id (nullable)
- FK: user_id -> app_user.user_id (nullable)
- infohash_v1 (char(40), nullable)
- infohash_v2 (char(64), nullable)
- magnet_hash (char(64), nullable)
- torrent_client_id (varchar(128), nullable)
- NN: torrent_client_name (enum)
- NN: started_at
- finished_at (nullable)
- NN: status (enum)
- failure_class (enum, nullable)
- failure_detail (varchar(256), nullable)

#### Notes

- At least one of infohash_v1, infohash_v2, or magnet_hash must be set.
- failure_class required when status=failed.
- failure_detail uses fixed codes (see list below).
- torznab_instance_id is set for Torznab-triggered acquisitions; NULL for native UI/API.
- origin records the caller context (torznab, ui, api, automation) and disambiguates
  when torznab_instance_id is NULL.
- On download endpoint hit, create acquisition_attempt with status=started.
- If no download target exists, set status=failed immediately.
- Torrent client integration marks succeeded on completion in v1.
- user_id is NULL for Torznab API-key auth in v1.
- UQ: (torrent_client_name, torrent_client_id)
  WHERE torrent_client_id IS NOT NULL AND torrent_client_name != 'unknown'.
- Mapping from torrent client events uses infohash_v2 > infohash_v1 > magnet_hash.
- failure_detail codes (v1): no_download_target, redirect_failed, client_rejected,
  client_error, unknown.

### source_reputation (derived)

- PK: source_reputation_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- NN: window_key (enum)
- NN: window_start
- NN: request_success_rate (numeric(5,4))
- NN: acquisition_success_rate (numeric(5,4))
- NN: fake_rate (numeric(5,4))
- NN: dmca_rate (numeric(5,4))
- NN: request_count
- NN: request_success_count
- NN: acquisition_count
- NN: acquisition_success_count
- NN: min_samples
- NN: computed_at
- UQ: (indexer_instance_id, window_key, window_start)

#### Notes

- All rate fields are constrained between 0 and 1.
- Trust threshold: request_count >= 30 or acquisition_count >= 10 per window.
- request_count derives from outbound_request_log rows (per outbound attempt/page),
  excluding rate_limited.
- request_success_count derives from outbound_request_log outcome=success (parse_ok=true).
- fake_rate numerator: acquisition_attempt failure_class in (corrupted, passworded) and
  user_result_action action = reported_fake; denominator = acquisition_count.
- dmca_rate numerator: acquisition_attempt failure_class = dmca; denominator = acquisition_count.
- request_success_rate = request_success_count / request_count.
- acquisition_success_rate = acquisition_success_count / acquisition_count.
- scoring uses acquisition_success_rate if acquisition_count >= 10; else request_success_rate
  if request_count >= 30; else 0.5 (neutral), then score_reputation = (rep_rate - 0.5) \* 10,
  capped to [-5, +5].

## 12. Indexer health telemetry

### outbound_request_log

High-volume request log for diagnostics and connectivity rollups.

- PK: outbound_request_log_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- FK: routing_policy_id -> routing_policy.routing_policy_id (nullable; null = direct)
- FK: search_request_id -> search_request.search_request_id (nullable)
- NN: request_type (outbound_request_type)
- NN: correlation_id (uuid)
- NN: retry_seq (smallint)
- NN: started_at
- NN: finished_at
- NN: outcome (outbound_request_outcome)
- NN: via_mitigation (outbound_via_mitigation)
- rate_limit_denied_scope (rate_limit_scope, nullable)
- error_class (error_class, nullable when outcome=success)
- http_status (nullable)
- latency_ms (nullable)
- NN: parse_ok (bool, default false)
- result_count (int, nullable)
- NN: cf_detected (bool, default false)
- page_number (int, nullable)
- page_cursor_key (varchar(64), nullable)
- NN: page_cursor_is_hashed (bool, default false)
- NN: created_at

#### Notes

- This is the authoritative sample stream for connectivity rollups.
- Connectivity rollups exclude error_class=rate_limited from total_samples and failure_count.
- Invariants:
    - outcome=success requires parse_ok=true and error_class NULL.
    - outcome=failure requires error_class NOT NULL.
- http_status may be NULL for network failures.
- via_mitigation records the logical route: none, proxy, or flaresolverr.
- For rate-limited deferrals, via_mitigation records the intended route.
- correlation_id is stable across rate-limited deferrals and retryable failures for a
  single logical page fetch; retries increment retry_seq starting at 0. A new
  correlation_id is created only when advancing to a new page fetch.
- search_request_indexer_run_correlation links runs to correlation_id values; last_correlation_id
  is a convenience pointer only.
- page_number and page_cursor_key are optional diagnostics for paginated runs.
- page_number is the indexer page fetch sequence within a run (1-based).
- page_cursor_key normalization before hashing/truncation:
    - trim whitespace.
    - treat as URL only if parsing succeeds AND scheme is http/https AND host exists:
        - lowercase scheme and host.
        - sort query params by (key, value) lexicographically; preserve duplicates.
        - do not URL-decode.
    - otherwise preserve case (opaque tokens may be case-sensitive).
      If normalized length > 64 chars, store the SHA-256 hex prefix (16 chars) and set
      page_cursor_is_hashed=true.
- retry_seq is 0-based (first attempt = 0).
- error_class mapping (v1):
    - if cf_detected=true and outcome=failure -> error_class=cf_challenge.
    - 200 with parse_ok=true -> success, error_class NULL.
    - 200 with parse_ok=false -> failure, error_class=parse_error.
    - 401 -> auth_error.
    - 403 -> http_403 (unless overridden by cf_detected rule).
    - 429 -> http_429.
    - 5xx -> http_5xx (unless overridden by cf_detected rule).
    - dns/tls/connect timeouts map to dns/tls/timeout.
    - other failures -> unknown.
    - rate-limited requests log outcome=failure, error_class=rate_limited.
- rate-limited entries set parse_ok=false and latency_ms=0.
- rate-limited entries set started_at=finished_at (deferral timestamp).
- rate-limited entries set result_count=0.
- rate-limited entries set rate_limit_denied_scope to the scope that denied.
- non-rate-limited failures must leave result_count NULL.
- parse_ok may be true for empty result sets; use result_count=0.
- result_count is required on success for caps/search/tvsearch/moviesearch/rss; optional for probe.
- result_count semantics:
    - caps: number of categories returned in the caps response.
    - search/tvsearch/moviesearch: post-filter count emitted to the client for this
      response (after offset/limit slicing).
    - rss: items_parsed (total items parsed from the feed; no offset/limit slicing).
- cf_detected heuristics (v1):
    - response headers include server=cloudflare, cf-ray, or cf-cache-status.
    - HTTP 403/503 body contains "Attention Required", "Cloudflare", "cf-challenge",
      "jschl", "challenge-platform", "Turnstile", or "/cdn-cgi/".
    - FlareSolverr reports a challenge solve attempt.
- Retention is enforced by background jobs using
  deployment_config.retention_outbound_request_log_days; use finished_at if present,
  otherwise started_at.
- Rate-limited entries set started_at=finished_at=now() and latency_ms=0.

### indexer_health_event

Raw events for debugging and trend analysis.

- PK: indexer_health_event_id
- FK: indexer_instance_id -> indexer_instance.indexer_instance_id
- NN: occurred_at
- NN: event_type (enum)
- latency_ms (nullable)
- http_status (nullable)
- error_class (enum, nullable)
- detail (varchar(1024), nullable)

#### Notes

- identity_conflict is emitted when durable hash or tracker metadata conflicts with
  incoming observations; corresponding source_metadata_conflict rows are recorded.
- indexer_health_event is diagnostic only and does not drive connectivity rollups.

## 13. Retention, jobs, and background work

### Retention policies (v1 defaults)

- search_request + pages + items + runs + observations: 7 days after finished_at
  (only purge searches where finished_at is not NULL).
- policy_snapshot rows are reusable and are not deleted with their search_request.
- policy_snapshot ref_count is decremented when a search_request is purged.
- policy_snapshot_gc hard-deletes snapshots where ref_count=0 and created_at older than 30 days.
- policy_snapshot_refcount_repair recomputes ref_count daily and fixes discrepancies.
  run refcount_repair before policy_snapshot_gc (daily ordering).
- search_filter_decision: deleted with the search_request tree.
- indexer_health_event: 14 days after occurred_at.
- outbound_request_log: deployment_config.retention_outbound_request_log_days after finished_at
  (fallback to started_at if finished_at is NULL; default 14 days).
- indexer_rss_item_seen: deployment_config.retention_rss_item_seen_days after first_seen_at
  (default 30 days).
- source_metadata_conflict: deployment_config.retention_source_metadata_conflict_days after observed_at
  (default 30 days).
- source_metadata_conflict_audit_log: deployment_config.retention_source_metadata_conflict_audit_days
  after occurred_at (default 90 days).
- rate_limit_state: purge minute buckets older than 6 hours.
- indexer_connectivity_profile: latest-only (no history) in v1.
- source_reputation: 180 days.
- canonical_torrent: keep indefinitely; prune low-confidence title_size_fallback canonicals
  older than 30 days when never acquired and never selected.
- canonical_torrent_source is durable and not deleted by retention.
- base scores and global best_source tables are durable and not deleted by retention.
- search_request-keyed context scores and best_source_context rows are purged with the
  search_request tree.
- auto-created request policy_sets (is_auto_created=true) tied to the purged search_request
  are deleted; user-supplied request policy_sets are retained.

### Source reputation windows and cadence

- Windows: 1h, 24h, 7d.
- Minimum samples: 30 requests or 10 acquisitions before trusted.
- Refresh cadence: 5 minutes (1h), hourly (24h and 7d).

### Connectivity and reputation samples

- Connectivity rollups are derived from outbound_request_log only.
- total_samples: outbound_request_log rows with request_type in
  (caps, search, tvsearch, moviesearch, rss, probe) excluding error_class=rate_limited.
- success_count: outcome=success AND parse_ok=true.
- failure_count: outcome=failure excluding error_class=rate_limited.
- identity_conflict events are diagnostic and excluded from sample counts.
- Reputation samples:
    - request_count: outbound_request_log attempts (per page) excluding rate_limited.
    - acquisition_count: grabs/downloads attempted from the indexer.
    - request_success: outbound_request_log outcome=success with parse_ok=true.
    - acquisition_success: download succeeded without failure_class.
- Reputation rates:
    - request_success_rate uses request_success_count / request_count.
    - acquisition_success_rate uses acquisition_success_count / acquisition_count.

### job_schedule

- PK: job_schedule_id
- NN: job_key (enum)
- NN: cadence_seconds
- NN: jitter_seconds (default 0)
- NN: enabled (bool, default true)
- last_run_at (nullable)
- NN: next_run_at
- locked_until (nullable)
- lock_owner (varchar(128), nullable)
- UQ: (job_key)

#### Notes

- Jobs are claimed via advisory lock plus locked_until update in a single transaction.
- job_schedule rows are seeded at deployment initialization with mandatory job_keys:
  retention_purge (3600s), reputation_rollup_1h (300s), reputation_rollup_24h (3600s),
  reputation_rollup_7d (21600s), connectivity_profile_refresh (300s),
  canonical_backfill_best_source (86400s), base_score_refresh_recent (3600s),
  canonical_prune_low_confidence (86400s), rss_poll (60s),
  rss_subscription_backfill (300s, enabled until first success).
- next_run_at seeding: now() + random_jitter_seconds(0..cadence_seconds-1).
- next_run_at update on job completion (success or failure): now() + cadence_seconds + jitter,
  where jitter is a uniform integer in [0, jitter_seconds] (inclusive) from OS CSPRNG.
  If jitter_seconds=0, jitter=0. Jitter is applied at run completion, not at claim time.
- Additional deployment-global jobs are seeded:
    - policy_snapshot_gc (86400s)
    - policy_snapshot_refcount_repair (86400s)
    - rate_limit_state_purge (3600s)
- No secret-rotation job in v1; rotation is manual.
- job_claim_next_v1 lease durations (seconds):
    - connectivity_profile_refresh: 30
    - reputation_rollup_1h: 60
    - reputation_rollup_24h: 300
    - reputation_rollup_7d: 600
    - retention_purge: 300
    - canonical_backfill_best_source: 900
    - base_score_refresh_recent: 900
    - canonical_prune_low_confidence: 900
    - policy_snapshot_gc: 900
    - policy_snapshot_refcount_repair: 900
    - rate_limit_state_purge: 300
    - rss_poll: 60
    - rss_subscription_backfill: 300

### Derived table refresh strategy

- indexer_connectivity_profile: job every 5 minutes from outbound_request_log aggregates.
- canonical_torrent_source_base_score and canonical_torrent_best_source_global:
    - recompute via base_score_refresh_recent (hourly) for canonicals with any durable
      source last_seen_at within 7 days.
    - no immediate recompute on material bucket changes in v1.
- canonical_torrent_source_context_score and canonical_torrent_best_source_context:
    - updated synchronously on ingest for search_request contexts only.
    - profile contexts are computed on read and cached in app memory for 1 hour in v1
      (not persisted).
    - best_source_context may update mid-search when a new source exceeds the current best
      by a material margin; page order remains stable.
- source_reputation: computed on cadence and stored as rollups.

### Background job execution

- Jobs run inside the Revaer server process.
- Postgres advisory locks ensure a single active worker per deployment and job.

## 14. Stored procedures (v1) and validation rules

### Versioning and error style

- All procs are versioned by name suffix: \_v1, \_v2, etc.
- Stable wrapper names call the latest version; wrappers are mandatory.
- Error style: raise exceptions with error_code (SQLSTATE or custom code).
- Public-facing procs accept keys for media_domain/trust_tier/tag and translate to ids,
  raising invalid_request with error_code=unknown_key on misses.
- Privileged procs resolve actor role from app_user.role for actor_user_public_id before
  authorization decisions.
- Procs invoked by Torznab or system contexts accept actor_user_public_id NULL; audit
  fields use the system sentinel (user_id=0 or all-zero UUID) instead of NULL.

### Seed and deployment procedures

- trust_tier_seed_defaults():
    - Insert required trust tiers if missing.
    - Enforce immutability of trust_tier_key.
- media_domain_seed_defaults():
    - Insert required domains if missing.
    - Keys must be lowercase; reject otherwise.
- deployment_init_v1(actor_user_public_id) -> ok
    - Require actor_user.is_email_verified = true.
    - Create deployment_config, deployment_maintenance_state, and mandatory job_schedule rows.
    - deployment_maintenance_state defaults:
        - rss_subscription_backfill_completed_at = NULL
        - last_updated_at = now()
    - Seed rate_limit_policy defaults (default_indexer, default_routing).
    - Ensure the system user row exists (user_id=0).
    - Do not seed starter tags in v1.

### app_user procedures

- app_user_create_v1(email, display_name) -> user_public_id
    - Normalize email (trim + lowercase) into email_normalized.
    - is_email_verified defaults to false.
- app_user_update_v1(user_public_id, display_name) -> ok
- app_user_verify_email_v1(user_public_id) -> ok
    - Sets is_email_verified=true.

### Import job procedures

- import_job_create_v1(
  actor_user_public_id,
  source,
  is_dry_run,
  target_search_profile_public_id null,
  target_torznab_instance_public_id null
  ) -> import_job_public_id
    - is_dry_run defaults to false when omitted.
    - If target\_\* is provided, it must belong to the same deployment and the runner must
      use it as the target (no auto-create).
    - If target\_\* is NULL, the runner auto-creates per import rules and stores IDs on
      import_job.
- import_job_run_prowlarr_api_v1(import_job_public_id, prowlarr_url,
  prowlarr_api_key_secret_public_id) -> ok
    - Persists config for async runner; runner is implemented in app code.
- import_job_run_prowlarr_backup_v1(import_job_public_id, backup_blob_ref) -> ok
- import_job_get_status_v1(import_job_public_id) -> status + counts
- import_job_list_results_v1(import_job_public_id) -> results list
    - Dry-run imports persist results, but do not write indexer_instance, routing_policy,
      or tag rows; import_indexer_result.indexer_instance_id remains NULL.

### Indexer instance procedures

- indexer_instance_create_v1(
  actor_user_public_id uuid,
  indexer_definition_id bigint,
  display_name varchar,
  priority int,
  trust_tier_key varchar null,
  routing_policy_public_id uuid null
  ) -> (indexer_instance_public_id uuid)
    - Validate actor role.
    - Validate indexer_definition exists and is not deprecated.
    - Reject indexer_definition.protocol != torrent with error_code=unsupported_protocol.
    - display_name unique within deployment and not soft-deleted.
    - Bounds for priority, timeouts, max_parallel_requests.
    - trust_tier_key exists if provided.
    - routing_policy_public_id exists and not soft-deleted if provided.
    - Effects: create indexer_instance, initialize indexer_cf_state (state=clear),
      auto-create indexer_rss_subscription with is_enabled = (is_enabled AND enable_rss),
      interval_seconds=900, next_poll_at=now()+random_jitter(0..60s) when enabled,
      otherwise NULL, last_polled_at=NULL,
      audit_log create.

- indexer_instance_update_v1(
  actor_user_public_id uuid,
  indexer_instance_public_id uuid,
  display_name varchar null,
  priority int null,
  trust_tier_key varchar null,
  routing_policy_public_id uuid null,
  is_enabled bool null,
  enable_rss bool null,
  enable_automatic_search bool null,
  enable_interactive_search bool null
  ) -> (indexer_instance_public_id uuid)
    - Validate updated fields; prevent name collisions.
    - If is_enabled becomes false OR enable_rss becomes false:
        - force indexer_rss_subscription.is_enabled=false.
        - set next_poll_at=NULL.
        - preserve interval_seconds, last_polled_at, last_error_class, backoff_seconds.
    - If is_enabled becomes true AND enable_rss becomes true:
        - do not auto-enable an existing subscription.
        - if subscription row is missing, auto-create with is_enabled=true and
          next_poll_at=now()+random_jitter(0..60s).
    - Effects: update row, audit_log update or enable/disable.

- indexer_rss_subscription_set_v1(
  actor_user_public_id uuid,
  indexer_instance_public_id uuid,
  is_enabled bool,
  interval_seconds int null
  ) -> ok
    - Validate indexer_instance exists in deployment.
    - If is_enabled=true and indexer_instance.is_enabled=false or enable_rss=false,
      reject 409 conflict with error_code=rss_enable_indexer_disabled.
    - If interval_seconds is provided, validate range 300..86400.
    - Upsert indexer_rss_subscription; on create set is_enabled from input,
      last_polled_at=NULL, last_error_class=NULL, backoff_seconds=NULL, and
      interval_seconds=coalesce(interval_seconds, 900).
    - Update indexer_rss_subscription.is_enabled.
    - If interval_seconds is provided, update interval_seconds; otherwise keep current.
    - If is_enabled becomes true:
        - set last_error_class=NULL and backoff_seconds=NULL.
        - set next_poll_at=now()+random_jitter(0..60s).
    - If is_enabled becomes false:
        - set next_poll_at=NULL.
        - preserve interval_seconds, last_polled_at, last_error_class, backoff_seconds.
    - If only interval_seconds changes and is_enabled stays true, leave next_poll_at unchanged.
    - audit_log action=update.

- indexer_rss_subscription_disable_v1(
  actor_user_public_id uuid,
  indexer_instance_public_id uuid
  ) -> ok
    - Upsert indexer_rss_subscription; on create set interval_seconds=900,
      last_polled_at=NULL, last_error_class=NULL, backoff_seconds=NULL.
    - Set indexer_rss_subscription.is_enabled=false.
    - Set next_poll_at=NULL.
    - Preserve interval_seconds, last_polled_at, last_error_class, backoff_seconds on
      existing rows.
    - audit_log action=update.

- indexer_instance_set_media_domains_v1(
  actor_user_public_id uuid,
  indexer_instance_public_id uuid,
  media_domain_keys (set of varchar)
  ) -> ok
    - Validate all domain keys exist.
    - Replace set atomically.
    - Audit log update.

- indexer_instance_set_tags_v1(
  actor_user_public_id uuid,
  indexer_instance_public_id uuid,
  tag_public_ids (set of uuid) null,
  tag_keys (set of varchar) null
  ) -> ok
    - Resolve tag_public_ids and tag_keys; if both provided they must resolve to the same set.
    - Reject unknown tags.
    - Replace set atomically.
    - Audit log update.

- indexer_instance_field_set_value_v1(
  actor_user_public_id uuid,
  indexer_instance_public_id uuid,
  field_name varchar,
  value_plain varchar null,
  value_int int null,
  value_decimal numeric null,
  value_bool bool null
  ) -> ok
    - Validate field exists and type matches definition.
    - Reject secret-backed field types.
    - Apply definition validations (min/max length, regex, allowed values, required_if).
    - Enforce typed value exclusivity.
    - Upsert value row; audit_log update.

- indexer_instance_field_bind_secret_v1(
  actor_user_public_id uuid,
  indexer_instance_public_id uuid,
  field_name varchar,
  secret_public_id uuid
  ) -> ok
    - Validate field is secret-backed type.
    - Validate secret exists, not revoked, same deployment.
    - Enforce no value\_\* columns for the field row.
    - Ensure value row exists; bind secret; audit_log update; secret_audit_log bind.

- indexer_instance_test_prepare_v1(
  actor_user_public_id uuid null,
  indexer_instance_public_id uuid
  ) -> (
    can_execute bool,
    error_class enum null,
    error_code varchar(64) null,
    detail varchar(256),
    engine enum,
    routing_policy_public_id uuid null,
    connect_timeout_ms int,
    read_timeout_ms int,
    field_names varchar[],
    field_types field_type[],
    value_plain varchar[],
    value_int int[],
    value_decimal numeric[],
    value_bool bool[],
    secret_public_ids uuid[]
  )
    - Validates actor role (owner/admin); system callers may pass NULL actor_user_public_id.
    - Resolves indexer_instance and routing_policy_public_id (nullable).
    - Determines required secret-backed fields from indexer_definition_field where
      field_type is secret-backed and is_required=true.
    - If any required secret bindings are missing:
        - return can_execute=false, error_class=auth_error, error_code=missing_secret.
        - detail lists missing required field names (<= 256 chars).
        - if migration_state is in migration flow, set migration_state=needs_secret and
          force is_enabled=false.
        - if migration_state is NULL, do not change migration_state or is_enabled.
        - config arrays are NULL.
    - If secrets are satisfied:
        - return can_execute=true and populate config arrays from
          indexer_instance_field_value + secret_binding (ordered by field_name).
        - secret_public_ids aligns to field_names; non-secret fields have NULL secret_public_ids.
        - value_* arrays align to field_names; secret-backed fields have all value_* NULL.

- indexer_instance_test_finalize_v1(
  actor_user_public_id uuid null,
  indexer_instance_public_id uuid,
  ok bool,
  error_class enum null,
  error_code varchar(64) null,
  detail varchar(256) null,
  result_count int null
  ) -> (ok bool, error_class enum null, error_code varchar(64) null,
  detail varchar(256), result_count int)
    - Called after the executor performs caps/minimal-search attempts.
    - Executor logs each attempt via outbound_request_log_write_v1(request_type=probe).
    - On ok=true:
        - if migration_state is not NULL, set migration_state=ready and clear migration_detail.
        - when migration_state was duplicate_suspected, do not auto-enable on success.
    - On missing required secrets (error_code=missing_secret):
        - set migration_state=needs_secret and force is_enabled=false when in migration flow.
        - if migration_state is NULL, do not change migration_state or is_enabled.
    - On other test failures:
        - if migration_state is NULL (non-imported), do not change migration_state and
          do not force is_enabled=false.
        - if migration_state is one of ready/needs_secret/test_failed/duplicate_suspected,
          set migration_state=test_failed, set migration_detail, and
          force is_enabled=false.

### Tag procedures

- tag_create_v1(actor_user_public_id, tag_key, display_name)
  -> tag_public_id
    - Validate lowercase key and uniqueness; enforce display_name length.
    - audit_log action=create.
- tag_update_v1(actor_user_public_id, tag_public_id,
  display_name) -> tag_public_id
    - v1: tag_key is immutable; only display_name may change.
    - audit_log action=update.
- tag_soft_delete_v1(actor_user_public_id, tag_public_id) -> ok
    - Sets deleted_at; tag_key remains reserved.
    - audit_log action=soft_delete.
    - Procs accept tag_public_id and/or tag_key; conflicting inputs return invalid_tag_reference.

### Routing policy procedures

- routing_policy_create_v1(
  actor_user_public_id uuid,
  display_name varchar,
  mode enum
  ) -> routing_policy_public_id
    - display_name unique in deployment.
    - Reject mode in (vpn_route, tor) with error_code=unsupported_routing_mode.
    - auto-create verify_tls parameter row with default true.
    - audit_log create.

- routing_policy_set_param_v1(
  actor_user_public_id uuid,
  routing_policy_public_id uuid,
  param_key enum,
  value_plain varchar null,
  value_int int null,
  value_bool bool null
  ) -> ok
    - routing_policy exists and not deleted.
    - param_key allowed for mode.
    - enforce bounds (ports, timeouts, ttl).
    - audit_log update.

- routing_policy_bind_secret_v1(
  actor_user_public_id uuid,
  routing_policy_public_id uuid,
  param_key enum,
  secret_public_id uuid
  ) -> ok
    - param_key allowed and secret-capable (http_proxy_auth, socks_proxy_auth).
    - secret exists, not revoked, same deployment.
    - ensure param row exists; bind secret; audit_log update; secret_audit_log bind.

### Cloudflare procedures

- indexer_cf_state_reset_v1(
  actor_user_public_id uuid,
  indexer_instance_public_id uuid,
  reason varchar null
  ) -> ok
    - owner/admin only.
    - Reset state=clear, consecutive_failures=0, cf_session_id/expires null,
      cooldown_until/backoff_seconds null, last_error_class null.
    - If indexer_connectivity_profile.status=quarantined and dominant error_class in
      (cf_challenge, http_429), set status=degraded, error_class=unknown, last_checked_at=now.
    - Does not clear non-CF health failures.
    - audit_log action=update on entity_type=indexer_instance with change_summary "cf_state reset".

### Secret procedures

- secret_create_v1(actor_user_public_id, secret_type, plaintext_value)
  -> secret_public_id
    - Encrypt to cipher_text; set key_id and created_at.
    - secret_audit_log action=create.
- secret_rotate_v1(actor_user_public_id, secret_public_id,
  new_plaintext_value) -> secret_public_id
    - Update cipher_text and rotated_at; secret_audit_log action=rotate.
- secret_revoke_v1(actor_user_public_id, secret_public_id) -> ok
    - Sets is_revoked=true; prevents new bindings; runtime fails closed.
    - secret_audit_log action=revoke.
- secret_read_v1(actor_user_public_id, secret_public_id) -> (secret_type, cipher_text, key_id)
    - owner/admin only; system callers may pass actor_user_public_id NULL.
    - Reject revoked secrets with error_code=secret_revoked.

### Policy procedures

- policy_set_create_v1(actor_user_public_id, display_name, scope, enabled)
  -> policy_set_public_id
- policy_set_update_v1(...) -> policy_set_public_id
- policy_set_enable_v1(...) -> ok
- policy_set_disable_v1(...) -> ok
    - Validate scope rules (global requires admin/owner; user scope must match actor).
    - scope=profile requires admin and a link via search_profile_policy_set.
    - audit_log for all mutations.
- policy_set_reorder_v1(actor_user_public_id, ordered_policy_set_public_ids[])
  -> ok
    - Batch renumber policy_set.sort_order as 10,20,30... in the provided order.
    - Validates actor role and scope visibility.

- policy_rule_create_v1(
  actor_user_public_id,
  policy_set_public_id,
  rule_type,
  match_field,
  match_operator,
  sort_order int null,
  match_value_text null,
  match_value_int null,
  match_value_uuid null,
  value_set_items optional (max 100),
  action,
  severity,
  is_case_insensitive bool null,
  rationale null,
  expires_at null
  ) -> policy_rule_public_id
    - policy_set exists and mutable.
    - Validate rule_type/action combinations.
    - Validate match_field/match_operator compatibility.
    - in_set requires value_set_id and no match_value.
    - id targeting uses match_value_uuid or uuid value set.
    - key targeting uses lowercase match_value_text.
    - block_infohash_v1 requires match_field=infohash_v1.
    - block_infohash_v2 requires match_field=infohash_v2.
    - block_magnet requires match_field=magnet_hash.
    - block_infohash_v1, block_infohash_v2, and block_magnet require severity=hard
      and action=drop_canonical.
    - require_trust_tier_min requires match_value_int and match_operator=eq.
    - is_case_insensitive default true; applies to regex compilation.
    - sort_order default 1000; ties resolved by policy_rule_public_id.
    - Regex length <= 512; compile check in app layer.
    - value_set_items payload schema (v1):
        - list of objects with exactly one of: value_text, value_int, value_bigint, value_uuid.
        - all items must match the parent value_set_type.
        - duplicates after normalization are rejected.
        - order preserved for display; evaluation treats as unordered.
        - value_text: lower(trim()), length <= 256.
        - value_uuid: canonical lowercase UUID.
    - policy rules are immutable in v1; updates are modeled as disable + create.
    - audit_log; value set replacement is atomic.

- policy_rule_disable_v1(actor_user_public_id, policy_rule_public_id)
  -> ok
    - Sets is_disabled = true and writes audit_log.
- policy_rule_enable_v1(actor_user_public_id, policy_rule_public_id)
  -> ok
    - Sets is_disabled = false and writes audit_log.
- policy_rule_reorder_v1(policy_set_public_id, ordered_rule_public_ids[]) -> ok
    - Batch renumber policy_rule.sort_order as 10,20,30... in the provided order.
    - Validates actor role and policy_set mutability.

### Search profile procedures

- search_profile_create_v1(...) -> search_profile_public_id
    - Accepts default_media_domain_key (nullable) and translates to id.
- search_profile_update_v1(...) -> search_profile_public_id
- search_profile_set_default_v1(...) -> ok
    - page_size clamped 10..200.
    - set_default unsets previous default in same scope within a transaction.
    - audit_log.

- search_profile_set_default_domain_v1(
  actor_user_public_id,
  search_profile_public_id,
  default_media_domain_key null
  ) -> ok
    - Translate key to id; store default_media_domain_id.
    - If allowlist exists, require default in allowlist.
    - audit_log action=update (entity_type=search_profile).

- search_profile_set_domain_allowlist_v1(
  actor_user_public_id,
  search_profile_public_id,
  media_domain_keys set
  ) -> ok
    - Atomically replace allowlist rows.
    - If allowlist becomes non-empty and default exists, require default in allowlist.
    - audit_log action=update (entity_type=search_profile_rule).

- search_profile_add_policy_set_v1(
  actor_user_public_id,
  search_profile_public_id,
  policy_set_public_id
  ) -> ok
    - Validate policy_set.scope=profile.
    - Insert search_profile_policy_set link.
    - audit_log action=update (entity_type=search_profile_rule).

- search_profile_remove_policy_set_v1(
  actor_user_public_id,
  search_profile_public_id,
  policy_set_public_id
  ) -> ok
    - Remove search_profile_policy_set link.
    - audit_log action=update (entity_type=search_profile_rule).

- search_profile_indexer_allow_v1(...) -> ok
- search_profile_indexer_block_v1(...) -> ok
- search_profile_tag_allow_v1(...) -> ok
- search_profile_tag_block_v1(...) -> ok
- search_profile_tag_prefer_v1(...) -> ok
    - prevent allow + block dual rows for the same target.
    - accept tag_public_ids and/or tag_keys; if both provided, must resolve to the same set.
    - reject unknown tags.
    - referenced indexers/tags must exist and not deleted.
    - audit_log update (entity_type=search_profile_rule).

### Torznab procedures

- torznab_instance_create_v1(
  actor_user_public_id,
  search_profile_public_id,
  display_name
  ) -> (torznab_instance_public_id, api_key_plaintext)
    - Validate search_profile exists and display_name unique.
    - Generate API key (32 random bytes, base64url without padding).
    - Hash API key with Argon2id; store only api_key_hash (PHC string).
    - Return api_key_plaintext once; it is not retrievable afterward.
    - audit_log action=create.
- torznab_instance_rotate_key_v1(
  actor_user_public_id,
  torznab_instance_public_id
  ) -> api_key_plaintext
    - Generate new API key; replace api_key_hash (Argon2id PHC string).
    - Old key invalid immediately.
    - Return api_key_plaintext once; it is not retrievable afterward.
    - audit_log action=update.
- torznab_instance_enable_disable_v1(
  actor_user_public_id,
  torznab_instance_public_id,
  is_enabled bool
  ) -> ok
    - audit_log action=update (enable/disable).
- torznab_instance_soft_delete_v1(
  actor_user_public_id,
  torznab_instance_public_id
  ) -> ok
    - Sets deleted_at; audit_log action=soft_delete.

### Category mapping procedures

- tracker_category_mapping_upsert_v1(
  actor_user_public_id,
  indexer_definition_upstream_slug null,
  tracker_category int,
  tracker_subcategory int,
  torznab_cat_id int,
  media_domain_key varchar
  ) -> ok
    - Validate torznab_cat_id and media_domain_key.
    - Resolve upstream_slug to indexer_definition_id when provided.
    - Unknown upstream_slug is a hard error.
    - Upsert definition-specific or global mapping row.
    - audit_log action=update.
- tracker_category_mapping_delete_v1(...) -> ok
    - audit_log action=soft_delete or delete (hard delete allowed).
- media_domain_to_torznab_category_upsert_v1(
  actor_user_public_id,
  media_domain_key,
  torznab_cat_id,
  is_primary bool
  ) -> ok
    - Validate media_domain_key and torznab_cat_id.
    - Enforce single primary per media_domain (unset previous in transaction).
    - audit_log action=update.
- media_domain_to_torznab_category_delete_v1(...) -> ok
    - audit_log action=soft_delete or delete (hard delete allowed).

### Rate limit procedures

- rate_limit_policy_create_v1(
  actor_user_public_id,
  display_name,
  rpm,
  burst,
  concurrent
  ) -> rate_limit_policy_public_id
    - Validate bounds; enforce unique display_name.
    - audit_log action=create.
- rate_limit_policy_update_v1(
  actor_user_public_id,
  rate_limit_policy_public_id,
  display_name null,
  rpm null,
  burst null,
  concurrent null
  ) -> ok
    - audit_log action=update.
    - Reject if is_system=true (system policies are fixed in v1).
- rate_limit_policy_soft_delete_v1(
  actor_user_public_id,
  rate_limit_policy_public_id
  ) -> ok
    - Reject if is_system=true.
    - Reject with 409 if the policy is assigned to any indexer_instance_rate_limit or
      routing_policy_rate_limit.
    - Sets deleted_at; audit_log action=soft_delete.
- indexer_instance_set_rate_limit_policy_v1(
  actor_user_public_id,
  indexer_instance_public_id,
  rate_limit_policy_public_id null
  ) -> ok
- routing_policy_set_rate_limit_policy_v1(
  actor_user_public_id,
  routing_policy_public_id,
  rate_limit_policy_public_id null
  ) -> ok

- rate_limit_try_consume_v1(
  scope_type enum,
  scope_id bigint,
  capacity int,
  tokens int default 1
  ) -> (allowed bool, tokens_used int)
    - Compute window_start internally as date_trunc('minute', now() AT TIME ZONE 'UTC').
    - SELECT ... FOR UPDATE on rate_limit_state row; upsert if missing.
    - If tokens_used + tokens <= capacity, update and allowed=true; else allowed=false.
- Direct routing invariant: when routing_policy_id is NULL, caller must pass
  scope_type=routing_policy and scope_id=0.

### Search request procedures

- search_request_create_v1(
  actor_user_public_id uuid null,
  query_text varchar,
  query_type,
  torznab_mode null,
  requested_media_domain_key varchar null,
  page_size int null,
  search_profile_public_id null,
  request_policy_set_public_id null,
  season_number int null,
  episode_number int null,
  identifiers optional set of (id_type, id_value_raw),
  torznab_cat_ids optional set of (torznab_cat_id int)
  ) -> (search_request_public_id, request_policy_set_public_id)
    - Validate actor role; REST callers require non-NULL actor_user_public_id.
    - actor_user_public_id may be NULL only for Torznab/API-key contexts and jobs.
    - For Torznab callers, search_request.user_id is stored as NULL.
    - page_size resolved by precedence and clamped 10..200.
    - requested_media_domain_key exists if provided; translate key to id in-proc.
    - query_text length <= 512 (empty allowed).
    - require non-empty query_text or at least one identifier.
    - imdb/tmdb/tvdb: parse identifier from query_text; if parse fails and no explicit
      identifier provided, reject invalid_request (REST) or return empty results (Torznab).
    - if explicit identifiers are provided, they win over parsed identifiers.
    - if multiple explicit identifiers are provided (more than one of imdb/tmdb/tvdb), invalid.
    - REST query_type rules:
        - identifiers are allowed regardless of query_type.
        - if exactly one identifier type is provided, query_type is coerced to that type.
        - if query_type is explicitly set to imdb/tmdb/tvdb and the provided identifier type
          does not match, reject invalid_identifier_mismatch.
    - season_episode requires season_number and episode_number >= 0 plus non-empty query_text
      or at least one identifier.
    - season_number and episode_number may be 0 (specials).
    - torznab_mode validation (only for Torznab callers; REST expects torznab_mode NULL):
        - generic: season/ep invalid (use tvsearch instead).
        - v1 does not validate tmdbid tv-vs-movie type.
        - tv: episode requires season; season-only requires query_text or identifier.
        - movie: season/ep invalid; tvdbid invalid.
        - conflicting identifiers (more than one of imdb/tmdb/tvdb) invalid.
    - q contains multiple identifier types and no explicit id params -> invalid.
    - q contains multiple matches of the same identifier type and no explicit id params -> invalid.
    - REST only: q contains multiple identifier types and no explicit id params, or
      multiple matches of the same identifier type and no explicit id params ->
      invalid_identifier_combo.
    - Invalid Torznab combinations return empty results with no committed DB writes; some
      are rejected by the handler before any proc call, others are rejected inside the proc
      and roll back. REST returns invalid_request with specific error codes (e.g.,
      invalid_identifier_combo, invalid_identifier_mismatch, invalid_season_episode_combo,
      invalid_category_filter).
- Handler-level Torznab short-circuits emit a metrics counter
  torznab.invalid_request_total{reason=...} and a debug log entry (no DB writes).
  Reasons: invalid_identifier_combo, invalid_season_episode_combo, invalid_query,
  invalid_category_filter (when cat is explicitly provided and the sanitized set is empty
  after dropping unknown IDs; or when explicit cat filters are reduced to an empty
  effective set by profile domain allowlist filtering).
    - Identifier normalization (imdb tt + digits, tmdb/tvdb digits).
    - Torznab query_type selection:
        - if exactly one identifier type is present (imdb/tmdb/tvdb), query_type = that type.
        - otherwise query_type = free_text.
    - If request_policy_set_public_id is provided:
        - require scope=request, user_id matches actor, is_enabled=true,
          and not soft-deleted; for actor_user_public_id NULL, require policy_set.user_id
          IS NULL. Otherwise reject invalid_request_policy_set (REST) or return empty
          results (Torznab handler maps the proc error; transaction rolls back).
    - If no request_policy_set_public_id is provided:
        - auto-create a request-scoped policy_set and return its public_id.
        - auto-created request policy_sets set is_auto_created=true and
          created_for_search_request_id; user_id is NULL for auto-created sets.
          Only those are cleaned up on purge.
    - Compute effective policy stack (request, profile, user, global), ordering policy_sets by
      sort_order ASC then created_at ASC then policy_set_public_id ASC, excluding disabled or
      expired rules, and materialize policy_snapshot (ordered rule ids + hash).
    - Store policy_snapshot.excluded_disabled_count and excluded_expired_count.
    - Increment policy_snapshot.ref_count for the chosen snapshot in the same transaction
      as search_request creation.
    - Store requested and effective torznab categories in join tables.
    - Drop unknown torznab_cat_ids; if cat was provided and the sanitized list is empty:
      REST returns 400 invalid_category_filter; Torznab returns empty results (no DB writes).
    - If explicit cat filters are provided and the effective category set becomes empty
      after profile domain allowlist filtering:
      REST returns 400 invalid_category_filter; Torznab returns empty results (no DB writes).
    - Compute effective_media_domain_id by intersecting:
        - requested_media_domain_id (explicit request wins; profile default used only when
          request omits domain),
        - torznab category mapping for categories that map to a media_domain
          (ignore 8000 and unmapped categories such as 3000/3010),
        - policy require_media_domain rules (if any),
        - search_profile media_domain allowlist (if present).
          If intersection size=0: finish search immediately. If size=1: set that domain.
          If size>1: set effective_media_domain_id=NULL to indicate multi-domain.
          If requested categories include only unmapped categories (e.g., 8000 or 3000/3010),
          do not narrow domains; effective_media_domain_id is determined by
          requested_media_domain_id/policy/allowlist constraints only (if none, it remains NULL).
          Special case: if profile media_domain allowlist has exactly one domain and no other
          domain constraints exist, set effective_media_domain_id to that single allowed domain.
    - Effects: create search_request (requested + effective domains), page 1 row,
      enqueue indexer runs for runnable indexers only.
    - Runnable indexers are those that pass all filters in this order:
        1. Hard eligibility:
            - indexer_instance.deleted_at IS NULL.
            - indexer_instance.is_enabled = true.
            - migration_state is NULL or ready.
            - interactive search only: enable_interactive_search = true.
        2. Profile allow/block (if profile attached):
            - if search_profile_indexer_allow has rows, indexer must be in allow list.
            - if indexer is in search_profile_indexer_block, exclude.
            - if search_profile_tag_allow has rows, indexer must have >=1 allowed tag.
            - if indexer has any blocked tag, exclude.
        3. Media-domain and category filters:
            - if effective_media_domain_id is non-null, indexer must have that domain.
            - if profile media_domain allowlist exists and the domain constraint set is
              non-empty (intersection of request/profile/cat/policy/allowlist domains),
              indexer must have a domain link intersecting that allowed set.
            - if requested categories list is non-empty, use only categories that map
              to a media_domain (ignore 8000 and unmapped categories such as 3000/3010).
              If any mapped categories remain, indexer must have at least one domain
              that maps to at least one of them; if none, exclude.
            - if requested categories include 8000 (Other) OR only unmapped categories
              remain, skip category gating (catch-all semantics).
        4. Policy allow_indexer_instance(require) gating:
            - if present, intersect runnable set with the allowed indexer instances.
            - if empty, search finishes immediately.
    - If zero runnable indexers remain after gating, mark search_request finished immediately
      and do not create run rows.
    - emit SSE state.

- search_request_cancel_v1(actor_user_public_id, search_request_public_id)
  -> ok
    - Idempotent if already terminal.
    - Sets status=canceled, canceled_at=now, finished_at=now.
    - Marks in-flight runs canceled (best-effort); ingests rejected after cancel.

### Search run procedures

- search_indexer_run_enqueue_v1(...)
- search_indexer_run_mark_started_v1(...)
- search_indexer_run_mark_finished_v1(...)
- search_indexer_run_mark_failed_v1(...)
- search_indexer_run_mark_canceled_v1(...)
    - Enforce legal state transitions.
    - mark_failed requires error_class and error_detail <= 1024.
    - If all runs terminal and request still running, coordinator must mark search finished.
    - attempt_count increments on every attempt (rate-limited, failed, or successful).
    - rate_limited_attempt_count increments only on rate-limited deferrals.
    - Rate-limited attempts keep status=queued; set next_attempt_at, last_error_class=rate_limited,
      last_rate_limit_scope, and outbound_request_log.rate_limit_denied_scope.
    - If rate_limited_attempt_count >= 10, mark the run failed with error_class=rate_limited
      and emit a final outbound_request_log entry.

### Outbound request log procedures

- outbound_request_log_write_v1(
  indexer_instance_public_id uuid,
  routing_policy_public_id uuid null,
  search_request_public_id uuid null,
  request_type enum,
  correlation_id uuid,
  retry_seq smallint,
  started_at timestamptz,
  finished_at timestamptz,
  outcome enum,
  via_mitigation enum,
  rate_limit_denied_scope enum null,
  error_class enum null,
  http_status int null,
  latency_ms int null,
  parse_ok bool,
  result_count int null,
  cf_detected bool,
  page_number int null,
  page_cursor_key varchar null
  ) -> ok
    - correlation_id is required for tracing; stable across deferrals and retries for the
      logical page fetch (new per page).
    - retry_seq is required for per-attempt sequencing (0-based).
    - via_mitigation is required (none/proxy/flaresolverr).
    - rate_limit_denied_scope is required when error_class=rate_limited; NULL otherwise.
    - outcome=success requires parse_ok=true and error_class NULL.
    - outcome=failure requires error_class NOT NULL.
    - latency_ms must be >= 0; caller supplies value (proc does not recompute).
    - result_count is required on success for caps/search/tvsearch/moviesearch/rss; optional for probe.
    - result_count semantics:
        - caps = number of categories returned.
        - search/tvsearch/moviesearch = post-filter count emitted to the client for this
          response (after offset/limit slicing).
        - rss = items_parsed (total items parsed from the feed; no offset/limit slicing).
    - parse_ok may be true for empty result sets (result_count=0).
    - non-rate-limited failures must leave result_count NULL; rate_limited failures set result_count=0.
    - page_number is the indexer page fetch sequence within the run (1-based).
    - page_cursor_key uses the normalization rules defined for outbound_request_log.
      If normalized length > 64, store the SHA-256 hex prefix (16 chars).
    - If search_request_public_id is provided, update search_request_indexer_run.last_correlation_id.
    - Insert a row into search_request_indexer_run_correlation (run_id, correlation_id, page_number).

### Search ingestion procedure

- search_result_ingest_v1(
  search_request_public_id uuid,
  indexer_instance_public_id uuid,
  source_guid varchar null,
  details_url varchar null,
  download_url varchar null,
  magnet_uri varchar null,
  title_raw varchar,
  size_bytes bigint null,
  infohash_v1 char(40) null,
  infohash_v2 char(64) null,
  magnet_hash char(64) null,
  seeders int null,
  leechers int null,
  published_at timestamptz null,
  uploader varchar null,
  observed_at timestamptz null,
  attr_keys observation_attr_key[] null,
  attr_types attr_value_type[] null,
  attr_value_text varchar[] null,
  attr_value_int int[] null,
  attr_value_bigint bigint[] null,
  attr_value_numeric numeric(12,4)[] null,
  attr_value_bool bool[] null,
  attr_value_uuid uuid[] null
  ) -> (canonical_torrent_public_id uuid, canonical_torrent_source_public_id uuid,
  observation_created bool, durable_source_created bool, canonical_changed bool)
    - Reject if search_request not running.
    - Validate indexer_instance enabled, not deleted, in the search.
    - title_raw is required; reject invalid_request (missing_title) if empty after trim.
    - Parse hashes from magnet, compute magnet_hash, normalize title, extract signals.
    - Apply policy_snapshot: hard drops, then drop_source, then downrank/prefer/flag.
    - Dropped sources/canonicals are persisted for audit (durable source, observation,
      observation attrs, search_filter_decision), but are excluded from
      search_request_canonical and paging.
    - Canonicalize with priority: infohash_v2 > infohash_v1 > magnet_hash > title+size.
    - For title_size_fallback, compute title_size_hash from title_normalized and size_bytes.
    - prevent_merge check against canonical_disambiguation_rule.
    - attr*types values: text, int, bigint, numeric, bool, uuid (exactly one value*\* set).
    - All attr\_\* arrays must be the same length; for each i, exactly one value array entry
      is set and matches attr_types[i].
    - attr*keys uses observation_attr_key (superset); durable keys write to
      canonical_torrent_source_attr, observation-only keys write to
      search_request_source_observation_attr; tracker*\* plus size_bytes_reported and
      files_count are mirrored to observation attrs when present.
    - release_group is observation-only; store in observation attrs and canonical signals
      only when parser confidence >= 0.8 and the group token is a terminal suffix.
    - attr_types must match the key type map (reject mismatches).
    - Reject unknown attr_key values.
    - Reject duplicate attr_keys within a single ingest call.
    - Reject if source_guid, infohash_v1, infohash_v2, magnet_hash, and size_bytes are
      all NULL (invalid_request: insufficient_identity).
    - Resolve durable source identity (per indexer instance); enforce durable idempotency.
    - If durable source_guid is NULL and observation provides source_guid, attempt backfill
      unless it conflicts with an existing durable source (mark guid_conflict and record
      source_metadata_conflict with conflict_type=source_guid).
    - source_guid conflicts store existing_value=canonical_torrent_source_public_id and
      incoming_value=the conflicting GUID string.
    - If source_guid arrives for an existing guid-less observation, update that observation
      instead of inserting a new row.
    - Upsert durable attributes (tracker metadata, external ids) when present:
        - tracker_name/category/subcategory are treated as stable; mismatches keep original
          and log identity_conflict and source_metadata_conflict.
        - hash fields backfill if NULL; conflicts keep original and log identity_conflict and
          source_metadata_conflict.
        - external IDs backfill if missing; conflicts write canonical_external_id and
          source_metadata_conflict.
    - Update last*seen*\* on the durable source only when observed_at is newer than
      last_seen_at.
    - If size_bytes > 0, and size_bytes <= 10 TiB unless media_domain is ebooks,
      audiobooks, or software (use effective_media_domain_id if set; if NULL, fall back
      to a single indexer_instance domain if exactly one), append to canonical_size_sample (keep
      newest 25 by observed_at), recompute canonical_size_rollup; update
      canonical_torrent.size_bytes only for
      hash-based identities. For title_size_fallback, size_bytes is immutable once set.
    - Create observation (per search) and enforce observation idempotency.
    - On idempotent observation conflicts, update snapshot fields (seeders/leechers/
      published*at/urls/uploader/observed_at) and durable last_seen*\* fields.
    - Store URLs and title*raw on the observation; observation-only attrs are stored in
      search_request_source_observation_attr (including tracker*\* and optional
      language_primary/subtitles_primary for as-seen diagnostics).
    - Upsert source_attr and canonical_torrent_signal rows (typed).
    - Recency scoring uses published_at when present; otherwise uses observed_at with
      half weight (w_age \* 0.5).
    - Base scores are refreshed by base_score_refresh_recent; ingest does not recompute
      canonical_torrent_source_base_score.
    - Compute context score for this search_request and upsert
      canonical_torrent_source_context_score (set is_dropped and a sentinel score for drops).
    - Update canonical_torrent_best_source_context for this search when a new source exceeds
      the current best by a material margin (score delta >= 2.0 or seed bucket jump 20->100+);
      page order remains stable.
    - Page insertion: upsert search_request_canonical, append to open page or create next,
      seal when page_size reached.
    - If dropped or downranked/flagged, write search_filter_decision with policy_rule_public_id
      and policy_snapshot_id; include observation_id when tied to a specific observation.

### Canonical maintenance procedures

- canonical_merge_by_infohash_v1(...)
- canonical_recompute_best_source_v1(canonical_torrent_public_id,
  scoring_context enum default global_current) -> winner_canonical_torrent_source_public_id
    - recompute using canonical_torrent_source_base_score for the canonical.
    - update canonical_torrent_best_source_global.
    - fallback: durable last_seen_seeders if no base scores exist.
    - tie-break when base scores are equal:
        - canonical_torrent_source.last_seen_at DESC,
        - canonical_torrent_source_public_id ASC.
- canonical_prune_low_confidence_v1(...)
    - identity_strategy = title_size_fallback and identity_confidence <= 0.60.
    - created_at older than 30 days.
    - no acquisition_attempt by canonical_torrent_id or hashes.
    - no user_result_action.selected or downloaded.
    - delete canonical only if it has no durable sources, or only sources tied to pruned canonicals.
- canonical_disambiguation_rule_create_v1(...)
    - Validate identity types and normalization; reject left==right.
    - Canonicalize ordering for symmetric uniqueness and write config_audit_log.

### Conflict resolution procedures

- source_metadata_conflict_resolve_v1(
  actor_user_public_id,
  conflict_id,
  resolution enum,
  resolution_note varchar(256) null
  ) -> ok
    - Admin/owner only.
    - accepted_incoming applies allowed backfills only (no overwrites):
        - source_guid when durable.source_guid IS NULL and no UQ conflict.
        - tracker_category/subcategory when NULL.
        - tracker_name when NULL.
    - kept_existing makes no mutations.
    - merged is reserved for v2; treat as kept_existing and record the note.
    - Sets resolved_at=now and resolved_by_user_id=actor.
    - Writes source_metadata_conflict_audit_log action=resolved.
- source_metadata_conflict_reopen_v1(
  actor_user_public_id,
  conflict_id,
  resolution_note varchar(256) null
  ) -> ok
    - Admin/owner only.
    - Clears resolved_at/resolution/resolved_by_user_id.
    - Writes source_metadata_conflict_audit_log action=reopened.

### Job runner procedures

- job_claim_next_v1(job_key)
    - schedule row exists and enabled; next_run_at <= now; lock expired or null.
    - sets locked_until and lock_owner.
    - lease duration is fixed per job_key (see job_schedule notes).
- job_run_retention_purge_v1()
    - deletes operational rows older than retention thresholds using finished_at/occurred_at.
    - deletes search_request trees (pages, items, runs, cursors) and observations only when
      search_request.finished_at IS NOT NULL.
    - deletes search_filter_decision rows with the search_request tree.
    - decrements policy_snapshot.ref_count for each purged search_request in the same
      transaction.
    - deletes outbound_request_log rows older than deployment_config.retention_outbound_request_log_days
      using finished_at if present, otherwise started_at.
    - deletes indexer_rss_item_seen rows older than deployment_config.retention_rss_item_seen_days
      using first_seen_at.
    - deletes source_metadata_conflict rows older than
      deployment_config.retention_source_metadata_conflict_days.
    - deletes source_metadata_conflict_audit_log rows older than
      deployment_config.retention_source_metadata_conflict_audit_days.
    - does not purge soft-deleted config or durable sources in v1.
- job_run_connectivity_profile_refresh_v1(...)
    - aggregates outbound_request_log into profile snapshot; upserts indexer_connectivity_profile
      and sets last_checked_at=now().
- job_run_reputation_rollup_v1(window enum)
    - upserts source_reputation for each indexer_instance with sufficient samples,
      including request_success_rate and acquisition_success_rate.
- job_run_canonical_backfill_best_source_v1(...)
    - recomputes best_source_global for recent or low-confidence canonicals.
- job_run_base_score_refresh_recent_v1()
    - recomputes base scores for canonicals with any durable source last_seen_at within
      the last 7 days; updates best_source_global if winner changes.
- job_run_rss_subscription_backfill_v1()
    - If deployment_maintenance_state.rss_subscription_backfill_completed_at IS NOT NULL,
      no-op and disable job_schedule row for job_key=rss_subscription_backfill.
    - For each indexer_instance lacking a subscription row, insert indexer_rss_subscription with:
        - is_enabled = indexer_instance.is_enabled AND indexer_instance.enable_rss.
        - interval_seconds = 900.
        - next_poll_at = now()+random_jitter(0..60s) if enabled else NULL.
        - last_polled_at = NULL.
    - Upsert deployment_maintenance_state and set
      rss_subscription_backfill_completed_at=now(), last_updated_at=now().
    - Disable job_schedule row for job_key=rss_subscription_backfill.
- rss_poll_claim_v1(limit int default 25) -> set of (
  rss_subscription_id bigint,
  indexer_instance_public_id uuid,
  routing_policy_public_id uuid null,
  interval_seconds int,
  connect_timeout_ms int,
  read_timeout_ms int,
  correlation_id uuid,
  retry_seq smallint
  )
    - Select due subscriptions:
        - WHERE is_enabled=true AND next_poll_at <= now()
        - AND indexer_instance.is_enabled=true AND indexer_instance.enable_rss=true
        - ORDER BY next_poll_at ASC
        - LIMIT limit (default 25)
        - FOR UPDATE SKIP LOCKED
    - For each claimed subscription:
        - generate correlation_id and retry_seq=0.
        - set next_poll_at = now() + interval_seconds (temporary claim to prevent double-poll).
    - Returns routing_policy_public_id for outbound_request_log_write_v1.

- rss_poll_apply_v1(
  rss_subscription_id bigint,
  correlation_id uuid,
  retry_seq smallint,
  started_at timestamptz,
  finished_at timestamptz,
  outcome outbound_request_outcome,
  error_class error_class null,
  http_status int null,
  latency_ms int null,
  parse_ok bool,
  result_count int null,
  via_mitigation outbound_via_mitigation,
  rate_limit_denied_scope rate_limit_scope null,
  cf_detected bool,
  cf_retryable bool,
  item_guid varchar[] null,
  infohash_v1 char(40)[] null,
  infohash_v2 char(64)[] null,
  magnet_hash char(64)[] null
  ) -> (
    items_parsed int,
    items_eligible int,
    items_inserted int,
    subscription_succeeded bool
  )
    - Writes outbound_request_log with request_type=rss (search_request_public_id NULL).
    - Inserts eligible RSS items into indexer_rss_item_seen; on UQ conflict do nothing.
    - Parsed success definition: outcome=success and parse_ok=true and result_count present.
      parse_ok=true when the feed parses successfully and is recognized as RSS/Atom,
      even if some items are skipped.
    - Update subscription on parsed success:
        - last_polled_at = now()
        - next_poll_at = now() + interval_seconds + random_jitter(0..60s)
        - last_error_class = NULL
        - backoff_seconds = NULL
    - Retryable error_class for RSS polling:
        - dns, tls, timeout, connection_refused, http_5xx, http_429, rate_limited.
        - cf_challenge only when cf_retryable=true (flaresolverr route exists).
    - Non-retryable error_class for RSS polling:
        - auth_error, http_403, parse_error, unknown.
        - HTTP success with parse_ok=false maps to parse_error and is non-retryable.
    - Failure handling (retryable error_class):
        - last_error_class = error_class
        - backoff_seconds = if NULL then 60 else min(backoff_seconds \* 2, 1800)
        - jitter_pct = uniform integer percent in [0, 25] from OS CSPRNG
        - next_poll_at = now() + backoff_seconds + floor(backoff_seconds \* jitter_pct / 100)
        - Do not set last_polled_at
        - If error_class=rate_limited: write outbound_request_log with outcome=failure,
          error_class=rate_limited, result_count=0, latency_ms=0, started_at=finished_at=now();
          items_parsed/items_eligible/items_inserted are 0 for that subscription.
    - Failure handling (non-retryable error_class):
        - last_error_class = error_class
        - backoff_seconds = NULL
        - next_poll_at = NULL
        - is_enabled = false
        - Do not set last_polled_at
        - Write config_audit_log (system):
            - entity_type=indexer_instance
            - action=update
            - changed_by_user_id=0
            - change_summary="RSS subscription auto-disabled: <error_class>"
    - If error_class=cf_challenge and cf_retryable=false:
        - update indexer_cf_state: state=challenged, last_changed_at=now(),
          consecutive_failures += 1, last_error_class=cf_challenge.
        - when consecutive_failures reaches 5, apply cooldown/backoff (v1 backoff rules).
        - do not update indexer_connectivity_profile here; it is updated by
          job_run_connectivity_profile_refresh_v1.
    - Counter semantics:
        - items_parsed: result_count (total RSS items parsed).
        - items_eligible: items with at least one identifier after normalization.
        - items_inserted: rows inserted into indexer_rss_item_seen (post-dedupe).
    - Operator-facing messaging: when auto-disabled for a non-retryable failure, surface
      "disabled due to non-retryable failure: <error_class>" in UI/CLI.
- job_run_policy_snapshot_gc_v1()
    - deletes policy_snapshot rows where ref_count=0 and created_at older than 30 days.
- job_run_policy_snapshot_refcount_repair_v1()
    - recomputes ref_count from search_request references and fixes discrepancies.
- job_run_rate_limit_state_purge_v1()
    - deletes rate_limit_state rows with window_start older than 6 hours.

## 15. Idempotency unique indexes (durable and observation)

### canonical_torrent_source (durable)

1. source_guid present:

- UQ: (indexer_instance_id, source_guid)
  WHERE source_guid IS NOT NULL

2. source_guid NULL and infohash_v2 present:

- UQ: (indexer_instance_id, infohash_v2)
  WHERE source_guid IS NULL AND infohash_v2 IS NOT NULL

3. source_guid NULL and infohash_v1 present:

- UQ: (indexer_instance_id, infohash_v1)
  WHERE source_guid IS NULL AND infohash_v2 IS NULL AND infohash_v1 IS NOT NULL

4. source_guid NULL and only magnet_hash present:

- UQ: (indexer_instance_id, magnet_hash)
  WHERE source_guid IS NULL AND infohash_v2 IS NULL AND infohash_v1 IS NULL
  AND magnet_hash IS NOT NULL

5. Fallback when size_bytes present:

- UQ: (indexer_instance_id, title_normalized, size_bytes)
  WHERE source_guid IS NULL AND infohash_v2 IS NULL AND infohash_v1 IS NULL
  AND magnet_hash IS NULL AND size_bytes IS NOT NULL

### search_request_source_observation (per search)

1. source_guid present:

- UQ: (search_request_id, indexer_instance_id, source_guid)
  WHERE source_guid IS NOT NULL

2. source_guid NULL:

- UQ: (search_request_id, indexer_instance_id, canonical_torrent_source_id)
  WHERE source_guid IS NULL

#### Notes

- If source_guid arrives for an existing guid-less observation, update that row
  (set source_guid + latest snapshot) instead of inserting a new row.

## 16. Constraint matrix (DB contract v1)

### app_user

- UQ: user_public_id.
- UQ: email.
- UQ: email_normalized.
- email_normalized stored lowercase and trimmed.
- email length <= 320.
- email_normalized length <= 320.
- is_email_verified bool NN.
- display_name length 1..256.
- role enum (deployment_role).

### indexer_definition

- UQ: (upstream_source, upstream_slug).
- upstream_slug length 1..128.
- schema_version >= 1.
- definition_hash lowercase hex length 64.

### indexer_definition_field

- UQ: (indexer_definition_id, name).
- name length 1..128.
- label length 1..256.
- field_type enum.
- is_required and is_advanced are NN.
- display_order int NN.
- default*value*\* exclusivity; no defaults for secret-backed field_type.

### indexer_definition_field_option

- UQ: (indexer_definition_field_id, option_value).
- option_value length 1..256.
- option_label length 1..256.
- sort_order >= 0.

### indexer_definition_field_validation

- UQ (v1 canonical uniqueness, enforced in DB with generated normalization columns):
  (indexer_definition_field_id, validation_type,
  coalesce(depends_on_field_name,''), coalesce(depends_on_operator,''),
  coalesce(text_value_norm,''), coalesce(int_value,-1), coalesce(numeric_value,-1),
  coalesce(value_set_id,0), coalesce(depends_on_value_set_id,0),
  coalesce(depends_on_value_plain_norm,''), coalesce(depends_on_value_int,-1),
  coalesce(depends_on_value_bool,false)).
- validation_type enum.
- Required columns per validation_type:
    - min_length: int_value >= 0.
    - max_length: int_value >= 0.
    - min_value: numeric_value NN.
    - max_value: numeric_value NN.
    - regex: text_value length <= 512.
    - allowed_value: exactly one of text_value or value_set_id.
    - required_if_field_equals:
        - depends_on_field_name length 1..128.
        - depends_on_operator in enum (eq, neq, in_set).
        - exactly one of depends_on_value_plain/int/bool or depends_on_value_set_id.
    - Normalization for uniqueness:
        - text_value_norm and depends_on_value_plain_norm are stored generated columns.
        - text_value_norm = lower(trim(text_value)) except when validation_type=regex
          (then text_value_norm = trim(text_value)).
        - depends_on_value_plain_norm = lower(trim(depends_on_value_plain)).

### indexer_definition_field_value_set

- UQ: (indexer_definition_field_validation_id) (0/1 per validation row).
- value_set_type enum (text, int, bigint).
- name length 1..128 when set.

### indexer_definition_field_value_set_item

- UQ: (value_set_id, value_text) where value_text is not null.
- UQ: (value_set_id, value_int) where value_int is not null.
- UQ: (value_set_id, value_bigint) where value_bigint is not null.
- Exactly one typed value set.
- text stored lowercase; length <= 256.

### trust_tier

- UQ: trust_tier_key.
- trust_tier_key length 1..128 lowercase.
- display_name length 1..256.
- default_weight numeric(12,4) range -50..50.
- rank smallint NN.

### media_domain

- UQ: media_domain_key.
- media_domain_key length 1..128 lowercase.
- display_name length 1..256.

### tag

- UQ: (tag_key), tag_public_id.
- tag_public_id NN.
- tag_key length 1..128 lowercase.
- display_name length 1..256.
- deleted_at reserves tag_key (no reuse).

### indexer_instance

- UQ: indexer_instance_public_id; (display_name).
- priority range 0..100.
- connect_timeout_ms range 500..60000.
- read_timeout_ms range 500..120000.
- max_parallel_requests range 1..16.
- is_enabled bool NN.
- enable_rss bool NN; enable_automatic_search bool NN; enable_interactive_search bool NN.
- migration_state enum; migration_detail length <= 256.
- trust_tier_key nullable but must exist in trust_tier if present.
- routing_policy_id nullable but must reference not-deleted row if present.

### indexer_instance_media_domain

- UQ: (indexer_instance_id, media_domain_id).

### indexer_instance_tag

- UQ: (indexer_instance_id, tag_id).

### indexer_rss_subscription

- UQ: (indexer_instance_id).
- interval_seconds range 300..86400.
- next_poll_at nullable; when is_enabled=false, next_poll_at is NULL.
- backoff_seconds nullable; range 0..1800 when present.
- last_error_class nullable.
- CHECK: (is_enabled = true AND next_poll_at IS NOT NULL) OR
  (is_enabled = false AND next_poll_at IS NULL).

### indexer_rss_item_seen

- At least one of item_guid/infohash_v1/infohash_v2/magnet_hash present.
- UQ: (indexer_instance_id, item_guid) WHERE item_guid IS NOT NULL.
- UQ: (indexer_instance_id, infohash_v2) WHERE infohash_v2 IS NOT NULL.
- UQ: (indexer_instance_id, infohash_v1) WHERE infohash_v1 IS NOT NULL.
- UQ: (indexer_instance_id, magnet_hash) WHERE magnet_hash IS NOT NULL.

### indexer_instance_field_value

- UQ: (indexer_instance_id, field_name).
- field_name length 1..128.
- field_type enum.
- value_plain length <= 2048.
- Non-secret field*types: exactly one value*\* set.
- Secret field*types: all value*\* NULL and secret_binding present.
- updated_by_user_id NN; 0 means system.

### indexer_instance_import_blob

- UQ: (indexer_instance_id, source_system).
- source_system enum; import_payload_format enum.
- import_payload_text NN; imported_at NN.

### import_job

- UQ: import_job_public_id.
- source enum; status enum.
- is_dry_run bool NN.
- error_detail length <= 1024.
- target_search_profile_id and target_torznab_instance_id nullable; if set, must belong
  to the same deployment as the import_job.

### import_indexer_result

- UQ: (import_job_id, prowlarr_identifier).
- status enum; detail length <= 512.

### secret

- UQ: secret_public_id.
- cipher_text NN; key_id length 1..128; is_revoked bool NN.

### secret_binding

- UQ: (bound_table, bound_id, binding_name).
- binding_name allowed for bound_table.

### routing_policy

- UQ: (display_name); routing_policy_public_id.
- mode enum; deleted_at nullable.

### routing_policy_parameter

- UQ: (routing_policy_id, param_key).
- Exactly one of value_plain/value_int/value_bool set or secret_binding exists.
- param_key allowed for routing_policy.mode.
- proxy_host/socks_host/fs_url length <= 2048.
- ports range 1..65535.
- fs_timeout_ms range 1000..300000.
- fs_session_ttl_seconds range 60..86400.

### rate_limit_policy

- UQ: rate_limit_policy_public_id; (display_name).
- requests_per_minute 1..6000.
- burst 0..6000.
- concurrent_requests 1..64.
- is_system bool NN.
- deleted_at nullable.

### indexer_instance_rate_limit

- UQ: (indexer_instance_id).

### routing_policy_rate_limit

- UQ: (routing_policy_id).

### rate_limit_state

- UQ: (scope_type, scope_id, window_start).
- tokens_used >= 0.

### indexer_cf_state

- UQ: (indexer_instance_id).
- state enum (cf_state).
- last_changed_at NN.
- consecutive_failures int >= 0.
- last_error_class enum nullable.
- cf_session_id length <= 256 if present.
- cooldown_until nullable.
- backoff_seconds nullable; if present >= 0.

### config_audit_log

- entity_type enum; action enum.
- At least one of entity_pk_bigint or entity_public_id set.
- change_summary length <= 1024.
- changed_by_user_id NN (0 = system).

### secret_audit_log

- action enum.
- detail length <= 256.

### search_profile

- UQ: search_profile_public_id.
- page_size range 10..200.
- default_media_domain_id must reference media_domain if set.

### search_profile_media_domain

- UQ: (search_profile_id, media_domain_id).

### search_profile_trust_tier

- UQ: (search_profile_id, trust_tier_id).
- weight_override numeric(12,4) optional; range -50..50.

### search_profile_indexer_allow

- UQ: (search_profile_id, indexer_instance_id).
- prevent dual allow + block rows.

### search_profile_indexer_block

- UQ: (search_profile_id, indexer_instance_id).
- prevent dual allow + block rows.

### search_profile_tag_allow

- UQ: (search_profile_id, tag_id).
- prevent dual allow + block rows.

### search_profile_tag_block

- UQ: (search_profile_id, tag_id).
- prevent dual allow + block rows.

### search_profile_tag_prefer

- UQ: (search_profile_id, tag_id).
- weight_override int default 5, range -50..50.

### search_profile_policy_set

- UQ: (search_profile_id, policy_set_id).

### torznab_instance

- UQ: torznab_instance_public_id; (display_name).
- display_name length 1..256.
- api_key_hash text NN (Argon2id PHC string).
- is_enabled bool NN.
- deleted_at nullable.

### torznab_category

- UQ: torznab_cat_id.
- torznab_cat_id int NN.
- name length 1..128.

### media_domain_to_torznab_category

- UQ: (media_domain_id, torznab_category_id).
- UQ: (media_domain_id) WHERE is_primary = true.

### tracker_category_mapping

- UQ: (indexer_definition_id, tracker_category, tracker_subcategory).
- tracker_category >= 0; tracker_subcategory >= 0 (default 0).
- confidence numeric(4,3) default 1.0 (0..1 recommended).

### policy_set

- UQ: policy_set_public_id.
- scope enum; is_enabled bool NN; deleted_at nullable.
- sort_order int NN default 1000.
- UQ: (scope) WHERE scope='global' AND is_enabled=true AND deleted_at IS NULL.
- UQ: (user_id) WHERE scope='user' AND is_enabled=true AND deleted_at IS NULL.
- is_auto_created bool NN default false.
- created_for_search_request_id nullable (FK to search_request if set).

### policy_rule

- UQ: policy_rule_public_id.
- rule_type enum; match_field enum; match_operator enum; action enum; severity enum.
- sort_order int NN default 1000.
- is_disabled bool NN default false.
- rationale length <= 1024.
- match*operator=in_set: value_set_id NN and all match_value*\* NULL.
- match_field indexer_instance_public_id uses match_value_uuid or uuid set.
- match_field trust_tier_key/media_domain_key uses match_value_text lowercase.
- match_field title/release_group/uploader/tracker uses match_value_text.
- match_field infohash_v1/infohash_v2/magnet_hash only allow eq/in_set with strict hash length.
- match_field indexer_instance_public_id only allow eq/in_set.
- require_trust_tier_min requires match_value_int (rank) and match_operator=eq.
- match_field for require_trust_tier_min must be trust_tier_rank.
- regex length <= 512.
- valid rule_type/action combinations enforced.
- is_case_insensitive default true.

### policy_rule_value_set

- UQ: (policy_rule_id) (0/1 per rule).
- match_operator must be in_set for the owning rule.

### policy_rule_value_set_item

- UQ: (value_set_id, value_text/value_bigint/value_int/value_uuid) as applicable.
- Exactly one typed value set.
- text stored lowercase; uuid stored canonical lowercase.

### policy_snapshot

- UQ: (snapshot_hash).
- snapshot_hash lowercase hex length 64.
- ref_count >= 0.
- excluded_disabled_count >= 0.
- excluded_expired_count >= 0.

### policy_snapshot_rule

- UQ: (policy_snapshot_id, rule_order) and (policy_snapshot_id, policy_rule_public_id).
- rule_order >= 1.
- FK policy_snapshot_id cascades on delete.

### search_request

- UQ: search_request_public_id.
- status enum; query_type enum.
- torznab_mode nullable (torznab_mode).
- query_text length 0..512 (empty string allowed).
- user_id nullable (Torznab uses NULL).
- page_size range 10..200.
- requested_media_domain_id and effective_media_domain_id must reference media_domain if set.
- if torznab_mode is NULL and query_type=season_episode: season_number and episode_number NN.
- if torznab_mode is NULL and query_type != season_episode: season_number and episode_number NULL.
- if torznab_mode is tv: episode_number requires season_number.
- if torznab_mode is generic or movie: season_number and episode_number NULL.
- season_number >= 0; episode_number >= 0 when present.
- terminal states require finished_at.
- if status=canceled: canceled_at NN; if status != canceled: canceled_at NULL.
- if status=failed: failure_class NN; error_detail length <= 1024.
- validation (proc-level): require non-empty query_text or at least one identifier; for
  season_episode, require season/episode plus non-empty query_text or identifier.

### search_request_identifier

- UQ: (search_request_id, id_type).
- id_value_normalized matches type (imdb tt + 7..9 digits; tmdb/tvdb 1..10 digits).
- id_value_raw NN.

### search_request_torznab_category_requested

- UQ: (search_request_id, torznab_category_id).

### search_request_torznab_category_effective

- UQ: (search_request_id, torznab_category_id).

### search_request_indexer_run

- UQ: (search_request_id, indexer_instance_id).
- items_seen_count, items_emitted_count, and canonical_added_count >= 0.
- attempt_count >= 0.
- rate_limited_attempt_count >= 0.
- last_error_class enum if present.
- last_rate_limit_scope enum (rate_limit_scope) if present.
- last_error_class=rate_limited requires last_rate_limit_scope NOT NULL.
- error_class is set only when status=failed.
- last_correlation_id uuid nullable.
- error_detail length <= 1024.
- started_at required when status in (running, finished, failed, canceled).
- finished_at required when status in (finished, failed, canceled).

### search_request_indexer_run_correlation

- UQ: (search_request_indexer_run_id, correlation_id).
- correlation_id uuid NN.
- page_number >= 1 if present.

### indexer_run_cursor

- UQ: (search_request_indexer_run_id).
- Required fields by cursor_type:
    - offset_limit: offset >= 0, limit 1..500.
    - page_number: page >= 1.
    - since_time: since NN.
    - opaque_token: opaque_token length <= 1024.
- Only required fields may be non-null.

### search_request_canonical

- UQ: (search_request_id, canonical_torrent_id).

### search_page

- UQ: (search_request_id, page_number).
- page_number >= 1.
- sealed_at only set, never unset (proc rule).

### search_page_item

- UQ: (search_page_id, position).
- UQ: (search_request_canonical_id).
- position >= 1.

### search_request_source_observation

- UQ: per idempotency rules in section 15.
- observed_at NN.
- title_raw length <= 512.
- uploader length <= 256.
- source_guid length <= 256.
- seeders and leechers >= 0 when present.
- size_bytes >= 0 when present.
- infohash_v1/infohash_v2/magnet_hash formats length and lowercase.
- details_url, download_url, and magnet_uri length <= 2048.
- guid_conflict bool NN default false.

### search_request_source_observation_attr

- UQ: (observation_id, attr_key).
- attr_key enum.
- Exactly one value\_\* set.
- value_text length <= 512.
- value_numeric numeric(12,4) if present.

### canonical_torrent

- UQ: canonical_torrent_public_id.
- UQ: (infohash_v2) where not null.
- UQ: (infohash_v1) where not null.
- UQ: (magnet_hash) where not null.
- UQ: (title_size_hash) where not null.
- At least one of infohash_v1/infohash_v2/magnet_hash/title_size_hash present.
- identity_confidence range 0..1.
- hash formats length and lowercase.
- title_display and title_normalized length 1..512.
- imdb_id matches tt[0-9]{7,9} if set.
- tmdb_id and tvdb_id > 0 if set.

### canonical_size_rollup

- UQ: (canonical_torrent_id).
- sample_count >= 0.
- size_median, size_min, size_max >= 0.

### canonical_size_sample

- UQ: (canonical_torrent_id, observed_at, size_bytes).
- size_bytes >= 0.

### canonical_external_id

- UQ: (canonical_torrent_id, id_type, id_value_text) WHERE id_value_text IS NOT NULL.
- UQ: (canonical_torrent_id, id_type, id_value_int) WHERE id_value_int IS NOT NULL.
- id_type enum (imdb/tmdb/tvdb).
- Exactly one of id_value_text or id_value_int.
- imdb id_value_text matches tt[0-9]{7,9} (lowercase).
- tmdb/tvdb id_value_int > 0.
- trust_tier_rank NN; use rank=0 when unknown.

### canonical_torrent_source

- UQ: canonical_torrent_source_public_id.
- UQ: (indexer_instance_id, source_guid) when source_guid present.
- Idempotency UQs apply when source_guid is NULL (see section 15).
- title_normalized length <= 512 and NN.
- size_bytes >= 0 when present.
- infohash_v1/infohash_v2/magnet_hash formats length and lowercase.
- source_guid length <= 256.
- last_seen_seeders and last_seen_leechers >= 0 when present.
- last_seen_download_url/last_seen_magnet_uri/last_seen_details_url length <= 2048.
- last_seen_uploader length <= 256.

### canonical_torrent_source_attr

- UQ: (canonical_torrent_source_id, attr_key).
- Exactly one typed value set (including value_bool).
- value_text length <= 512; value_numeric numeric(12,4).
- imdb_id matches tt[0-9]{7,9}.
- tmdb_id and tvdb_id > 0.
- tracker_category and tracker_subcategory >= 0.

### source_metadata_conflict

- observed_at NN; resolved_at nullable.
- conflict_type enum (conflict_type).
- resolution enum nullable (conflict_resolution).
- existing_value and incoming_value length <= 256.
- resolved_by_user_id nullable.
- resolution_note length <= 256.

### source_metadata_conflict_audit_log

- action enum (source_metadata_conflict_action).
- note length <= 256.

### canonical_torrent_best_source_global

- UQ: (canonical_torrent_id).
- computed_at NN.

### canonical_torrent_best_source_context

- UQ: (context_key_type, context_key_id, canonical_torrent_id).
- computed_at NN.

### canonical_torrent_source_base_score

- UQ: (canonical_torrent_id, canonical_torrent_source_id).
- component scores numeric(12,4); computed_at NN.

### canonical_torrent_source_context_score

- UQ: (context_key_type, context_key_id, canonical_torrent_id, canonical_torrent_source_id).
- score_total_context numeric(12,4); computed_at NN.
- is_dropped bool NN.

### canonical_torrent_signal

- UQ: (canonical_torrent_id, signal_key, value_text, value_int).
- Exactly one value_text/value_int.
- value_text length <= 128.
- confidence range 0..1.
- parser_version >= 1.

### search_filter_decision

- decision enum; decision_detail length <= 512.
- Must reference canonical_torrent_id or canonical_torrent_source_id.
- policy_rule_public_id NN.
- policy_snapshot_id NN.
- observation_id may be NULL unless tied to a specific observation.

### user_result_action

- reason_text length <= 512.

### user_result_action_kv

- UQ: (user_result_action_id, key).
- value length <= 512.

### acquisition_attempt

- status enum; failure_class required if status=failed.
- At least one of infohash_v1/infohash_v2/magnet_hash present.
- torznab_instance_id nullable; canonical_torrent_id and canonical_torrent_source_id required.
- origin enum (acquisition_origin).
- torrent_client_id nullable; partial UQ applies only when present and name != unknown.
- torrent_client_id length <= 128.
- failure_detail length <= 256.
- finished_at required for terminal statuses.
- Optional UQ: (torrent_client_name, torrent_client_id)
  WHERE torrent_client_id IS NOT NULL AND torrent_client_name != 'unknown'.

### outbound_request_log

- request_type enum (outbound_request_type).
- outcome enum (outbound_request_outcome).
- correlation_id uuid NN.
- retry_seq >= 0.
- via_mitigation enum (outbound_via_mitigation).
- rate_limit_denied_scope enum (rate_limit_scope), nullable.
- error_class nullable when outcome=success.
- http_status 100..599 if present.
- latency_ms >= 0 if present.
- parse_ok bool NN.
- result_count >= 0 if present.
- cf_detected bool NN.
- page_number >= 1 if present.
- page_cursor_key length <= 64 if present.
- page_cursor_is_hashed bool NN.
- outcome=success requires parse_ok=true and error_class NULL.
- outcome=failure requires error_class NOT NULL.
- outcome=success and request_type in (caps, search, tvsearch, moviesearch, rss) requires
  result_count NN.
- outcome=failure and error_class != rate_limited requires result_count NULL.
- error_class=rate_limited requires result_count = 0.
- error_class=rate_limited requires rate_limit_denied_scope NOT NULL.

### indexer_health_event

- event_type enum.
- latency_ms >= 0 if present.
- http_status 100..599 if present.
- detail length <= 1024.

### indexer_connectivity_profile

- PK: indexer_instance_id.
- status enum.
- if status=healthy: error_class NULL; else NOT NULL.
- latency_p50_ms and latency_p95_ms >= 0 if present.
- success_rate_1h and success_rate_24h range 0..1.
- last_checked_at NN.

### source_reputation

- UQ: (indexer_instance_id, window_key, window_start).
- request_success_rate, acquisition_success_rate, fake_rate, dmca_rate range 0..1.
- request_count, request_success_count, acquisition_count, acquisition_success_count >= 0.
- min_samples >= 0.
- computed_at NN.

### job_schedule

- UQ: (job_key).
- cadence_seconds range 30..604800.
- jitter_seconds <= cadence_seconds.
- lock_owner length <= 128.

### deployment_config

- default_page_size range 10..200.
- retention_search_days 1..90.
- retention_health_events_days 1..90.
- retention_reputation_days 30..3650.
- retention_outbound_request_log_days 1..90.
- retention_source_metadata_conflict_days 1..365.
- retention_source_metadata_conflict_audit_days 7..3650.
- retention_rss_item_seen_days 1..365.
- connectivity_refresh_seconds 30..3600 if present.

### deployment_maintenance_state

- Singleton row per deployment.
- rss_subscription_backfill_completed_at nullable.
- last_updated_at NN.

### canonical_disambiguation_rule

- identity types enforce typed value columns.
- canonical_public_id uses value_uuid.
- infohash_v1 uses value_text length 40 hex lowercase.
- infohash_v2 and magnet_hash use value_text length 64 hex lowercase.
- reason length <= 256.

## 17. Query path index matrix (non-unique indexes)

### Search streaming page fetch

- search_request: (status, created_at DESC).
- search_request: (user_id, created_at DESC).
- search_request: (effective_media_domain_id, created_at DESC).
- search_page: (search_request_id, sealed_at) to find open page.
- search_page_item: (search_request_canonical_id) for dedupe checks.
- canonical_torrent_best_source_global: (canonical_torrent_id).
- canonical_torrent_best_source_context: (context_key_type, context_key_id, canonical_torrent_id).
- search_request_source_observation: (search_request_id, canonical_torrent_id, observed_at DESC).
- search_request_source_observation: (search_request_id, canonical_torrent_source_id, observed_at DESC).
- search_request_source_observation: (search_request_id, indexer_instance_id, observed_at DESC).
- canonical_torrent: (updated_at DESC).
- canonical_torrent: (title_normalized) if title search is needed.

### Torznab endpoints

- torznab_instance: (is_enabled).
- torznab_instance: (search_profile_id).
- tracker_category_mapping: (indexer_definition_id, tracker_category, tracker_subcategory).
- tracker_category_mapping: (tracker_category, tracker_subcategory)
  WHERE indexer_definition_id IS NULL.
- search_request_torznab_category_requested: (search_request_id).
- search_request_torznab_category_effective: (search_request_id).

### Observation attrs

- search_request_source_observation_attr: (attr_key).
- search_request_source_observation_attr: (observation_id).

### Source metadata conflicts

- source_metadata_conflict: (canonical_torrent_source_id, observed_at DESC).

### Canonical lookup by hash

- canonical_torrent: (infohash_v2) WHERE infohash_v2 IS NOT NULL.
- canonical_torrent: (infohash_v1) WHERE infohash_v1 IS NOT NULL.
- canonical_torrent: (magnet_hash) WHERE magnet_hash IS NOT NULL.
- canonical_torrent: (title_size_hash) WHERE title_size_hash IS NOT NULL.
- canonical_torrent: (title_normalized, size_bytes) WHERE size_bytes IS NOT NULL.
- canonical_torrent_source: (indexer_instance_id, infohash_v2)
  WHERE infohash_v2 IS NOT NULL AND source_guid IS NULL.
- canonical_torrent_source: (indexer_instance_id, infohash_v1)
  WHERE infohash_v1 IS NOT NULL AND source_guid IS NULL.
- canonical_torrent_source: (indexer_instance_id, magnet_hash)
  WHERE magnet_hash IS NOT NULL AND source_guid IS NULL.
- canonical_torrent_source: (indexer_instance_id, title_normalized, size_bytes)
  WHERE size_bytes IS NOT NULL AND source_guid IS NULL AND infohash_v2 IS NULL
  AND infohash_v1 IS NULL AND magnet_hash IS NULL.
- canonical_torrent_source: (last_seen_at DESC).
- canonical_disambiguation_rule: (identity_left_type, identity_left_value_text,
  identity_left_value_uuid).
- canonical_disambiguation_rule: (identity_right_type, identity_right_value_text,
  identity_right_value_uuid).
- canonical_disambiguation_rule: (identity_left_type, identity_left_value_text,
  identity_left_value_uuid, identity_right_type, identity_right_value_text,
  identity_right_value_uuid).
- acquisition_attempt: (infohash_v2, started_at DESC) WHERE infohash_v2 IS NOT NULL.
- acquisition_attempt: (infohash_v1, started_at DESC) WHERE infohash_v1 IS NOT NULL.
- acquisition_attempt: (magnet_hash, started_at DESC) WHERE magnet_hash IS NOT NULL.

### Policy evaluation join patterns

- policy_snapshot: (created_at DESC).
- policy_snapshot: (snapshot_hash).
- policy_snapshot_rule: (policy_snapshot_id, rule_order).
- policy_snapshot_rule: (policy_rule_public_id).
- policy_rule: (policy_set_id, rule_type).
- policy_rule: (policy_set_id, sort_order, policy_rule_public_id).
- search_profile_policy_set: (search_profile_id).
- policy_rule_value_set: (policy_rule_id).
- policy_rule_value_set_item: (value_set_id, value_text) WHERE value_text IS NOT NULL.
- policy_rule_value_set_item: (value_set_id, value_bigint) WHERE value_bigint IS NOT NULL.
- policy_rule_value_set_item: (value_set_id, value_int) WHERE value_int IS NOT NULL.
- policy_rule_value_set_item: (value_set_id, value_uuid) WHERE value_uuid IS NOT NULL.
- search_filter_decision: (search_request_id, decided_at DESC).
- search_filter_decision: (search_request_id, canonical_torrent_source_id, decided_at DESC)
  WHERE canonical_torrent_source_id IS NOT NULL.
- search_filter_decision: (observation_id, decided_at DESC).
- search_filter_decision: (canonical_torrent_id, decided_at DESC) WHERE canonical_torrent_id IS NOT NULL.
- search_filter_decision: (canonical_torrent_source_id, decided_at DESC)
  WHERE canonical_torrent_source_id IS NOT NULL.
- search_filter_decision: (policy_snapshot_id, decided_at DESC).

### Scoring and best source

- canonical_torrent_source_base_score: (canonical_torrent_id, score_total_base DESC).
- canonical_torrent_source_context_score: (context_key_type, context_key_id,
  canonical_torrent_id, score_total_context DESC).
- canonical_torrent_best_source_global: (canonical_torrent_id).
- canonical_torrent_best_source_context: (context_key_type, context_key_id, canonical_torrent_id).

### Outbound request telemetry

- outbound_request_log: (indexer_instance_id, started_at DESC).
- outbound_request_log: (indexer_instance_id, request_type, started_at DESC).
- outbound_request_log: (started_at DESC).
- outbound_request_log: (indexer_instance_id, outcome, started_at DESC).
- outbound_request_log: (indexer_instance_id, error_class, started_at DESC)
  WHERE error_class IS NOT NULL.
- outbound_request_log: (correlation_id, retry_seq).
- search_request_indexer_run_correlation: (search_request_indexer_run_id, created_at DESC).
- search_request_indexer_run_correlation: (correlation_id).

### RSS polling

- indexer_rss_subscription: (is_enabled, next_poll_at)
  WHERE is_enabled = true.

### CF state and rate limits

- indexer_cf_state: (state, last_changed_at DESC).
- indexer_instance_rate_limit: (rate_limit_policy_id).
- routing_policy_rate_limit: (rate_limit_policy_id).

### Health aggregation windows

- indexer_health_event: (indexer_instance_id, occurred_at DESC).
- indexer_health_event: (indexer_instance_id, event_type, occurred_at DESC).
- indexer_health_event: (occurred_at DESC).
- indexer_health_event: (indexer_instance_id, error_class, occurred_at DESC)
  WHERE error_class IS NOT NULL.
- indexer_connectivity_profile: (status).
- source_reputation: (indexer_instance_id, window_key, window_start DESC).
- source_reputation: (window_key, window_start DESC).
- job_schedule: (enabled, next_run_at) WHERE enabled = true.
- job_schedule: (job_key).

#### Indexing note

- Skip indexes that duplicate UQ or PK indexes; confirm before adding.
- If policy evaluation loads snapshots into memory, match-field indexes are lower priority.

## 18. Relationship summary (high level)

- deployment (implicit) 1 - 1 deployment_config
- deployment (implicit) 1 - 0/1 deployment_maintenance_state
- deployment (implicit) 1 - N torznab_instance
- torznab_instance 0..1 - N acquisition_attempt (nullable for non-Torznab)
- deployment (implicit) 1 - N rate_limit_policy
- deployment (implicit) 1 - N rate_limit_state
- deployment (implicit) 1 - N outbound_request_log
- deployment (implicit) 1 - N import_job
- indexer_definition 1 - N indexer_definition_field
- indexer_definition_field 1 - N indexer_definition_field_validation
- indexer_definition_field_validation 1 - 0/1 indexer_definition_field_value_set
- indexer_definition_field_value_set 1 - N indexer_definition_field_value_set_item
- indexer_definition_field 1 - N indexer_definition_field_option
- indexer_definition 1 - N indexer_instance
- indexer_instance N - N media_domain (via indexer_instance_media_domain)
- indexer_instance N - N tag (via indexer_instance_tag)
- indexer_instance 1 - 0/1 indexer_rss_subscription
- indexer_instance 1 - N indexer_rss_item_seen
- indexer_instance 1 - 0/1 routing_policy
- routing_policy 1 - N routing_policy_parameter
- import_job 1 - N import_indexer_result
- import_job 0..1 - 1 search_profile (target_search_profile_id)
- import_job 0..1 - 1 torznab_instance (target_torznab_instance_id)
- rate_limit_policy 1 - N indexer_instance_rate_limit
- rate_limit_policy 1 - N routing_policy_rate_limit
- indexer_instance 1 - 0/1 indexer_instance_rate_limit
- routing_policy 1 - 0/1 routing_policy_rate_limit
- rate_limit_state tracks token usage per indexer_instance or routing_policy (by scope_type)
  scoped by deployment.
- indexer_instance 1 - 1 indexer_cf_state
- indexer_instance 1 - N indexer_instance_field_value (secrets via secret_binding)
- secret 1 - N secret_binding
- secret 1 - N secret_audit_log
- search_profile 1 - N search_profile_media_domain
- search_profile 1 - N search_profile_trust_tier
- search_profile 1 - N search_profile_indexer_allow
- search_profile 1 - N search_profile_indexer_block
- search_profile 1 - N search_profile_tag_allow
- search_profile 1 - N search_profile_tag_block
- search_profile 1 - N search_profile_tag_prefer
- search_profile 1 - N search_profile_policy_set
- search_profile 1 - N torznab_instance
- deployment (implicit) 1 - N indexer_rss_subscription
- deployment (implicit) 1 - N indexer_rss_item_seen
- torznab_category 1 - N media_domain_to_torznab_category
- media_domain 1 - N media_domain_to_torznab_category
- tracker_category_mapping links indexer_definition (nullable) to torznab_category and media_domain
- policy_set 1 - N policy_rule
- policy_rule 1 - 0/1 policy_rule_value_set
- policy_rule_value_set 1 - N policy_rule_value_set_item
- policy_snapshot 1 - N policy_snapshot_rule
- search_request N - 1 policy_snapshot
- search_request 1 - N search_request_identifier
- search_request 1 - N search_request_torznab_category_requested
- search_request 1 - N search_request_torznab_category_effective
- search_request 1 - N search_request_indexer_run
- search_request_indexer_run 1 - N search_request_indexer_run_correlation
- search_request_indexer_run 1 - 0/1 indexer_run_cursor
- search_request 1 - N search_request_canonical -> canonical_torrent
- search_request 1 - N search_page 1 - N search_page_item -> search_request_canonical
- search_request 1 - N search_request_source_observation -> canonical_torrent_source
- search_request 1 - N acquisition_attempt (optional)
- search_request_source_observation 1 - N search_request_source_observation_attr
- search_request_source_observation 1 - N search_filter_decision (optional)
- canonical_torrent_source 1 - N canonical_torrent_source_attr
- canonical_torrent_source 1 - N acquisition_attempt
- canonical_torrent_source 1 - N source_metadata_conflict
- source_metadata_conflict 1 - N source_metadata_conflict_audit_log
- canonical_torrent 1 - N canonical_torrent_source_base_score
- canonical_torrent 1 - N acquisition_attempt
- canonical_torrent_source 1 - N canonical_torrent_source_base_score
- canonical_torrent 1 - N canonical_torrent_source_context_score
- canonical_torrent_source 1 - N canonical_torrent_source_context_score
- canonical_torrent 1 - 0/1 canonical_torrent_best_source_global
- canonical_torrent 1 - N canonical_torrent_best_source_context
- canonical_torrent 1 - N canonical_torrent_signal
- canonical_torrent 1 - N canonical_external_id
- canonical_torrent 1 - 0/1 canonical_size_rollup
- canonical_torrent 1 - N canonical_size_sample
- canonical_disambiguation_rule references canonical_torrent by public_id or hashes
- search_filter_decision links policy_snapshot and policy_rule_public_id to canonical_torrent
  or canonical_torrent_source, with optional observation_id
- user_result_action 1 - N user_result_action_kv
- acquisition_attempt links optional torznab_instance plus canonical_torrent,
  canonical_torrent_source, and optional search_request/user_id for audit.
- acquisition_attempt and user_result_action feed source_reputation and ranking
- outbound_request_log aggregates into indexer_connectivity_profile
- indexer_health_event is diagnostic only
- indexer_instance 1 - 1 indexer_connectivity_profile
- source_reputation rolls up by indexer_instance
- outbound_request_log references indexer_instance, routing_policy, and search_request
- config_audit_log references config entities
- job_schedule runs retention and reputation jobs

## 19. Revaer <- Prowlarr Migration Acceptance Checklist (v1)

### 0) Definition of “Seamless Migration”

A migration is considered successful if a user can:

- Switch Sonarr/Radarr Torznab URLs to Revaer.
- Import existing Prowlarr indexers.
- Resume normal grabbing without behavior regressions.
- Understand why nothing happens if nothing happens.
- Roll back instantly by changing a URL.

Anything that violates this causes abandonment.

### 1) Prowlarr Import (Hard Blocker)

#### 1.1 Import entry points

- Import via Prowlarr API.
- Import via Prowlarr backup/export.
- Dry-run mode (no writes) available.

#### 1.2 Indexer definition mapping

- Every imported indexer resolves to a known indexer_definition by upstream_slug.
- Unknown definitions are surfaced as:
    - state = unmapped
    - explanation shown
    - indexer disabled (never silently dropped)

#### 1.3 Indexer instance creation

- All indexers are created as indexer_instance.
- Enabled/disabled state preserved.
- Categories, tags, and priorities preserved.
- Missing secrets are detected immediately.

#### 1.4 Secrets handling

- Missing secrets produce a blocking state (“needs secret”).
- No indexer with missing secrets is silently runnable.
- Secret binding UX supports:
    - add secret
    - test indexer immediately
    - test reports exact failure class (auth, CF, rate limit, parse)

FAIL if: imported indexer “looks enabled” but never runs.

### 2) Torznab Parity (Hard Blocker)

#### 2.1 Endpoint fidelity

- Endpoint format: /torznab/{torznab_instance_public_id}/api.
- Auth via apikey query param only.
- Empty q + identifiers works.
- Invalid Torznab requests return empty results, not errors.
- No DB writes for invalid Torznab requests.

#### 2.2 Query semantics

- t=search|tvsearch|movie handled per spec.
- Season-only TV searches allowed.
- Episode requires season.
- Multiple explicit identifiers -> empty results.
- Explicit identifier overrides parsed identifier.

#### 2.3 Category handling

- Full seeded Torznab category list present.
- 8000 (Other) always present.
- Unknown category IDs are ignored (not errors).
- cat omitted = no restriction.
- cat includes 8000 means catch-all: allow results that map to requested categories
  OR do not map to any requested category.
- cat includes 3000/3010 (Music) does not constrain domain; these categories are
  unmapped in v1 and only filter results that explicitly map to them.
- Multi-cat filters map to a domain set; if more than one domain remains,
  effective_media_domain_id is NULL (multi-domain) and filtering relies on the
  category list.

#### 2.4 Pagination

- offset/limit maps to stable append-order results.
- No score-based reshuffling in Torznab responses.
- Paging is deterministic across retries.
- If offset exceeds currently available items while search is running, return empty.

FAIL if: Sonarr/Radarr behaves differently than with Prowlarr.

### 3) Download Reliability (Hard Blocker)

#### 3.1 Download endpoint

- /torznab/{instance}/download/{source}?apikey=...
- Records acquisition_attempt every time.
- Redirects (302) to:
    - magnet_uri first
    - download_url second
- If neither exists:
    - returns 404
    - still records failed acquisition_attempt

#### 3.2 Audit integrity

- acquisition_attempt stores:
    - canonical_torrent_id
    - canonical_torrent_source_id
    - torznab_instance_id (nullable for non-Torznab)
    - origin (torznab/ui/api/automation)
    - search_request_id (if applicable)

FAIL if: download fails silently or loses audit trail.

### 4) Rate Limiting & Cloudflare (Hard Blocker)

#### 4.1 Rate limit defaults

- Default indexer rate limit seeded.
- Default routing/proxy rate limit seeded.
- No “unlimited” behavior exists.

#### 4.2 Enforcement semantics

- Token bucket enforced via rate_limit_try_consume_v1.
- Per-indexer and per-routing budgets both enforced.
- Rate-limited attempts:
    - logged
    - do not consume outbound tokens
    - do not count as connectivity failures

#### 4.3 Retry behavior

- Rate-limited runs remain queued with backoff.
- Separate rate_limited_attempt_count.
- Cap applies only to rate-limited attempts.
- Non-rate-limited transient failures retry <= 3/page.

#### 4.4 Cloudflare handling

- CF detection heuristics implemented.
- CF state transitions deterministic.
- FlareSolverr preferred when challenged.
- CF reset clears only CF-related backoff/quarantine.

FAIL if: Revaer returns fewer results than Prowlarr due to bans.

### 5) Search UX & Stability (Soft but Fatal)

#### 5.1 Streaming behavior

- First page emitted as soon as possible.
- Results appended only (no reorder).
- Pages seal deterministically.

#### 5.2 Zero results explainability

- Zero runnable indexers -> search finishes immediately.
- UI/API exposes:
    - indexers skipped (why)
    - results blocked (which rule)
    - rate-limited / retrying

FAIL if: “Nothing found” with no explanation.

### 6) Policy & Rules (Trust Builder)

#### 6.1 Snapshot semantics

- policy_snapshot reused via hash.
- ref_count tracked transactionally.
- GC job removes unreferenced snapshots.

#### 6.2 Rule lifecycle

- Rules immutable post-reference (except is_disabled).
- Disable/enable supported explicitly.
- Auto-created request policy_sets hard-deleted on purge.
- User policy_sets never hard-deleted.

#### 6.3 Dropped results

- Hard-dropped sources still persisted for audit.
- Dropped sources never appear in paging.
- search_filter_decision includes:
    - canonical_id
    - source_id
    - observation_id (when applicable)

### 7) Canonicalization & Data Integrity

#### 7.1 Identity safety

- No-identity sources (no guid/hash/size) are rejected.
- title_size_fallback size_bytes immutable once set.
- Hash-based canonicals allow median size updates.

#### 7.2 Observations

- Latest observation by observed_at is authoritative.
- Durable last*seen*\* updated only monotonically.
- Observation attrs persisted per whitelist.
- Duplicate attrs in ingest rejected.

#### 7.3 Conflicts

- source_guid conflicts create conflict rows.
- Conflicts audited and retained.
- No silent overwrites of durable identity fields.

### 8) Stats & Insight (Trust Retention)

#### 8.1 Health & telemetry

- outbound_request_log is authoritative.
- result_count required on success.
- parse_ok true for empty results.
- rate_limited excluded from health stats.

#### 8.2 Reputation

- request_count derived from outbound logs.
- acquisition_success tracked separately.
- Reputation affects scoring deterministically.

FAIL if: stats contradict observable behavior.

### 9) Reversibility (User Safety Net)

- Revaer can run alongside Prowlarr.
- No destructive changes to Arr configs.
- Rollback = change Torznab URL back.
- No irreversible migration steps.

### 10) Final Acceptance Criteria (Go / No-Go)

Migration is ACCEPTED only if:

- All Hard Blockers pass.
- Torznab behavior matches Prowlarr under identical queries.
- Indexers produce equal or better results.
- Failure modes are explicit and inspectable.
- User can roll back without cleanup.

Migration is REJECTED if:

- Any indexer appears enabled but never runs.
- Downloads fail without explanation.
- Searches silently return nothing.
- Torznab behavior diverges from expectations.
- Cloudflare/rate limiting worsens reliability.
