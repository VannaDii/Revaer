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
        src_path = CASE
            WHEN revaer_runtime.fs_jobs.status = 'failed'::revaer_runtime.fs_status
                THEN revaer_runtime.fs_jobs.src_path
            ELSE EXCLUDED.src_path
        END,
        status = CASE
            WHEN revaer_runtime.fs_jobs.status IN (
                'moved'::revaer_runtime.fs_status,
                'failed'::revaer_runtime.fs_status
            ) THEN revaer_runtime.fs_jobs.status
            ELSE 'moving'::revaer_runtime.fs_status
        END,
        attempt = CASE
            WHEN revaer_runtime.fs_jobs.status IN (
                'moved'::revaer_runtime.fs_status,
                'failed'::revaer_runtime.fs_status
            ) THEN revaer_runtime.fs_jobs.attempt
            ELSE revaer_runtime.fs_jobs.attempt + 1
        END,
        last_error = CASE
            WHEN revaer_runtime.fs_jobs.status = 'failed'::revaer_runtime.fs_status
                THEN revaer_runtime.fs_jobs.last_error
            ELSE NULL
        END,
        updated_at = now();
END;
$$ LANGUAGE plpgsql;
