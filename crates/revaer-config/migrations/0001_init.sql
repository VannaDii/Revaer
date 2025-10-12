CREATE EXTENSION IF NOT EXISTS pgcrypto;

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
    sequential_default BOOLEAN NOT NULL DEFAULT TRUE,
    resume_dir TEXT NOT NULL,
    download_root TEXT NOT NULL,
    tracker JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
    ,
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
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
    ,
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
    hash BYTEA NOT NULL,
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
    token_hash BYTEA NOT NULL,
    issued_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    consumed_at TIMESTAMPTZ,
    issued_by TEXT,
);

CREATE UNIQUE INDEX IF NOT EXISTS setup_tokens_active_unique
    ON setup_tokens ((TRUE))
    WHERE consumed_at IS NULL;

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
    new_revision BIGINT;
    payload TEXT;
BEGIN
    UPDATE settings_revision
    SET revision = revision + 1, updated_at = now()
    WHERE id = 1
    RETURNING revision INTO new_revision;

    payload := format('%s:%s:%s', TG_TABLE_NAME, new_revision, TG_OP);
    PERFORM pg_notify('revaer_settings_changed', payload);

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    ELSE
        RETURN NEW;
    END IF;
END;
$$ LANGUAGE plpgsql;

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
