-- Centralised engine profile update to prevent per-field drift.

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
        tracker = COALESCE(_tracker, '{}'::jsonb)
    WHERE id = _id;
END;
$$ LANGUAGE plpgsql;
