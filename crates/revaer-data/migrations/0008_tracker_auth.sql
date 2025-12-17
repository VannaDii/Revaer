-- Add tracker authentication/cookie support for trackers.

ALTER TABLE public.engine_tracker_config
    ADD COLUMN IF NOT EXISTS auth_username_secret TEXT,
    ADD COLUMN IF NOT EXISTS auth_password_secret TEXT,
    ADD COLUMN IF NOT EXISTS auth_cookie_secret TEXT;

DROP FUNCTION IF EXISTS revaer_config.normalize_tracker_proxy(JSONB);
DROP FUNCTION IF EXISTS revaer_config.normalize_tracker_config(JSONB);
DROP FUNCTION IF EXISTS revaer_config.persist_tracker_config(UUID, JSONB);
DROP FUNCTION IF EXISTS revaer_config.render_tracker_config(UUID);
DROP FUNCTION IF EXISTS revaer_config.fetch_engine_profile_row(UUID);
DROP FUNCTION IF EXISTS revaer_config.fetch_engine_profile_json(UUID);
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
    JSONB
);

CREATE OR REPLACE FUNCTION revaer_config.normalize_tracker_proxy(_proxy JSONB)
RETURNS JSONB AS
$$
DECLARE
    host TEXT;
    port INTEGER;
    kind TEXT;
    username_secret TEXT;
    password_secret TEXT;
    proxy_peers BOOLEAN;
BEGIN
    IF jsonb_typeof(_proxy) IS DISTINCT FROM 'object' THEN
        RAISE EXCEPTION 'engine_profile.tracker.proxy must be an object';
    END IF;
    host := NULLIF(trim(both FROM _proxy->>'host'), '');
    port := (_proxy->>'port')::INTEGER;
    kind := NULLIF(trim(both FROM _proxy->>'kind'), '');
    proxy_peers := FALSE;

    IF host IS NULL OR port IS NULL OR kind IS NULL THEN
        RAISE EXCEPTION 'engine_profile.tracker.proxy requires host, port, and kind';
    END IF;
    IF port < 1 OR port > 65535 THEN
        RAISE EXCEPTION 'engine_profile.tracker.proxy.port must be between 1 and 65535';
    END IF;

    IF kind NOT IN ('http', 'https', 'socks5') THEN
        RAISE EXCEPTION 'engine_profile.tracker.proxy.kind % is not supported', kind;
    END IF;

    IF _proxy ? 'proxy_peers' THEN
        proxy_peers := COALESCE((_proxy->>'proxy_peers')::BOOLEAN, FALSE);
    END IF;

    username_secret := NULLIF(trim(both FROM _proxy->>'username_secret'), '');
    password_secret := NULLIF(trim(both FROM _proxy->>'password_secret'), '');

    RETURN jsonb_build_object(
        'host', host,
        'port', port,
        'kind', kind,
        'proxy_peers', proxy_peers
    )
        || CASE WHEN username_secret IS NOT NULL THEN jsonb_build_object('username_secret', username_secret) ELSE '{}'::jsonb END
        || CASE WHEN password_secret IS NOT NULL THEN jsonb_build_object('password_secret', password_secret) ELSE '{}'::jsonb END;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.normalize_tracker_auth(_auth JSONB)
RETURNS JSONB AS
$$
DECLARE
    username_secret TEXT;
    password_secret TEXT;
    cookie_secret TEXT;
BEGIN
    IF jsonb_typeof(_auth) IS DISTINCT FROM 'object' THEN
        RAISE EXCEPTION 'engine_profile.tracker.auth must be an object';
    END IF;

    username_secret := NULLIF(trim(both FROM _auth->>'username_secret'), '');
    password_secret := NULLIF(trim(both FROM _auth->>'password_secret'), '');
    cookie_secret := NULLIF(trim(both FROM _auth->>'cookie_secret'), '');

    IF username_secret IS NULL
       AND password_secret IS NULL
       AND cookie_secret IS NULL THEN
        RAISE EXCEPTION 'engine_profile.tracker.auth requires at least one secret reference';
    END IF;

    RETURN jsonb_strip_nulls(
        jsonb_build_object(
            'username_secret', username_secret,
            'password_secret', password_secret,
            'cookie_secret', cookie_secret
        )
    );
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.normalize_tracker_config(_tracker JSONB)
RETURNS JSONB AS
$$
DECLARE
    cfg JSONB := COALESCE(_tracker, '{}'::jsonb);
    user_agent TEXT;
    announce_ip TEXT;
    listen_interface TEXT;
    timeout_ms INTEGER;
    replace_trackers BOOLEAN := FALSE;
    announce_all BOOLEAN := FALSE;
    result JSONB := '{}'::jsonb;
BEGIN
    IF jsonb_typeof(cfg) IS NOT NULL AND jsonb_typeof(cfg) IS DISTINCT FROM 'object' THEN
        RAISE EXCEPTION 'engine_profile.tracker must be an object when provided';
    END IF;

    IF cfg ? 'replace' THEN
        replace_trackers := COALESCE((cfg->>'replace')::BOOLEAN, FALSE);
    END IF;
    IF cfg ? 'announce_to_all' THEN
        announce_all := COALESCE((cfg->>'announce_to_all')::BOOLEAN, FALSE);
    END IF;

    result := jsonb_build_object(
        'default', revaer_config.normalize_tracker_list(COALESCE(cfg->'default', '[]'::jsonb)),
        'extra', revaer_config.normalize_tracker_list(COALESCE(cfg->'extra', '[]'::jsonb)),
        'replace', replace_trackers,
        'announce_to_all', announce_all
    );

    user_agent := NULLIF(trim(both FROM cfg->>'user_agent'), '');
    IF user_agent IS NOT NULL THEN
        IF length(user_agent) > 255 THEN
            RAISE EXCEPTION 'engine_profile.tracker.user_agent exceeds 255 characters';
        END IF;
        result := result || jsonb_build_object('user_agent', user_agent);
    END IF;

    announce_ip := NULLIF(trim(both FROM cfg->>'announce_ip'), '');
    IF announce_ip IS NOT NULL THEN
        result := result || jsonb_build_object('announce_ip', announce_ip);
    END IF;

    listen_interface := NULLIF(trim(both FROM cfg->>'listen_interface'), '');
    IF listen_interface IS NOT NULL THEN
        result := result || jsonb_build_object('listen_interface', listen_interface);
    END IF;

    IF cfg ? 'request_timeout_ms' THEN
        timeout_ms := (cfg->>'request_timeout_ms')::INTEGER;
        IF timeout_ms IS NULL THEN
            RAISE EXCEPTION 'engine_profile.tracker.request_timeout_ms must be an integer';
        END IF;
        IF timeout_ms < 0 OR timeout_ms > 900000 THEN
            RAISE EXCEPTION 'engine_profile.tracker.request_timeout_ms must be between 0 and 900000';
        END IF;
        result := result || jsonb_build_object('request_timeout_ms', timeout_ms);
    END IF;

    IF cfg ? 'proxy' AND cfg->'proxy' IS NOT NULL THEN
        result := result || jsonb_build_object(
            'proxy',
            revaer_config.normalize_tracker_proxy(cfg->'proxy')
        );
    END IF;

    IF cfg ? 'auth' AND cfg->'auth' IS NOT NULL THEN
        result := result || jsonb_build_object(
            'auth',
            revaer_config.normalize_tracker_auth(cfg->'auth')
        );
    END IF;

    RETURN result;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.persist_tracker_config(
    _profile_id UUID,
    _tracker JSONB
) RETURNS VOID AS
$$
DECLARE
    normalized JSONB;
    default_urls JSONB;
    extra_urls JSONB;
    replace_flag BOOLEAN := FALSE;
    announce_all BOOLEAN := FALSE;
    user_agent TEXT;
    announce_ip TEXT;
    listen_interface TEXT;
    request_timeout_ms INTEGER;
    proxy JSONB;
    proxy_host TEXT;
    proxy_port INTEGER;
    proxy_kind TEXT;
    proxy_username_secret TEXT;
    proxy_password_secret TEXT;
    proxy_peers BOOLEAN := FALSE;
    auth JSONB;
    auth_username_secret TEXT;
    auth_password_secret TEXT;
    auth_cookie_secret TEXT;
BEGIN
    normalized := revaer_config.normalize_tracker_config(_tracker);
    default_urls := COALESCE(normalized->'default', '[]'::jsonb);
    extra_urls := COALESCE(normalized->'extra', '[]'::jsonb);
    replace_flag := COALESCE((normalized->>'replace')::BOOLEAN, FALSE);
    announce_all := COALESCE((normalized->>'announce_to_all')::BOOLEAN, FALSE);
    user_agent := NULLIF(normalized->>'user_agent', '');
    announce_ip := NULLIF(normalized->>'announce_ip', '');
    listen_interface := NULLIF(normalized->>'listen_interface', '');
    request_timeout_ms := (normalized->>'request_timeout_ms')::INTEGER;

    IF normalized ? 'proxy' THEN
        proxy := normalized->'proxy';
        proxy_host := NULLIF(proxy->>'host', '');
        proxy_port := (proxy->>'port')::INTEGER;
        proxy_kind := NULLIF(proxy->>'kind', '');
        proxy_username_secret := NULLIF(proxy->>'username_secret', '');
        proxy_password_secret := NULLIF(proxy->>'password_secret', '');
        proxy_peers := COALESCE((proxy->>'proxy_peers')::BOOLEAN, FALSE);
    ELSE
        proxy_host := NULL;
        proxy_port := NULL;
        proxy_kind := NULL;
        proxy_username_secret := NULL;
        proxy_password_secret := NULL;
        proxy_peers := FALSE;
    END IF;

    IF normalized ? 'auth' THEN
        auth := normalized->'auth';
        auth_username_secret := NULLIF(auth->>'username_secret', '');
        auth_password_secret := NULLIF(auth->>'password_secret', '');
        auth_cookie_secret := NULLIF(auth->>'cookie_secret', '');
    ELSE
        auth_username_secret := NULL;
        auth_password_secret := NULL;
        auth_cookie_secret := NULL;
    END IF;

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
        auth_username_secret,
        auth_password_secret,
        auth_cookie_secret
    )
    VALUES (
        _profile_id,
        user_agent,
        announce_ip,
        listen_interface,
        request_timeout_ms,
        announce_all,
        replace_flag,
        proxy_host,
        proxy_port,
        proxy_kind,
        proxy_username_secret,
        proxy_password_secret,
        proxy_peers,
        auth_username_secret,
        auth_password_secret,
        auth_cookie_secret
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
        auth_username_secret = EXCLUDED.auth_username_secret,
        auth_password_secret = EXCLUDED.auth_password_secret,
        auth_cookie_secret = EXCLUDED.auth_cookie_secret,
        updated_at = now();

    DELETE FROM public.engine_tracker_endpoints WHERE profile_id = _profile_id;

    INSERT INTO public.engine_tracker_endpoints (profile_id, kind, url, ord)
    SELECT _profile_id, 'default', elem, ord::INTEGER
    FROM jsonb_array_elements_text(default_urls) WITH ORDINALITY AS t(elem, ord);

    INSERT INTO public.engine_tracker_endpoints (profile_id, kind, url, ord)
    SELECT _profile_id, 'extra', elem, ord::INTEGER
    FROM jsonb_array_elements_text(extra_urls) WITH ORDINALITY AS t(elem, ord);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.render_tracker_config(_profile_id UUID)
RETURNS JSONB AS
$$
DECLARE
    cfg RECORD;
    default_urls JSONB;
    extra_urls JSONB;
    payload JSONB := '{}'::jsonb;
BEGIN
    SELECT *
    INTO cfg
    FROM public.engine_tracker_config
    WHERE profile_id = _profile_id;

    SELECT COALESCE(jsonb_agg(url ORDER BY ord, id), '[]'::jsonb)
    INTO default_urls
    FROM public.engine_tracker_endpoints
    WHERE profile_id = _profile_id
      AND kind = 'default';

    SELECT COALESCE(jsonb_agg(url ORDER BY ord, id), '[]'::jsonb)
    INTO extra_urls
    FROM public.engine_tracker_endpoints
    WHERE profile_id = _profile_id
      AND kind = 'extra';

    IF cfg IS NULL
       AND default_urls = '[]'::jsonb
       AND extra_urls = '[]'::jsonb THEN
        RETURN '{}'::jsonb;
    END IF;

    payload := jsonb_build_object(
        'default', default_urls,
        'extra', extra_urls,
        'replace', COALESCE(cfg.replace_trackers, FALSE),
        'announce_to_all', COALESCE(cfg.announce_to_all, FALSE)
    );

    IF cfg.user_agent IS NOT NULL THEN
        payload := payload || jsonb_build_object('user_agent', cfg.user_agent);
    END IF;
    IF cfg.announce_ip IS NOT NULL THEN
        payload := payload || jsonb_build_object('announce_ip', cfg.announce_ip);
    END IF;
    IF cfg.listen_interface IS NOT NULL THEN
        payload := payload || jsonb_build_object('listen_interface', cfg.listen_interface);
    END IF;
    IF cfg.request_timeout_ms IS NOT NULL THEN
        payload := payload || jsonb_build_object('request_timeout_ms', cfg.request_timeout_ms);
    END IF;

    IF cfg.proxy_host IS NOT NULL THEN
        payload := payload
            || jsonb_build_object(
                'proxy',
                jsonb_strip_nulls(
                    jsonb_build_object(
                        'host', cfg.proxy_host,
                        'port', cfg.proxy_port,
                        'kind', cfg.proxy_kind,
                        'proxy_peers', COALESCE(cfg.proxy_peers, FALSE),
                        'username_secret', cfg.proxy_username_secret,
                        'password_secret', cfg.proxy_password_secret
                    )
                )
            );
    END IF;

    IF cfg.auth_username_secret IS NOT NULL
       OR cfg.auth_password_secret IS NOT NULL
       OR cfg.auth_cookie_secret IS NOT NULL THEN
        payload := payload
            || jsonb_build_object(
                'auth',
                jsonb_strip_nulls(
                    jsonb_build_object(
                        'username_secret', cfg.auth_username_secret,
                        'password_secret', cfg.auth_password_secret,
                        'cookie_secret', cfg.auth_cookie_secret
                    )
                )
            );
    END IF;

    IF payload ? 'default'
       AND (payload->'default') = '[]'::jsonb
       AND (payload->'extra') = '[]'::jsonb
       AND COALESCE(payload->>'replace', 'false') = 'false'
       AND COALESCE(payload->>'announce_to_all', 'false') = 'false'
       AND NOT (payload ? 'user_agent')
       AND NOT (payload ? 'announce_ip')
       AND NOT (payload ? 'listen_interface')
       AND NOT (payload ? 'request_timeout_ms')
       AND NOT (payload ? 'proxy')
       AND NOT (payload ? 'auth') THEN
        RETURN '{}'::jsonb;
    END IF;

    RETURN payload;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.fetch_engine_profile_row(_id UUID)
RETURNS TABLE(
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
    tracker JSONB,
    enable_lsd BOOLEAN,
    enable_upnp BOOLEAN,
    enable_natpmp BOOLEAN,
    enable_pex BOOLEAN,
    dht_bootstrap_nodes JSONB,
    dht_router_nodes JSONB,
    ip_filter JSONB,
    listen_interfaces JSONB,
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
    alt_speed JSONB,
    stats_interval_ms INTEGER
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
           revaer_config.render_tracker_config(ep.id),
           ep.enable_lsd,
           ep.enable_upnp,
           ep.enable_natpmp,
           ep.enable_pex,
           ep.dht_bootstrap_nodes,
           ep.dht_router_nodes,
           ep.ip_filter,
           ep.listen_interfaces,
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
           ep.alt_speed,
           ep.stats_interval_ms
    FROM public.engine_profile AS ep
    WHERE ep.id = _id;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.fetch_engine_profile_json(_id UUID)
RETURNS JSONB AS
$$
DECLARE
    body JSONB;
BEGIN
    SELECT jsonb_build_object(
        'id', ep.id,
        'implementation', ep.implementation,
        'listen_port', ep.listen_port,
        'dht', ep.dht,
        'encryption', ep.encryption,
        'max_active', ep.max_active,
        'max_download_bps', ep.max_download_bps,
        'max_upload_bps', ep.max_upload_bps,
        'seed_ratio_limit', ep.seed_ratio_limit,
        'seed_time_limit', ep.seed_time_limit,
        'sequential_default', ep.sequential_default,
        'auto_managed', ep.auto_managed,
        'auto_manage_prefer_seeds', ep.auto_manage_prefer_seeds,
        'dont_count_slow_torrents', ep.dont_count_slow_torrents,
        'super_seeding', ep.super_seeding,
        'resume_dir', ep.resume_dir,
        'download_root', ep.download_root,
        'created_at', ep.created_at,
        'updated_at', ep.updated_at,
        'tracker', revaer_config.render_tracker_config(ep.id),
        'enable_lsd', ep.enable_lsd,
        'enable_upnp', ep.enable_upnp,
        'enable_natpmp', ep.enable_natpmp,
        'enable_pex', ep.enable_pex,
        'dht_bootstrap_nodes', ep.dht_bootstrap_nodes,
        'dht_router_nodes', ep.dht_router_nodes,
        'ip_filter', ep.ip_filter,
        'listen_interfaces', ep.listen_interfaces,
        'ipv6_mode', ep.ipv6_mode,
        'anonymous_mode', ep.anonymous_mode,
        'force_proxy', ep.force_proxy,
        'prefer_rc4', ep.prefer_rc4,
        'allow_multiple_connections_per_ip', ep.allow_multiple_connections_per_ip,
        'enable_outgoing_utp', ep.enable_outgoing_utp,
        'enable_incoming_utp', ep.enable_incoming_utp,
        'outgoing_port_min', ep.outgoing_port_min,
        'outgoing_port_max', ep.outgoing_port_max,
        'peer_dscp', ep.peer_dscp,
        'connections_limit', ep.connections_limit,
        'connections_limit_per_torrent', ep.connections_limit_per_torrent,
        'unchoke_slots', ep.unchoke_slots,
        'half_open_limit', ep.half_open_limit,
        'alt_speed', ep.alt_speed,
        'stats_interval_ms', ep.stats_interval_ms
    )
    INTO body
    FROM public.engine_profile AS ep
    WHERE ep.id = _id;
    RETURN body;
END;
$$ LANGUAGE plpgsql STABLE;

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
    _tracker JSONB,
    _lsd BOOLEAN,
    _upnp BOOLEAN,
    _natpmp BOOLEAN,
    _pex BOOLEAN,
    _dht_bootstrap_nodes JSONB,
    _dht_router_nodes JSONB,
    _ip_filter JSONB,
    _listen_interfaces JSONB,
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
    _alt_speed JSONB,
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
        enable_lsd = _lsd,
        enable_upnp = _upnp,
        enable_natpmp = _natpmp,
        enable_pex = _pex,
        dht_bootstrap_nodes = _dht_bootstrap_nodes,
        dht_router_nodes = _dht_router_nodes,
        ip_filter = _ip_filter,
        listen_interfaces = _listen_interfaces,
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
        alt_speed = _alt_speed,
        stats_interval_ms = _stats_interval_ms
    WHERE id = _id;

    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
END;
$$ LANGUAGE plpgsql;
