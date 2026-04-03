# ERD Indexers Checklist

Derived from `ERD_INDEXERS.md`. Ordered for dependency-first implementation, clean layering, and testability. Check each item only when implemented and validated.

SSOT: `ERD_INDEXERS.md` is the source of truth. This checklist is a tracking index only. If any item conflicts with the ERD, the ERD wins and this file must be updated to match it.

## Phase 0 - Architecture and test harness
Source: `ERD_INDEXERS.md` §Scope and non-goals (v1), §Core data types and ID exposure, §Tenancy and scope.
- [x] Map ERD domains to crates/modules (config/migrations, api, ui, events, telemetry, services).
- [x] Define domain boundaries and DI interfaces for indexers, routing, secrets, policies, search, canonicalization, import, torznab, jobs, telemetry.
- [x] Establish SQL stored-proc test harness (transactional, seeded, deterministic clocks).
- [x] Define error-code taxonomy and constant error messages for DB procs and API responses.
- [x] Decide migration versioning strategy and wrapper proc naming (_v1 plus stable wrappers).
- [x] Confirm v1 scope/non-goals are enforced in architecture and route planning.

## Phase 1 - Schema foundations

### 1.1 Global conventions
Source: `ERD_INDEXERS.md` §Core data types and ID exposure, §Timestamp conventions, §Text column caps, §Hash identity rules, §Tenancy and scope, §Roles and permissions (v1), §Seed data ownership, §Soft delete boundaries, §FK on-delete rules (high level), §Audit fields, §JSON or JSONB prohibition, §Secrets linkage.
- [x] Enforce bigint PK identity + UUID public_id rules across required tables.
- [x] Ensure API boundaries use UUIDs/keys only (no internal bigint exposure across boundaries).
- [x] Ensure external references store UUIDs or keys only, never internal PKs.
- [x] Enforce created_at/updated_at defaults and audit fields (created_by/updated_by/changed_by).
- [x] Enforce system sentinel usage for system actions (user_id=0 or all-zero UUIDs).
- [x] Enforce deleted_at soft delete on specified tables.
- [x] Enforce text caps and lowercase key checks (varchar(128) keys, varchar(256) names, varchar(2048) URLs, varchar(512) regex, varchar(1024) notes).
- [x] Implement generated normalized columns (email_normalized, *_norm) where specified.
- [x] Implement hash identity rules for infohash_v1, infohash_v2, magnet_hash, title_size_hash, and title normalization.
- [x] Enforce mandatory public_id columns and omit them where disallowed (indexer_definition has no public_id in v1).
- [x] Enforce no JSON/JSONB types anywhere in the schema.
- [x] Enforce secrets linkage via `secret_binding` only (no inline secret_id columns).
- [x] Enforce single-tenant scope (no tenant scoping tables; global catalog tables are deployment-wide).

### 1.2 Enum types (all values exact)
Source: `ERD_INDEXERS.md` §Enum catalog and usage (all subsections).
- [x] upstream_source = {prowlarr_indexers}
- [x] protocol = {torrent, usenet}
- [x] engine = {torznab, cardigann}
- [x] field_type = {string, password, api_key, cookie, token, header_value, number_int, number_decimal, bool, select_single}
- [x] validation_type = {min_length, max_length, min_value, max_value, regex, allowed_value, required_if_field_equals}
- [x] depends_on_operator = {eq, neq, in_set}
- [x] value_set_type = {text, int, bigint, uuid}
- [x] trust_tier_key = {public, semi_private, private, invite_only}
- [x] media_domain_key = {movies, tv, audiobooks, ebooks, software, adult_movies, adult_scenes}
- [x] secret_type = {api_key, password, cookie, token, header_value}
- [x] secret_bound_table = {indexer_instance_field_value, routing_policy_parameter}
- [x] secret_binding_name = {api_key, password, cookie, token, header_value, proxy_password, socks_password}
- [x] routing_policy_mode = {direct, http_proxy, socks_proxy, flaresolverr, vpn_route, tor}
- [x] routing_param_key = {verify_tls, proxy_host, proxy_port, proxy_username, proxy_use_tls, http_proxy_auth, socks_host, socks_port, socks_username, socks_proxy_auth, fs_url, fs_timeout_ms, fs_session_ttl_seconds, fs_user_agent}
- [x] import_source_system = {prowlarr}
- [x] import_payload_format = {prowlarr_indexer_json_v1}
- [x] audit_entity_type = {indexer_instance, indexer_instance_field_value, routing_policy, routing_policy_parameter, policy_set, policy_rule, search_profile, search_profile_rule, tag, canonical_disambiguation_rule, torznab_instance, rate_limit_policy, tracker_category_mapping, media_domain_to_torznab_category}
- [x] audit_action = {create, update, enable, disable, soft_delete, restore}
- [x] secret_audit_action = {create, rotate, revoke, bind, unbind}
- [x] policy_scope = {global, user, profile, request}
- [x] policy_rule_type = {block_infohash_v1, block_infohash_v2, block_magnet, block_title_regex, block_release_group, block_uploader, block_tracker, block_indexer_instance, allow_release_group, allow_title_regex, allow_indexer_instance, downrank_title_regex, require_trust_tier_min, require_media_domain, prefer_indexer_instance, prefer_trust_tier}
- [x] policy_match_field = {infohash_v1, infohash_v2, magnet_hash, title, release_group, uploader, tracker, indexer_instance_public_id, media_domain_key, trust_tier_key, trust_tier_rank}
- [x] policy_match_operator = {eq, contains, regex, starts_with, ends_with, in_set}
- [x] policy_action = {drop_canonical, drop_source, downrank, require, prefer, flag}
- [x] policy_severity = {hard, soft}
- [x] deployment_role = {owner, admin, user}
- [x] import_source = {prowlarr_api, prowlarr_backup}
- [x] import_job_status = {pending, running, completed, failed, canceled}
- [x] import_indexer_result_status = {imported_ready, imported_needs_secret, imported_test_failed, unmapped_definition, skipped_duplicate}
- [x] indexer_instance_migration_state = {ready, needs_secret, test_failed, unmapped_definition, duplicate_suspected}
- [x] identifier_type = {imdb, tmdb, tvdb}
- [x] query_type = {free_text, imdb, tmdb, tvdb, season_episode}
- [x] torznab_mode = {generic, tv, movie}
- [x] search_status = {running, canceled, finished, failed}
- [x] failure_class = {coordinator_error, db_error, auth_error, invalid_request, timeout, canceled_by_system}
- [x] run_status = {queued, running, finished, failed, canceled}
- [x] error_class = {dns, tls, timeout, connection_refused, http_403, http_429, http_5xx, parse_error, auth_error, cf_challenge, rate_limited, unknown}
- [x] outbound_request_type = {caps, search, tvsearch, moviesearch, rss, probe}
- [x] outbound_request_outcome = {success, failure}
- [x] outbound_via_mitigation = {none, proxy, flaresolverr}
- [x] rate_limit_scope = {indexer_instance, routing_policy}
- [x] cf_state = {clear, challenged, solved, banned, cooldown}
- [x] cursor_type = {offset_limit, page_number, since_time, opaque_token}
- [x] identity_strategy = {infohash_v1, infohash_v2, magnet_hash, title_size_fallback}
- [x] durable_source_attr_key = {tracker_name, tracker_category, tracker_subcategory, size_bytes_reported, files_count, imdb_id, tmdb_id, tvdb_id, season, episode, year}
- [x] observation_attr_key = {tracker_name, tracker_category, tracker_subcategory, size_bytes_reported, files_count, imdb_id, tmdb_id, tvdb_id, season, episode, year, release_group, freeleech, internal_flag, scene_flag, minimum_ratio, minimum_seed_time_hours, language_primary, subtitles_primary}
- [x] attr_value_type = {text, int, bigint, numeric, bool, uuid}
- [x] signal_key = {release_group, resolution, source_type, codec, audio_codec, container, language, subtitles, edition, year, season, episode}
- [x] decision_type = {drop_canonical, drop_source, downrank, flag}
- [x] user_action = {viewed, selected, deselected, downloaded, blocked, reported_fake, preferred_source, separated_canonical, feedback_positive, feedback_negative}
- [x] user_reason_code = {none, wrong_title, wrong_language, wrong_quality, suspicious, known_bad_group, dmca_risk, dead_torrent, duplicate, personal_preference, other}
- [x] user_action_kv_key = {ui_surface, device, chosen_indexer_instance_public_id, chosen_source_public_id, note_short}
- [x] acquisition_status = {started, succeeded, failed, canceled}
- [x] acquisition_origin = {torznab, ui, api, automation}
- [x] acquisition_failure_class = {dead, dmca, passworded, corrupted, stalled, not_enough_space, auth_error, network_error, client_error, user_canceled, unknown}
- [x] torrent_client_name = {revaer_internal, transmission, qbittorrent, deluge, rtorrent, aria2, unknown}
- [x] health_event_type = {identity_conflict}
- [x] connectivity_status = {healthy, degraded, failing, quarantined}
- [x] reputation_window = {1h, 24h, 7d}
- [x] context_key_type = {policy_snapshot, search_profile, search_request}
- [x] scoring_context = {global_current}
- [x] job_key = {retention_purge, reputation_rollup_1h, reputation_rollup_24h, reputation_rollup_7d, connectivity_profile_refresh, canonical_backfill_best_source, base_score_refresh_recent, canonical_prune_low_confidence, policy_snapshot_gc, policy_snapshot_refcount_repair, rate_limit_state_purge, rss_poll, rss_subscription_backfill}
- [x] disambiguation_rule_type = {prevent_merge}
- [x] disambiguation_identity_type = {infohash_v1, infohash_v2, magnet_hash, canonical_public_id}
- [x] conflict_type = {tracker_name, tracker_category, external_id, hash, source_guid}
- [x] conflict_resolution = {accepted_incoming, kept_existing, merged, ignored}
- [x] source_metadata_conflict_action = {created, resolved, reopened, ignored}

## Phase 2 - Tables and relationships (dependency order)
Source: `ERD_INDEXERS.md` §§1-13 table definitions, §18 Relationship summary (high level), §Soft delete boundaries, §FK on-delete rules (high level).
- [x] Implement all tables with columns, CHECKs, defaults, UQ, and FK constraints exactly as specified in ERD.
- [x] Apply all per-table Notes blocks (validation rules, computed fields, invariants).

### 2.1 Deployment, users, and global config
- [x] Create `app_user`.
- [x] Create `deployment_config`.
- [x] Create `deployment_maintenance_state`.

### 2.2 Indexer definitions (global catalog)
- [x] Create `indexer_definition`.
- [x] Create `indexer_definition_field`.
- [x] Create `indexer_definition_field_validation`.
- [x] Create `indexer_definition_field_value_set`.
- [x] Create `indexer_definition_field_value_set_item`.
- [x] Create `indexer_definition_field_option`.

### 2.3 Trust tiers, media domains, tags
- [x] Create `trust_tier`.
- [x] Create `media_domain`.
- [x] Create `tag`.

### 2.4 Indexer instances and RSS
- [x] Create `indexer_instance`.
- [x] Create `indexer_instance_media_domain`.
- [x] Create `indexer_instance_tag`.
- [x] Create `indexer_rss_subscription`.
- [x] Create `indexer_rss_item_seen`.
- [x] Create `indexer_instance_field_value`.
- [x] Create `indexer_instance_import_blob`.

### 2.5 Import pipeline
- [x] Create `import_job`.
- [x] Create `import_indexer_result`.

### 2.6 Secrets
- [x] Create `secret`.
- [x] Create `secret_binding`.
- [x] Create `secret_audit_log`.

### 2.7 Routing policies
- [x] Create `routing_policy`.
- [x] Create `routing_policy_parameter`.

### 2.8 Rate limiting and CF state
- [x] Create `rate_limit_policy`.
- [x] Create `indexer_instance_rate_limit`.
- [x] Create `routing_policy_rate_limit`.
- [x] Create `rate_limit_state`.
- [x] Create `indexer_cf_state`.

### 2.9 Connectivity and audit
- [x] Create `indexer_connectivity_profile` (derived).
- [x] Create `indexer_health_event`.
- [x] Create `config_audit_log`.

### 2.10 Search profiles
- [x] Create `search_profile`.
- [x] Create `search_profile_media_domain`.
- [x] Create `search_profile_trust_tier`.
- [x] Create `search_profile_indexer_allow`.
- [x] Create `search_profile_indexer_block`.
- [x] Create `search_profile_tag_allow`.
- [x] Create `search_profile_tag_block`.
- [x] Create `search_profile_tag_prefer`.
- [x] Create `search_profile_policy_set`.

### 2.11 Torznab and categories
- [x] Create `torznab_instance`.
- [x] Create `torznab_category`.
- [x] Create `media_domain_to_torznab_category`.
- [x] Create `tracker_category_mapping`.

### 2.12 Policy sets and rules
- [x] Create `policy_set`.
- [x] Create `policy_rule`.
- [x] Create `policy_rule_value_set`.
- [x] Create `policy_rule_value_set_item`.
- [x] Create `policy_snapshot`.
- [x] Create `policy_snapshot_rule`.

### 2.13 Search requests and runs
- [x] Create `search_request`.
- [x] Create `search_request_identifier`.
- [x] Create `search_request_torznab_category_requested`.
- [x] Create `search_request_torznab_category_effective`.
- [x] Create `search_request_indexer_run`.
- [x] Create `search_request_indexer_run_correlation`.
- [x] Create `indexer_run_cursor`.
- [x] Create `search_request_canonical`.
- [x] Create `search_page`.
- [x] Create `search_page_item`.
- [x] Create `search_request_source_observation`.
- [x] Create `search_request_source_observation_attr`.

### 2.14 Canonicalization and sources
- [x] Create `canonical_torrent`.
- [x] Create `canonical_size_rollup`.
- [x] Create `canonical_size_sample`.
- [x] Create `canonical_external_id`.
- [x] Create `canonical_torrent_source`.
- [x] Create `canonical_torrent_source_attr`.
- [x] Create `canonical_torrent_signal`.
- [x] Create `canonical_disambiguation_rule`.

### 2.15 Scoring and best-source materializations
- [x] Create `canonical_torrent_source_base_score`.
- [x] Create `canonical_torrent_source_context_score`.
- [x] Create `canonical_torrent_best_source_global` (derived).
- [x] Create `canonical_torrent_best_source_context` (derived).

### 2.16 Conflicts and decisions
- [x] Create `source_metadata_conflict`.
- [x] Create `source_metadata_conflict_audit_log`.
- [x] Create `search_filter_decision`.

### 2.17 User actions and acquisition
- [x] Create `user_result_action`.
- [x] Create `user_result_action_kv`.
- [x] Create `acquisition_attempt`.

### 2.18 Telemetry and reputation
- [x] Create `outbound_request_log`.
- [x] Create `source_reputation` (derived).

### 2.19 Jobs and scheduling
- [x] Create `job_schedule`.

### 2.20 Relationships and on-delete rules
- [x] Enforce all FK relationships and on-delete behaviors from ERD section 18.
- [x] Verify soft delete boundaries match ERD section "Soft delete boundaries".

## Phase 3 - Seed data and deployment initialization
Source: `ERD_INDEXERS.md` §Seed data ownership, §Seed and deployment procedures.
- [x] Implement `trust_tier_seed_defaults()` with immutability checks.
- [x] Implement `media_domain_seed_defaults()` with lowercase enforcement.
- [x] Seed torznab categories and media-domain primary mappings.
- [x] Seed tracker_category_mapping global defaults.
- [x] Seed default rate_limit_policy rows (default_indexer, default_routing).
- [x] Seed `job_schedule` rows with required job keys and cadences.
- [x] Ensure system user row (user_id=0, public_id all-zero UUID).
- [x] Enforce seed ownership rules (trust_tier/media_domain/torznab_category seeded; tags are user-created only).

## Phase 4 - Indexes and query paths (non-unique)
Source: `ERD_INDEXERS.md` §17 Query path index matrix (non-unique indexes).
- [x] search_request(status, created_at DESC)
- [x] search_request(user_id, created_at DESC)
- [x] search_request(effective_media_domain_id, created_at DESC)
- [x] search_page(search_request_id, sealed_at)
- [x] search_page_item(search_request_canonical_id)
- [x] canonical_torrent_best_source_global(canonical_torrent_id)
- [x] canonical_torrent_best_source_context(context_key_type, context_key_id, canonical_torrent_id)
- [x] search_request_source_observation(search_request_id, canonical_torrent_id, observed_at DESC)
- [x] search_request_source_observation(search_request_id, canonical_torrent_source_id, observed_at DESC)
- [x] search_request_source_observation(search_request_id, indexer_instance_id, observed_at DESC)
- [x] canonical_torrent(updated_at DESC)
- [x] canonical_torrent(title_normalized) when title search is required
- [x] torznab_instance(is_enabled)
- [x] torznab_instance(search_profile_id)
- [x] tracker_category_mapping(indexer_definition_id, tracker_category, tracker_subcategory)
- [x] tracker_category_mapping(tracker_category, tracker_subcategory) WHERE indexer_definition_id IS NULL
- [x] search_request_torznab_category_requested(search_request_id)
- [x] search_request_torznab_category_effective(search_request_id)
- [x] search_request_source_observation_attr(attr_key)
- [x] search_request_source_observation_attr(observation_id)
- [x] source_metadata_conflict(canonical_torrent_source_id, observed_at DESC)
- [x] canonical_torrent(infohash_v2) WHERE infohash_v2 IS NOT NULL
- [x] canonical_torrent(infohash_v1) WHERE infohash_v1 IS NOT NULL
- [x] canonical_torrent(magnet_hash) WHERE magnet_hash IS NOT NULL
- [x] canonical_torrent(title_size_hash) WHERE title_size_hash IS NOT NULL
- [x] canonical_torrent(title_normalized, size_bytes) WHERE size_bytes IS NOT NULL
- [x] canonical_torrent_source(indexer_instance_id, infohash_v2) WHERE infohash_v2 IS NOT NULL AND source_guid IS NULL
- [x] canonical_torrent_source(indexer_instance_id, infohash_v1) WHERE infohash_v1 IS NOT NULL AND source_guid IS NULL
- [x] canonical_torrent_source(indexer_instance_id, magnet_hash) WHERE magnet_hash IS NOT NULL AND source_guid IS NULL
- [x] canonical_torrent_source(indexer_instance_id, title_normalized, size_bytes) WHERE size_bytes IS NOT NULL AND source_guid IS NULL AND infohash_v2 IS NULL AND infohash_v1 IS NULL AND magnet_hash IS NULL
- [x] canonical_torrent_source(last_seen_at DESC)
- [x] canonical_disambiguation_rule(identity_left_type, identity_left_value_text, identity_left_value_uuid)
- [x] canonical_disambiguation_rule(identity_right_type, identity_right_value_text, identity_right_value_uuid)
- [x] canonical_disambiguation_rule(identity_left_type, identity_left_value_text, identity_left_value_uuid, identity_right_type, identity_right_value_text, identity_right_value_uuid)
- [x] acquisition_attempt(infohash_v2, started_at DESC) WHERE infohash_v2 IS NOT NULL
- [x] acquisition_attempt(infohash_v1, started_at DESC) WHERE infohash_v1 IS NOT NULL
- [x] acquisition_attempt(magnet_hash, started_at DESC) WHERE magnet_hash IS NOT NULL
- [x] policy_snapshot(created_at DESC)
- [x] policy_snapshot(snapshot_hash)
- [x] policy_snapshot_rule(policy_snapshot_id, rule_order)
- [x] policy_snapshot_rule(policy_rule_public_id)
- [x] policy_rule(policy_set_id, rule_type)
- [x] policy_rule(policy_set_id, sort_order, policy_rule_public_id)
- [x] search_profile_policy_set(search_profile_id)
- [x] policy_rule_value_set(policy_rule_id)
- [x] policy_rule_value_set_item(value_set_id, value_text) WHERE value_text IS NOT NULL
- [x] policy_rule_value_set_item(value_set_id, value_bigint) WHERE value_bigint IS NOT NULL
- [x] policy_rule_value_set_item(value_set_id, value_int) WHERE value_int IS NOT NULL
- [x] policy_rule_value_set_item(value_set_id, value_uuid) WHERE value_uuid IS NOT NULL
- [x] search_filter_decision(search_request_id, decided_at DESC)
- [x] search_filter_decision(search_request_id, canonical_torrent_source_id, decided_at DESC) WHERE canonical_torrent_source_id IS NOT NULL
- [x] search_filter_decision(observation_id, decided_at DESC)
- [x] search_filter_decision(canonical_torrent_id, decided_at DESC) WHERE canonical_torrent_id IS NOT NULL
- [x] search_filter_decision(canonical_torrent_source_id, decided_at DESC) WHERE canonical_torrent_source_id IS NOT NULL
- [x] search_filter_decision(policy_snapshot_id, decided_at DESC)
- [x] canonical_torrent_source_base_score(canonical_torrent_id, score_total_base DESC)
- [x] canonical_torrent_source_context_score(context_key_type, context_key_id, canonical_torrent_id, score_total_context DESC)
- [x] canonical_torrent_best_source_global(canonical_torrent_id)
- [x] canonical_torrent_best_source_context(context_key_type, context_key_id, canonical_torrent_id)
- [x] outbound_request_log(indexer_instance_id, started_at DESC)
- [x] outbound_request_log(indexer_instance_id, request_type, started_at DESC)
- [x] outbound_request_log(started_at DESC)
- [x] outbound_request_log(indexer_instance_id, outcome, started_at DESC)
- [x] outbound_request_log(indexer_instance_id, error_class, started_at DESC) WHERE error_class IS NOT NULL
- [x] outbound_request_log(correlation_id, retry_seq)
- [x] search_request_indexer_run_correlation(search_request_indexer_run_id, created_at DESC)
- [x] search_request_indexer_run_correlation(correlation_id)
- [x] indexer_rss_subscription(is_enabled, next_poll_at) WHERE is_enabled = true
- [x] indexer_cf_state(state, last_changed_at DESC)
- [x] indexer_instance_rate_limit(rate_limit_policy_id)
- [x] routing_policy_rate_limit(rate_limit_policy_id)
- [x] indexer_health_event(indexer_instance_id, occurred_at DESC)
- [x] indexer_health_event(indexer_instance_id, event_type, occurred_at DESC)
- [x] indexer_health_event(occurred_at DESC)
- [x] indexer_health_event(indexer_instance_id, error_class, occurred_at DESC) WHERE error_class IS NOT NULL
- [x] indexer_connectivity_profile(status)
- [x] source_reputation(indexer_instance_id, window_key, window_start DESC)
- [x] source_reputation(window_key, window_start DESC)
- [x] job_schedule(enabled, next_run_at) WHERE enabled = true
- [x] job_schedule(job_key)
- [x] Skip indexes that duplicate PK/UQ indexes; confirm before adding.
- [x] If policy evaluation loads snapshots into memory, treat match-field indexes as lower priority (per ERD note).

## Gap review (2026-01-28)
Cross-checking the current implementation against `ERD_INDEXERS.md` and parity targets with Prowlarr surfaced the following gaps that still need implementation or verification. Leave these unchecked until code, schema, UI, and tests are in place.
- [x] Indexer app sync UX: UI/API surfaces to link indexers to downstream apps with tag-based scoping, sync profile selection, and per-app category filtering (parity with Prowlarr’s App Sync and tag rules).
  2026-03-21: `/indexers` now provisions downstream app sync paths by reusing or creating search profiles, applying media-domain/indexer/tag scoping, and issuing Torznab credentials in a single operator workflow.
- [x] Per-indexer proxy & CF/flaresolverr controls: configuration fields and stored procs for per-indexer proxy selection, Cloudflare challenge state, and flaresolverr toggle, including UI controls and health visibility.
- [x] Manual/interactive search UI: feature pages and API endpoints for parameter-based manual searches, category-level filtering, and pushing multiple results to download clients.
- [x] Category override support: allow custom category IDs per app/indexer (see Prowlarr feature request #1897) without breaking sync; schema+procs to persist overrides.
  2026-03-21: app-scoped Torznab-instance overrides now persist through stored procedures, round-trip through the `/indexers` admin console, and drive Torznab feed category emission with app-aware precedence instead of raw tracker ids.
- [x] Import pipeline UX: end-to-end UI and service paths for Cardigann/YAML definition import, status tracking, and conflict resolution beyond existing stored procs.
  2026-03-22: source metadata conflict list/resolve/reopen now round-trip through `/v1/indexers/conflicts` and the `/indexers` admin console.
  2026-03-21: Cardigann YAML definitions now import through `/v1/indexers/definitions/import/cardigann`, normalize into the catalog via stored procedures, and surface import status in the `/indexers` catalog UI.
- [x] Health & notifications: indexer health dashboard with status badges, failure reason drill-down, and notification hooks (email/webhook) matching Prowlarr indexer health.
  2026-03-21: `/indexers` now exposes operator-managed email/webhook health notification hooks backed by stored procedures, alongside the previously landed connectivity badges and health-event drill-down.
- [x] Per-indexer rate limits and schedule controls in UI (align with ERD rate_limit_policy linkage and indexer_instance_rate_limit table).
- [x] RSS management screens: enable/disable RSS per instance, view recent items, and mark-seen handling in UI (mapping to indexer_rss_subscription and indexer_rss_item_seen).
- [x] Connectivity profile & reputation views: expose indexer_connectivity_profile and source_reputation data in UI, with remediation actions.
- [x] Backup/restore of indexer settings: user-facing flows to export/import indexer configuration (definitions, instances, secrets bindings, routing policies).

## Branch analysis follow-up (2026-04-03)
Deep branch review against `ERD_INDEXERS.md` re-opened the following end-to-end gaps. These stay unchecked until the runtime path, operator surfaces, and acceptance coverage exist together.
- [ ] Implement a real search execution coordinator for manual search and Torznab requests: select runnable indexers, perform outbound requests through routing/rate-limit/CF mitigation, persist `outbound_request_log`, drive `search_indexer_run_mark_*`, and call `search_result_ingest` so `/v1/indexers/search-requests` and `/torznab/{instance}/api` return live results instead of request rows with no executor behind them.
- [ ] Implement runtime import executors for `prowlarr_api` and `prowlarr_backup`: fetch/parse remote payloads or backup blobs, resolve definitions/categories/tags/secrets, and populate `import_indexer_result`/`import_job` state beyond the current proc-triggered status markers.
- [ ] Implement the in-process indexer job scheduler/worker required by the ERD: claim due work from `job_schedule`, run retention/connectivity/reputation/best-source/policy GC/rate-limit purge jobs, and execute RSS polling/backfill on cadence inside the Revaer server process.
  2026-04-03: maintenance scheduler wiring now runs claimed stored-proc jobs in-process for retention, rollups, canonical upkeep, policy GC, rate-limit purge, and RSS subscription backfill; live RSS/search/import executor lanes remain open in the adjacent items.
- [x] Add read/list management surfaces for existing indexer resources across API and UI so operators can inspect and manage instances, routing policies, search profiles, policy sets/rules, Torznab instances, rate-limit policies, tags, and secret metadata without manually pasting known public IDs into action forms.
  2026-04-03: active tag and secret metadata inventories are now readable over `/v1/indexers/tags` and `/v1/indexers/secrets`, and the `/indexers` console can use those inventories to fill tag CRUD, tag allowlists, routing secret binds, field secret binds, and Prowlarr secret references.
  2026-04-03: routing policies, rate-limit policies, and indexer instances are now listable over `/v1/indexers/routing-policies`, `/v1/indexers/rate-limits`, and `/v1/indexers/instances`, and the `/indexers` console can use those inventories to prefill routing/rate-limit assignment and instance update/RSS/test actions.
  2026-04-03: search profiles, policy sets with rules, and Torznab instances are now listable over `/v1/indexers/search-profiles`, `/v1/indexers/policies`, and `/v1/indexers/torznab-instances`, and the `/indexers` console can use those inventories to prefill profile, policy, app-sync, Torznab, and category-mapping actions from live data.
- [ ] Extend CLI parity beyond the currently landed import/Torznab/policy/test commands to cover list/read and CRUD flows for tags, secrets, routing policies, rate limits, search profiles, backup/restore, RSS state, health/connectivity, and category mappings.
  2026-04-03: `revaer indexer read ...` now covers operator read/list flows for tags, secrets, search profiles, policy sets, routing policies (list/detail), rate-limit policies, indexer instances, Torznab instances, backup export, and per-instance connectivity, reputation, health-event, RSS status, and RSS seen-item inspection; CRUD parity and category-mapping commands still remain.
- [ ] Replace smoke-level acceptance coverage with live end-to-end parity coverage for import, search, Torznab, RSS polling, backup/restore, category overrides, and download audit trails; the current UI indexer spec is render-only and the final acceptance API spec mostly checks route presence and status codes.

## Phase 5 - Stored procedures and validation rules
Source: `ERD_INDEXERS.md` §14 Stored procedures (v1) and validation rules, §Versioning and error style, and all procedure sections.
 - [x] Add stable wrapper procs for each versioned proc (no version suffix).
- [x] Enforce error style and error_code mapping in all procedures.
- [x] Enforce role-based authorization and scope rules at the proc level.
- [x] Translate key-based inputs (trust_tier/media_domain/tag) to ids in-proc and raise invalid_request on unknown keys.
- [x] Ensure Torznab/system contexts accept actor_user_public_id NULL with sentinel audit fields.

### 5.1 Seed and deployment procedures
- [x] trust_tier_seed_defaults()
- [x] media_domain_seed_defaults()
- [x] deployment_init_v1(...)

### 5.2 app_user procedures
- [x] app_user_create_v1(...)
- [x] app_user_update_v1(...)
- [x] app_user_verify_email_v1(...)

### 5.3 Import job procedures
- [x] import_job_create_v1(...)
- [x] import_job_run_prowlarr_api_v1(...)
- [x] import_job_run_prowlarr_backup_v1(...)
- [x] import_job_get_status_v1(...)
- [x] import_job_list_results_v1(...)

### 5.4 Indexer instance procedures
- [x] indexer_instance_create_v1(...)
- [x] indexer_instance_update_v1(...)
- [x] indexer_rss_subscription_set_v1(...)
- [x] indexer_rss_subscription_disable_v1(...)
- [x] indexer_instance_set_media_domains_v1(...)
- [x] indexer_instance_set_tags_v1(...)
- [x] indexer_instance_field_set_value_v1(...)
- [x] indexer_instance_field_bind_secret_v1(...)
- [x] indexer_instance_test_prepare_v1(...)
- [x] indexer_instance_test_finalize_v1(...)

### 5.5 Tag procedures
- [x] tag_create_v1(...)
- [x] tag_update_v1(...)
- [x] tag_soft_delete_v1(...)

### 5.6 Routing policy procedures
- [x] routing_policy_create_v1(...)
- [x] routing_policy_set_param_v1(...)
- [x] routing_policy_bind_secret_v1(...)

### 5.7 Cloudflare procedures
- [x] indexer_cf_state_reset_v1(...)

### 5.8 Secret procedures
- [x] secret_create_v1(...)
- [x] secret_rotate_v1(...)
- [x] secret_revoke_v1(...)
- [x] secret_read_v1(...)

### 5.9 Policy procedures
- [x] policy_set_create_v1(...)
- [x] policy_set_update_v1(...)
- [x] policy_set_enable_v1(...)
- [x] policy_set_disable_v1(...)
- [x] policy_set_reorder_v1(...)
- [x] policy_rule_create_v1(...)
- [x] policy_rule_disable_v1(...)
- [x] policy_rule_enable_v1(...)
- [x] policy_rule_reorder_v1(...)

### 5.10 Search profile procedures
- [x] search_profile_create_v1(...)
- [x] search_profile_update_v1(...)
- [x] search_profile_set_default_v1(...)
- [x] search_profile_set_default_domain_v1(...)
- [x] search_profile_set_domain_allowlist_v1(...)
- [x] search_profile_add_policy_set_v1(...)
- [x] search_profile_remove_policy_set_v1(...)
- [x] search_profile_indexer_allow_v1(...)
- [x] search_profile_indexer_block_v1(...)
- [x] search_profile_tag_allow_v1(...)
- [x] search_profile_tag_block_v1(...)
- [x] search_profile_tag_prefer_v1(...)

### 5.11 Torznab procedures
- [x] torznab_instance_create_v1(...)
- [x] torznab_instance_rotate_key_v1(...)
- [x] torznab_instance_enable_disable_v1(...)
- [x] torznab_instance_soft_delete_v1(...)

### 5.12 Category mapping procedures
- [x] tracker_category_mapping_upsert_v1(...)
- [x] tracker_category_mapping_delete_v1(...)
- [x] media_domain_to_torznab_category_upsert_v1(...)
- [x] media_domain_to_torznab_category_delete_v1(...)

### 5.13 Rate limit procedures
- [x] rate_limit_policy_create_v1(...)
- [x] rate_limit_policy_update_v1(...)
- [x] rate_limit_policy_soft_delete_v1(...)
- [x] indexer_instance_set_rate_limit_policy_v1(...)
- [x] routing_policy_set_rate_limit_policy_v1(...)
- [x] rate_limit_try_consume_v1(...)

### 5.14 Search request procedures
- [x] search_request_create_v1(...)
- [x] search_request_cancel_v1(...)

### 5.15 Search run procedures
- [x] search_indexer_run_enqueue_v1(...)
- [x] search_indexer_run_mark_started_v1(...)
- [x] search_indexer_run_mark_finished_v1(...)
- [x] search_indexer_run_mark_failed_v1(...)
- [x] search_indexer_run_mark_canceled_v1(...)

### 5.16 Outbound request log procedures
- [x] outbound_request_log_write_v1(...)

### 5.17 Search ingestion procedure
- [x] search_result_ingest_v1(...)

### 5.18 Canonical maintenance procedures
- [x] canonical_merge_by_infohash_v1(...)
- [x] canonical_recompute_best_source_v1(...)
- [x] canonical_prune_low_confidence_v1(...)
- [x] canonical_disambiguation_rule_create_v1(...)

### 5.19 Conflict resolution procedures
- [x] source_metadata_conflict_resolve_v1(...)
- [x] source_metadata_conflict_reopen_v1(...)

### 5.20 Job runner procedures
- [x] job_claim_next_v1(...)
- [x] job_run_retention_purge_v1(...)
- [x] job_run_connectivity_profile_refresh_v1(...)
- [x] job_run_reputation_rollup_v1(...)
- [x] job_run_canonical_backfill_best_source_v1(...)
- [x] job_run_base_score_refresh_recent_v1(...)
- [x] job_run_rss_subscription_backfill_v1(...)
- [x] rss_poll_claim_v1(...)
- [x] rss_poll_apply_v1(...)
- [x] job_run_policy_snapshot_gc_v1(...)
- [x] job_run_policy_snapshot_refcount_repair_v1(...)
- [x] job_run_rate_limit_state_purge_v1(...)

## Phase 6 - Runtime logic and clean architecture
Source: `ERD_INDEXERS.md` §Core data types and ID exposure, §Roles and permissions (v1), §Secrets linkage.
- [x] Define per-crate error enums with constant messages and context fields.
- [x] Enforce Result-only returns and no panics/unwrap/expect in production paths.
- [x] Implement stored-proc callers with named bind parameters and explicit Result returns.
- [x] Wrap external system calls in tryOp-style functions (HTTP, FS, crypto, etc).
- [x] Implement normalization helpers (hashing, magnet normalization, key normalization).
- [x] Build domain services for indexer catalog sync, indexer instance management, routing policies, secrets, tags, search profiles, policy evaluation, search orchestration, canonicalization, reputation, torznab, import, jobs.
- [x] Enforce DI rules: only bootstrap creates concrete implementations and reads environment.
- [x] Add metrics and spans on all external boundaries and domain operations.
- [x] Ensure API surface only accepts UUIDs/keys and never exposes internal PKs.

## Phase 7 - Behavioral rules from ERD (domain logic)
Source: `ERD_INDEXERS.md` §Hash identity rules, §Retention policies (v1 defaults), §Source reputation windows and cadence, §Connectivity and reputation samples, §Derived table refresh strategy, §Background job execution.
- [x] Enforce hash identity rules for infohash_v1/v2 and magnet_hash derivation.
- [x] Enforce canonicalization rules (no-identity rejection, title_size_fallback invariants, median size updates, conflict handling).
- [x] Enforce observation rules (latest observation wins, monotonic last_seen, whitelisted attrs, duplicate attr rejection).
- [x] Enforce policy snapshot reuse by hash and ref_count tracking.
- [x] Enforce policy rule immutability and disable/enable semantics.
- [x] Enforce category mapping and domain filtering rules for Torznab and REST.
- [x] Enforce search request validation rules for identifiers, torznab_mode, season/episode, and category filters.
- [x] Enforce rate limit rules (token bucket, retry behavior, rate_limited semantics).
- [x] Enforce Cloudflare state transitions and mitigation choices.
- [x] Enforce RSS subscription scheduling, backoff, and dedupe behavior.
- [x] Enforce retention policies and derived table refresh strategy.
- [x] Enforce error logging at origin only; no re-logging on propagation.

## Phase 8 - API, Torznab, and surfaces
Source: `ERD_INDEXERS.md` §Roles and permissions (v1), §Torznab procedures, §Search request procedures, §Revaer <- Prowlarr Migration Acceptance Checklist (v1).
- [x] Implement REST endpoints for indexer definitions, instances, routing policies, secrets, tags, rate limits, and category mappings.
- [x] Implement REST endpoints for torznab instance management (create/rotate/enable/disable/delete).
- [x] Implement REST endpoints for search profiles.
- [x] Implement REST endpoints for policies.
- [x] Implement REST endpoints for import jobs.
- [x] Implement indexer tag create/update/delete endpoints at `/v1/indexers/tags`.
- [x] Implement rate limit policy CRUD and assignment endpoints for indexer instances and routing policies.
- [x] Implement category mapping endpoints for tracker and media-domain mappings.
- [x] Implement Torznab caps response (/torznab/{public_id}/api?t=caps) with API key auth and seeded categories.
- [x] Implement Torznab endpoints: /torznab/{public_id}/api and /torznab/{public_id}/download/{source}.
- [x] Implement Torznab auth via apikey query parameter only.
- [x] Ensure invalid Torznab requests return empty results with no DB writes.
- [x] Implement REST endpoints for search request create/cancel (v1).
- [x] Expose search streaming and page sealing behavior via API/SSE as specified.
- [x] Update OpenAPI export and docs for all new endpoints and schemas.
- [x] Update CLI commands for the currently landed indexer paths (import, indexer test, policy management, torznab keys).
  - [x] Import job commands.
  - [x] Torznab instance keys and lifecycle commands.
  - [x] Policy set and rule management commands.
  - [x] Indexer instance test command.
- [x] Implement UI flows for indexer management, secrets, policies, search profiles, and health insights if applicable.
- [x] Enforce auth requirements for search_request and canonical_torrent public_id access (v1 rule).
- [x] Ensure trust_tier/media_domain/tag are referenced by key in APIs and translated server-side.

## Phase 9 - Background jobs, retention, and derived refresh
Source: `ERD_INDEXERS.md` §Retention policies (v1 defaults), §Source reputation windows and cadence, §Connectivity and reputation samples, §Derived table refresh strategy, §Background job execution, §job_schedule.
- [x] Implement job claiming with advisory locks and locked_until semantics.
- [x] Implement retention purge logic per table and retention windows.
- [x] Implement policy_snapshot_gc and policy_snapshot_refcount_repair ordering.
- [x] Implement rate_limit_state purge (minute buckets older than 6 hours).
- [x] Implement connectivity_profile_refresh aggregation from outbound_request_log.
- [x] Implement reputation rollups for 1h, 24h, 7d windows with sample thresholds.
- [x] Implement canonical_backfill_best_source and base_score_refresh_recent cadence rules.
- [x] Implement canonical_prune_low_confidence policy.
- [x] Implement rss_poll and rss_subscription_backfill workflows.
- [x] Ensure derived tables refresh according to ERD timing and caching rules.
- [x] Seed next_run_at with jitter and update on completion per ERD.
- [x] Apply job lease durations per job_key when claiming work.

## Phase 10 - Observability and metrics
Source: `ERD_INDEXERS.md` §Observability requirements embedded across procedures and acceptance checklist.
- [x] Add tracing spans for http.request, indexer operations, search orchestration, policy evaluation, ingestion, canonicalization, torznab requests, and jobs.
- [x] Emit metrics for invalid Torznab requests, rate limiting, search throughput, and job outcomes.
- [x] Ensure error logging occurs only at origin and includes structured context fields.

## Phase 11 - Testing and validation
Source: `ERD_INDEXERS.md` §All sections; ensure tests cover each rule, procedure, and acceptance gate.
- [x] Add migration tests for all tables, constraints, and seeded data.
- [x] Add stored-proc tests for each procedure group and error paths.
- [x] Add unit tests for canonicalization, policy evaluation, category mapping, and search validation.
- [x] Add integration tests for REST endpoints and Torznab parity.
- [x] Add job runner tests for retention and rollups.
- [x] Add E2E tests for Prowlarr import, Torznab parity, and download flows.
- [x] Run just fmt, just lint, just udeps, just audit, just deny, just test, just cov, just build-release, and just ui-e2e.

## Phase 12 - Migration acceptance criteria (ERD section 19)
Source: `ERD_INDEXERS.md` §19 Revaer <- Prowlarr Migration Acceptance Checklist (v1).
- [x] Prowlarr import supports API and backup sources with dry-run mode.
- [x] Imported indexers map definitions or surface unmapped state explicitly.
- [x] Imported indexers preserve enabled/disabled, categories, tags, priorities, and detect missing secrets.
- [x] Secret binding UX supports add, test, and clear error classes.
- [x] Torznab endpoint format, auth, and invalid request handling match ERD.
- [x] Torznab query semantics (search/tvsearch/movie, identifiers, season/episode rules) match ERD.
- [x] Torznab categories, multi-cat domain mapping, and 8000 behavior match ERD.
- [x] Torznab pagination is deterministic and append-only.
- [x] Download endpoint redirects to magnet or download_url and always records acquisition_attempt.
- [x] Rate limit defaults exist and are enforced for indexer and routing scopes.
- [x] Retry behavior for rate-limited and transient errors matches ERD.
- [x] Cloudflare detection, state transitions, and FlareSolverr preference match ERD.
- [x] Streaming search behavior (first page early, append-only, deterministic seal) works.
- [x] Zero-result explainability surfaces skipped, blocked, and rate-limited info.
- [x] Policy snapshot reuse and GC rules match ERD.
- [x] Dropped sources are persisted for audit but excluded from paging.
- [x] Canonicalization safety rules and conflict logging are enforced.
- [x] Health and reputation stats match outbound_request_log semantics.
- [x] Revaer runs alongside Prowlarr and rollback is URL-only.
- [ ] Final acceptance criteria (all hard blockers) pass.
  2026-04-03: branch analysis re-opened this item because runtime search execution, runtime import execution, scheduled job workers, operator read/list surfaces, and live parity coverage are still incomplete.
