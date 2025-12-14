-- Multiple listen interfaces and IPv6 policy for engine profiles.

ALTER TABLE public.engine_profile
    ADD COLUMN IF NOT EXISTS listen_interfaces JSONB NOT NULL DEFAULT '[]'::jsonb,
    ADD COLUMN IF NOT EXISTS ipv6_mode TEXT NOT NULL DEFAULT 'disabled';

-- Drop older projections so return signatures can expand.
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
    TEXT
);

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
    tracker JSONB,
    enable_lsd BOOLEAN,
    enable_upnp BOOLEAN,
    enable_natpmp BOOLEAN,
    enable_pex BOOLEAN,
    dht_bootstrap_nodes JSONB,
    dht_router_nodes JSONB,
    ip_filter JSONB,
    listen_interfaces JSONB,
    ipv6_mode TEXT
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
           revaer_config.render_tracker_config(ep.id),
           ep.enable_lsd,
           ep.enable_upnp,
           ep.enable_natpmp,
           ep.enable_pex,
           ep.dht_bootstrap_nodes,
           ep.dht_router_nodes,
           ep.ip_filter,
           ep.listen_interfaces,
           ep.ipv6_mode
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
        'ipv6_mode', ep.ipv6_mode
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
    _ipv6_mode TEXT
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
        enable_lsd = _lsd,
        enable_upnp = _upnp,
        enable_natpmp = _natpmp,
        enable_pex = _pex,
        dht_bootstrap_nodes = _dht_bootstrap_nodes,
        dht_router_nodes = _dht_router_nodes,
        ip_filter = _ip_filter,
        listen_interfaces = _listen_interfaces,
        ipv6_mode = _ipv6_mode
    WHERE id = _id;

    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
END;
$$ LANGUAGE plpgsql;
