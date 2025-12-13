-- Tracker configuration validation and normalisation

CREATE OR REPLACE FUNCTION revaer_config.normalize_tracker_list(_value JSONB)
RETURNS JSONB AS
$$
DECLARE
    cleaned JSONB;
BEGIN
    IF jsonb_typeof(_value) IS DISTINCT FROM 'array' THEN
        RAISE EXCEPTION 'engine_profile.tracker trackers must be an array';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM jsonb_array_elements_text(_value) WITH ORDINALITY AS t(elem, _)
        WHERE length(trim(both FROM elem)) > 512
    ) THEN
        RAISE EXCEPTION 'engine_profile.tracker entries must be shorter than 512 characters';
    END IF;

    cleaned := (
        SELECT COALESCE(
                   jsonb_agg(to_jsonb(val) ORDER BY ord),
                   '[]'::jsonb
               )
        FROM (
            SELECT DISTINCT ON (val) val, ord
            FROM (
                SELECT NULLIF(trim(both FROM elem), '') AS val, ord
                FROM jsonb_array_elements_text(_value) WITH ORDINALITY AS t(elem, ord)
            ) AS cleaned
            WHERE val IS NOT NULL
            ORDER BY val, ord
        ) AS deduped
    );

    RETURN cleaned;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.normalize_tracker_proxy(_proxy JSONB)
RETURNS JSONB AS
$$
DECLARE
    host TEXT;
    port INTEGER;
    username_secret TEXT;
    password_secret TEXT;
    proxy_peers BOOLEAN := FALSE;
    kind TEXT := 'http';
BEGIN
    IF jsonb_typeof(_proxy) IS DISTINCT FROM 'object' THEN
        RAISE EXCEPTION 'engine_profile.tracker.proxy must be an object';
    END IF;

    host := NULLIF(trim(both FROM _proxy->>'host'), '');
    IF host IS NULL THEN
        RAISE EXCEPTION 'engine_profile.tracker.proxy.host is required when proxy is set';
    END IF;

    port := (_proxy->>'port')::INTEGER;
    IF port IS NULL THEN
        RAISE EXCEPTION 'engine_profile.tracker.proxy.port is required and must be an integer';
    END IF;
    IF port < 1 OR port > 65535 THEN
        RAISE EXCEPTION 'engine_profile.tracker.proxy.port must be between 1 and 65535';
    END IF;

    IF _proxy ? 'kind' THEN
        kind := _proxy->>'kind';
        IF kind NOT IN ('http', 'https', 'socks5') THEN
            RAISE EXCEPTION 'engine_profile.tracker.proxy.kind % is not supported', kind;
        END IF;
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

    RETURN result;
END;
$$ LANGUAGE plpgsql;

-- Apply normalisation in the canonical updater.
CREATE OR REPLACE FUNCTION revaer_config.update_engine_profile(
    _id UUID,
    _implementation TEXT,
    _listen_port INTEGER,
    _dht BOOLEAN,
    _encryption TEXT,
    _max_active INTEGER,
    _max_download_bps BIGINT,
    _max_upload_bps BIGINT,
    _sequential_default BOOLEAN,
    _resume_dir TEXT,
    _download_root TEXT,
    _tracker JSONB
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
        sequential_default = _sequential_default,
        resume_dir = _resume_dir,
        download_root = _download_root,
        tracker = revaer_config.normalize_tracker_config(_tracker)
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;
