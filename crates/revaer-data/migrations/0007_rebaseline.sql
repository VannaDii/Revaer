-- Revaer consolidated migration (no JSON persistence).

CREATE SCHEMA IF NOT EXISTS revaer_config;
CREATE SCHEMA IF NOT EXISTS revaer_runtime;

CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- Core configuration tables -------------------------------------------------

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
    auth_mode TEXT NOT NULL DEFAULT 'api_key' CHECK (auth_mode IN ('api_key', 'none')),
    instance_name TEXT NOT NULL,
    http_port INTEGER NOT NULL DEFAULT 7070,
    bind_addr INET NOT NULL DEFAULT '127.0.0.1',
    telemetry_level TEXT,
    telemetry_format TEXT,
    telemetry_otel_enabled BOOLEAN,
    telemetry_otel_service_name TEXT,
    telemetry_otel_endpoint TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT app_profile_singleton CHECK (id = '00000000-0000-0000-0000-000000000001')
);

ALTER TABLE public.app_profile
    ADD COLUMN IF NOT EXISTS auth_mode TEXT NOT NULL DEFAULT 'api_key',
    ADD COLUMN IF NOT EXISTS telemetry_level TEXT,
    ADD COLUMN IF NOT EXISTS telemetry_format TEXT,
    ADD COLUMN IF NOT EXISTS telemetry_otel_enabled BOOLEAN,
    ADD COLUMN IF NOT EXISTS telemetry_otel_service_name TEXT,
    ADD COLUMN IF NOT EXISTS telemetry_otel_endpoint TEXT;

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
    auto_managed BOOLEAN NOT NULL DEFAULT TRUE,
    auto_manage_prefer_seeds BOOLEAN NOT NULL DEFAULT FALSE,
    dont_count_slow_torrents BOOLEAN NOT NULL DEFAULT TRUE,
    super_seeding BOOLEAN NOT NULL DEFAULT FALSE,
    choking_algorithm TEXT NOT NULL DEFAULT 'fixed_slots',
    seed_choking_algorithm TEXT NOT NULL DEFAULT 'round_robin',
    strict_super_seeding BOOLEAN NOT NULL DEFAULT FALSE,
    optimistic_unchoke_slots INTEGER,
    max_queued_disk_bytes BIGINT,
    resume_dir TEXT NOT NULL,
    download_root TEXT NOT NULL,
    storage_mode TEXT NOT NULL DEFAULT 'sparse',
    use_partfile BOOLEAN NOT NULL DEFAULT TRUE,
    cache_size INTEGER,
    cache_expiry INTEGER,
    coalesce_reads BOOLEAN NOT NULL DEFAULT TRUE,
    coalesce_writes BOOLEAN NOT NULL DEFAULT TRUE,
    use_disk_cache_pool BOOLEAN NOT NULL DEFAULT TRUE,
    disk_read_mode TEXT,
    disk_write_mode TEXT,
    verify_piece_hashes BOOLEAN NOT NULL DEFAULT TRUE,
    enable_lsd BOOLEAN NOT NULL DEFAULT FALSE,
    enable_upnp BOOLEAN NOT NULL DEFAULT FALSE,
    enable_natpmp BOOLEAN NOT NULL DEFAULT FALSE,
    enable_pex BOOLEAN NOT NULL DEFAULT FALSE,
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
    stats_interval_ms INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT engine_profile_singleton CHECK (id = '00000000-0000-0000-0000-000000000002')
);

ALTER TABLE public.engine_profile
    ADD COLUMN IF NOT EXISTS auto_managed BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS auto_manage_prefer_seeds BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS dont_count_slow_torrents BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS super_seeding BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS choking_algorithm TEXT NOT NULL DEFAULT 'fixed_slots',
    ADD COLUMN IF NOT EXISTS seed_choking_algorithm TEXT NOT NULL DEFAULT 'round_robin',
    ADD COLUMN IF NOT EXISTS strict_super_seeding BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS optimistic_unchoke_slots INTEGER,
    ADD COLUMN IF NOT EXISTS max_queued_disk_bytes BIGINT,
    ADD COLUMN IF NOT EXISTS storage_mode TEXT NOT NULL DEFAULT 'sparse',
    ADD COLUMN IF NOT EXISTS use_partfile BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS cache_size INTEGER,
    ADD COLUMN IF NOT EXISTS cache_expiry INTEGER,
    ADD COLUMN IF NOT EXISTS coalesce_reads BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS coalesce_writes BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS use_disk_cache_pool BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS disk_read_mode TEXT,
    ADD COLUMN IF NOT EXISTS disk_write_mode TEXT,
    ADD COLUMN IF NOT EXISTS verify_piece_hashes BOOLEAN NOT NULL DEFAULT TRUE,
    ADD COLUMN IF NOT EXISTS stats_interval_ms INTEGER,
    ADD COLUMN IF NOT EXISTS ipv6_mode TEXT NOT NULL DEFAULT 'disabled',
    ADD COLUMN IF NOT EXISTS anonymous_mode BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS force_proxy BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS prefer_rc4 BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS allow_multiple_connections_per_ip BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS enable_outgoing_utp BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS enable_incoming_utp BOOLEAN NOT NULL DEFAULT FALSE;

CREATE TABLE IF NOT EXISTS fs_policy (
    id UUID PRIMARY KEY,
    library_root TEXT NOT NULL,
    "extract" BOOLEAN NOT NULL DEFAULT FALSE,
    par2 TEXT NOT NULL DEFAULT 'off',
    flatten BOOLEAN NOT NULL DEFAULT FALSE,
    move_mode TEXT NOT NULL DEFAULT 'hardlink',
    chmod_file TEXT,
    chmod_dir TEXT,
    owner TEXT,
    "group" TEXT,
    umask TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT fs_policy_singleton CHECK (id = '00000000-0000-0000-0000-000000000003')
);

ALTER TABLE public.fs_policy
    ADD COLUMN IF NOT EXISTS "extract" BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS par2 TEXT NOT NULL DEFAULT 'off',
    ADD COLUMN IF NOT EXISTS flatten BOOLEAN NOT NULL DEFAULT FALSE,
    ADD COLUMN IF NOT EXISTS move_mode TEXT NOT NULL DEFAULT 'hardlink',
    ADD COLUMN IF NOT EXISTS chmod_file TEXT,
    ADD COLUMN IF NOT EXISTS chmod_dir TEXT,
    ADD COLUMN IF NOT EXISTS owner TEXT,
    ADD COLUMN IF NOT EXISTS "group" TEXT,
    ADD COLUMN IF NOT EXISTS umask TEXT;

CREATE TABLE IF NOT EXISTS auth_api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_id TEXT NOT NULL UNIQUE,
    hash TEXT NOT NULL,
    label TEXT,
    enabled BOOLEAN NOT NULL DEFAULT TRUE,
    expires_at TIMESTAMPTZ,
    rate_limit_burst INTEGER,
    rate_limit_per_seconds BIGINT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE public.auth_api_keys
    ADD COLUMN IF NOT EXISTS expires_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS rate_limit_burst INTEGER,
    ADD COLUMN IF NOT EXISTS rate_limit_per_seconds BIGINT;

CREATE INDEX IF NOT EXISTS auth_api_keys_enabled_idx ON auth_api_keys (enabled) WHERE enabled = TRUE;

CREATE TABLE IF NOT EXISTS query_presets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name TEXT NOT NULL UNIQUE,
    expression TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
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

-- Tracker configuration -----------------------------------------------------

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
    ssl_cert TEXT,
    ssl_private_key TEXT,
    ssl_ca_cert TEXT,
    ssl_tracker_verify BOOLEAN,
    auth_username_secret TEXT,
    auth_password_secret TEXT,
    auth_cookie_secret TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE public.engine_tracker_config
    ADD COLUMN IF NOT EXISTS ssl_cert TEXT,
    ADD COLUMN IF NOT EXISTS ssl_private_key TEXT,
    ADD COLUMN IF NOT EXISTS ssl_ca_cert TEXT,
    ADD COLUMN IF NOT EXISTS ssl_tracker_verify BOOLEAN,
    ADD COLUMN IF NOT EXISTS auth_username_secret TEXT,
    ADD COLUMN IF NOT EXISTS auth_password_secret TEXT,
    ADD COLUMN IF NOT EXISTS auth_cookie_secret TEXT;

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

-- Normalized settings tables ------------------------------------------------

CREATE TABLE IF NOT EXISTS public.app_profile_immutable_keys (
    profile_id UUID NOT NULL REFERENCES public.app_profile(id) ON DELETE CASCADE,
    key TEXT NOT NULL,
    ord INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_id, key)
);

CREATE UNIQUE INDEX IF NOT EXISTS app_profile_immutable_keys_order
    ON public.app_profile_immutable_keys (profile_id, ord);

CREATE TABLE IF NOT EXISTS public.app_label_policies (
    profile_id UUID NOT NULL REFERENCES public.app_profile(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('category', 'tag')),
    name TEXT NOT NULL,
    download_dir TEXT,
    rate_limit_download_bps BIGINT,
    rate_limit_upload_bps BIGINT,
    queue_position INTEGER,
    auto_managed BOOLEAN,
    seed_ratio_limit DOUBLE PRECISION,
    seed_time_limit BIGINT,
    cleanup_seed_ratio_limit DOUBLE PRECISION,
    cleanup_seed_time_limit BIGINT,
    cleanup_remove_data BOOLEAN,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_id, kind, name)
);

CREATE TABLE IF NOT EXISTS public.engine_profile_list_values (
    profile_id UUID NOT NULL REFERENCES public.engine_profile(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('listen_interfaces', 'dht_bootstrap_nodes', 'dht_router_nodes')),
    ord INTEGER NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_id, kind, ord)
);

CREATE UNIQUE INDEX IF NOT EXISTS engine_profile_list_values_dedup
    ON public.engine_profile_list_values (profile_id, kind, value);

CREATE TABLE IF NOT EXISTS public.engine_ip_filter (
    profile_id UUID PRIMARY KEY REFERENCES public.engine_profile(id) ON DELETE CASCADE,
    blocklist_url TEXT,
    etag TEXT,
    last_updated_at TIMESTAMPTZ,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS public.engine_ip_filter_entries (
    profile_id UUID NOT NULL REFERENCES public.engine_profile(id) ON DELETE CASCADE,
    ord INTEGER NOT NULL,
    cidr TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_id, ord)
);

CREATE UNIQUE INDEX IF NOT EXISTS engine_ip_filter_entries_dedup
    ON public.engine_ip_filter_entries (profile_id, cidr);

CREATE TABLE IF NOT EXISTS public.engine_alt_speed (
    profile_id UUID PRIMARY KEY REFERENCES public.engine_profile(id) ON DELETE CASCADE,
    download_bps BIGINT,
    upload_bps BIGINT,
    schedule_start_minutes INTEGER,
    schedule_end_minutes INTEGER,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS public.engine_alt_speed_days (
    profile_id UUID NOT NULL REFERENCES public.engine_profile(id) ON DELETE CASCADE,
    ord INTEGER NOT NULL,
    day TEXT NOT NULL CHECK (day IN ('mon', 'tue', 'wed', 'thu', 'fri', 'sat', 'sun')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_id, ord)
);

CREATE UNIQUE INDEX IF NOT EXISTS engine_alt_speed_days_dedup
    ON public.engine_alt_speed_days (profile_id, day);

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

CREATE TABLE IF NOT EXISTS public.fs_policy_list_values (
    policy_id UUID NOT NULL REFERENCES public.fs_policy(id) ON DELETE CASCADE,
    kind TEXT NOT NULL CHECK (kind IN ('cleanup_keep', 'cleanup_drop', 'allow_paths')),
    ord INTEGER NOT NULL,
    value TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (policy_id, kind, ord)
);

CREATE UNIQUE INDEX IF NOT EXISTS fs_policy_list_values_dedup
    ON public.fs_policy_list_values (policy_id, kind, value);

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
    comment TEXT,
    source TEXT,
    private BOOLEAN,
    added_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    completed_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

ALTER TABLE revaer_runtime.torrents
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

DROP TRIGGER IF EXISTS app_profile_touch_updated_at ON app_profile;
CREATE TRIGGER app_profile_touch_updated_at
BEFORE UPDATE ON app_profile
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS engine_profile_touch_updated_at ON engine_profile;
CREATE TRIGGER engine_profile_touch_updated_at
BEFORE UPDATE ON engine_profile
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS fs_policy_touch_updated_at ON fs_policy;
CREATE TRIGGER fs_policy_touch_updated_at
BEFORE UPDATE ON fs_policy
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS auth_api_keys_touch_updated_at ON auth_api_keys;
CREATE TRIGGER auth_api_keys_touch_updated_at
BEFORE UPDATE ON auth_api_keys
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS query_presets_touch_updated_at ON query_presets;
CREATE TRIGGER query_presets_touch_updated_at
BEFORE UPDATE ON query_presets
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS app_profile_immutable_keys_touch_updated_at ON public.app_profile_immutable_keys;
CREATE TRIGGER app_profile_immutable_keys_touch_updated_at
BEFORE UPDATE ON public.app_profile_immutable_keys
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS app_label_policies_touch_updated_at ON public.app_label_policies;
CREATE TRIGGER app_label_policies_touch_updated_at
BEFORE UPDATE ON public.app_label_policies
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS engine_profile_list_values_touch_updated_at ON public.engine_profile_list_values;
CREATE TRIGGER engine_profile_list_values_touch_updated_at
BEFORE UPDATE ON public.engine_profile_list_values
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS engine_ip_filter_touch_updated_at ON public.engine_ip_filter;
CREATE TRIGGER engine_ip_filter_touch_updated_at
BEFORE UPDATE ON public.engine_ip_filter
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS engine_ip_filter_entries_touch_updated_at ON public.engine_ip_filter_entries;
CREATE TRIGGER engine_ip_filter_entries_touch_updated_at
BEFORE UPDATE ON public.engine_ip_filter_entries
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS engine_alt_speed_touch_updated_at ON public.engine_alt_speed;
CREATE TRIGGER engine_alt_speed_touch_updated_at
BEFORE UPDATE ON public.engine_alt_speed
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS engine_alt_speed_days_touch_updated_at ON public.engine_alt_speed_days;
CREATE TRIGGER engine_alt_speed_days_touch_updated_at
BEFORE UPDATE ON public.engine_alt_speed_days
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS engine_tracker_config_touch_updated_at ON public.engine_tracker_config;
CREATE TRIGGER engine_tracker_config_touch_updated_at
BEFORE UPDATE ON public.engine_tracker_config
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS engine_tracker_endpoints_touch_updated_at ON public.engine_tracker_endpoints;
CREATE TRIGGER engine_tracker_endpoints_touch_updated_at
BEFORE UPDATE ON public.engine_tracker_endpoints
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS fs_policy_list_values_touch_updated_at ON public.fs_policy_list_values;
CREATE TRIGGER fs_policy_list_values_touch_updated_at
BEFORE UPDATE ON public.fs_policy_list_values
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS revaer_runtime_torrents_touch_updated_at ON revaer_runtime.torrents;
CREATE TRIGGER revaer_runtime_torrents_touch_updated_at
BEFORE UPDATE ON revaer_runtime.torrents
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS revaer_runtime_fs_jobs_touch_updated_at ON revaer_runtime.fs_jobs;
CREATE TRIGGER revaer_runtime_fs_jobs_touch_updated_at
BEFORE UPDATE ON revaer_runtime.fs_jobs
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS torrent_files_touch_updated_at ON revaer_runtime.torrent_files;
CREATE TRIGGER torrent_files_touch_updated_at
BEFORE UPDATE ON revaer_runtime.torrent_files
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

DROP TRIGGER IF EXISTS app_profile_bump_revision ON app_profile;
CREATE TRIGGER app_profile_bump_revision
AFTER INSERT OR UPDATE ON app_profile
FOR EACH ROW
EXECUTE FUNCTION revaer_bump_revision();

DROP TRIGGER IF EXISTS engine_profile_bump_revision ON engine_profile;
CREATE TRIGGER engine_profile_bump_revision
AFTER INSERT OR UPDATE ON engine_profile
FOR EACH ROW
EXECUTE FUNCTION revaer_bump_revision();

DROP TRIGGER IF EXISTS fs_policy_bump_revision ON fs_policy;
CREATE TRIGGER fs_policy_bump_revision
AFTER INSERT OR UPDATE ON fs_policy
FOR EACH ROW
EXECUTE FUNCTION revaer_bump_revision();

DROP TRIGGER IF EXISTS auth_api_keys_bump_revision ON auth_api_keys;
CREATE TRIGGER auth_api_keys_bump_revision
AFTER INSERT OR UPDATE OR DELETE ON auth_api_keys
FOR EACH ROW
EXECUTE FUNCTION revaer_bump_revision();

DROP TRIGGER IF EXISTS query_presets_bump_revision ON query_presets;
CREATE TRIGGER query_presets_bump_revision
AFTER INSERT OR UPDATE OR DELETE ON query_presets
FOR EACH ROW
EXECUTE FUNCTION revaer_bump_revision();

-- Config schema helpers ----------------------------------------------------

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

DROP FUNCTION IF EXISTS revaer_config.fetch_app_profile_row(UUID);
CREATE OR REPLACE FUNCTION revaer_config.fetch_app_profile_row(_id UUID)
RETURNS TABLE (
    id UUID,
    instance_name TEXT,
    mode TEXT,
    auth_mode TEXT,
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
           ap.auth_mode,
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

DROP FUNCTION IF EXISTS revaer_config.fetch_engine_profile_row(UUID);
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

DROP FUNCTION IF EXISTS revaer_config.fetch_fs_policy_row(UUID);
CREATE OR REPLACE FUNCTION revaer_config.fetch_fs_policy_row(_id UUID)
RETURNS TABLE (
    id UUID,
    library_root TEXT,
    "extract" BOOLEAN,
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
           fp."extract",
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

-- API key helpers.
DROP FUNCTION IF EXISTS revaer_config.fetch_api_keys();
DROP FUNCTION IF EXISTS revaer_config.fetch_api_key_auth(TEXT);
CREATE OR REPLACE FUNCTION revaer_config.fetch_api_keys()
RETURNS TABLE (
    key_id TEXT,
    label TEXT,
    enabled BOOLEAN,
    rate_limit_burst INTEGER,
    rate_limit_per_seconds BIGINT,
    expires_at TIMESTAMPTZ
) AS
$$
BEGIN
    RETURN QUERY
    SELECT key_id,
           label,
           enabled,
           rate_limit_burst,
           rate_limit_per_seconds,
           expires_at
    FROM public.auth_api_keys
    WHERE enabled = TRUE
      AND (expires_at IS NULL OR expires_at > now())
    ORDER BY created_at;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.fetch_api_key_auth(_key_id TEXT)
RETURNS TABLE (
    hash TEXT,
    enabled BOOLEAN,
    label TEXT,
    rate_limit_burst INTEGER,
    rate_limit_per_seconds BIGINT,
    expires_at TIMESTAMPTZ
) AS
$$
BEGIN
    RETURN QUERY
    SELECT ak.hash,
           ak.enabled,
           ak.label,
           ak.rate_limit_burst,
           ak.rate_limit_per_seconds,
           ak.expires_at
    FROM public.auth_api_keys AS ak
    WHERE ak.key_id = _key_id
      AND (ak.expires_at IS NULL OR ak.expires_at > now());
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

CREATE OR REPLACE FUNCTION revaer_config.update_api_key_expires_at(
    _key_id TEXT,
    _expires_at TIMESTAMPTZ
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.auth_api_keys
    SET expires_at = _expires_at
    WHERE key_id = _key_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.insert_api_key(
    _key_id TEXT,
    _hash TEXT,
    _label TEXT,
    _enabled BOOLEAN,
    _burst INTEGER,
    _per_seconds BIGINT,
    _expires_at TIMESTAMPTZ
) RETURNS VOID AS
$$
BEGIN
    INSERT INTO public.auth_api_keys AS ak (
        key_id,
        hash,
        label,
        enabled,
        expires_at,
        rate_limit_burst,
        rate_limit_per_seconds
    )
    VALUES (
        _key_id,
        _hash,
        _label,
        _enabled,
        _expires_at,
        _burst,
        _per_seconds
    )
    ON CONFLICT (key_id) DO UPDATE
    SET hash = EXCLUDED.hash,
        label = EXCLUDED.label,
        enabled = EXCLUDED.enabled,
        expires_at = EXCLUDED.expires_at,
        rate_limit_burst = EXCLUDED.rate_limit_burst,
        rate_limit_per_seconds = EXCLUDED.rate_limit_per_seconds;
END;
$$ LANGUAGE plpgsql;

-- App profile updates.
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

CREATE OR REPLACE FUNCTION revaer_config.update_app_auth_mode(
    _id UUID,
    _auth_mode TEXT
) RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET auth_mode = _auth_mode
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
    SET bind_addr = _bind_addr::INET
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

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

CREATE OR REPLACE FUNCTION revaer_config.bump_app_profile_version(_id UUID)
RETURNS VOID AS
$$
BEGIN
    UPDATE public.app_profile
    SET version = version + 1
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;

-- Engine profile helpers.
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
    _values TEXT[]
) RETURNS VOID AS
$$
BEGIN
    PERFORM revaer_config.set_fs_list(_id, _column, _values);
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

-- Factory reset -------------------------------------------------------------

DROP FUNCTION IF EXISTS revaer_config.factory_reset();
CREATE OR REPLACE FUNCTION revaer_config.factory_reset()
RETURNS VOID AS
$$
DECLARE
    rec RECORD;
BEGIN
    PERFORM set_config('lock_timeout', '5s', true);

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
        '.server_root/resume',
        '.server_root/downloads'
    );

    INSERT INTO public.fs_policy (id, library_root)
    VALUES (
        '00000000-0000-0000-0000-000000000003',
        '.server_root/library'
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
        ARRAY['.server_root/downloads', '.server_root/library']::TEXT[]
    );
END;
$$ LANGUAGE plpgsql;

-- Runtime procedures --------------------------------------------------------

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
    TEXT,
    TEXT,
    BOOLEAN,
    INTEGER[],
    TEXT[],
    BIGINT[],
    BIGINT[],
    TEXT[],
    BOOLEAN[],
    TIMESTAMPTZ,
    TIMESTAMPTZ,
    TIMESTAMPTZ
);

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

CREATE OR REPLACE FUNCTION revaer_runtime.delete_torrent(_torrent_id UUID)
RETURNS VOID AS
$$
BEGIN
    DELETE FROM revaer_runtime.torrents WHERE torrent_id = _torrent_id;
END;
$$ LANGUAGE plpgsql;

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

DROP FUNCTION IF EXISTS revaer_runtime.list_torrent_files(UUID);
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

-- Seed defaults -------------------------------------------------------------

INSERT INTO public.app_profile (id, mode, instance_name)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'setup',
    'revaer'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO public.engine_profile (id, implementation, resume_dir, download_root)
VALUES (
    '00000000-0000-0000-0000-000000000002',
    'libtorrent',
    '.server_root/resume',
    '.server_root/downloads'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO public.fs_policy (id, library_root)
VALUES (
    '00000000-0000-0000-0000-000000000003',
    '.server_root/library'
)
ON CONFLICT (id) DO NOTHING;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1
        FROM public.fs_policy_list_values
        WHERE policy_id = '00000000-0000-0000-0000-000000000003'
          AND kind = 'allow_paths'
    ) THEN
        PERFORM revaer_config.set_fs_list(
            '00000000-0000-0000-0000-000000000003',
            'allow_paths',
            ARRAY['.server_root/downloads', '.server_root/library']::TEXT[]
        );
    END IF;
END;
$$;

-- Drop legacy JSON columns/tables ------------------------------------------

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

ALTER TABLE revaer_runtime.torrents
    DROP COLUMN IF EXISTS payload,
    DROP COLUMN IF EXISTS files;

DROP TABLE IF EXISTS public.settings_history;
