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
