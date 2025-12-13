-- Normalize tracker storage into first-class tables instead of JSONB.

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

CREATE TRIGGER engine_tracker_config_touch_updated_at
BEFORE UPDATE ON public.engine_tracker_config
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER engine_tracker_endpoints_touch_updated_at
BEFORE UPDATE ON public.engine_tracker_endpoints
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

-- Normalise and persist tracker configuration into the new tables.
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

-- Render the tracker payload from the normalized tables for API consumption.
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

-- Rehydrate existing tracker JSON into the new tables before dropping the column.
DO
$$
DECLARE
    existing JSONB;
    profile_id UUID;
BEGIN
    SELECT id, tracker INTO profile_id, existing FROM public.engine_profile LIMIT 1;
    IF profile_id IS NOT NULL THEN
        PERFORM revaer_config.persist_tracker_config(profile_id, existing);
    END IF;
END;
$$;

ALTER TABLE public.engine_profile DROP COLUMN IF EXISTS tracker;

-- Re-expose tracker payloads via normalized tables.
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
           revaer_config.render_tracker_config(ep.id)
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
        'tracker', revaer_config.render_tracker_config(ep.id)
    )
    INTO body
    FROM public.engine_profile AS ep
    WHERE ep.id = _id;
    RETURN body;
END;
$$ LANGUAGE plpgsql STABLE;

-- Update engine profile writes to hydrate normalized tracker tables.
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
        download_root = _download_root
    WHERE id = _id;

    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
END;
$$ LANGUAGE plpgsql;
