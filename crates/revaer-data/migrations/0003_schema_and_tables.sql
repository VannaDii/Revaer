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
