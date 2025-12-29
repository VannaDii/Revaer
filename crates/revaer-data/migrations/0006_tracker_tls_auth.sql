-- BEGIN 0006_tracker_tls_auth.sql
-- Extend tracker configuration with TLS and auth fields.

ALTER TABLE public.engine_tracker_config
    ADD COLUMN IF NOT EXISTS ssl_cert TEXT,
    ADD COLUMN IF NOT EXISTS ssl_private_key TEXT,
    ADD COLUMN IF NOT EXISTS ssl_ca_cert TEXT,
    ADD COLUMN IF NOT EXISTS ssl_tracker_verify BOOLEAN,
    ADD COLUMN IF NOT EXISTS auth_username_secret TEXT,
    ADD COLUMN IF NOT EXISTS auth_password_secret TEXT,
    ADD COLUMN IF NOT EXISTS auth_cookie_secret TEXT;

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
    tracker_auth_username_secret TEXT,
    tracker_auth_password_secret TEXT,
    tracker_auth_cookie_secret TEXT,
    tracker_ssl_cert TEXT,
    tracker_ssl_private_key TEXT,
    tracker_ssl_ca_cert TEXT,
    tracker_ssl_verify BOOLEAN,
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
           etc.auth_username_secret,
           etc.auth_password_secret,
           etc.auth_cookie_secret,
           etc.ssl_cert,
           etc.ssl_private_key,
           etc.ssl_ca_cert,
           etc.ssl_tracker_verify,
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
    _ssl_cert TEXT,
    _ssl_private_key TEXT,
    _ssl_ca_cert TEXT,
    _ssl_tracker_verify BOOLEAN,
    _auth_username_secret TEXT,
    _auth_password_secret TEXT,
    _auth_cookie_secret TEXT,
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
        proxy_peers,
        ssl_cert,
        ssl_private_key,
        ssl_ca_cert,
        ssl_tracker_verify,
        auth_username_secret,
        auth_password_secret,
        auth_cookie_secret
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
        COALESCE(_proxy_peers, FALSE),
        NULLIF(_ssl_cert, ''),
        NULLIF(_ssl_private_key, ''),
        NULLIF(_ssl_ca_cert, ''),
        COALESCE(_ssl_tracker_verify, TRUE),
        NULLIF(_auth_username_secret, ''),
        NULLIF(_auth_password_secret, ''),
        NULLIF(_auth_cookie_secret, '')
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
        ssl_cert = EXCLUDED.ssl_cert,
        ssl_private_key = EXCLUDED.ssl_private_key,
        ssl_ca_cert = EXCLUDED.ssl_ca_cert,
        ssl_tracker_verify = EXCLUDED.ssl_tracker_verify,
        auth_username_secret = EXCLUDED.auth_username_secret,
        auth_password_secret = EXCLUDED.auth_password_secret,
        auth_cookie_secret = EXCLUDED.auth_cookie_secret,
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
        NULL,
        NULL,
        NULL,
        TRUE,
        NULL,
        NULL,
        NULL,
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

-- END 0006_tracker_tls_auth.sql
