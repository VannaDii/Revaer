-- BEGIN 0005_no_jsonb.sql
-- Remove JSONB usage from configuration and runtime schemas.

-- Backfill normalized tables from legacy JSON columns (if present).
DO $$
DECLARE
    app_id UUID := '00000000-0000-0000-0000-000000000001';
    engine_id UUID := '00000000-0000-0000-0000-000000000002';
    fs_id UUID := '00000000-0000-0000-0000-000000000003';
    api_rec RECORD;
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'app_profile'
          AND column_name = 'telemetry'
    ) THEN
        PERFORM revaer_config.persist_app_telemetry(
            app_id,
            (SELECT telemetry FROM public.app_profile WHERE id = app_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'app_profile'
          AND column_name = 'features'
    ) THEN
        PERFORM revaer_config.persist_app_features(
            app_id,
            (SELECT features FROM public.app_profile WHERE id = app_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'app_profile'
          AND column_name = 'immutable_keys'
    ) THEN
        PERFORM revaer_config.persist_app_immutable_keys(
            app_id,
            (SELECT immutable_keys FROM public.app_profile WHERE id = app_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'engine_profile'
          AND column_name = 'listen_interfaces'
    ) THEN
        PERFORM revaer_config.persist_engine_list(
            engine_id,
            'listen_interfaces',
            (SELECT listen_interfaces FROM public.engine_profile WHERE id = engine_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'engine_profile'
          AND column_name = 'dht_bootstrap_nodes'
    ) THEN
        PERFORM revaer_config.persist_engine_list(
            engine_id,
            'dht_bootstrap_nodes',
            (SELECT dht_bootstrap_nodes FROM public.engine_profile WHERE id = engine_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'engine_profile'
          AND column_name = 'dht_router_nodes'
    ) THEN
        PERFORM revaer_config.persist_engine_list(
            engine_id,
            'dht_router_nodes',
            (SELECT dht_router_nodes FROM public.engine_profile WHERE id = engine_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'engine_profile'
          AND column_name = 'ip_filter'
    ) THEN
        PERFORM revaer_config.persist_ip_filter_config(
            engine_id,
            (SELECT ip_filter FROM public.engine_profile WHERE id = engine_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'engine_profile'
          AND column_name = 'alt_speed'
    ) THEN
        PERFORM revaer_config.persist_alt_speed_config(
            engine_id,
            (SELECT alt_speed FROM public.engine_profile WHERE id = engine_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'fs_policy'
          AND column_name = 'cleanup_keep'
    ) THEN
        PERFORM revaer_config.persist_fs_list(
            fs_id,
            'cleanup_keep',
            (SELECT cleanup_keep FROM public.fs_policy WHERE id = fs_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'fs_policy'
          AND column_name = 'cleanup_drop'
    ) THEN
        PERFORM revaer_config.persist_fs_list(
            fs_id,
            'cleanup_drop',
            (SELECT cleanup_drop FROM public.fs_policy WHERE id = fs_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'fs_policy'
          AND column_name = 'allow_paths'
    ) THEN
        PERFORM revaer_config.persist_fs_list(
            fs_id,
            'allow_paths',
            (SELECT allow_paths FROM public.fs_policy WHERE id = fs_id)
        );
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'public'
          AND table_name = 'auth_api_keys'
          AND column_name = 'rate_limit'
    ) THEN
        FOR api_rec IN
            SELECT key_id, rate_limit
            FROM public.auth_api_keys
        LOOP
            PERFORM revaer_config.update_api_key_rate_limit(api_rec.key_id, api_rec.rate_limit);
        END LOOP;
    END IF;
END;
$$;

-- Runtime metadata: capture comment/source/private and file list in normalized tables.
ALTER TABLE IF EXISTS revaer_runtime.torrents
    ADD COLUMN IF NOT EXISTS comment TEXT,
    ADD COLUMN IF NOT EXISTS source TEXT,
    ADD COLUMN IF NOT EXISTS private BOOLEAN;

CREATE TABLE IF NOT EXISTS revaer_runtime.torrent_files (
    torrent_id UUID NOT NULL REFERENCES revaer_runtime.torrents(torrent_id) ON DELETE CASCADE,
    file_index INTEGER NOT NULL,
    path TEXT NOT NULL,
    size_bytes BIGINT NOT NULL,
    bytes_completed BIGINT NOT NULL,
    priority TEXT NOT NULL,
    selected BOOLEAN NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (torrent_id, file_index)
);

CREATE TRIGGER torrent_files_touch_updated_at
BEFORE UPDATE ON revaer_runtime.torrent_files
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'revaer_runtime'
          AND table_name = 'torrents'
          AND column_name = 'payload'
    ) THEN
        UPDATE revaer_runtime.torrents
        SET comment = NULLIF(payload->>'comment', ''),
            source = NULLIF(payload->>'source', ''),
            private = CASE
                WHEN jsonb_typeof(payload->'private') = 'boolean'
                    THEN (payload->>'private')::BOOLEAN
                ELSE NULL
            END
        WHERE payload IS NOT NULL;
    END IF;

    IF EXISTS (
        SELECT 1
        FROM information_schema.columns
        WHERE table_schema = 'revaer_runtime'
          AND table_name = 'torrents'
          AND column_name = 'files'
    ) THEN
        DELETE FROM revaer_runtime.torrent_files;
        INSERT INTO revaer_runtime.torrent_files (
            torrent_id,
            file_index,
            path,
            size_bytes,
            bytes_completed,
            priority,
            selected
        )
        SELECT t.torrent_id,
               (entry->>'index')::INTEGER,
               entry->>'path',
               COALESCE((entry->>'size_bytes')::BIGINT, 0),
               COALESCE((entry->>'bytes_completed')::BIGINT, 0),
               COALESCE(entry->>'priority', 'normal'),
               COALESCE((entry->>'selected')::BOOLEAN, TRUE)
        FROM revaer_runtime.torrents AS t
        JOIN LATERAL jsonb_array_elements(t.files) AS entry ON TRUE
        WHERE t.files IS NOT NULL
          AND jsonb_typeof(t.files) = 'array';
    END IF;
END;
$$;

-- Drop legacy JSON columns.
ALTER TABLE public.app_profile
    DROP COLUMN IF EXISTS telemetry,
    DROP COLUMN IF EXISTS features,
    DROP COLUMN IF EXISTS immutable_keys;

ALTER TABLE public.engine_profile
    DROP COLUMN IF EXISTS dht_bootstrap_nodes,
    DROP COLUMN IF EXISTS dht_router_nodes,
    DROP COLUMN IF EXISTS ip_filter,
    DROP COLUMN IF EXISTS listen_interfaces,
    DROP COLUMN IF EXISTS alt_speed;

ALTER TABLE public.fs_policy
    DROP COLUMN IF EXISTS cleanup_keep,
    DROP COLUMN IF EXISTS cleanup_drop,
    DROP COLUMN IF EXISTS allow_paths;

ALTER TABLE public.auth_api_keys
    DROP COLUMN IF EXISTS rate_limit;

DROP TABLE IF EXISTS public.settings_history;

ALTER TABLE revaer_runtime.torrents
    DROP COLUMN IF EXISTS payload,
    DROP COLUMN IF EXISTS files;

-- Remove legacy JSON-based helpers.
DROP FUNCTION IF EXISTS revaer_config.insert_history(TEXT, JSONB, JSONB, TEXT, TEXT, BIGINT);
DROP FUNCTION IF EXISTS revaer_config.fetch_app_profile_json(UUID);
DROP FUNCTION IF EXISTS revaer_config.fetch_engine_profile_json(UUID);
DROP FUNCTION IF EXISTS revaer_config.fetch_fs_policy_json(UUID);
DROP FUNCTION IF EXISTS revaer_config.fetch_api_keys_json();
DROP FUNCTION IF EXISTS revaer_config.persist_app_immutable_keys(UUID, JSONB);
DROP FUNCTION IF EXISTS revaer_config.persist_app_telemetry(UUID, JSONB);
DROP FUNCTION IF EXISTS revaer_config.persist_app_features(UUID, JSONB);
DROP FUNCTION IF EXISTS revaer_config.render_app_immutable_keys(UUID);
DROP FUNCTION IF EXISTS revaer_config.render_app_telemetry(UUID);
DROP FUNCTION IF EXISTS revaer_config.render_app_features(UUID);
DROP FUNCTION IF EXISTS revaer_config.persist_engine_list(UUID, TEXT, JSONB);
DROP FUNCTION IF EXISTS revaer_config.persist_ip_filter_config(UUID, JSONB);
DROP FUNCTION IF EXISTS revaer_config.persist_alt_speed_config(UUID, JSONB);
DROP FUNCTION IF EXISTS revaer_config.persist_fs_list(UUID, TEXT, JSONB);
DROP FUNCTION IF EXISTS revaer_config.render_engine_list(UUID, TEXT);
DROP FUNCTION IF EXISTS revaer_config.render_ip_filter_config(UUID);
DROP FUNCTION IF EXISTS revaer_config.render_alt_speed_config(UUID);
DROP FUNCTION IF EXISTS revaer_config.render_fs_list(UUID, TEXT);
DROP FUNCTION IF EXISTS revaer_config.persist_peer_class_config(UUID, JSONB);
DROP FUNCTION IF EXISTS revaer_config.render_peer_classes(UUID);
DROP FUNCTION IF EXISTS revaer_config.render_tracker_config(UUID);
DROP FUNCTION IF EXISTS revaer_config.normalize_alt_speed(JSONB);
DROP FUNCTION IF EXISTS revaer_config.update_app_telemetry(UUID, JSONB);
DROP FUNCTION IF EXISTS revaer_config.update_app_features(UUID, JSONB);
DROP FUNCTION IF EXISTS revaer_config.update_app_immutable_keys(UUID, JSONB);
DROP FUNCTION IF EXISTS revaer_config.update_engine_profile(
    UUID,
    TEXT,
    INTEGER,
    BOOLEAN,
    TEXT,
    INTEGER,
    BIGINT,
    BIGINT,
    DOUBLE PRECISION,
    BIGINT,
    BOOLEAN,
    BOOLEAN,
    BOOLEAN,
    BOOLEAN,
    BOOLEAN,
    TEXT,
    TEXT,
    BOOLEAN,
    INTEGER,
    BIGINT,
    TEXT,
    TEXT,
    TEXT,
    BOOLEAN,
    INTEGER,
    INTEGER,
    BOOLEAN,
    BOOLEAN,
    BOOLEAN,
    TEXT,
    TEXT,
    BOOLEAN,
    JSONB,
    BOOLEAN,
    BOOLEAN,
    BOOLEAN,
    BOOLEAN,
    JSONB,
    JSONB,
    JSONB,
    JSONB,
    TEXT,
    BOOLEAN,
    BOOLEAN,
    BOOLEAN,
    BOOLEAN,
    BOOLEAN,
    BOOLEAN,
    INTEGER,
    INTEGER,
    INTEGER,
    INTEGER,
    INTEGER,
    INTEGER,
    INTEGER,
    JSONB,
    INTEGER,
    JSONB
);
DROP FUNCTION IF EXISTS revaer_config.update_fs_array_field(UUID, TEXT, JSONB);
DROP FUNCTION IF EXISTS revaer_config.update_api_key_rate_limit(TEXT, JSONB);
DROP FUNCTION IF EXISTS revaer_config.insert_api_key(TEXT, TEXT, TEXT, BOOLEAN, JSONB);
DROP FUNCTION IF EXISTS revaer_runtime.upsert_torrent(
    UUID,
    TEXT,
    TEXT,
    TEXT,
    BIGINT,
    BIGINT,
    BIGINT,
    BIGINT,
    BIGINT,
    DOUBLE PRECISION,
    BOOLEAN,
    TEXT,
    TEXT,
    JSONB,
    JSONB,
    TIMESTAMPTZ,
    TIMESTAMPTZ,
    TIMESTAMPTZ
);
DROP FUNCTION IF EXISTS revaer_runtime.list_torrents();

-- App profile fetch + updates.
CREATE OR REPLACE FUNCTION revaer_config.fetch_app_profile_row(_id UUID)
RETURNS TABLE (
    id UUID,
    instance_name TEXT,
    mode TEXT,
    version BIGINT,
    http_port INTEGER,
    bind_addr TEXT,
    telemetry_level TEXT,
    telemetry_format TEXT,
    telemetry_otel_enabled BOOLEAN,
    telemetry_otel_service_name TEXT,
    telemetry_otel_endpoint TEXT,
    immutable_keys TEXT[]
) AS
$$
BEGIN
    RETURN QUERY
    SELECT ap.id,
           ap.instance_name,
           ap.mode,
           ap.version,
           ap.http_port,
           ap.bind_addr::TEXT,
           ap.telemetry_level,
           ap.telemetry_format,
           ap.telemetry_otel_enabled,
           ap.telemetry_otel_service_name,
           ap.telemetry_otel_endpoint,
           COALESCE(
               (
                   SELECT array_agg(key ORDER BY ord)
                   FROM public.app_profile_immutable_keys
                   WHERE profile_id = ap.id
               ),
               ARRAY[]::TEXT[]
           )
    FROM public.app_profile AS ap
    WHERE ap.id = _id;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.list_app_label_policies(_profile_id UUID)
RETURNS TABLE (
    kind TEXT,
    name TEXT,
    download_dir TEXT,
    rate_limit_download_bps BIGINT,
    rate_limit_upload_bps BIGINT,
    queue_position INTEGER,
    auto_managed BOOLEAN,
    seed_ratio_limit DOUBLE PRECISION,
    seed_time_limit BIGINT,
    cleanup_seed_ratio_limit DOUBLE PRECISION,
    cleanup_seed_time_limit BIGINT,
    cleanup_remove_data BOOLEAN
) AS
$$
BEGIN
    RETURN QUERY
    SELECT alp.kind,
           alp.name,
           alp.download_dir,
           alp.rate_limit_download_bps,
           alp.rate_limit_upload_bps,
           alp.queue_position,
           alp.auto_managed,
           alp.seed_ratio_limit,
           alp.seed_time_limit,
           alp.cleanup_seed_ratio_limit,
           alp.cleanup_seed_time_limit,
           alp.cleanup_remove_data
    FROM public.app_label_policies AS alp
    WHERE alp.profile_id = _profile_id
    ORDER BY alp.kind, alp.name;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.update_app_telemetry(
    _id UUID,
    _level TEXT,
    _format TEXT,
    _otel_enabled BOOLEAN,
    _otel_service_name TEXT,
    _otel_endpoint TEXT
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET telemetry_level = _level,
        telemetry_format = _format,
        telemetry_otel_enabled = _otel_enabled,
        telemetry_otel_service_name = _otel_service_name,
        telemetry_otel_endpoint = _otel_endpoint
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_app_immutable_keys(
    _profile_id UUID,
    _keys TEXT[]
) RETURNS VOID AS
$$
BEGIN
    DELETE FROM public.app_profile_immutable_keys WHERE profile_id = _profile_id;

    INSERT INTO public.app_profile_immutable_keys (profile_id, key, ord)
    SELECT _profile_id,
           btrim(value),
           ord
    FROM unnest(COALESCE(_keys, ARRAY[]::TEXT[])) WITH ORDINALITY AS t(value, ord)
    WHERE btrim(value) <> '';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.replace_app_label_policies(
    _profile_id UUID,
    _kinds TEXT[],
    _names TEXT[],
    _download_dirs TEXT[],
    _rate_limit_download_bps BIGINT[],
    _rate_limit_upload_bps BIGINT[],
    _queue_positions INTEGER[],
    _auto_managed BOOLEAN[],
    _seed_ratio_limits DOUBLE PRECISION[],
    _seed_time_limits BIGINT[],
    _cleanup_seed_ratio_limits DOUBLE PRECISION[],
    _cleanup_seed_time_limits BIGINT[],
    _cleanup_remove_data BOOLEAN[]
) RETURNS VOID AS
$$
BEGIN
    DELETE FROM public.app_label_policies WHERE profile_id = _profile_id;

    INSERT INTO public.app_label_policies (
        profile_id,
        kind,
        name,
        download_dir,
        rate_limit_download_bps,
        rate_limit_upload_bps,
        queue_position,
        auto_managed,
        seed_ratio_limit,
        seed_time_limit,
        cleanup_seed_ratio_limit,
        cleanup_seed_time_limit,
        cleanup_remove_data
    )
    SELECT _profile_id,
           btrim(kind),
           btrim(name),
           NULLIF(btrim(download_dir), ''),
           rate_limit_download_bps,
           rate_limit_upload_bps,
           queue_position,
           auto_managed,
           seed_ratio_limit,
           seed_time_limit,
           cleanup_seed_ratio_limit,
           cleanup_seed_time_limit,
           cleanup_remove_data
    FROM unnest(
        COALESCE(_kinds, ARRAY[]::TEXT[]),
        COALESCE(_names, ARRAY[]::TEXT[]),
        COALESCE(_download_dirs, ARRAY[]::TEXT[]),
        COALESCE(_rate_limit_download_bps, ARRAY[]::BIGINT[]),
        COALESCE(_rate_limit_upload_bps, ARRAY[]::BIGINT[]),
        COALESCE(_queue_positions, ARRAY[]::INTEGER[]),
        COALESCE(_auto_managed, ARRAY[]::BOOLEAN[]),
        COALESCE(_seed_ratio_limits, ARRAY[]::DOUBLE PRECISION[]),
        COALESCE(_seed_time_limits, ARRAY[]::BIGINT[]),
        COALESCE(_cleanup_seed_ratio_limits, ARRAY[]::DOUBLE PRECISION[]),
        COALESCE(_cleanup_seed_time_limits, ARRAY[]::BIGINT[]),
        COALESCE(_cleanup_remove_data, ARRAY[]::BOOLEAN[])
    ) AS t(
        kind,
        name,
        download_dir,
        rate_limit_download_bps,
        rate_limit_upload_bps,
        queue_position,
        auto_managed,
        seed_ratio_limit,
        seed_time_limit,
        cleanup_seed_ratio_limit,
        cleanup_seed_time_limit,
        cleanup_remove_data
    )
    WHERE btrim(kind) <> ''
      AND btrim(name) <> '';
END;
$$ LANGUAGE plpgsql;

-- Engine profile helpers.
CREATE OR REPLACE FUNCTION revaer_config.fetch_engine_profile_row(_id UUID)
RETURNS TABLE (
    id UUID,
    implementation TEXT,
    listen_port INTEGER,
    dht BOOLEAN,
    encryption TEXT,
    max_active INTEGER,
    max_download_bps BIGINT,
    max_upload_bps BIGINT,
    seed_ratio_limit DOUBLE PRECISION,
    seed_time_limit BIGINT,
    sequential_default BOOLEAN,
    auto_managed BOOLEAN,
    auto_manage_prefer_seeds BOOLEAN,
    dont_count_slow_torrents BOOLEAN,
    super_seeding BOOLEAN,
    choking_algorithm TEXT,
    seed_choking_algorithm TEXT,
    strict_super_seeding BOOLEAN,
    optimistic_unchoke_slots INTEGER,
    max_queued_disk_bytes BIGINT,
    resume_dir TEXT,
    download_root TEXT,
    storage_mode TEXT,
    use_partfile BOOLEAN,
    cache_size INTEGER,
    cache_expiry INTEGER,
    coalesce_reads BOOLEAN,
    coalesce_writes BOOLEAN,
    use_disk_cache_pool BOOLEAN,
    disk_read_mode TEXT,
    disk_write_mode TEXT,
    verify_piece_hashes BOOLEAN,
    enable_lsd BOOLEAN,
    enable_upnp BOOLEAN,
    enable_natpmp BOOLEAN,
    enable_pex BOOLEAN,
    listen_interfaces TEXT[],
    dht_bootstrap_nodes TEXT[],
    dht_router_nodes TEXT[],
    ipv6_mode TEXT,
    anonymous_mode BOOLEAN,
    force_proxy BOOLEAN,
    prefer_rc4 BOOLEAN,
    allow_multiple_connections_per_ip BOOLEAN,
    enable_outgoing_utp BOOLEAN,
    enable_incoming_utp BOOLEAN,
    outgoing_port_min INTEGER,
    outgoing_port_max INTEGER,
    peer_dscp INTEGER,
    connections_limit INTEGER,
    connections_limit_per_torrent INTEGER,
    unchoke_slots INTEGER,
    half_open_limit INTEGER,
    stats_interval_ms INTEGER,
    alt_speed_download_bps BIGINT,
    alt_speed_upload_bps BIGINT,
    alt_speed_schedule_start_minutes INTEGER,
    alt_speed_schedule_end_minutes INTEGER,
    alt_speed_days TEXT[],
    ip_filter_blocklist_url TEXT,
    ip_filter_etag TEXT,
    ip_filter_last_updated_at TIMESTAMPTZ,
    ip_filter_last_error TEXT,
    ip_filter_cidrs TEXT[],
    tracker_user_agent TEXT,
    tracker_announce_ip TEXT,
    tracker_listen_interface TEXT,
    tracker_request_timeout_ms INTEGER,
    tracker_announce_to_all BOOLEAN,
    tracker_replace_trackers BOOLEAN,
    tracker_proxy_host TEXT,
    tracker_proxy_port INTEGER,
    tracker_proxy_kind TEXT,
    tracker_proxy_username_secret TEXT,
    tracker_proxy_password_secret TEXT,
    tracker_proxy_peers BOOLEAN,
    tracker_default_urls TEXT[],
    tracker_extra_urls TEXT[],
    peer_class_ids SMALLINT[],
    peer_class_labels TEXT[],
    peer_class_download_priorities SMALLINT[],
    peer_class_upload_priorities SMALLINT[],
    peer_class_connection_limit_factors SMALLINT[],
    peer_class_ignore_unchoke_slots BOOLEAN[],
    peer_class_default_ids SMALLINT[]
) AS
$$
BEGIN
    RETURN QUERY
    SELECT ep.id,
           ep.implementation,
           ep.listen_port,
           ep.dht,
           ep.encryption,
           ep.max_active,
           ep.max_download_bps,
           ep.max_upload_bps,
           ep.seed_ratio_limit,
           ep.seed_time_limit,
           ep.sequential_default,
           ep.auto_managed,
           ep.auto_manage_prefer_seeds,
           ep.dont_count_slow_torrents,
           ep.super_seeding,
           ep.choking_algorithm,
           ep.seed_choking_algorithm,
           ep.strict_super_seeding,
           ep.optimistic_unchoke_slots,
           ep.max_queued_disk_bytes,
           ep.resume_dir,
           ep.download_root,
           ep.storage_mode,
           ep.use_partfile,
           ep.cache_size,
           ep.cache_expiry,
           ep.coalesce_reads,
           ep.coalesce_writes,
           ep.use_disk_cache_pool,
           ep.disk_read_mode,
           ep.disk_write_mode,
           ep.verify_piece_hashes,
           ep.enable_lsd,
           ep.enable_upnp,
           ep.enable_natpmp,
           ep.enable_pex,
           COALESCE(
               (
                   SELECT array_agg(value ORDER BY ord)
                   FROM public.engine_profile_list_values
                   WHERE profile_id = ep.id AND kind = 'listen_interfaces'
               ),
               ARRAY[]::TEXT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(value ORDER BY ord)
                   FROM public.engine_profile_list_values
                   WHERE profile_id = ep.id AND kind = 'dht_bootstrap_nodes'
               ),
               ARRAY[]::TEXT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(value ORDER BY ord)
                   FROM public.engine_profile_list_values
                   WHERE profile_id = ep.id AND kind = 'dht_router_nodes'
               ),
               ARRAY[]::TEXT[]
           ),
           ep.ipv6_mode,
           ep.anonymous_mode,
           ep.force_proxy,
           ep.prefer_rc4,
           ep.allow_multiple_connections_per_ip,
           ep.enable_outgoing_utp,
           ep.enable_incoming_utp,
           ep.outgoing_port_min,
           ep.outgoing_port_max,
           ep.peer_dscp,
           ep.connections_limit,
           ep.connections_limit_per_torrent,
           ep.unchoke_slots,
           ep.half_open_limit,
           ep.stats_interval_ms,
           ea.download_bps,
           ea.upload_bps,
           ea.schedule_start_minutes,
           ea.schedule_end_minutes,
           COALESCE(
               (
                   SELECT array_agg(day ORDER BY ord)
                   FROM public.engine_alt_speed_days
                   WHERE profile_id = ep.id
               ),
               ARRAY[]::TEXT[]
           ),
           eif.blocklist_url,
           eif.etag,
           eif.last_updated_at,
           eif.last_error,
           COALESCE(
               (
                   SELECT array_agg(cidr ORDER BY ord)
                   FROM public.engine_ip_filter_entries
                   WHERE profile_id = ep.id
               ),
               ARRAY[]::TEXT[]
           ),
           etc.user_agent,
           etc.announce_ip,
           etc.listen_interface,
           etc.request_timeout_ms,
           etc.announce_to_all,
           etc.replace_trackers,
           etc.proxy_host,
           etc.proxy_port,
           etc.proxy_kind,
           etc.proxy_username_secret,
           etc.proxy_password_secret,
           etc.proxy_peers,
           COALESCE(
               (
                   SELECT array_agg(url ORDER BY ord)
                   FROM public.engine_tracker_endpoints
                   WHERE profile_id = ep.id AND kind = 'default'
               ),
               ARRAY[]::TEXT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(url ORDER BY ord)
                   FROM public.engine_tracker_endpoints
                   WHERE profile_id = ep.id AND kind = 'extra'
               ),
               ARRAY[]::TEXT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(class_id ORDER BY class_id)
                   FROM public.engine_peer_classes
                   WHERE profile_id = ep.id
               ),
               ARRAY[]::SMALLINT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(label ORDER BY class_id)
                   FROM public.engine_peer_classes
                   WHERE profile_id = ep.id
               ),
               ARRAY[]::TEXT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(download_priority ORDER BY class_id)
                   FROM public.engine_peer_classes
                   WHERE profile_id = ep.id
               ),
               ARRAY[]::SMALLINT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(upload_priority ORDER BY class_id)
                   FROM public.engine_peer_classes
                   WHERE profile_id = ep.id
               ),
               ARRAY[]::SMALLINT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(connection_limit_factor ORDER BY class_id)
                   FROM public.engine_peer_classes
                   WHERE profile_id = ep.id
               ),
               ARRAY[]::SMALLINT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(ignore_unchoke_slots ORDER BY class_id)
                   FROM public.engine_peer_classes
                   WHERE profile_id = ep.id
               ),
               ARRAY[]::BOOLEAN[]
           ),
           COALESCE(
               (
                   SELECT array_agg(class_id ORDER BY class_id)
                   FROM public.engine_peer_class_defaults
                   WHERE profile_id = ep.id
               ),
               ARRAY[]::SMALLINT[]
           )
    FROM public.engine_profile AS ep
    LEFT JOIN public.engine_alt_speed AS ea ON ea.profile_id = ep.id
    LEFT JOIN public.engine_ip_filter AS eif ON eif.profile_id = ep.id
    LEFT JOIN public.engine_tracker_config AS etc ON etc.profile_id = ep.id
    WHERE ep.id = _id;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.list_engine_peer_classes(_profile_id UUID)
RETURNS TABLE (
    class_id SMALLINT,
    label TEXT,
    download_priority SMALLINT,
    upload_priority SMALLINT,
    connection_limit_factor SMALLINT,
    ignore_unchoke_slots BOOLEAN
) AS
$$
BEGIN
    RETURN QUERY
    SELECT epc.class_id,
           epc.label,
           epc.download_priority,
           epc.upload_priority,
           epc.connection_limit_factor,
           epc.ignore_unchoke_slots
    FROM public.engine_peer_classes AS epc
    WHERE epc.profile_id = _profile_id
    ORDER BY epc.class_id;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.list_engine_peer_class_defaults(_profile_id UUID)
RETURNS TABLE (class_id SMALLINT) AS
$$
BEGIN
    RETURN QUERY
    SELECT epd.class_id
    FROM public.engine_peer_class_defaults AS epd
    WHERE epd.profile_id = _profile_id
    ORDER BY epd.class_id;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.set_engine_list_values(
    _profile_id UUID,
    _kind TEXT,
    _values TEXT[]
) RETURNS VOID AS
$$
BEGIN
    IF _kind NOT IN ('listen_interfaces', 'dht_bootstrap_nodes', 'dht_router_nodes') THEN
        RAISE EXCEPTION 'engine list kind % is not supported', _kind;
    END IF;

    DELETE FROM public.engine_profile_list_values
    WHERE profile_id = _profile_id AND kind = _kind;

    INSERT INTO public.engine_profile_list_values (profile_id, kind, ord, value)
    SELECT _profile_id,
           _kind,
           ord,
           btrim(value)
    FROM unnest(COALESCE(_values, ARRAY[]::TEXT[])) WITH ORDINALITY AS t(value, ord)
    WHERE btrim(value) <> '';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.set_engine_ip_filter(
    _profile_id UUID,
    _blocklist_url TEXT,
    _etag TEXT,
    _last_updated_at TIMESTAMPTZ,
    _last_error TEXT,
    _cidrs TEXT[]
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO public.engine_ip_filter AS eif (
        profile_id,
        blocklist_url,
        etag,
        last_updated_at,
        last_error
    )
    VALUES (
        _profile_id,
        NULLIF(_blocklist_url, ''),
        NULLIF(_etag, ''),
        _last_updated_at,
        NULLIF(_last_error, '')
    )
    ON CONFLICT (profile_id) DO UPDATE
    SET blocklist_url = EXCLUDED.blocklist_url,
        etag = EXCLUDED.etag,
        last_updated_at = EXCLUDED.last_updated_at,
        last_error = EXCLUDED.last_error,
        updated_at = now();

    DELETE FROM public.engine_ip_filter_entries
    WHERE profile_id = _profile_id;

    INSERT INTO public.engine_ip_filter_entries (profile_id, ord, cidr)
    SELECT _profile_id,
           ord,
           btrim(value)
    FROM unnest(COALESCE(_cidrs, ARRAY[]::TEXT[])) WITH ORDINALITY AS t(value, ord)
    WHERE btrim(value) <> '';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.set_engine_alt_speed(
    _profile_id UUID,
    _download_bps BIGINT,
    _upload_bps BIGINT,
    _schedule_start_minutes INTEGER,
    _schedule_end_minutes INTEGER,
    _days TEXT[]
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO public.engine_alt_speed AS eas (
        profile_id,
        download_bps,
        upload_bps,
        schedule_start_minutes,
        schedule_end_minutes
    )
    VALUES (
        _profile_id,
        _download_bps,
        _upload_bps,
        _schedule_start_minutes,
        _schedule_end_minutes
    )
    ON CONFLICT (profile_id) DO UPDATE
    SET download_bps = EXCLUDED.download_bps,
        upload_bps = EXCLUDED.upload_bps,
        schedule_start_minutes = EXCLUDED.schedule_start_minutes,
        schedule_end_minutes = EXCLUDED.schedule_end_minutes,
        updated_at = now();

    DELETE FROM public.engine_alt_speed_days
    WHERE profile_id = _profile_id;

    INSERT INTO public.engine_alt_speed_days (profile_id, ord, day)
    SELECT _profile_id,
           ord,
           day
    FROM unnest(COALESCE(_days, ARRAY[]::TEXT[])) WITH ORDINALITY AS t(day, ord)
    WHERE day IN ('mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun');
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.set_tracker_config(
    _profile_id UUID,
    _user_agent TEXT,
    _announce_ip TEXT,
    _listen_interface TEXT,
    _request_timeout_ms INTEGER,
    _announce_to_all BOOLEAN,
    _replace_trackers BOOLEAN,
    _proxy_host TEXT,
    _proxy_port INTEGER,
    _proxy_kind TEXT,
    _proxy_username_secret TEXT,
    _proxy_password_secret TEXT,
    _proxy_peers BOOLEAN,
    _default_urls TEXT[],
    _extra_urls TEXT[]
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO public.engine_tracker_config AS etc (
        profile_id,
        user_agent,
        announce_ip,
        listen_interface,
        request_timeout_ms,
        announce_to_all,
        replace_trackers,
        proxy_host,
        proxy_port,
        proxy_kind,
        proxy_username_secret,
        proxy_password_secret,
        proxy_peers
    )
    VALUES (
        _profile_id,
        NULLIF(_user_agent, ''),
        NULLIF(_announce_ip, ''),
        NULLIF(_listen_interface, ''),
        _request_timeout_ms,
        COALESCE(_announce_to_all, FALSE),
        COALESCE(_replace_trackers, FALSE),
        NULLIF(_proxy_host, ''),
        _proxy_port,
        NULLIF(_proxy_kind, ''),
        NULLIF(_proxy_username_secret, ''),
        NULLIF(_proxy_password_secret, ''),
        COALESCE(_proxy_peers, FALSE)
    )
    ON CONFLICT (profile_id) DO UPDATE
    SET user_agent = EXCLUDED.user_agent,
        announce_ip = EXCLUDED.announce_ip,
        listen_interface = EXCLUDED.listen_interface,
        request_timeout_ms = EXCLUDED.request_timeout_ms,
        announce_to_all = EXCLUDED.announce_to_all,
        replace_trackers = EXCLUDED.replace_trackers,
        proxy_host = EXCLUDED.proxy_host,
        proxy_port = EXCLUDED.proxy_port,
        proxy_kind = EXCLUDED.proxy_kind,
        proxy_username_secret = EXCLUDED.proxy_username_secret,
        proxy_password_secret = EXCLUDED.proxy_password_secret,
        proxy_peers = EXCLUDED.proxy_peers,
        updated_at = now();

    DELETE FROM public.engine_tracker_endpoints
    WHERE profile_id = _profile_id;

    INSERT INTO public.engine_tracker_endpoints (profile_id, kind, url, ord)
    SELECT _profile_id,
           'default',
           btrim(url),
           ord
    FROM unnest(COALESCE(_default_urls, ARRAY[]::TEXT[])) WITH ORDINALITY AS t(url, ord)
    WHERE btrim(url) <> '';

    INSERT INTO public.engine_tracker_endpoints (profile_id, kind, url, ord)
    SELECT _profile_id,
           'extra',
           btrim(url),
           ord
    FROM unnest(COALESCE(_extra_urls, ARRAY[]::TEXT[])) WITH ORDINALITY AS t(url, ord)
    WHERE btrim(url) <> '';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.set_peer_classes(
    _profile_id UUID,
    _class_ids SMALLINT[],
    _labels TEXT[],
    _download_priorities SMALLINT[],
    _upload_priorities SMALLINT[],
    _connection_limit_factors SMALLINT[],
    _ignore_unchoke_slots BOOLEAN[],
    _default_class_ids SMALLINT[]
) RETURNS VOID AS
$$
BEGIN
    DELETE FROM public.engine_peer_class_defaults WHERE profile_id = _profile_id;
    DELETE FROM public.engine_peer_classes WHERE profile_id = _profile_id;

    INSERT INTO public.engine_peer_classes AS epc (
        profile_id,
        class_id,
        label,
        download_priority,
        upload_priority,
        connection_limit_factor,
        ignore_unchoke_slots
    )
    SELECT _profile_id,
           class_id,
           COALESCE(NULLIF(btrim(label), ''), format('class_%s', class_id)),
           download_priority,
           upload_priority,
           connection_limit_factor,
           ignore_unchoke_slots
    FROM unnest(
        COALESCE(_class_ids, ARRAY[]::SMALLINT[]),
        COALESCE(_labels, ARRAY[]::TEXT[]),
        COALESCE(_download_priorities, ARRAY[]::SMALLINT[]),
        COALESCE(_upload_priorities, ARRAY[]::SMALLINT[]),
        COALESCE(_connection_limit_factors, ARRAY[]::SMALLINT[]),
        COALESCE(_ignore_unchoke_slots, ARRAY[]::BOOLEAN[])
    ) AS t(
        class_id,
        label,
        download_priority,
        upload_priority,
        connection_limit_factor,
        ignore_unchoke_slots
    )
    WHERE class_id BETWEEN 0 AND 31
      AND download_priority BETWEEN 1 AND 255
      AND upload_priority BETWEEN 1 AND 255
      AND connection_limit_factor >= 1;

    INSERT INTO public.engine_peer_class_defaults (profile_id, class_id)
    SELECT _profile_id, class_id
    FROM unnest(COALESCE(_default_class_ids, ARRAY[]::SMALLINT[])) AS class_id
    WHERE EXISTS (
        SELECT 1
        FROM public.engine_peer_classes epc
        WHERE epc.profile_id = _profile_id
          AND epc.class_id = class_id
    )
    ON CONFLICT (profile_id, class_id) DO NOTHING;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_engine_profile(
    _id UUID,
    _implementation TEXT,
    _listen_port INTEGER,
    _dht BOOLEAN,
    _encryption TEXT,
    _max_active INTEGER,
    _max_download_bps BIGINT,
    _max_upload_bps BIGINT,
    _seed_ratio_limit DOUBLE PRECISION,
    _seed_time_limit BIGINT,
    _sequential_default BOOLEAN,
    _auto_managed BOOLEAN,
    _auto_manage_prefer_seeds BOOLEAN,
    _dont_count_slow_torrents BOOLEAN,
    _super_seeding BOOLEAN,
    _choking_algorithm TEXT,
    _seed_choking_algorithm TEXT,
    _strict_super_seeding BOOLEAN,
    _optimistic_unchoke_slots INTEGER,
    _max_queued_disk_bytes BIGINT,
    _resume_dir TEXT,
    _download_root TEXT,
    _storage_mode TEXT,
    _use_partfile BOOLEAN,
    _cache_size INTEGER,
    _cache_expiry INTEGER,
    _coalesce_reads BOOLEAN,
    _coalesce_writes BOOLEAN,
    _use_disk_cache_pool BOOLEAN,
    _disk_read_mode TEXT,
    _disk_write_mode TEXT,
    _verify_piece_hashes BOOLEAN,
    _lsd BOOLEAN,
    _upnp BOOLEAN,
    _natpmp BOOLEAN,
    _pex BOOLEAN,
    _ipv6_mode TEXT,
    _anonymous_mode BOOLEAN,
    _force_proxy BOOLEAN,
    _prefer_rc4 BOOLEAN,
    _allow_multiple_connections_per_ip BOOLEAN,
    _enable_outgoing_utp BOOLEAN,
    _enable_incoming_utp BOOLEAN,
    _outgoing_port_min INTEGER,
    _outgoing_port_max INTEGER,
    _peer_dscp INTEGER,
    _connections_limit INTEGER,
    _connections_limit_per_torrent INTEGER,
    _unchoke_slots INTEGER,
    _half_open_limit INTEGER,
    _stats_interval_ms INTEGER
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.engine_profile
    SET implementation = _implementation,
        listen_port = _listen_port,
        dht = _dht,
        encryption = _encryption,
        max_active = _max_active,
        max_download_bps = _max_download_bps,
        max_upload_bps = _max_upload_bps,
        seed_ratio_limit = _seed_ratio_limit,
        seed_time_limit = _seed_time_limit,
        sequential_default = _sequential_default,
        auto_managed = _auto_managed,
        auto_manage_prefer_seeds = _auto_manage_prefer_seeds,
        dont_count_slow_torrents = _dont_count_slow_torrents,
        super_seeding = _super_seeding,
        choking_algorithm = _choking_algorithm,
        seed_choking_algorithm = _seed_choking_algorithm,
        strict_super_seeding = _strict_super_seeding,
        optimistic_unchoke_slots = _optimistic_unchoke_slots,
        max_queued_disk_bytes = _max_queued_disk_bytes,
        resume_dir = _resume_dir,
        download_root = _download_root,
        storage_mode = _storage_mode,
        use_partfile = _use_partfile,
        cache_size = _cache_size,
        cache_expiry = _cache_expiry,
        coalesce_reads = _coalesce_reads,
        coalesce_writes = _coalesce_writes,
        use_disk_cache_pool = _use_disk_cache_pool,
        disk_read_mode = _disk_read_mode,
        disk_write_mode = _disk_write_mode,
        verify_piece_hashes = _verify_piece_hashes,
        enable_lsd = _lsd,
        enable_upnp = _upnp,
        enable_natpmp = _natpmp,
        enable_pex = _pex,
        ipv6_mode = _ipv6_mode,
        anonymous_mode = _anonymous_mode,
        force_proxy = _force_proxy,
        prefer_rc4 = _prefer_rc4,
        allow_multiple_connections_per_ip = _allow_multiple_connections_per_ip,
        enable_outgoing_utp = _enable_outgoing_utp,
        enable_incoming_utp = _enable_incoming_utp,
        outgoing_port_min = _outgoing_port_min,
        outgoing_port_max = _outgoing_port_max,
        peer_dscp = _peer_dscp,
        connections_limit = _connections_limit,
        connections_limit_per_torrent = _connections_limit_per_torrent,
        unchoke_slots = _unchoke_slots,
        half_open_limit = _half_open_limit,
        stats_interval_ms = _stats_interval_ms
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

-- Filesystem policy helpers.
CREATE OR REPLACE FUNCTION revaer_config.fetch_fs_policy_row(_id UUID)
RETURNS TABLE (
    id UUID,
    library_root TEXT,
    extract BOOLEAN,
    par2 TEXT,
    flatten BOOLEAN,
    move_mode TEXT,
    chmod_file TEXT,
    chmod_dir TEXT,
    owner TEXT,
    "group" TEXT,
    umask TEXT,
    cleanup_keep TEXT[],
    cleanup_drop TEXT[],
    allow_paths TEXT[]
) AS
$$
BEGIN
    RETURN QUERY
    SELECT fp.id,
           fp.library_root,
           fp.extract,
           fp.par2,
           fp.flatten,
           fp.move_mode,
           fp.chmod_file,
           fp.chmod_dir,
           fp.owner,
           fp."group",
           fp.umask,
           COALESCE(
               (
                   SELECT array_agg(value ORDER BY ord)
                   FROM public.fs_policy_list_values
                   WHERE policy_id = fp.id AND kind = 'cleanup_keep'
               ),
               ARRAY[]::TEXT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(value ORDER BY ord)
                   FROM public.fs_policy_list_values
                   WHERE policy_id = fp.id AND kind = 'cleanup_drop'
               ),
               ARRAY[]::TEXT[]
           ),
           COALESCE(
               (
                   SELECT array_agg(value ORDER BY ord)
                   FROM public.fs_policy_list_values
                   WHERE policy_id = fp.id AND kind = 'allow_paths'
               ),
               ARRAY[]::TEXT[]
           )
    FROM public.fs_policy AS fp
    WHERE fp.id = _id;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.set_fs_list(
    _policy_id UUID,
    _kind TEXT,
    _values TEXT[]
) RETURNS VOID AS
$$
BEGIN
    IF _kind NOT IN ('cleanup_keep', 'cleanup_drop', 'allow_paths') THEN
        RAISE EXCEPTION 'fs policy list kind % is not supported', _kind;
    END IF;

    DELETE FROM public.fs_policy_list_values
    WHERE policy_id = _policy_id AND kind = _kind;

    INSERT INTO public.fs_policy_list_values (policy_id, kind, ord, value)
    SELECT _policy_id,
           _kind,
           ord,
           btrim(value)
    FROM unnest(COALESCE(_values, ARRAY[]::TEXT[])) WITH ORDINALITY AS t(value, ord)
    WHERE btrim(value) <> '';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_fs_array_field(
    _id UUID,
    _column TEXT,
    _values TEXT[]
) RETURNS VOID AS
$$
BEGIN
    PERFORM revaer_config.set_fs_list(_id, _column, _values);
END;
$$ LANGUAGE plpgsql;

-- API key helpers.
CREATE OR REPLACE FUNCTION revaer_config.fetch_api_keys()
RETURNS TABLE (
    key_id TEXT,
    label TEXT,
    enabled BOOLEAN,
    rate_limit_burst INTEGER,
    rate_limit_per_seconds BIGINT
) AS
$$
BEGIN
    RETURN QUERY
    SELECT key_id,
           label,
           enabled,
           rate_limit_burst,
           rate_limit_per_seconds
    FROM public.auth_api_keys
    ORDER BY created_at;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.update_api_key_rate_limit(
    _key_id TEXT,
    _burst INTEGER,
    _per_seconds BIGINT
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.auth_api_keys
    SET rate_limit_burst = _burst,
        rate_limit_per_seconds = _per_seconds
    WHERE key_id = _key_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.insert_api_key(
    _key_id TEXT,
    _hash TEXT,
    _label TEXT,
    _enabled BOOLEAN,
    _burst INTEGER,
    _per_seconds BIGINT
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO public.auth_api_keys AS ak (
        key_id,
        hash,
        label,
        enabled,
        rate_limit_burst,
        rate_limit_per_seconds
    )
    VALUES (
        _key_id,
        _hash,
        _label,
        _enabled,
        _burst,
        _per_seconds
    )
    ON CONFLICT (key_id) DO UPDATE
    SET hash = EXCLUDED.hash,
        label = EXCLUDED.label,
        enabled = EXCLUDED.enabled,
        rate_limit_burst = EXCLUDED.rate_limit_burst,
        rate_limit_per_seconds = EXCLUDED.rate_limit_per_seconds;
END;
$$ LANGUAGE plpgsql;

-- Factory reset default data (no JSON).
DROP FUNCTION IF EXISTS revaer_config.factory_reset();

CREATE OR REPLACE FUNCTION revaer_config.factory_reset()
RETURNS VOID AS
$$
DECLARE
    rec RECORD;
BEGIN
    FOR rec IN
        SELECT schemaname, tablename
        FROM pg_tables
        WHERE schemaname IN ('public', 'revaer_runtime')
          AND tablename <> '_sqlx_migrations'
    LOOP
        EXECUTE format(
            'TRUNCATE TABLE %I.%I RESTART IDENTITY CASCADE',
            rec.schemaname,
            rec.tablename
        );
    END LOOP;

    INSERT INTO public.settings_revision (id, revision)
    VALUES (1, 0)
    ON CONFLICT (id) DO UPDATE
    SET revision = EXCLUDED.revision,
        updated_at = now();

    INSERT INTO public.app_profile (id, mode, instance_name)
    VALUES (
        '00000000-0000-0000-0000-000000000001',
        'setup',
        'revaer'
    );

    INSERT INTO public.engine_profile (id, implementation, resume_dir, download_root)
    VALUES (
        '00000000-0000-0000-0000-000000000002',
        'libtorrent',
        '/var/lib/revaer/state',
        '/data/staging'
    );

    INSERT INTO public.fs_policy (id, library_root)
    VALUES (
        '00000000-0000-0000-0000-000000000003',
        '/data/library'
    );

    PERFORM revaer_config.update_app_telemetry(
        '00000000-0000-0000-0000-000000000001',
        NULL,
        NULL,
        NULL,
        NULL,
        NULL
    );
    PERFORM revaer_config.update_app_immutable_keys(
        '00000000-0000-0000-0000-000000000001',
        ARRAY[]::TEXT[]
    );
    PERFORM revaer_config.replace_app_label_policies(
        '00000000-0000-0000-0000-000000000001',
        ARRAY[]::TEXT[],
        ARRAY[]::TEXT[],
        ARRAY[]::TEXT[],
        ARRAY[]::BIGINT[],
        ARRAY[]::BIGINT[],
        ARRAY[]::INTEGER[],
        ARRAY[]::BOOLEAN[],
        ARRAY[]::DOUBLE PRECISION[],
        ARRAY[]::BIGINT[],
        ARRAY[]::DOUBLE PRECISION[],
        ARRAY[]::BIGINT[],
        ARRAY[]::BOOLEAN[]
    );

    PERFORM revaer_config.set_engine_list_values(
        '00000000-0000-0000-0000-000000000002',
        'listen_interfaces',
        ARRAY[]::TEXT[]
    );
    PERFORM revaer_config.set_engine_list_values(
        '00000000-0000-0000-0000-000000000002',
        'dht_bootstrap_nodes',
        ARRAY[]::TEXT[]
    );
    PERFORM revaer_config.set_engine_list_values(
        '00000000-0000-0000-0000-000000000002',
        'dht_router_nodes',
        ARRAY[]::TEXT[]
    );
    PERFORM revaer_config.set_engine_ip_filter(
        '00000000-0000-0000-0000-000000000002',
        NULL,
        NULL,
        NULL,
        NULL,
        ARRAY[]::TEXT[]
    );
    PERFORM revaer_config.set_engine_alt_speed(
        '00000000-0000-0000-0000-000000000002',
        NULL,
        NULL,
        NULL,
        NULL,
        ARRAY[]::TEXT[]
    );
    PERFORM revaer_config.set_tracker_config(
        '00000000-0000-0000-0000-000000000002',
        NULL,
        NULL,
        NULL,
        NULL,
        FALSE,
        FALSE,
        NULL,
        NULL,
        NULL,
        NULL,
        NULL,
        FALSE,
        ARRAY[]::TEXT[],
        ARRAY[]::TEXT[]
    );
    PERFORM revaer_config.set_peer_classes(
        '00000000-0000-0000-0000-000000000002',
        ARRAY[]::SMALLINT[],
        ARRAY[]::TEXT[],
        ARRAY[]::SMALLINT[],
        ARRAY[]::SMALLINT[],
        ARRAY[]::SMALLINT[],
        ARRAY[]::BOOLEAN[],
        ARRAY[]::SMALLINT[]
    );

    PERFORM revaer_config.set_fs_list(
        '00000000-0000-0000-0000-000000000003',
        'cleanup_keep',
        ARRAY[]::TEXT[]
    );
    PERFORM revaer_config.set_fs_list(
        '00000000-0000-0000-0000-000000000003',
        'cleanup_drop',
        ARRAY[]::TEXT[]
    );
    PERFORM revaer_config.set_fs_list(
        '00000000-0000-0000-0000-000000000003',
        'allow_paths',
        ARRAY['/data/staging', '/data/library']::TEXT[]
    );
END;
$$ LANGUAGE plpgsql;

-- Runtime procedures (no JSONB).
CREATE OR REPLACE FUNCTION revaer_runtime.upsert_torrent(
    _torrent_id UUID,
    _name TEXT,
    _state TEXT,
    _state_message TEXT,
    _progress_bytes_downloaded BIGINT,
    _progress_bytes_total BIGINT,
    _progress_eta_seconds BIGINT,
    _download_bps BIGINT,
    _upload_bps BIGINT,
    _ratio DOUBLE PRECISION,
    _sequential BOOLEAN,
    _library_path TEXT,
    _download_dir TEXT,
    _comment TEXT,
    _source TEXT,
    _private BOOLEAN,
    _file_indexes INTEGER[],
    _file_paths TEXT[],
    _file_sizes BIGINT[],
    _file_bytes_completed BIGINT[],
    _file_priorities TEXT[],
    _file_selected BOOLEAN[],
    _added_at TIMESTAMPTZ,
    _completed_at TIMESTAMPTZ,
    _updated_at TIMESTAMPTZ
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO revaer_runtime.torrents (
        torrent_id,
        name,
        state,
        state_message,
        progress_bytes_downloaded,
        progress_bytes_total,
        progress_eta_seconds,
        download_bps,
        upload_bps,
        ratio,
        sequential,
        library_path,
        download_dir,
        comment,
        source,
        private,
        added_at,
        completed_at,
        updated_at
    )
    VALUES (
        _torrent_id,
        _name,
        _state::revaer_runtime.torrent_state,
        _state_message,
        _progress_bytes_downloaded,
        _progress_bytes_total,
        _progress_eta_seconds,
        _download_bps,
        _upload_bps,
        _ratio,
        _sequential,
        _library_path,
        _download_dir,
        NULLIF(_comment, ''),
        NULLIF(_source, ''),
        _private,
        _added_at,
        _completed_at,
        _updated_at
    )
    ON CONFLICT (torrent_id) DO UPDATE
    SET name = EXCLUDED.name,
        state = EXCLUDED.state,
        state_message = EXCLUDED.state_message,
        progress_bytes_downloaded = EXCLUDED.progress_bytes_downloaded,
        progress_bytes_total = EXCLUDED.progress_bytes_total,
        progress_eta_seconds = EXCLUDED.progress_eta_seconds,
        download_bps = EXCLUDED.download_bps,
        upload_bps = EXCLUDED.upload_bps,
        ratio = EXCLUDED.ratio,
        sequential = EXCLUDED.sequential,
        library_path = EXCLUDED.library_path,
        download_dir = EXCLUDED.download_dir,
        comment = EXCLUDED.comment,
        source = EXCLUDED.source,
        private = EXCLUDED.private,
        added_at = EXCLUDED.added_at,
        completed_at = EXCLUDED.completed_at,
        updated_at = EXCLUDED.updated_at;

    DELETE FROM revaer_runtime.torrent_files
    WHERE torrent_id = _torrent_id;

    INSERT INTO revaer_runtime.torrent_files (
        torrent_id,
        file_index,
        path,
        size_bytes,
        bytes_completed,
        priority,
        selected
    )
    SELECT _torrent_id,
           file_index,
           path,
           size_bytes,
           bytes_completed,
           priority,
           selected
    FROM unnest(
        COALESCE(_file_indexes, ARRAY[]::INTEGER[]),
        COALESCE(_file_paths, ARRAY[]::TEXT[]),
        COALESCE(_file_sizes, ARRAY[]::BIGINT[]),
        COALESCE(_file_bytes_completed, ARRAY[]::BIGINT[]),
        COALESCE(_file_priorities, ARRAY[]::TEXT[]),
        COALESCE(_file_selected, ARRAY[]::BOOLEAN[])
    ) AS t(
        file_index,
        path,
        size_bytes,
        bytes_completed,
        priority,
        selected
    );
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_runtime.list_torrents()
RETURNS TABLE (
    torrent_id UUID,
    name TEXT,
    state TEXT,
    state_message TEXT,
    progress_bytes_downloaded BIGINT,
    progress_bytes_total BIGINT,
    progress_eta_seconds BIGINT,
    download_bps BIGINT,
    upload_bps BIGINT,
    ratio DOUBLE PRECISION,
    sequential BOOLEAN,
    library_path TEXT,
    download_dir TEXT,
    comment TEXT,
    source TEXT,
    private BOOLEAN,
    added_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
) AS
$$
BEGIN
    RETURN QUERY
    SELECT t.torrent_id,
           t.name,
           t.state::TEXT,
           t.state_message,
           t.progress_bytes_downloaded,
           t.progress_bytes_total,
           t.progress_eta_seconds,
           t.download_bps,
           t.upload_bps,
           t.ratio,
           t.sequential,
           t.library_path,
           t.download_dir,
           t.comment,
           t.source,
           t.private,
           t.added_at,
           t.completed_at,
           t.updated_at
    FROM revaer_runtime.torrents AS t;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_runtime.list_torrent_files(_torrent_id UUID)
RETURNS TABLE (
    file_index INTEGER,
    path TEXT,
    size_bytes BIGINT,
    bytes_completed BIGINT,
    priority TEXT,
    selected BOOLEAN
) AS
$$
BEGIN
    RETURN QUERY
    SELECT tf.file_index,
           tf.path,
           tf.size_bytes,
           tf.bytes_completed,
           tf.priority,
           tf.selected
    FROM revaer_runtime.torrent_files AS tf
    WHERE tf.torrent_id = _torrent_id
    ORDER BY tf.file_index;
END;
$$ LANGUAGE plpgsql STABLE;

-- END 0005_no_jsonb.sql
