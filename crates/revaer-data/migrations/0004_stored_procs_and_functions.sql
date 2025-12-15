-- Stored procedures, functions, and triggers.

-- Common helpers -----------------------------------------------------------

CREATE OR REPLACE FUNCTION revaer_touch_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_bump_revision()
RETURNS TRIGGER AS $$
DECLARE
    revision_setting TEXT;
    effective_revision BIGINT;
BEGIN
    BEGIN
        revision_setting := current_setting('revaer.current_revision', true);
    EXCEPTION
        WHEN others THEN
            revision_setting := NULL;
    END;

    IF revision_setting IS NULL OR revision_setting = '' THEN
        UPDATE settings_revision
        SET revision = revision + 1,
            updated_at = now()
        WHERE id = 1
        RETURNING revision INTO effective_revision;

        PERFORM set_config('revaer.current_revision', effective_revision::TEXT, true);
    ELSE
        effective_revision := revision_setting::BIGINT;
    END IF;

    PERFORM pg_notify(
        'revaer_settings_changed',
        format('%s:%s:%s', TG_TABLE_NAME, effective_revision, TG_OP)
    );

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

-- Timestamp + revision triggers -------------------------------------------

CREATE TRIGGER app_profile_touch_updated_at
BEFORE UPDATE ON app_profile
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER engine_profile_touch_updated_at
BEFORE UPDATE ON engine_profile
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER fs_policy_touch_updated_at
BEFORE UPDATE ON fs_policy
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER auth_api_keys_touch_updated_at
BEFORE UPDATE ON auth_api_keys
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER query_presets_touch_updated_at
BEFORE UPDATE ON query_presets
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER app_profile_bump_revision
AFTER INSERT OR UPDATE ON app_profile
FOR EACH ROW
EXECUTE FUNCTION revaer_bump_revision();

CREATE TRIGGER engine_profile_bump_revision
AFTER INSERT OR UPDATE ON engine_profile
FOR EACH ROW
EXECUTE FUNCTION revaer_bump_revision();

CREATE TRIGGER fs_policy_bump_revision
AFTER INSERT OR UPDATE ON fs_policy
FOR EACH ROW
EXECUTE FUNCTION revaer_bump_revision();

CREATE TRIGGER auth_api_keys_bump_revision
AFTER INSERT OR UPDATE OR DELETE ON auth_api_keys
FOR EACH ROW
EXECUTE FUNCTION revaer_bump_revision();

CREATE TRIGGER query_presets_bump_revision
AFTER INSERT OR UPDATE OR DELETE ON query_presets
FOR EACH ROW
EXECUTE FUNCTION revaer_bump_revision();

-- Config schema helpers ----------------------------------------------------

CREATE SCHEMA IF NOT EXISTS revaer_config;

CREATE OR REPLACE FUNCTION revaer_config.insert_history(
    _kind TEXT,
    _old JSONB,
    _new JSONB,
    _actor TEXT,
    _reason TEXT,
    _revision BIGINT
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO public.settings_history (kind, old, new, actor, reason, revision)
    VALUES (_kind, _old, _new, _actor, _reason, _revision);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.bump_revision(_source_table TEXT)
RETURNS BIGINT AS
$$
DECLARE
    new_revision BIGINT;
BEGIN
    UPDATE public.settings_revision
    SET revision = revision + 1,
        updated_at = now()
    WHERE id = 1
    RETURNING revision INTO new_revision;

    PERFORM pg_notify('revaer_settings_changed', format('%s:%s:UPDATE', _source_table, new_revision));
    RETURN new_revision;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.fetch_revision()
RETURNS BIGINT AS
$$
DECLARE
    current_revision BIGINT;
BEGIN
    SELECT revision INTO current_revision FROM public.settings_revision WHERE id = 1;
    RETURN current_revision;
END;
$$ LANGUAGE plpgsql STABLE;

-- Setup tokens --------------------------------------------------------------

CREATE OR REPLACE FUNCTION revaer_config.cleanup_expired_setup_tokens()
RETURNS VOID AS
$$
BEGIN
    DELETE FROM public.setup_tokens
    WHERE consumed_at IS NULL
      AND expires_at <= now();
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.invalidate_active_setup_tokens()
RETURNS VOID AS
$$
BEGIN
    UPDATE public.setup_tokens
    SET consumed_at = now()
    WHERE consumed_at IS NULL;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.insert_setup_token(
    _token_hash TEXT,
    _expires_at TIMESTAMPTZ,
    _issued_by TEXT
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO public.setup_tokens (token_hash, expires_at, issued_by)
    VALUES (_token_hash, _expires_at, _issued_by);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.consume_setup_token(_token_id UUID)
RETURNS VOID AS
$$
BEGIN
    UPDATE public.setup_tokens
    SET consumed_at = now()
    WHERE id = _token_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.fetch_active_setup_token()
RETURNS TABLE (
    id UUID,
    token_hash TEXT,
    expires_at TIMESTAMPTZ
) AS
$$
BEGIN
    RETURN QUERY
    SELECT st.id, st.token_hash, st.expires_at
    FROM public.setup_tokens AS st
    WHERE st.consumed_at IS NULL
    ORDER BY st.issued_at DESC
    LIMIT 1
    FOR UPDATE;
END;
$$ LANGUAGE plpgsql;

-- Secrets -------------------------------------------------------------------

CREATE OR REPLACE FUNCTION revaer_config.fetch_secret_by_name(_name TEXT)
RETURNS TABLE (name TEXT, ciphertext BYTEA) AS
$$
BEGIN
    RETURN QUERY
    SELECT ss.name, ss.ciphertext
    FROM public.settings_secret AS ss
    WHERE ss.name = _name;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.upsert_secret(
    _name TEXT,
    _ciphertext BYTEA,
    _actor TEXT
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO public.settings_secret (name, ciphertext, created_by, created_at)
    VALUES (_name, _ciphertext, _actor, now())
    ON CONFLICT (name)
    DO UPDATE
    SET ciphertext = EXCLUDED.ciphertext,
        created_by = EXCLUDED.created_by,
        created_at = now();
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.delete_secret(_name TEXT)
RETURNS BIGINT AS
$$
DECLARE
    removed BIGINT;
BEGIN
    DELETE FROM public.settings_secret WHERE name = _name;
    GET DIAGNOSTICS removed = ROW_COUNT;
    RETURN removed;
END;
$$ LANGUAGE plpgsql;

-- Profile fetch helpers -----------------------------------------------------

CREATE OR REPLACE FUNCTION revaer_config.fetch_app_profile_row(_id UUID)
RETURNS TABLE (
    id UUID,
    instance_name TEXT,
    mode TEXT,
    version BIGINT,
    http_port INTEGER,
    bind_addr TEXT,
    telemetry JSONB,
    features JSONB,
    immutable_keys JSONB
) AS
$$
BEGIN
    RETURN QUERY
    SELECT ap.id,
           ap.instance_name,
           ap.mode,
           ap.version,
           ap.http_port,
           ap.bind_addr::text,
           ap.telemetry,
           ap.features,
           ap.immutable_keys
    FROM public.app_profile AS ap
    WHERE ap.id = _id;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.fetch_fs_policy_row(_id UUID)
RETURNS TABLE (
    id UUID,
    library_root TEXT,
    "extract" BOOLEAN,
    par2 TEXT,
    flatten BOOLEAN,
    move_mode TEXT,
    cleanup_keep JSONB,
    cleanup_drop JSONB,
    chmod_file TEXT,
    chmod_dir TEXT,
    owner TEXT,
    "group" TEXT,
    umask TEXT,
    allow_paths JSONB
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
           fp.cleanup_keep,
           fp.cleanup_drop,
           fp.chmod_file,
           fp.chmod_dir,
           fp.owner,
           fp."group",
           fp.umask,
           fp.allow_paths
    FROM public.fs_policy AS fp
    WHERE fp.id = _id;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.fetch_app_profile_json(_id UUID)
RETURNS JSONB AS
$$
DECLARE
    body JSONB;
BEGIN
    SELECT to_jsonb(ap.*) INTO body FROM public.app_profile AS ap WHERE ap.id = _id;
    RETURN body;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.fetch_fs_policy_json(_id UUID)
RETURNS JSONB AS
$$
DECLARE
    body JSONB;
BEGIN
    SELECT to_jsonb(fp.*) INTO body FROM public.fs_policy AS fp WHERE fp.id = _id;
    RETURN body;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.fetch_api_keys_json()
RETURNS JSONB AS
$$
DECLARE
    payload JSONB;
BEGIN
    SELECT COALESCE(
        jsonb_agg(
            json_build_object(
                'key_id', key_id,
                'label', label,
                'enabled', enabled,
                'rate_limit', rate_limit
            )
            ORDER BY created_at
        ),
        '[]'::jsonb
    )
    INTO payload
    FROM public.auth_api_keys;

    RETURN payload;
END;
$$ LANGUAGE plpgsql STABLE;

-- API Keys ------------------------------------------------------------------

CREATE OR REPLACE FUNCTION revaer_config.fetch_api_key_auth(_key_id TEXT)
RETURNS TABLE (
    hash TEXT,
    enabled BOOLEAN,
    label TEXT,
    rate_limit JSONB
) AS
$$
BEGIN
    RETURN QUERY
    SELECT ak.hash, ak.enabled, ak.label, ak.rate_limit
    FROM public.auth_api_keys AS ak
    WHERE ak.key_id = _key_id;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.fetch_api_key_hash(_key_id TEXT)
RETURNS TEXT AS
$$
DECLARE
    digest TEXT;
BEGIN
    SELECT hash INTO digest FROM public.auth_api_keys WHERE key_id = _key_id;
    RETURN digest;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.insert_api_key(
    _key_id TEXT,
    _hash TEXT,
    _label TEXT,
    _enabled BOOLEAN,
    _rate_limit JSONB
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO public.auth_api_keys (key_id, hash, label, enabled, rate_limit)
    VALUES (_key_id, _hash, _label, _enabled, COALESCE(_rate_limit, '{}'::jsonb));
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.delete_api_key(_key_id TEXT)
RETURNS BIGINT AS
$$
DECLARE
    removed BIGINT;
BEGIN
    DELETE FROM public.auth_api_keys WHERE key_id = _key_id;
    GET DIAGNOSTICS removed = ROW_COUNT;
    RETURN removed;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_api_key_hash(
    _key_id TEXT,
    _hash TEXT
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.auth_api_keys
    SET hash = _hash,
        updated_at = now()
    WHERE key_id = _key_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_api_key_label(
    _key_id TEXT,
    _label TEXT
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.auth_api_keys
    SET label = _label,
        updated_at = now()
    WHERE key_id = _key_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_api_key_enabled(
    _key_id TEXT,
    _enabled BOOLEAN
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.auth_api_keys
    SET enabled = _enabled,
        updated_at = now()
    WHERE key_id = _key_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_api_key_rate_limit(
    _key_id TEXT,
    _rate_limit JSONB
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.auth_api_keys
    SET rate_limit = COALESCE(_rate_limit, '{}'::jsonb),
        updated_at = now()
    WHERE key_id = _key_id;
END;
$$ LANGUAGE plpgsql;

-- Secrets history helpers ---------------------------------------------------

CREATE OR REPLACE FUNCTION revaer_config.fetch_api_keys_revision_payload()
RETURNS JSONB AS
$$
BEGIN
    RETURN revaer_config.fetch_api_keys_json();
END;
$$ LANGUAGE plpgsql STABLE;

-- Application profile mutations --------------------------------------------

CREATE OR REPLACE FUNCTION revaer_config.update_app_instance_name(
    _id UUID,
    _instance_name TEXT
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET instance_name = _instance_name
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_app_mode(
    _id UUID,
    _mode TEXT
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET mode = _mode
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_app_http_port(
    _id UUID,
    _port INTEGER
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET http_port = _port
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_app_bind_addr(
    _id UUID,
    _bind_addr TEXT
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET bind_addr = _bind_addr::inet
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_app_telemetry(
    _id UUID,
    _telemetry JSONB
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET telemetry = COALESCE(_telemetry, '{}'::jsonb)
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_app_features(
    _id UUID,
    _features JSONB
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET features = COALESCE(_features, '{}'::jsonb)
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_app_immutable_keys(
    _id UUID,
    _immutable JSONB
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET immutable_keys = COALESCE(_immutable, '[]'::jsonb)
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.bump_app_profile_version(_id UUID)
RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET version = version + 1
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

-- Filesystem policy mutations ----------------------------------------------

CREATE OR REPLACE FUNCTION revaer_config.update_fs_string_field(
    _id UUID,
    _column TEXT,
    _value TEXT
) RETURNS VOID AS
$$
BEGIN
    IF _column NOT IN ('library_root', 'par2', 'move_mode') THEN
        RAISE EXCEPTION 'Unsupported fs_policy string column: %', _column;
    END IF;

    EXECUTE format('UPDATE public.fs_policy SET %I = $1 WHERE id = $2', _column)
    USING _value, _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_fs_boolean_field(
    _id UUID,
    _column TEXT,
    _value BOOLEAN
) RETURNS VOID AS
$$
BEGIN
    IF _column NOT IN ('extract', 'flatten') THEN
        RAISE EXCEPTION 'Unsupported fs_policy boolean column: %', _column;
    END IF;

    EXECUTE format('UPDATE public.fs_policy SET %I = $1 WHERE id = $2', _column)
    USING _value, _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_fs_array_field(
    _id UUID,
    _column TEXT,
    _value JSONB
) RETURNS VOID AS
$$
BEGIN
    IF _column NOT IN ('cleanup_keep', 'cleanup_drop', 'allow_paths') THEN
        RAISE EXCEPTION 'Unsupported fs_policy array column: %', _column;
    END IF;

    EXECUTE format('UPDATE public.fs_policy SET %I = COALESCE($1, ''[]''::jsonb) WHERE id = $2', _column)
    USING _value, _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_fs_optional_string_field(
    _id UUID,
    _column TEXT,
    _value TEXT
) RETURNS VOID AS
$$
BEGIN
    IF _column NOT IN ('chmod_file', 'chmod_dir', 'owner', 'group', 'umask') THEN
        RAISE EXCEPTION 'Unsupported fs_policy optional string column: %', _column;
    END IF;

    EXECUTE format('UPDATE public.fs_policy SET %I = $1 WHERE id = $2', _column)
    USING _value, _id;
END;
$$ LANGUAGE plpgsql;

-- Tracker normalisation -----------------------------------------------------

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
        proxy_peers
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

    IF payload ? 'default'
       AND (payload->'default') = '[]'::jsonb
       AND (payload->'extra') = '[]'::jsonb
       AND COALESCE(payload->>'replace', 'false') = 'false'
       AND COALESCE(payload->>'announce_to_all', 'false') = 'false'
       AND NOT (payload ? 'user_agent')
       AND NOT (payload ? 'announce_ip')
       AND NOT (payload ? 'listen_interface')
       AND NOT (payload ? 'request_timeout_ms')
       AND NOT (payload ? 'proxy') THEN
        RETURN '{}'::jsonb;
    END IF;

    RETURN payload;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE TRIGGER engine_tracker_config_touch_updated_at
BEFORE UPDATE ON public.engine_tracker_config
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER engine_tracker_endpoints_touch_updated_at
BEFORE UPDATE ON public.engine_tracker_endpoints
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

-- Engine profile projections -----------------------------------------------

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
    alt_speed JSONB
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
           ep.alt_speed
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
        'alt_speed', ep.alt_speed
    )
    INTO body
    FROM public.engine_profile AS ep
    WHERE ep.id = _id;
    RETURN body;
END;
$$ LANGUAGE plpgsql STABLE;

-- Engine profile mutations --------------------------------------------------

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
    _alt_speed JSONB
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
        alt_speed = _alt_speed
    WHERE id = _id;

    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
END;
$$ LANGUAGE plpgsql;

-- Runtime procedures --------------------------------------------------------

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
    _payload JSONB,
    _files JSONB,
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
        payload,
        files,
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
        COALESCE(_payload, '{}'::jsonb),
        _files,
        _added_at,
        _completed_at,
        _updated_at
    )
    ON CONFLICT (torrent_id) DO UPDATE
    SET
        name = EXCLUDED.name,
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
        payload = EXCLUDED.payload,
        files = EXCLUDED.files,
        added_at = EXCLUDED.added_at,
        completed_at = EXCLUDED.completed_at,
        updated_at = EXCLUDED.updated_at;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_runtime.delete_torrent(_torrent_id UUID)
RETURNS VOID AS
$$
BEGIN
    DELETE FROM revaer_runtime.torrents WHERE torrent_id = _torrent_id;
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
    files JSONB,
    added_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ
) AS
$$
BEGIN
    RETURN QUERY
    SELECT
        t.torrent_id,
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
        t.files,
        t.added_at,
        t.completed_at,
        t.updated_at
    FROM revaer_runtime.torrents AS t;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_runtime.mark_fs_job_started(
    _torrent_id UUID,
    _src_path TEXT
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO revaer_runtime.fs_jobs (
        torrent_id,
        src_path,
        status,
        attempt
    )
    VALUES (
        _torrent_id,
        _src_path,
        'moving'::revaer_runtime.fs_status,
        1
    )
    ON CONFLICT (torrent_id) DO UPDATE
    SET
        src_path = EXCLUDED.src_path,
        status = CASE
            WHEN revaer_runtime.fs_jobs.status = 'moved'::revaer_runtime.fs_status THEN 'moved'::revaer_runtime.fs_status
            ELSE 'moving'::revaer_runtime.fs_status
        END,
        attempt = CASE
            WHEN revaer_runtime.fs_jobs.status = 'moved' THEN revaer_runtime.fs_jobs.attempt
            ELSE revaer_runtime.fs_jobs.attempt + 1
        END,
        last_error = NULL,
        updated_at = now();
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_runtime.mark_fs_job_completed(
    _torrent_id UUID,
    _src_path TEXT,
    _dst_path TEXT,
    _transfer_mode TEXT
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO revaer_runtime.fs_jobs (
        torrent_id,
        src_path,
        dst_path,
        transfer_mode,
        status,
        attempt
    )
    VALUES (
        _torrent_id,
        _src_path,
        _dst_path,
        _transfer_mode,
        'moved'::revaer_runtime.fs_status,
        1
    )
    ON CONFLICT (torrent_id) DO UPDATE
    SET
        src_path = EXCLUDED.src_path,
        dst_path = EXCLUDED.dst_path,
        transfer_mode = EXCLUDED.transfer_mode,
        status = 'moved'::revaer_runtime.fs_status,
        attempt = CASE
            WHEN revaer_runtime.fs_jobs.attempt > 0 THEN revaer_runtime.fs_jobs.attempt
            ELSE 1
        END,
        last_error = NULL,
        updated_at = now();
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_runtime.mark_fs_job_failed(
    _torrent_id UUID,
    _error TEXT
) RETURNS VOID AS
$$
BEGIN
    UPDATE revaer_runtime.fs_jobs
    SET
        status = 'failed'::revaer_runtime.fs_status,
        attempt = attempt + 1,
        last_error = _error,
        updated_at = now()
    WHERE torrent_id = _torrent_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_runtime.fs_job_state(_torrent_id UUID)
RETURNS TABLE (
    status TEXT,
    attempt SMALLINT,
    src_path TEXT,
    dst_path TEXT,
    transfer_mode TEXT,
    last_error TEXT,
    updated_at TIMESTAMPTZ
) AS
$$
BEGIN
    RETURN QUERY
    SELECT
        fs.status::TEXT,
        fs.attempt,
        fs.src_path,
        fs.dst_path,
        fs.transfer_mode,
        fs.last_error,
        fs.updated_at
    FROM revaer_runtime.fs_jobs AS fs
    WHERE fs.torrent_id = _torrent_id;
END;
$$ LANGUAGE plpgsql STABLE;

-- Runtime triggers ----------------------------------------------------------

CREATE TRIGGER revaer_runtime_torrents_touch_updated_at
BEFORE UPDATE ON revaer_runtime.torrents
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER revaer_runtime_fs_jobs_touch_updated_at
BEFORE UPDATE ON revaer_runtime.fs_jobs
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();
