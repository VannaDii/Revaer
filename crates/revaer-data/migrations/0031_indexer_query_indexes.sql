-- Query path indexes for indexer ERD.

CREATE INDEX IF NOT EXISTS idx_search_request_status_created_at
    ON search_request (status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_search_request_user_created_at
    ON search_request (user_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_search_request_domain_created_at
    ON search_request (effective_media_domain_id, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_search_page_request_sealed
    ON search_page (search_request_id, sealed_at);

CREATE INDEX IF NOT EXISTS idx_srch_obs_req_canon_time
    ON search_request_source_observation (
        search_request_id,
        canonical_torrent_id,
        observed_at DESC
    );

CREATE INDEX IF NOT EXISTS idx_srch_obs_req_source_time
    ON search_request_source_observation (
        search_request_id,
        canonical_torrent_source_id,
        observed_at DESC
    );

CREATE INDEX IF NOT EXISTS idx_srch_obs_req_indexer_time
    ON search_request_source_observation (
        search_request_id,
        indexer_instance_id,
        observed_at DESC
    );

CREATE INDEX IF NOT EXISTS idx_canonical_torrent_updated_at
    ON canonical_torrent (updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_canonical_torrent_title_norm
    ON canonical_torrent (title_normalized);

CREATE INDEX IF NOT EXISTS idx_torznab_instance_enabled
    ON torznab_instance (is_enabled);

CREATE INDEX IF NOT EXISTS idx_torznab_instance_profile
    ON torznab_instance (search_profile_id);

CREATE INDEX IF NOT EXISTS idx_tracker_map_def_cat_sub
    ON tracker_category_mapping (
        indexer_definition_id,
        tracker_category,
        tracker_subcategory
    );

CREATE INDEX IF NOT EXISTS idx_tracker_map_global_cat_sub
    ON tracker_category_mapping (tracker_category, tracker_subcategory)
    WHERE indexer_definition_id IS NULL;

CREATE INDEX IF NOT EXISTS idx_srch_req_cat_requested
    ON search_request_torznab_category_requested (search_request_id);

CREATE INDEX IF NOT EXISTS idx_srch_req_cat_effective
    ON search_request_torznab_category_effective (search_request_id);

CREATE INDEX IF NOT EXISTS idx_srch_obs_attr_key
    ON search_request_source_observation_attr (attr_key);

CREATE INDEX IF NOT EXISTS idx_srch_obs_attr_observation
    ON search_request_source_observation_attr (observation_id);

CREATE INDEX IF NOT EXISTS idx_source_metadata_conflict_source_time
    ON source_metadata_conflict (canonical_torrent_source_id, observed_at DESC);

CREATE INDEX IF NOT EXISTS idx_canonical_torrent_title_size
    ON canonical_torrent (title_normalized, size_bytes)
    WHERE size_bytes IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_canon_source_idx_v2
    ON canonical_torrent_source (indexer_instance_id, infohash_v2)
    WHERE infohash_v2 IS NOT NULL AND source_guid IS NULL;

CREATE INDEX IF NOT EXISTS idx_canon_source_idx_v1
    ON canonical_torrent_source (indexer_instance_id, infohash_v1)
    WHERE infohash_v1 IS NOT NULL AND source_guid IS NULL;

CREATE INDEX IF NOT EXISTS idx_canon_source_idx_magnet
    ON canonical_torrent_source (indexer_instance_id, magnet_hash)
    WHERE magnet_hash IS NOT NULL AND source_guid IS NULL;

CREATE INDEX IF NOT EXISTS idx_canon_source_idx_title_size
    ON canonical_torrent_source (indexer_instance_id, title_normalized, size_bytes)
    WHERE size_bytes IS NOT NULL
      AND source_guid IS NULL
      AND infohash_v2 IS NULL
      AND infohash_v1 IS NULL
      AND magnet_hash IS NULL;

CREATE INDEX IF NOT EXISTS idx_canon_source_last_seen
    ON canonical_torrent_source (last_seen_at DESC);

CREATE INDEX IF NOT EXISTS idx_disambig_left_identity
    ON canonical_disambiguation_rule (
        identity_left_type,
        identity_left_value_text,
        identity_left_value_uuid
    );

CREATE INDEX IF NOT EXISTS idx_disambig_right_identity
    ON canonical_disambiguation_rule (
        identity_right_type,
        identity_right_value_text,
        identity_right_value_uuid
    );

CREATE INDEX IF NOT EXISTS idx_disambig_pair_identity
    ON canonical_disambiguation_rule (
        identity_left_type,
        identity_left_value_text,
        identity_left_value_uuid,
        identity_right_type,
        identity_right_value_text,
        identity_right_value_uuid
    );

CREATE INDEX IF NOT EXISTS idx_acquisition_infohash_v2_started
    ON acquisition_attempt (infohash_v2, started_at DESC)
    WHERE infohash_v2 IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_acquisition_infohash_v1_started
    ON acquisition_attempt (infohash_v1, started_at DESC)
    WHERE infohash_v1 IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_acquisition_magnet_started
    ON acquisition_attempt (magnet_hash, started_at DESC)
    WHERE magnet_hash IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_policy_snapshot_created_at
    ON policy_snapshot (created_at DESC);

CREATE INDEX IF NOT EXISTS idx_policy_snapshot_rule_public
    ON policy_snapshot_rule (policy_rule_public_id);

CREATE INDEX IF NOT EXISTS idx_policy_rule_set_type
    ON policy_rule (policy_set_id, rule_type);

CREATE INDEX IF NOT EXISTS idx_policy_rule_set_sort_pub
    ON policy_rule (policy_set_id, sort_order, policy_rule_public_id);

CREATE INDEX IF NOT EXISTS idx_search_profile_policy_set_profile
    ON search_profile_policy_set (search_profile_id);

CREATE INDEX IF NOT EXISTS idx_search_filter_decision_request_time
    ON search_filter_decision (search_request_id, decided_at DESC);

CREATE INDEX IF NOT EXISTS idx_search_filter_decision_request_source_time
    ON search_filter_decision (
        search_request_id,
        canonical_torrent_source_id,
        decided_at DESC
    )
    WHERE canonical_torrent_source_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_search_filter_decision_observation_time
    ON search_filter_decision (observation_id, decided_at DESC);

CREATE INDEX IF NOT EXISTS idx_search_filter_decision_canon_time
    ON search_filter_decision (canonical_torrent_id, decided_at DESC)
    WHERE canonical_torrent_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_search_filter_decision_source_time
    ON search_filter_decision (canonical_torrent_source_id, decided_at DESC)
    WHERE canonical_torrent_source_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_search_filter_decision_snapshot_time
    ON search_filter_decision (policy_snapshot_id, decided_at DESC);

CREATE INDEX IF NOT EXISTS idx_canon_source_base_score
    ON canonical_torrent_source_base_score (
        canonical_torrent_id,
        score_total_base DESC
    );

CREATE INDEX IF NOT EXISTS idx_canon_source_context_score
    ON canonical_torrent_source_context_score (
        context_key_type,
        context_key_id,
        canonical_torrent_id,
        score_total_context DESC
    );

CREATE INDEX IF NOT EXISTS idx_outbound_log_instance_started
    ON outbound_request_log (indexer_instance_id, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_outbound_log_instance_type_started
    ON outbound_request_log (indexer_instance_id, request_type, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_outbound_log_started
    ON outbound_request_log (started_at DESC);

CREATE INDEX IF NOT EXISTS idx_outbound_log_instance_outcome_started
    ON outbound_request_log (indexer_instance_id, outcome, started_at DESC);

CREATE INDEX IF NOT EXISTS idx_outbound_log_instance_error_started
    ON outbound_request_log (indexer_instance_id, error_class, started_at DESC)
    WHERE error_class IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_outbound_log_correlation_retry
    ON outbound_request_log (correlation_id, retry_seq);

CREATE INDEX IF NOT EXISTS idx_run_correlation_run_created
    ON search_request_indexer_run_correlation (
        search_request_indexer_run_id,
        created_at DESC
    );

CREATE INDEX IF NOT EXISTS idx_run_correlation_id
    ON search_request_indexer_run_correlation (correlation_id);

CREATE INDEX IF NOT EXISTS idx_rss_subscription_enabled_next
    ON indexer_rss_subscription (is_enabled, next_poll_at)
    WHERE is_enabled = TRUE;

CREATE INDEX IF NOT EXISTS idx_cf_state_status_changed
    ON indexer_cf_state (state, last_changed_at DESC);

CREATE INDEX IF NOT EXISTS idx_instance_rate_limit_policy
    ON indexer_instance_rate_limit (rate_limit_policy_id);

CREATE INDEX IF NOT EXISTS idx_routing_rate_limit_policy
    ON routing_policy_rate_limit (rate_limit_policy_id);

CREATE INDEX IF NOT EXISTS idx_health_event_instance_time
    ON indexer_health_event (indexer_instance_id, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_health_event_instance_type_time
    ON indexer_health_event (indexer_instance_id, event_type, occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_health_event_time
    ON indexer_health_event (occurred_at DESC);

CREATE INDEX IF NOT EXISTS idx_health_event_instance_error_time
    ON indexer_health_event (indexer_instance_id, error_class, occurred_at DESC)
    WHERE error_class IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_connectivity_profile_status
    ON indexer_connectivity_profile (status);

CREATE INDEX IF NOT EXISTS idx_source_reputation_window_start
    ON source_reputation (window_key, window_start DESC);

CREATE INDEX IF NOT EXISTS idx_job_schedule_enabled_next
    ON job_schedule (enabled, next_run_at)
    WHERE enabled = TRUE;
