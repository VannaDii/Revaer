-- BEGIN 0001_db_init.sql
-- Core database scaffolding for Revaer.

CREATE SCHEMA IF NOT EXISTS revaer_config;
CREATE SCHEMA IF NOT EXISTS revaer_runtime;
-- END 0001_db_init.sql

-- BEGIN 0002_addons_and_plugins.sql
-- Required add-ons and plugins.

CREATE EXTENSION IF NOT EXISTS pgcrypto;
-- END 0002_addons_and_plugins.sql

-- BEGIN 0003_schema_and_tables.sql
-- Schemas, tables, types, and seed data.

-- Configuration core -------------------------------------------------------

CREATE TABLE IF NOT EXISTS settings_revision (
    id SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    revision BIGINT NOT NULL DEFAULT 0,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

INSERT INTO settings_revision (id)
VALUES (1)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS app_profile (
    id UUID PRIMARY KEY,
    version BIGINT NOT NULL DEFAULT 0,
    mode TEXT NOT NULL CHECK (mode IN ('setup', 'active')),
    instance_name TEXT NOT NULL,
    http_port INTEGER NOT NULL DEFAULT 7070,
    bind_addr INET NOT NULL DEFAULT '127.0.0.1',
    telemetry JSONB NOT NULL DEFAULT '{}'::jsonb,
    features JSONB NOT NULL DEFAULT '{}'::jsonb,
    immutable_keys JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT app_profile_singleton CHECK (id = '00000000-0000-0000-0000-000000000001')
);

INSERT INTO app_profile (id, mode, instance_name)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'setup',
    'revaer'
)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS engine_profile (
    id UUID PRIMARY KEY,
    implementation TEXT NOT NULL,
    listen_port INTEGER,
    dht BOOLEAN NOT NULL DEFAULT FALSE,
    encryption TEXT NOT NULL DEFAULT 'require',
    max_active INTEGER,
    max_download_bps BIGINT,
    max_upload_bps BIGINT,
    seed_ratio_limit DOUBLE PRECISION,
    seed_time_limit BIGINT,
    sequential_default BOOLEAN NOT NULL DEFAULT TRUE,
    resume_dir TEXT NOT NULL,
    download_root TEXT NOT NULL,
    enable_lsd BOOLEAN NOT NULL DEFAULT FALSE,
    enable_upnp BOOLEAN NOT NULL DEFAULT FALSE,
    enable_natpmp BOOLEAN NOT NULL DEFAULT FALSE,
    enable_pex BOOLEAN NOT NULL DEFAULT FALSE,
    dht_bootstrap_nodes JSONB NOT NULL DEFAULT '[]'::jsonb,
    dht_router_nodes JSONB NOT NULL DEFAULT '[]'::jsonb,
    ip_filter JSONB NOT NULL DEFAULT '{}'::jsonb,
    listen_interfaces JSONB NOT NULL DEFAULT '[]'::jsonb,
    ipv6_mode TEXT NOT NULL DEFAULT 'disabled',
    anonymous_mode BOOLEAN NOT NULL DEFAULT FALSE,
    force_proxy BOOLEAN NOT NULL DEFAULT FALSE,
    prefer_rc4 BOOLEAN NOT NULL DEFAULT FALSE,
    allow_multiple_connections_per_ip BOOLEAN NOT NULL DEFAULT FALSE,
    enable_outgoing_utp BOOLEAN NOT NULL DEFAULT FALSE,
    enable_incoming_utp BOOLEAN NOT NULL DEFAULT FALSE,
    outgoing_port_min INTEGER,
    outgoing_port_max INTEGER,
    peer_dscp INTEGER,
    connections_limit INTEGER,
    connections_limit_per_torrent INTEGER,
    unchoke_slots INTEGER,
    half_open_limit INTEGER,
    alt_speed JSONB DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT engine_profile_singleton CHECK (id = '00000000-0000-0000-0000-000000000002')
);

INSERT INTO engine_profile (
    id,
    implementation,
    resume_dir,
    download_root
)
VALUES (
    '00000000-0000-0000-0000-000000000002',
    'libtorrent',
    '/var/lib/revaer/state',
    '/data/staging'
)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS fs_policy (
    id UUID PRIMARY KEY,
    library_root TEXT NOT NULL,
    extract BOOLEAN NOT NULL DEFAULT FALSE,
    par2 TEXT NOT NULL DEFAULT 'off',
    flatten BOOLEAN NOT NULL DEFAULT FALSE,
    move_mode TEXT NOT NULL DEFAULT 'hardlink',
    cleanup_keep JSONB NOT NULL DEFAULT '[]'::jsonb,
    cleanup_drop JSONB NOT NULL DEFAULT '[]'::jsonb,
    chmod_file TEXT,
    chmod_dir TEXT,
    owner TEXT,
    "group" TEXT,
    umask TEXT,
    allow_paths JSONB NOT NULL DEFAULT '[]'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT fs_policy_singleton CHECK (id = '00000000-0000-0000-0000-000000000003')
);

INSERT INTO fs_policy (id, library_root, allow_paths)
VALUES (
    '00000000-0000-0000-0000-000000000003',
    '/data/library',
    '["/data/staging", "/data/library"]'::jsonb
)
ON CONFLICT (id) DO NOTHING;

CREATE TABLE IF NOT EXISTS auth_api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_id TEXT NOT NULL UNIQUE,
    hash TEXT NOT NULL,
    label TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    rate_limit JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS auth_api_keys_enabled_idx ON auth_api_keys (enabled) WHERE enabled = TRUE;

CREATE TABLE IF NOT EXISTS query_presets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    expression TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS settings_history (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    kind TEXT NOT NULL,
    old JSONB,
    new JSONB,
    actor TEXT,
    reason TEXT,
    revision BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS settings_secret (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    ciphertext BYTEA NOT NULL,
    created_by TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS setup_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token_hash TEXT NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    issued_by TEXT
);

CREATE UNIQUE INDEX IF NOT EXISTS setup_tokens_active_unique
    ON setup_tokens ((TRUE))
    WHERE consumed_at IS NULL;

-- Tracker normalization tables ---------------------------------------------

CREATE TABLE IF NOT EXISTS public.engine_tracker_config (
    profile_id UUID PRIMARY KEY REFERENCES public.engine_profile(id) ON DELETE CASCADE,
    user_agent TEXT,
    announce_ip TEXT,
    listen_interface TEXT,
    request_timeout_ms INTEGER,
    announce_to_all BOOLEAN NOT NULL DEFAULT FALSE,
    replace_trackers BOOLEAN NOT NULL DEFAULT FALSE,
    proxy_host TEXT,
    proxy_port INTEGER,
    proxy_kind TEXT,
    proxy_username_secret TEXT,
    proxy_password_secret TEXT,
    proxy_peers BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS public.engine_tracker_endpoints (
    id BIGSERIAL PRIMARY KEY,
    profile_id UUID NOT NULL REFERENCES public.engine_profile(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('default', 'extra')),
    url TEXT NOT NULL,
    ord INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS engine_tracker_endpoints_dedup
    ON public.engine_tracker_endpoints (profile_id, kind, url);

CREATE UNIQUE INDEX IF NOT EXISTS engine_tracker_endpoints_order
    ON public.engine_tracker_endpoints (profile_id, kind, ord);

-- Runtime enums and tables --------------------------------------------------

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_type AS t
        JOIN pg_namespace AS n ON n.oid = t.typnamespace
        WHERE n.nspname = 'revaer_runtime'
          AND t.typname = 'torrent_state'
    ) THEN
        CREATE TYPE revaer_runtime.torrent_state AS ENUM (
            'queued',
            'fetching_metadata',
            'downloading',
            'seeding',
            'completed',
            'failed',
            'stopped'
        );
    END IF;
END;
$$;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM pg_type AS t
        JOIN pg_namespace AS n ON n.oid = t.typnamespace
        WHERE n.nspname = 'revaer_runtime'
          AND t.typname = 'fs_status'
    ) THEN
        CREATE TYPE revaer_runtime.fs_status AS ENUM (
            'pending',
            'moving',
            'moved',
            'failed',
            'skipped'
        );
    END IF;
END;
$$;

CREATE TABLE IF NOT EXISTS revaer_runtime.torrents (
    torrent_id UUID PRIMARY KEY,
    name TEXT,
    state revaer_runtime.torrent_state NOT NULL,
    state_message TEXT,
    progress_bytes_downloaded BIGINT NOT NULL DEFAULT 0,
    progress_bytes_total BIGINT NOT NULL DEFAULT 0,
    progress_eta_seconds BIGINT,
    download_bps BIGINT NOT NULL DEFAULT 0,
    upload_bps BIGINT NOT NULL DEFAULT 0,
    ratio DOUBLE PRECISION NOT NULL DEFAULT 0,
    sequential BOOLEAN NOT NULL DEFAULT FALSE,
    library_path TEXT,
    download_dir TEXT,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    files JSONB,
    added_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS revaer_runtime.fs_jobs (
    id BIGSERIAL PRIMARY KEY,
    torrent_id UUID NOT NULL REFERENCES revaer_runtime.torrents(torrent_id) ON DELETE CASCADE,
    src_path TEXT NOT NULL,
    dst_path TEXT,
    transfer_mode TEXT,
    status revaer_runtime.fs_status NOT NULL DEFAULT 'pending',
    attempt SMALLINT NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS revaer_runtime_fs_jobs_torrent_idx
    ON revaer_runtime.fs_jobs (torrent_id);
-- END 0003_schema_and_tables.sql

-- BEGIN 0004_stored_procs_and_functions.sql
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
        alt_speed = revaer_config.normalize_alt_speed(_alt_speed)
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
-- END 0004_stored_procs_and_functions.sql

-- BEGIN 0005_queue_auto_manage.sql
-- Queue priority defaults and auto-managed toggle.

ALTER TABLE public.engine_profile
    ADD COLUMN IF NOT EXISTS auto_managed BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS auto_manage_prefer_seeds BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS dont_count_slow_torrents BOOLEAN NOT NULL DEFAULT TRUE;

CREATE OR REPLACE FUNCTION revaer_config.fetch_engine_profile(
    _id UUID
) RETURNS TABLE (
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
           ep.auto_managed,
           ep.auto_manage_prefer_seeds,
           ep.dont_count_slow_torrents,
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
        'auto_managed', ep.auto_managed,
        'auto_manage_prefer_seeds', ep.auto_manage_prefer_seeds,
        'dont_count_slow_torrents', ep.dont_count_slow_torrents,
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

CREATE OR REPLACE FUNCTION revaer_config.normalize_weekday_label(_label TEXT)
RETURNS TEXT AS
$$
BEGIN
    CASE lower(btrim(_label))
        WHEN 'mon', 'monday' THEN RETURN 'mon';
        WHEN 'tue', 'tues', 'tuesday' THEN RETURN 'tue';
        WHEN 'wed', 'wednesday' THEN RETURN 'wed';
        WHEN 'thu', 'thur', 'thurs', 'thursday' THEN RETURN 'thu';
        WHEN 'fri', 'friday' THEN RETURN 'fri';
        WHEN 'sat', 'saturday' THEN RETURN 'sat';
        WHEN 'sun', 'sunday' THEN RETURN 'sun';
        ELSE RETURN NULL;
    END CASE;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION revaer_config.weekday_order(_label TEXT)
RETURNS INTEGER AS
$$
BEGIN
    CASE _label
        WHEN 'mon' THEN RETURN 1;
        WHEN 'tue' THEN RETURN 2;
        WHEN 'wed' THEN RETURN 3;
        WHEN 'thu' THEN RETURN 4;
        WHEN 'fri' THEN RETURN 5;
        WHEN 'sat' THEN RETURN 6;
        WHEN 'sun' THEN RETURN 7;
        ELSE RETURN 0;
    END CASE;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION revaer_config.parse_alt_speed_minutes(_value TEXT)
RETURNS INTEGER AS
$$
DECLARE
    trimmed TEXT;
    hours INTEGER;
    minutes INTEGER;
BEGIN
    trimmed := btrim(_value);
    IF trimmed IS NULL OR trimmed = '' THEN
        RETURN NULL;
    END IF;
    IF trimmed !~ '^\d{2}:\d{2}$' THEN
        RETURN NULL;
    END IF;
    hours := split_part(trimmed, ':', 1)::INTEGER;
    minutes := split_part(trimmed, ':', 2)::INTEGER;
    IF hours < 0 OR hours > 23 OR minutes < 0 OR minutes > 59 THEN
        RETURN NULL;
    END IF;
    RETURN hours * 60 + minutes;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION revaer_config.format_alt_speed_minutes(_minutes INTEGER)
RETURNS TEXT AS
$$
DECLARE
    hours INTEGER;
    mins INTEGER;
BEGIN
    hours := _minutes / 60;
    mins := _minutes % 60;
    RETURN lpad(hours::TEXT, 2, '0') || ':' || lpad(mins::TEXT, 2, '0');
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION revaer_config.normalize_alt_speed_schedule(_schedule JSONB)
RETURNS JSONB AS
$$
DECLARE
    day_entry JSONB;
    day_label TEXT;
    canonical TEXT;
    days TEXT[] := ARRAY[]::TEXT[];
    start_text TEXT;
    end_text TEXT;
    start_minutes INTEGER;
    end_minutes INTEGER;
BEGIN
    IF _schedule IS NULL OR _schedule = 'null'::jsonb THEN
        RETURN NULL;
    END IF;
    IF jsonb_typeof(_schedule) <> 'object' THEN
        RETURN NULL;
    END IF;
    IF EXISTS (
        SELECT 1
        FROM jsonb_object_keys(_schedule) AS key
        WHERE key NOT IN ('days', 'start', 'end')
    ) THEN
        RETURN NULL;
    END IF;
    IF NOT (_schedule ? 'days') THEN
        RETURN NULL;
    END IF;
    IF jsonb_typeof(_schedule->'days') <> 'array' THEN
        RETURN NULL;
    END IF;
    FOR day_entry IN SELECT value FROM jsonb_array_elements(_schedule->'days')
    LOOP
        IF jsonb_typeof(day_entry) <> 'string' THEN
            RETURN NULL;
        END IF;
        day_label := btrim(day_entry::TEXT, '"');
        day_label := btrim(day_label);
        IF day_label = '' THEN
            CONTINUE;
        END IF;
        canonical := revaer_config.normalize_weekday_label(day_label);
        IF canonical IS NULL THEN
            RETURN NULL;
        END IF;
        IF array_position(days, canonical) IS NULL THEN
            days := array_append(days, canonical);
        END IF;
    END LOOP;
    IF array_length(days, 1) IS NULL THEN
        RETURN NULL;
    END IF;

    start_text := _schedule->>'start';
    end_text := _schedule->>'end';
    IF start_text IS NULL OR end_text IS NULL THEN
        RETURN NULL;
    END IF;
    start_minutes := revaer_config.parse_alt_speed_minutes(start_text);
    end_minutes := revaer_config.parse_alt_speed_minutes(end_text);
    IF start_minutes IS NULL OR end_minutes IS NULL THEN
        RETURN NULL;
    END IF;
    IF start_minutes = end_minutes THEN
        RETURN NULL;
    END IF;

    SELECT array_agg(day ORDER BY revaer_config.weekday_order(day))
    INTO days
    FROM unnest(days) AS day;

    RETURN jsonb_build_object(
        'days', to_jsonb(days),
        'start', revaer_config.format_alt_speed_minutes(start_minutes),
        'end', revaer_config.format_alt_speed_minutes(end_minutes)
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

CREATE OR REPLACE FUNCTION revaer_config.normalize_alt_speed(_alt_speed JSONB)
RETURNS JSONB AS
$$
DECLARE
    download_bps BIGINT;
    upload_bps BIGINT;
    schedule JSONB;
    download_text TEXT;
    upload_text TEXT;
BEGIN
    IF _alt_speed IS NULL OR _alt_speed = 'null'::jsonb THEN
        RETURN '{}'::jsonb;
    END IF;
    IF jsonb_typeof(_alt_speed) <> 'object' THEN
        RETURN '{}'::jsonb;
    END IF;
    IF EXISTS (
        SELECT 1
        FROM jsonb_object_keys(_alt_speed) AS key
        WHERE key NOT IN ('download_bps', 'upload_bps', 'schedule')
    ) THEN
        RETURN '{}'::jsonb;
    END IF;

    IF _alt_speed ? 'download_bps' THEN
        IF jsonb_typeof(_alt_speed->'download_bps') = 'null' THEN
            download_bps := NULL;
        ELSIF jsonb_typeof(_alt_speed->'download_bps') <> 'number' THEN
            RETURN '{}'::jsonb;
        ELSE
            download_text := _alt_speed->>'download_bps';
            IF download_text !~ '^-?\d+$' THEN
                RETURN '{}'::jsonb;
            END IF;
            download_bps := download_text::BIGINT;
        END IF;
    END IF;

    IF _alt_speed ? 'upload_bps' THEN
        IF jsonb_typeof(_alt_speed->'upload_bps') = 'null' THEN
            upload_bps := NULL;
        ELSIF jsonb_typeof(_alt_speed->'upload_bps') <> 'number' THEN
            RETURN '{}'::jsonb;
        ELSE
            upload_text := _alt_speed->>'upload_bps';
            IF upload_text !~ '^-?\d+$' THEN
                RETURN '{}'::jsonb;
            END IF;
            upload_bps := upload_text::BIGINT;
        END IF;
    END IF;

    IF download_bps IS NOT NULL THEN
        IF download_bps <= 0 THEN
            download_bps := NULL;
        ELSIF download_bps > 5000000000 THEN
            download_bps := 5000000000;
        END IF;
    END IF;
    IF upload_bps IS NOT NULL THEN
        IF upload_bps <= 0 THEN
            upload_bps := NULL;
        ELSIF upload_bps > 5000000000 THEN
            upload_bps := 5000000000;
        END IF;
    END IF;

    schedule := revaer_config.normalize_alt_speed_schedule(_alt_speed->'schedule');
    IF schedule IS NULL THEN
        RETURN '{}'::jsonb;
    END IF;
    IF download_bps IS NULL AND upload_bps IS NULL THEN
        RETURN '{}'::jsonb;
    END IF;

    RETURN jsonb_strip_nulls(jsonb_build_object(
        'download_bps', download_bps,
        'upload_bps', upload_bps,
        'schedule', schedule
    ));
END;
$$ LANGUAGE plpgsql IMMUTABLE;

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
        auto_managed = _auto_managed,
        auto_manage_prefer_seeds = _auto_manage_prefer_seeds,
        dont_count_slow_torrents = _dont_count_slow_torrents,
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
        alt_speed = revaer_config.normalize_alt_speed(_alt_speed)
    WHERE id = _id;

    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
END;
$$ LANGUAGE plpgsql;
-- END 0005_queue_auto_manage.sql

-- BEGIN 0006_choking_and_super_seeding.sql
-- Choking/unchoke strategy and super-seeding defaults.

ALTER TABLE public.engine_profile
    ADD COLUMN IF NOT EXISTS choking_algorithm TEXT NOT NULL DEFAULT 'fixed_slots',
    ADD COLUMN IF NOT EXISTS seed_choking_algorithm TEXT NOT NULL DEFAULT 'round_robin',
    ADD COLUMN IF NOT EXISTS strict_super_seeding BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS optimistic_unchoke_slots INTEGER,
    ADD COLUMN IF NOT EXISTS max_queued_disk_bytes BIGINT,
    ADD COLUMN IF NOT EXISTS super_seeding BOOLEAN NOT NULL DEFAULT FALSE;

DROP FUNCTION IF EXISTS revaer_config.fetch_engine_profile(UUID);
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
    JSONB
);

CREATE OR REPLACE FUNCTION revaer_config.fetch_engine_profile(
    _id UUID
) RETURNS TABLE (
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
           ep.alt_speed
    FROM public.engine_profile AS ep
    WHERE ep.id = _id;
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
        'auto_managed', ep.auto_managed,
        'auto_manage_prefer_seeds', ep.auto_manage_prefer_seeds,
        'dont_count_slow_torrents', ep.dont_count_slow_torrents,
        'super_seeding', ep.super_seeding,
        'choking_algorithm', ep.choking_algorithm,
        'seed_choking_algorithm', ep.seed_choking_algorithm,
        'strict_super_seeding', ep.strict_super_seeding,
        'optimistic_unchoke_slots', ep.optimistic_unchoke_slots,
        'max_queued_disk_bytes', ep.max_queued_disk_bytes,
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
        alt_speed = revaer_config.normalize_alt_speed(_alt_speed)
    WHERE id = _id;

    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
END;
$$ LANGUAGE plpgsql;
-- END 0006_choking_and_super_seeding.sql

-- BEGIN 0007_stats_interval.sql
-- Add configurable stats interval for libtorrent alerts.

ALTER TABLE engine_profile
    ADD COLUMN IF NOT EXISTS stats_interval_ms INTEGER;

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

-- Refresh engine profile fetch helpers with the new column.
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

-- Update mutator to accept stats interval.
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
        alt_speed = revaer_config.normalize_alt_speed(_alt_speed),
        stats_interval_ms = _stats_interval_ms
    WHERE id = _id;

    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
END;
$$ LANGUAGE plpgsql;
-- END 0007_stats_interval.sql

-- BEGIN 0008_tracker_auth.sql
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
        alt_speed = revaer_config.normalize_alt_speed(_alt_speed),
        stats_interval_ms = _stats_interval_ms
    WHERE id = _id;

    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
END;
$$ LANGUAGE plpgsql;
-- END 0008_tracker_auth.sql

-- BEGIN 0009_storage_options.sql
-- Add storage allocation and partfile toggles to engine profile.

ALTER TABLE public.engine_profile
    ADD COLUMN IF NOT EXISTS storage_mode TEXT NOT NULL DEFAULT 'sparse',
    ADD COLUMN IF NOT EXISTS use_partfile BOOLEAN NOT NULL DEFAULT TRUE;

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
    JSONB,
    INTEGER
);

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
    storage_mode TEXT,
    use_partfile BOOLEAN,
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
           ep.storage_mode,
           ep.use_partfile,
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
        'storage_mode', ep.storage_mode,
        'use_partfile', ep.use_partfile,
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
    _storage_mode TEXT,
    _use_partfile BOOLEAN,
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
        storage_mode = _storage_mode,
        use_partfile = _use_partfile,
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
        alt_speed = revaer_config.normalize_alt_speed(_alt_speed),
        stats_interval_ms = _stats_interval_ms
    WHERE id = _id;
    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
END;
$$ LANGUAGE plpgsql;
-- END 0009_storage_options.sql

-- BEGIN 0010_disk_cache_options.sql
-- Add disk cache configuration toggles to engine profile.

ALTER TABLE public.engine_profile
    ADD COLUMN IF NOT EXISTS cache_size INTEGER,
    ADD COLUMN IF NOT EXISTS cache_expiry INTEGER,
    ADD COLUMN IF NOT EXISTS coalesce_reads BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS coalesce_writes BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS use_disk_cache_pool BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS disk_read_mode TEXT,
    ADD COLUMN IF NOT EXISTS disk_write_mode TEXT,
    ADD COLUMN IF NOT EXISTS verify_piece_hashes BOOLEAN NOT NULL DEFAULT TRUE;

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
    JSONB,
    INTEGER
);

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
        'storage_mode', ep.storage_mode
    )
    || jsonb_build_object(
        'use_partfile', ep.use_partfile,
        'cache_size', ep.cache_size,
        'cache_expiry', ep.cache_expiry,
        'coalesce_reads', ep.coalesce_reads,
        'coalesce_writes', ep.coalesce_writes,
        'use_disk_cache_pool', ep.use_disk_cache_pool,
        'disk_read_mode', ep.disk_read_mode,
        'disk_write_mode', ep.disk_write_mode,
        'verify_piece_hashes', ep.verify_piece_hashes,
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
        'listen_interfaces', ep.listen_interfaces
    )
    || jsonb_build_object(
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
        alt_speed = revaer_config.normalize_alt_speed(_alt_speed),
        stats_interval_ms = _stats_interval_ms
    WHERE id = _id;
    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
END;
$$ LANGUAGE plpgsql;
-- END 0010_disk_cache_options.sql

-- BEGIN 0011_peer_classes.sql
-- Introduce peer class configuration tables and wiring.

-- Normalised peer class definitions per engine profile.
CREATE TABLE IF NOT EXISTS public.engine_peer_classes (
    profile_id UUID NOT NULL REFERENCES public.engine_profile(id) ON DELETE CASCADE,
    class_id SMALLINT NOT NULL,
    label TEXT NOT NULL,
    download_priority SMALLINT NOT NULL,
    upload_priority SMALLINT NOT NULL,
    connection_limit_factor SMALLINT NOT NULL DEFAULT 100,
    ignore_unchoke_slots BOOLEAN NOT NULL DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_id, class_id),
    CONSTRAINT engine_peer_class_id_bounds CHECK (class_id >= 0 AND class_id <= 31),
    CONSTRAINT engine_peer_class_download_priority_bounds CHECK (download_priority >= 1 AND download_priority <= 255),
    CONSTRAINT engine_peer_class_upload_priority_bounds CHECK (upload_priority >= 1 AND upload_priority <= 255),
    CONSTRAINT engine_peer_class_connection_limit_factor_bounds CHECK (connection_limit_factor >= 1)
);

CREATE TABLE IF NOT EXISTS public.engine_peer_class_defaults (
    profile_id UUID NOT NULL REFERENCES public.engine_profile(id) ON DELETE CASCADE,
    class_id SMALLINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_id, class_id),
    CONSTRAINT engine_peer_class_defaults_fk FOREIGN KEY (profile_id, class_id)
        REFERENCES public.engine_peer_classes(profile_id, class_id)
        ON DELETE CASCADE
);

DROP FUNCTION IF EXISTS revaer_config.render_peer_classes(UUID);
DROP FUNCTION IF EXISTS revaer_config.persist_peer_class_config(UUID, JSONB);

CREATE OR REPLACE FUNCTION revaer_config.persist_peer_class_config(
    _profile_id UUID,
    _peer_classes JSONB
) RETURNS VOID AS
$$
DECLARE
    classes JSONB := COALESCE(_peer_classes->'classes', '[]'::jsonb);
    defaults JSONB := COALESCE(_peer_classes->'default', '[]'::jsonb);
    entry JSONB;
    class_id SMALLINT;
    label TEXT;
    download_priority SMALLINT;
    upload_priority SMALLINT;
    connection_limit_factor SMALLINT;
    ignore_unchoke_slots BOOLEAN;
BEGIN
    DELETE FROM public.engine_peer_class_defaults WHERE profile_id = _profile_id;
    DELETE FROM public.engine_peer_classes WHERE profile_id = _profile_id;

    FOR entry IN SELECT * FROM jsonb_array_elements(classes)
    LOOP
        class_id := COALESCE((entry->>'id')::SMALLINT, -1);
        label := NULLIF(entry->>'label', '');
        download_priority := COALESCE((entry->>'download_priority')::SMALLINT, 1);
        upload_priority := COALESCE((entry->>'upload_priority')::SMALLINT, 1);
        connection_limit_factor := COALESCE((entry->>'connection_limit_factor')::SMALLINT, 100);
        ignore_unchoke_slots := COALESCE((entry->>'ignore_unchoke_slots')::BOOLEAN, FALSE);

        IF class_id < 0 OR class_id > 31 THEN
            CONTINUE;
        END IF;
        IF download_priority < 1 OR download_priority > 255 THEN
            CONTINUE;
        END IF;
        IF upload_priority < 1 OR upload_priority > 255 THEN
            CONTINUE;
        END IF;
        IF connection_limit_factor < 1 THEN
            CONTINUE;
        END IF;
        IF label IS NULL THEN
            label := format('class_%s', class_id);
        END IF;

        INSERT INTO public.engine_peer_classes AS epc (
            profile_id,
            class_id,
            label,
            download_priority,
            upload_priority,
            connection_limit_factor,
            ignore_unchoke_slots
        ) VALUES (
            _profile_id,
            class_id,
            label,
            download_priority,
            upload_priority,
            connection_limit_factor,
            ignore_unchoke_slots
        )
        ON CONFLICT (profile_id, class_id) DO UPDATE
        SET label = EXCLUDED.label,
            download_priority = EXCLUDED.download_priority,
            upload_priority = EXCLUDED.upload_priority,
            connection_limit_factor = EXCLUDED.connection_limit_factor,
            ignore_unchoke_slots = EXCLUDED.ignore_unchoke_slots,
            updated_at = now();
    END LOOP;

    INSERT INTO public.engine_peer_class_defaults (profile_id, class_id)
    SELECT _profile_id, elem::SMALLINT
    FROM jsonb_array_elements_text(defaults) AS t(elem)
    WHERE EXISTS (
        SELECT 1 FROM public.engine_peer_classes epc
        WHERE epc.profile_id = _profile_id AND epc.class_id = (t.elem)::SMALLINT
    )
    ON CONFLICT (profile_id, class_id) DO NOTHING;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.render_peer_classes(_profile_id UUID)
RETURNS JSONB AS
$$
DECLARE
    classes JSONB;
    defaults JSONB;
BEGIN
    SELECT COALESCE(
        jsonb_agg(
            jsonb_build_object(
                'id', class_id,
                'label', label,
                'download_priority', download_priority,
                'upload_priority', upload_priority,
                'connection_limit_factor', connection_limit_factor,
                'ignore_unchoke_slots', ignore_unchoke_slots
            ) ORDER BY class_id
        ), '[]'::jsonb
    ) INTO classes
    FROM public.engine_peer_classes
    WHERE profile_id = _profile_id;

    SELECT COALESCE(jsonb_agg(class_id ORDER BY class_id), '[]'::jsonb)
    INTO defaults
    FROM public.engine_peer_class_defaults
    WHERE profile_id = _profile_id;

    RETURN jsonb_build_object('classes', classes, 'default', defaults);
END;
$$ LANGUAGE plpgsql STABLE;

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
    JSONB,
    INTEGER
);

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
    stats_interval_ms INTEGER,
    peer_classes JSONB
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
           ep.stats_interval_ms,
           revaer_config.render_peer_classes(ep.id)
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
        'choking_algorithm', ep.choking_algorithm,
        'seed_choking_algorithm', ep.seed_choking_algorithm,
        'strict_super_seeding', ep.strict_super_seeding,
        'optimistic_unchoke_slots', ep.optimistic_unchoke_slots,
        'max_queued_disk_bytes', ep.max_queued_disk_bytes,
        'resume_dir', ep.resume_dir,
        'download_root', ep.download_root,
        'storage_mode', ep.storage_mode,
        'use_partfile', ep.use_partfile,
        'cache_size', ep.cache_size,
        'cache_expiry', ep.cache_expiry,
        'coalesce_reads', ep.coalesce_reads,
        'coalesce_writes', ep.coalesce_writes,
        'use_disk_cache_pool', ep.use_disk_cache_pool,
        'disk_read_mode', ep.disk_read_mode,
        'disk_write_mode', ep.disk_write_mode,
        'verify_piece_hashes', ep.verify_piece_hashes,
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
        'stats_interval_ms', ep.stats_interval_ms,
        'peer_classes', revaer_config.render_peer_classes(ep.id)
    ) INTO body
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
    _stats_interval_ms INTEGER,
    _peer_classes JSONB
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
        alt_speed = revaer_config.normalize_alt_speed(_alt_speed),
        stats_interval_ms = _stats_interval_ms
    WHERE id = _id;

    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
    PERFORM revaer_config.persist_peer_class_config(_id, _peer_classes);
END;
$$ LANGUAGE plpgsql;
-- END 0011_peer_classes.sql

-- BEGIN 0012_split_engine_profile_json.sql
-- Fix json build argument limits for engine profile rendering.
-- This migration recreates fetch_engine_profile_json using multiple jsonb_build_object
-- calls to stay under PostgreSQL's 100-argument limit.

DROP FUNCTION IF EXISTS revaer_config.fetch_engine_profile_json(UUID);

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
               'choking_algorithm', ep.choking_algorithm,
               'seed_choking_algorithm', ep.seed_choking_algorithm,
               'strict_super_seeding', ep.strict_super_seeding,
               'optimistic_unchoke_slots', ep.optimistic_unchoke_slots,
               'max_queued_disk_bytes', ep.max_queued_disk_bytes,
               'resume_dir', ep.resume_dir,
               'download_root', ep.download_root,
               'storage_mode', ep.storage_mode,
               'use_partfile', ep.use_partfile,
               'cache_size', ep.cache_size,
               'cache_expiry', ep.cache_expiry,
               'coalesce_reads', ep.coalesce_reads,
               'coalesce_writes', ep.coalesce_writes,
               'use_disk_cache_pool', ep.use_disk_cache_pool
           )
           || jsonb_build_object(
               'disk_read_mode', ep.disk_read_mode,
               'disk_write_mode', ep.disk_write_mode,
               'verify_piece_hashes', ep.verify_piece_hashes,
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
               'stats_interval_ms', ep.stats_interval_ms,
               'peer_classes', revaer_config.render_peer_classes(ep.id)
           )
    INTO body
    FROM public.engine_profile AS ep
    WHERE ep.id = _id;

    RETURN body;
END;
$$ LANGUAGE plpgsql STABLE;

-- END 0012_split_engine_profile_json.sql

-- BEGIN 0013_fix_peer_class_conflict.sql
-- Resolve name collisions between variables and column references in peer class persistence.

DROP FUNCTION IF EXISTS revaer_config.persist_peer_class_config(UUID, JSONB);

CREATE OR REPLACE FUNCTION revaer_config.persist_peer_class_config(
    _profile_id UUID,
    _peer_classes JSONB
) RETURNS VOID AS
$$
DECLARE
    classes JSONB := COALESCE(_peer_classes->'classes', '[]'::jsonb);
    defaults JSONB := COALESCE(_peer_classes->'default', '[]'::jsonb);
    entry JSONB;
    class_identifier SMALLINT;
    label TEXT;
    download_priority SMALLINT;
    upload_priority SMALLINT;
    connection_limit_factor SMALLINT;
    ignore_unchoke_slots BOOLEAN;
BEGIN
    DELETE FROM public.engine_peer_class_defaults WHERE profile_id = _profile_id;
    DELETE FROM public.engine_peer_classes WHERE profile_id = _profile_id;

    FOR entry IN SELECT * FROM jsonb_array_elements(classes)
    LOOP
        class_identifier := COALESCE((entry->>'id')::SMALLINT, -1);
        label := NULLIF(entry->>'label', '');
        download_priority := COALESCE((entry->>'download_priority')::SMALLINT, 1);
        upload_priority := COALESCE((entry->>'upload_priority')::SMALLINT, 1);
        connection_limit_factor := COALESCE((entry->>'connection_limit_factor')::SMALLINT, 100);
        ignore_unchoke_slots := COALESCE((entry->>'ignore_unchoke_slots')::BOOLEAN, FALSE);

        IF class_identifier < 0 OR class_identifier > 31 THEN
            CONTINUE;
        END IF;
        IF download_priority < 1 OR download_priority > 255 THEN
            CONTINUE;
        END IF;
        IF upload_priority < 1 OR upload_priority > 255 THEN
            CONTINUE;
        END IF;
        IF connection_limit_factor < 1 THEN
            CONTINUE;
        END IF;
        IF label IS NULL THEN
            label := format('class_%s', class_identifier);
        END IF;

        INSERT INTO public.engine_peer_classes AS epc (
            profile_id,
            class_id,
            label,
            download_priority,
            upload_priority,
            connection_limit_factor,
            ignore_unchoke_slots
        ) VALUES (
            _profile_id,
            class_identifier,
            label,
            download_priority,
            upload_priority,
            connection_limit_factor,
            ignore_unchoke_slots
        )
        ON CONFLICT (profile_id, class_id) DO UPDATE
        SET label = EXCLUDED.label,
            download_priority = EXCLUDED.download_priority,
            upload_priority = EXCLUDED.upload_priority,
            connection_limit_factor = EXCLUDED.connection_limit_factor,
            ignore_unchoke_slots = EXCLUDED.ignore_unchoke_slots,
            updated_at = now();
    END LOOP;

    INSERT INTO public.engine_peer_class_defaults (profile_id, class_id)
    SELECT _profile_id, elem::SMALLINT
    FROM jsonb_array_elements_text(defaults) AS t(elem)
    WHERE EXISTS (
        SELECT 1 FROM public.engine_peer_classes epc
        WHERE epc.profile_id = _profile_id AND epc.class_id = (t.elem)::SMALLINT
    )
    ON CONFLICT (profile_id, class_id) DO NOTHING;
END;
$$ LANGUAGE plpgsql;

-- END 0013_fix_peer_class_conflict.sql

-- BEGIN 0014_torrent_metadata_payload.sql
-- Expose torrent payload metadata in the runtime list function.

DROP FUNCTION IF EXISTS revaer_runtime.list_torrents();

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
    payload JSONB,
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
        t.payload,
        t.files,
        t.added_at,
        t.completed_at,
        t.updated_at
    FROM revaer_runtime.torrents AS t;
END;
$$ LANGUAGE plpgsql STABLE;
-- END 0014_torrent_metadata_payload.sql
