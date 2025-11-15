CREATE SCHEMA IF NOT EXISTS revaer_config;

-- History + revision --------------------------------------------------------

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
    sequential_default BOOLEAN,
    resume_dir TEXT,
    download_root TEXT,
    tracker JSONB
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
           ep.sequential_default,
           ep.resume_dir,
           ep.download_root,
           ep.tracker
    FROM public.engine_profile AS ep
    WHERE ep.id = _id;
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

CREATE OR REPLACE FUNCTION revaer_config.fetch_engine_profile_json(_id UUID)
RETURNS JSONB AS
$$
DECLARE
    body JSONB;
BEGIN
    SELECT to_jsonb(ep.*) INTO body FROM public.engine_profile AS ep WHERE ep.id = _id;
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

-- Engine profile mutations --------------------------------------------------

CREATE OR REPLACE FUNCTION revaer_config.update_engine_implementation(
    _id UUID,
    _implementation TEXT
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.engine_profile
    SET implementation = _implementation
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_engine_listen_port(
    _id UUID,
    _port INTEGER
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.engine_profile
    SET listen_port = _port
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_engine_boolean_field(
    _id UUID,
    _column TEXT,
    _value BOOLEAN
) RETURNS VOID AS
$$
BEGIN
    IF _column = 'dht' THEN
        UPDATE public.engine_profile SET dht = _value WHERE id = _id;
    ELSIF _column = 'sequential_default' THEN
        UPDATE public.engine_profile SET sequential_default = _value WHERE id = _id;
    ELSE
        RAISE EXCEPTION 'Unsupported engine boolean column: %', _column;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_engine_encryption(
    _id UUID,
    _mode TEXT
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.engine_profile
    SET encryption = _mode
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_engine_max_active(
    _id UUID,
    _max_active INTEGER
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.engine_profile
    SET max_active = _max_active
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_engine_rate_field(
    _id UUID,
    _column TEXT,
    _value BIGINT
) RETURNS VOID AS
$$
BEGIN
    IF _column = 'max_download_bps' THEN
        UPDATE public.engine_profile SET max_download_bps = _value WHERE id = _id;
    ELSIF _column = 'max_upload_bps' THEN
        UPDATE public.engine_profile SET max_upload_bps = _value WHERE id = _id;
    ELSE
        RAISE EXCEPTION 'Unsupported engine rate column: %', _column;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_engine_text_field(
    _id UUID,
    _column TEXT,
    _value TEXT
) RETURNS VOID AS
$$
BEGIN
    IF _column = 'resume_dir' THEN
        UPDATE public.engine_profile SET resume_dir = _value WHERE id = _id;
    ELSIF _column = 'download_root' THEN
        UPDATE public.engine_profile SET download_root = _value WHERE id = _id;
    ELSE
        RAISE EXCEPTION 'Unsupported engine text column: %', _column;
    END IF;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_engine_tracker(
    _id UUID,
    _tracker JSONB
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.engine_profile
    SET tracker = COALESCE(_tracker, '{}'::jsonb)
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
