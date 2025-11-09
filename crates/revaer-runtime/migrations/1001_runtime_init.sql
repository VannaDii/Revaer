CREATE SCHEMA IF NOT EXISTS revaer_runtime;

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

CREATE OR REPLACE FUNCTION revaer_touch_updated_at()
RETURNS TRIGGER AS $$
BEGIN
    NEW.updated_at = now();
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER revaer_runtime_torrents_touch_updated_at
BEFORE UPDATE ON revaer_runtime.torrents
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER revaer_runtime_fs_jobs_touch_updated_at
BEFORE UPDATE ON revaer_runtime.fs_jobs
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();
