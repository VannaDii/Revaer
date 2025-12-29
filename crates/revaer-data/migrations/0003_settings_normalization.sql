-- BEGIN 0003_settings_normalization.sql
-- Normalize JSON settings into relational tables and refresh stored procedures.

-- App profile: telemetry columns and normalized tables.
ALTER TABLE public.app_profile
    ADD COLUMN IF NOT EXISTS telemetry_level TEXT,
    ADD COLUMN IF NOT EXISTS telemetry_format TEXT,
    ADD COLUMN IF NOT EXISTS telemetry_otel_enabled BOOLEAN,
    ADD COLUMN IF NOT EXISTS telemetry_otel_service_name TEXT,
    ADD COLUMN IF NOT EXISTS telemetry_otel_endpoint TEXT;

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

CREATE TRIGGER app_profile_immutable_keys_touch_updated_at
BEFORE UPDATE ON public.app_profile_immutable_keys
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER app_label_policies_touch_updated_at
BEFORE UPDATE ON public.app_label_policies
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

-- Engine profile: normalize lists, alt speed, and IP filter.
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

CREATE TRIGGER engine_profile_list_values_touch_updated_at
BEFORE UPDATE ON public.engine_profile_list_values
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER engine_ip_filter_touch_updated_at
BEFORE UPDATE ON public.engine_ip_filter
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER engine_ip_filter_entries_touch_updated_at
BEFORE UPDATE ON public.engine_ip_filter_entries
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER engine_alt_speed_touch_updated_at
BEFORE UPDATE ON public.engine_alt_speed
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE TRIGGER engine_alt_speed_days_touch_updated_at
BEFORE UPDATE ON public.engine_alt_speed_days
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

-- Filesystem policy: normalize list fields.
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

CREATE TRIGGER fs_policy_list_values_touch_updated_at
BEFORE UPDATE ON public.fs_policy_list_values
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

-- API keys: normalize rate limits.
ALTER TABLE public.auth_api_keys
    ADD COLUMN IF NOT EXISTS rate_limit_burst INTEGER,
    ADD COLUMN IF NOT EXISTS rate_limit_per_seconds BIGINT;

-- Helper functions for normalized settings.
CREATE OR REPLACE FUNCTION revaer_config.persist_app_immutable_keys(
    _profile_id UUID,
    _immutable JSONB
) RETURNS VOID AS
$$
BEGIN
    DELETE FROM public.app_profile_immutable_keys WHERE profile_id = _profile_id;

    IF _immutable IS NULL OR _immutable = 'null'::jsonb THEN
        RETURN;
    END IF;
    IF jsonb_typeof(_immutable) <> 'array' THEN
        RAISE EXCEPTION 'app_profile.immutable_keys must be an array';
    END IF;

    INSERT INTO public.app_profile_immutable_keys (profile_id, key, ord)
    SELECT _profile_id,
           btrim(value),
           ord
    FROM jsonb_array_elements_text(_immutable) WITH ORDINALITY AS t(value, ord)
    WHERE btrim(value) <> '';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.render_app_immutable_keys(
    _profile_id UUID
) RETURNS JSONB AS
$$
DECLARE
    payload JSONB;
BEGIN
    SELECT COALESCE(jsonb_agg(key ORDER BY ord), '[]'::jsonb)
    INTO payload
    FROM public.app_profile_immutable_keys
    WHERE profile_id = _profile_id;
    RETURN payload;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.persist_app_telemetry(
    _profile_id UUID,
    _telemetry JSONB
) RETURNS VOID AS
$$
DECLARE
    payload JSONB := COALESCE(_telemetry, '{}'::jsonb);
BEGIN
    IF jsonb_typeof(payload) <> 'object' THEN
        RAISE EXCEPTION 'app_profile.telemetry must be an object';
    END IF;

    UPDATE public.app_profile
    SET telemetry_level = NULLIF(payload->>'level', ''),
        telemetry_format = NULLIF(payload->>'format', ''),
        telemetry_otel_enabled = CASE
            WHEN jsonb_typeof(payload->'otel_enabled') = 'boolean'
                THEN (payload->>'otel_enabled')::BOOLEAN
            ELSE NULL
        END,
        telemetry_otel_service_name = NULLIF(payload->>'otel_service_name', ''),
        telemetry_otel_endpoint = NULLIF(payload->>'otel_endpoint', '')
    WHERE id = _profile_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.render_app_telemetry(
    _profile_id UUID
) RETURNS JSONB AS
$$
DECLARE
    payload JSONB;
BEGIN
    SELECT jsonb_strip_nulls(jsonb_build_object(
        'level', telemetry_level,
        'format', telemetry_format,
        'otel_enabled', telemetry_otel_enabled,
        'otel_service_name', telemetry_otel_service_name,
        'otel_endpoint', telemetry_otel_endpoint
    ))
    INTO payload
    FROM public.app_profile
    WHERE id = _profile_id;

    RETURN COALESCE(payload, '{}'::jsonb);
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.persist_app_features(
    _profile_id UUID,
    _features JSONB
) RETURNS VOID AS
$$
DECLARE
    payload JSONB := COALESCE(_features, '{}'::jsonb);
    categories JSONB;
    tags JSONB;
BEGIN
    DELETE FROM public.app_label_policies WHERE profile_id = _profile_id;

    IF payload IS NULL OR payload = 'null'::jsonb THEN
        RETURN;
    END IF;
    IF jsonb_typeof(payload) <> 'object' THEN
        RAISE EXCEPTION 'app_profile.features must be an object';
    END IF;

    categories := COALESCE(payload->'torrent_categories', '{}'::jsonb);
    tags := COALESCE(payload->'torrent_tags', '{}'::jsonb);

    IF jsonb_typeof(categories) <> 'object' THEN
        RAISE EXCEPTION 'app_profile.features.torrent_categories must be an object';
    END IF;
    IF jsonb_typeof(tags) <> 'object' THEN
        RAISE EXCEPTION 'app_profile.features.torrent_tags must be an object';
    END IF;

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
           'category',
           entry.key,
           NULLIF(entry.value->>'download_dir', ''),
           CASE
               WHEN jsonb_typeof(entry.value->'rate_limit'->'download_bps') = 'number'
                   THEN (entry.value->'rate_limit'->>'download_bps')::BIGINT
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'rate_limit'->'upload_bps') = 'number'
                   THEN (entry.value->'rate_limit'->>'upload_bps')::BIGINT
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'queue_position') = 'number'
                   THEN (entry.value->>'queue_position')::INTEGER
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'auto_managed') = 'boolean'
                   THEN (entry.value->>'auto_managed')::BOOLEAN
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'seed_ratio_limit') = 'number'
                   THEN (entry.value->>'seed_ratio_limit')::DOUBLE PRECISION
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'seed_time_limit') = 'number'
                   THEN (entry.value->>'seed_time_limit')::BIGINT
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'cleanup'->'seed_ratio_limit') = 'number'
                   THEN (entry.value->'cleanup'->>'seed_ratio_limit')::DOUBLE PRECISION
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'cleanup'->'seed_time_limit') = 'number'
                   THEN (entry.value->'cleanup'->>'seed_time_limit')::BIGINT
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'cleanup'->'remove_data') = 'boolean'
                   THEN (entry.value->'cleanup'->>'remove_data')::BOOLEAN
               ELSE NULL
           END
    FROM jsonb_each(categories) AS entry(key, value)
    WHERE entry.key <> '';

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
           'tag',
           entry.key,
           NULLIF(entry.value->>'download_dir', ''),
           CASE
               WHEN jsonb_typeof(entry.value->'rate_limit'->'download_bps') = 'number'
                   THEN (entry.value->'rate_limit'->>'download_bps')::BIGINT
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'rate_limit'->'upload_bps') = 'number'
                   THEN (entry.value->'rate_limit'->>'upload_bps')::BIGINT
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'queue_position') = 'number'
                   THEN (entry.value->>'queue_position')::INTEGER
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'auto_managed') = 'boolean'
                   THEN (entry.value->>'auto_managed')::BOOLEAN
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'seed_ratio_limit') = 'number'
                   THEN (entry.value->>'seed_ratio_limit')::DOUBLE PRECISION
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'seed_time_limit') = 'number'
                   THEN (entry.value->>'seed_time_limit')::BIGINT
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'cleanup'->'seed_ratio_limit') = 'number'
                   THEN (entry.value->'cleanup'->>'seed_ratio_limit')::DOUBLE PRECISION
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'cleanup'->'seed_time_limit') = 'number'
                   THEN (entry.value->'cleanup'->>'seed_time_limit')::BIGINT
               ELSE NULL
           END,
           CASE
               WHEN jsonb_typeof(entry.value->'cleanup'->'remove_data') = 'boolean'
                   THEN (entry.value->'cleanup'->>'remove_data')::BOOLEAN
               ELSE NULL
           END
    FROM jsonb_each(tags) AS entry(key, value)
    WHERE entry.key <> '';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.render_app_features(
    _profile_id UUID
) RETURNS JSONB AS
$$
DECLARE
    categories JSONB;
    tags JSONB;
BEGIN
    SELECT COALESCE(
        jsonb_object_agg(name, policy_json),
        '{}'::jsonb
    )
    INTO categories
    FROM (
        SELECT name,
            jsonb_strip_nulls(
                jsonb_build_object(
                    'download_dir', download_dir,
                    'queue_position', queue_position,
                    'auto_managed', auto_managed,
                    'seed_ratio_limit', seed_ratio_limit,
                    'seed_time_limit', seed_time_limit
                )
                || CASE
                    WHEN rate_limit_download_bps IS NULL AND rate_limit_upload_bps IS NULL THEN '{}'::jsonb
                    ELSE jsonb_build_object(
                        'rate_limit',
                        jsonb_strip_nulls(jsonb_build_object(
                            'download_bps', rate_limit_download_bps,
                            'upload_bps', rate_limit_upload_bps
                        ))
                    )
                END
                || CASE
                    WHEN cleanup_seed_ratio_limit IS NULL
                        AND cleanup_seed_time_limit IS NULL
                        AND cleanup_remove_data IS NULL THEN '{}'::jsonb
                    ELSE jsonb_build_object(
                        'cleanup',
                        jsonb_strip_nulls(jsonb_build_object(
                            'seed_ratio_limit', cleanup_seed_ratio_limit,
                            'seed_time_limit', cleanup_seed_time_limit,
                            'remove_data', cleanup_remove_data
                        ))
                    )
                END
            ) AS policy_json
        FROM public.app_label_policies
        WHERE profile_id = _profile_id
          AND kind = 'category'
    ) AS entries;

    SELECT COALESCE(
        jsonb_object_agg(name, policy_json),
        '{}'::jsonb
    )
    INTO tags
    FROM (
        SELECT name,
            jsonb_strip_nulls(
                jsonb_build_object(
                    'download_dir', download_dir,
                    'queue_position', queue_position,
                    'auto_managed', auto_managed,
                    'seed_ratio_limit', seed_ratio_limit,
                    'seed_time_limit', seed_time_limit
                )
                || CASE
                    WHEN rate_limit_download_bps IS NULL AND rate_limit_upload_bps IS NULL THEN '{}'::jsonb
                    ELSE jsonb_build_object(
                        'rate_limit',
                        jsonb_strip_nulls(jsonb_build_object(
                            'download_bps', rate_limit_download_bps,
                            'upload_bps', rate_limit_upload_bps
                        ))
                    )
                END
                || CASE
                    WHEN cleanup_seed_ratio_limit IS NULL
                        AND cleanup_seed_time_limit IS NULL
                        AND cleanup_remove_data IS NULL THEN '{}'::jsonb
                    ELSE jsonb_build_object(
                        'cleanup',
                        jsonb_strip_nulls(jsonb_build_object(
                            'seed_ratio_limit', cleanup_seed_ratio_limit,
                            'seed_time_limit', cleanup_seed_time_limit,
                            'remove_data', cleanup_remove_data
                        ))
                    )
                END
            ) AS policy_json
        FROM public.app_label_policies
        WHERE profile_id = _profile_id
          AND kind = 'tag'
    ) AS entries;

    RETURN jsonb_build_object(
        'torrent_categories', COALESCE(categories, '{}'::jsonb),
        'torrent_tags', COALESCE(tags, '{}'::jsonb)
    );
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.persist_engine_list(
    _profile_id UUID,
    _kind TEXT,
    _items JSONB
) RETURNS VOID AS
$$
BEGIN
    DELETE FROM public.engine_profile_list_values
    WHERE profile_id = _profile_id
      AND kind = _kind;

    IF _items IS NULL OR _items = 'null'::jsonb THEN
        RETURN;
    END IF;
    IF jsonb_typeof(_items) <> 'array' THEN
        RAISE EXCEPTION 'engine_profile.% must be an array', _kind;
    END IF;

    INSERT INTO public.engine_profile_list_values (profile_id, kind, ord, value)
    SELECT _profile_id,
           _kind,
           ord,
           btrim(value)
    FROM jsonb_array_elements_text(_items) WITH ORDINALITY AS t(value, ord)
    WHERE btrim(value) <> '';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.render_engine_list(
    _profile_id UUID,
    _kind TEXT
) RETURNS JSONB AS
$$
DECLARE
    payload JSONB;
BEGIN
    SELECT COALESCE(jsonb_agg(value ORDER BY ord), '[]'::jsonb)
    INTO payload
    FROM public.engine_profile_list_values
    WHERE profile_id = _profile_id
      AND kind = _kind;
    RETURN payload;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.persist_ip_filter_config(
    _profile_id UUID,
    _ip_filter JSONB
) RETURNS VOID AS
$$
DECLARE
    payload JSONB := COALESCE(_ip_filter, '{}'::jsonb);
    cidrs JSONB;
BEGIN
    DELETE FROM public.engine_ip_filter_entries WHERE profile_id = _profile_id;

    IF payload IS NULL OR payload = 'null'::jsonb THEN
        DELETE FROM public.engine_ip_filter WHERE profile_id = _profile_id;
        RETURN;
    END IF;
    IF jsonb_typeof(payload) <> 'object' THEN
        RAISE EXCEPTION 'engine_profile.ip_filter must be an object';
    END IF;

    cidrs := COALESCE(payload->'cidrs', '[]'::jsonb);
    IF jsonb_typeof(cidrs) <> 'array' THEN
        RAISE EXCEPTION 'engine_profile.ip_filter.cidrs must be an array';
    END IF;

    INSERT INTO public.engine_ip_filter AS cfg (
        profile_id,
        blocklist_url,
        etag,
        last_updated_at,
        last_error
    )
    VALUES (
        _profile_id,
        NULLIF(payload->>'blocklist_url', ''),
        NULLIF(payload->>'etag', ''),
        CASE
            WHEN jsonb_typeof(payload->'last_updated_at') = 'string'
                THEN (payload->>'last_updated_at')::TIMESTAMPTZ
            ELSE NULL
        END,
        NULLIF(payload->>'last_error', '')
    )
    ON CONFLICT (profile_id) DO UPDATE
    SET blocklist_url = EXCLUDED.blocklist_url,
        etag = EXCLUDED.etag,
        last_updated_at = EXCLUDED.last_updated_at,
        last_error = EXCLUDED.last_error;

    INSERT INTO public.engine_ip_filter_entries (profile_id, ord, cidr)
    SELECT _profile_id,
           ord,
           btrim(value)
    FROM jsonb_array_elements_text(cidrs) WITH ORDINALITY AS t(value, ord)
    WHERE btrim(value) <> '';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.render_ip_filter_config(
    _profile_id UUID
) RETURNS JSONB AS
$$
DECLARE
    entries JSONB;
    payload JSONB;
BEGIN
    SELECT COALESCE(jsonb_agg(cidr ORDER BY ord), '[]'::jsonb)
    INTO entries
    FROM public.engine_ip_filter_entries
    WHERE profile_id = _profile_id;

    SELECT jsonb_strip_nulls(jsonb_build_object(
        'cidrs', COALESCE(entries, '[]'::jsonb),
        'blocklist_url', cfg.blocklist_url,
        'etag', cfg.etag,
        'last_updated_at', cfg.last_updated_at,
        'last_error', cfg.last_error
    ))
    INTO payload
    FROM public.engine_ip_filter AS cfg
    WHERE cfg.profile_id = _profile_id;

    RETURN COALESCE(payload, jsonb_build_object('cidrs', COALESCE(entries, '[]'::jsonb)));
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.persist_alt_speed_config(
    _profile_id UUID,
    _alt_speed JSONB
) RETURNS VOID AS
$$
DECLARE
    normalized JSONB := revaer_config.normalize_alt_speed(_alt_speed);
    schedule JSONB;
BEGIN
    DELETE FROM public.engine_alt_speed_days WHERE profile_id = _profile_id;

    IF normalized IS NULL OR normalized = '{}'::jsonb THEN
        DELETE FROM public.engine_alt_speed WHERE profile_id = _profile_id;
        RETURN;
    END IF;

    INSERT INTO public.engine_alt_speed AS cfg (
        profile_id,
        download_bps,
        upload_bps,
        schedule_start_minutes,
        schedule_end_minutes
    )
    VALUES (
        _profile_id,
        CASE
            WHEN jsonb_typeof(normalized->'download_bps') = 'number'
                THEN (normalized->>'download_bps')::BIGINT
            ELSE NULL
        END,
        CASE
            WHEN jsonb_typeof(normalized->'upload_bps') = 'number'
                THEN (normalized->>'upload_bps')::BIGINT
            ELSE NULL
        END,
        CASE
            WHEN jsonb_typeof(normalized->'schedule'->'start') = 'string'
                THEN revaer_config.parse_alt_speed_minutes(normalized->'schedule'->>'start')
            ELSE NULL
        END,
        CASE
            WHEN jsonb_typeof(normalized->'schedule'->'end') = 'string'
                THEN revaer_config.parse_alt_speed_minutes(normalized->'schedule'->>'end')
            ELSE NULL
        END
    )
    ON CONFLICT (profile_id) DO UPDATE
    SET download_bps = EXCLUDED.download_bps,
        upload_bps = EXCLUDED.upload_bps,
        schedule_start_minutes = EXCLUDED.schedule_start_minutes,
        schedule_end_minutes = EXCLUDED.schedule_end_minutes;

    schedule := normalized->'schedule';
    IF schedule IS NULL OR schedule = 'null'::jsonb THEN
        RETURN;
    END IF;

    INSERT INTO public.engine_alt_speed_days (profile_id, ord, day)
    SELECT _profile_id,
           ord,
           value
    FROM jsonb_array_elements_text(schedule->'days') WITH ORDINALITY AS t(value, ord)
    WHERE value IN ('mon','tue','wed','thu','fri','sat','sun');
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.render_alt_speed_config(
    _profile_id UUID
) RETURNS JSONB AS
$$
DECLARE
    days JSONB;
    payload JSONB;
    schedule JSONB;
BEGIN
    SELECT COALESCE(jsonb_agg(day ORDER BY ord), '[]'::jsonb)
    INTO days
    FROM public.engine_alt_speed_days
    WHERE profile_id = _profile_id;

    SELECT jsonb_strip_nulls(jsonb_build_object(
        'download_bps', cfg.download_bps,
        'upload_bps', cfg.upload_bps
    ))
    INTO payload
    FROM public.engine_alt_speed AS cfg
    WHERE cfg.profile_id = _profile_id;

    IF payload IS NULL OR payload = '{}'::jsonb THEN
        RETURN '{}'::jsonb;
    END IF;

    IF days IS NULL OR days = '[]'::jsonb THEN
        RETURN payload;
    END IF;

    SELECT jsonb_strip_nulls(jsonb_build_object(
        'days', days,
        'start', revaer_config.format_alt_speed_minutes(cfg.schedule_start_minutes),
        'end', revaer_config.format_alt_speed_minutes(cfg.schedule_end_minutes)
    ))
    INTO schedule
    FROM public.engine_alt_speed AS cfg
    WHERE cfg.profile_id = _profile_id;

    IF schedule IS NULL OR schedule = '{}'::jsonb THEN
        RETURN payload;
    END IF;

    RETURN jsonb_strip_nulls(payload || jsonb_build_object('schedule', schedule));
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.persist_fs_list(
    _policy_id UUID,
    _kind TEXT,
    _items JSONB
) RETURNS VOID AS
$$
BEGIN
    DELETE FROM public.fs_policy_list_values
    WHERE policy_id = _policy_id
      AND kind = _kind;

    IF _items IS NULL OR _items = 'null'::jsonb THEN
        RETURN;
    END IF;
    IF jsonb_typeof(_items) <> 'array' THEN
        RAISE EXCEPTION 'fs_policy.% must be an array', _kind;
    END IF;

    INSERT INTO public.fs_policy_list_values (policy_id, kind, ord, value)
    SELECT _policy_id,
           _kind,
           ord,
           btrim(value)
    FROM jsonb_array_elements_text(_items) WITH ORDINALITY AS t(value, ord)
    WHERE btrim(value) <> '';
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.render_fs_list(
    _policy_id UUID,
    _kind TEXT
) RETURNS JSONB AS
$$
DECLARE
    payload JSONB;
BEGIN
    SELECT COALESCE(jsonb_agg(value ORDER BY ord), '[]'::jsonb)
    INTO payload
    FROM public.fs_policy_list_values
    WHERE policy_id = _policy_id
      AND kind = _kind;
    RETURN payload;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.render_api_key_rate_limit(
    _burst INTEGER,
    _per_seconds BIGINT
) RETURNS JSONB AS
$$
BEGIN
    IF _burst IS NULL OR _per_seconds IS NULL THEN
        RETURN '{}'::jsonb;
    END IF;
    RETURN jsonb_build_object(
        'burst', _burst,
        'per_seconds', _per_seconds
    );
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Migrate existing JSON data into normalized tables.
UPDATE public.app_profile
SET telemetry_level = CASE
        WHEN jsonb_typeof(telemetry->'level') = 'string' THEN NULLIF(telemetry->>'level', '')
        ELSE telemetry_level
    END,
    telemetry_format = CASE
        WHEN jsonb_typeof(telemetry->'format') = 'string' THEN NULLIF(telemetry->>'format', '')
        ELSE telemetry_format
    END,
    telemetry_otel_enabled = CASE
        WHEN jsonb_typeof(telemetry->'otel_enabled') = 'boolean'
            THEN (telemetry->>'otel_enabled')::BOOLEAN
        ELSE telemetry_otel_enabled
    END,
    telemetry_otel_service_name = CASE
        WHEN jsonb_typeof(telemetry->'otel_service_name') = 'string'
            THEN NULLIF(telemetry->>'otel_service_name', '')
        ELSE telemetry_otel_service_name
    END,
    telemetry_otel_endpoint = CASE
        WHEN jsonb_typeof(telemetry->'otel_endpoint') = 'string'
            THEN NULLIF(telemetry->>'otel_endpoint', '')
        ELSE telemetry_otel_endpoint
    END;

DO $$
DECLARE
    ap RECORD;
    ep RECORD;
    fp RECORD;
BEGIN
    FOR ap IN SELECT id, features, immutable_keys FROM public.app_profile
    LOOP
        PERFORM revaer_config.persist_app_immutable_keys(ap.id, ap.immutable_keys);
        PERFORM revaer_config.persist_app_features(ap.id, ap.features);
    END LOOP;

    FOR ep IN SELECT id, listen_interfaces, dht_bootstrap_nodes, dht_router_nodes, ip_filter, alt_speed FROM public.engine_profile
    LOOP
        PERFORM revaer_config.persist_engine_list(ep.id, 'listen_interfaces', ep.listen_interfaces);
        PERFORM revaer_config.persist_engine_list(ep.id, 'dht_bootstrap_nodes', ep.dht_bootstrap_nodes);
        PERFORM revaer_config.persist_engine_list(ep.id, 'dht_router_nodes', ep.dht_router_nodes);
        PERFORM revaer_config.persist_ip_filter_config(ep.id, ep.ip_filter);
        PERFORM revaer_config.persist_alt_speed_config(ep.id, ep.alt_speed);
    END LOOP;

    FOR fp IN SELECT id, cleanup_keep, cleanup_drop, allow_paths FROM public.fs_policy
    LOOP
        PERFORM revaer_config.persist_fs_list(fp.id, 'cleanup_keep', fp.cleanup_keep);
        PERFORM revaer_config.persist_fs_list(fp.id, 'cleanup_drop', fp.cleanup_drop);
        PERFORM revaer_config.persist_fs_list(fp.id, 'allow_paths', fp.allow_paths);
    END LOOP;
END $$;

UPDATE public.auth_api_keys
SET rate_limit_burst = CASE
        WHEN jsonb_typeof(rate_limit->'burst') = 'number'
            THEN (rate_limit->>'burst')::INTEGER
        ELSE rate_limit_burst
    END,
    rate_limit_per_seconds = CASE
        WHEN jsonb_typeof(rate_limit->'per_seconds') = 'number'
            THEN (rate_limit->>'per_seconds')::BIGINT
        ELSE rate_limit_per_seconds
    END;

-- Drop legacy JSON columns.
ALTER TABLE public.app_profile
    DROP COLUMN IF EXISTS telemetry,
    DROP COLUMN IF EXISTS features,
    DROP COLUMN IF EXISTS immutable_keys;

ALTER TABLE public.engine_profile
    DROP COLUMN IF EXISTS listen_interfaces,
    DROP COLUMN IF EXISTS dht_bootstrap_nodes,
    DROP COLUMN IF EXISTS dht_router_nodes,
    DROP COLUMN IF EXISTS ip_filter,
    DROP COLUMN IF EXISTS alt_speed;

ALTER TABLE public.fs_policy
    DROP COLUMN IF EXISTS cleanup_keep,
    DROP COLUMN IF EXISTS cleanup_drop,
    DROP COLUMN IF EXISTS allow_paths;

ALTER TABLE public.auth_api_keys
    DROP COLUMN IF EXISTS rate_limit;

-- Refresh stored procedures to use normalized tables.
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
           revaer_config.render_app_telemetry(ap.id),
           revaer_config.render_app_features(ap.id),
           revaer_config.render_app_immutable_keys(ap.id)
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
           revaer_config.render_fs_list(fp.id, 'cleanup_keep'),
           revaer_config.render_fs_list(fp.id, 'cleanup_drop'),
           fp.chmod_file,
           fp.chmod_dir,
           fp.owner,
           fp."group",
           fp.umask,
           revaer_config.render_fs_list(fp.id, 'allow_paths')
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
    SELECT jsonb_build_object(
        'id', ap.id,
        'instance_name', ap.instance_name,
        'mode', ap.mode,
        'version', ap.version,
        'http_port', ap.http_port,
        'bind_addr', ap.bind_addr::text,
        'telemetry', revaer_config.render_app_telemetry(ap.id),
        'features', revaer_config.render_app_features(ap.id),
        'immutable_keys', revaer_config.render_app_immutable_keys(ap.id)
    )
    INTO body
    FROM public.app_profile AS ap
    WHERE ap.id = _id;
    RETURN body;
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
           revaer_config.render_engine_list(ep.id, 'dht_bootstrap_nodes'),
           revaer_config.render_engine_list(ep.id, 'dht_router_nodes'),
           revaer_config.render_ip_filter_config(ep.id),
           revaer_config.render_engine_list(ep.id, 'listen_interfaces'),
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
           revaer_config.render_alt_speed_config(ep.id),
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
        'dht_bootstrap_nodes', revaer_config.render_engine_list(ep.id, 'dht_bootstrap_nodes'),
        'dht_router_nodes', revaer_config.render_engine_list(ep.id, 'dht_router_nodes'),
        'ip_filter', revaer_config.render_ip_filter_config(ep.id),
        'listen_interfaces', revaer_config.render_engine_list(ep.id, 'listen_interfaces'),
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
        'alt_speed', revaer_config.render_alt_speed_config(ep.id),
        'stats_interval_ms', ep.stats_interval_ms,
        'peer_classes', revaer_config.render_peer_classes(ep.id)
    )
    INTO body
    FROM public.engine_profile AS ep
    WHERE ep.id = _id;
    RETURN body;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.fetch_fs_policy_json(_id UUID)
RETURNS JSONB AS
$$
DECLARE
    body JSONB;
BEGIN
    SELECT jsonb_build_object(
        'id', fp.id,
        'library_root', fp.library_root,
        'extract', fp.extract,
        'par2', fp.par2,
        'flatten', fp.flatten,
        'move_mode', fp.move_mode,
        'cleanup_keep', revaer_config.render_fs_list(fp.id, 'cleanup_keep'),
        'cleanup_drop', revaer_config.render_fs_list(fp.id, 'cleanup_drop'),
        'chmod_file', fp.chmod_file,
        'chmod_dir', fp.chmod_dir,
        'owner', fp.owner,
        'group', fp."group",
        'umask', fp.umask,
        'allow_paths', revaer_config.render_fs_list(fp.id, 'allow_paths')
    )
    INTO body
    FROM public.fs_policy AS fp
    WHERE fp.id = _id;
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
            jsonb_build_object(
                'key_id', ak.key_id,
                'label', ak.label,
                'enabled', ak.enabled,
                'rate_limit', revaer_config.render_api_key_rate_limit(ak.rate_limit_burst, ak.rate_limit_per_seconds)
            )
            ORDER BY ak.key_id
        ),
        '[]'::jsonb
    )
    INTO payload
    FROM public.auth_api_keys AS ak;
    RETURN payload;
END;
$$ LANGUAGE plpgsql STABLE;

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
    SELECT ak.hash,
           ak.enabled,
           ak.label,
           revaer_config.render_api_key_rate_limit(ak.rate_limit_burst, ak.rate_limit_per_seconds)
    FROM public.auth_api_keys AS ak
    WHERE ak.key_id = _key_id;
END;
$$ LANGUAGE plpgsql STABLE;

CREATE OR REPLACE FUNCTION revaer_config.update_app_telemetry(
    _id UUID,
    _telemetry JSONB
) RETURNS VOID AS
$$
BEGIN
    PERFORM revaer_config.persist_app_telemetry(_id, _telemetry);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_app_features(
    _id UUID,
    _features JSONB
) RETURNS VOID AS
$$
BEGIN
    PERFORM revaer_config.persist_app_features(_id, _features);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_app_immutable_keys(
    _id UUID,
    _immutable JSONB
) RETURNS VOID AS
$$
BEGIN
    PERFORM revaer_config.persist_app_immutable_keys(_id, _immutable);
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
        stats_interval_ms = _stats_interval_ms,
        updated_at = now()
    WHERE id = _id;

    PERFORM revaer_config.persist_tracker_config(_id, _tracker);
    PERFORM revaer_config.persist_peer_class_config(_id, _peer_classes);
    PERFORM revaer_config.persist_engine_list(_id, 'listen_interfaces', _listen_interfaces);
    PERFORM revaer_config.persist_engine_list(_id, 'dht_bootstrap_nodes', _dht_bootstrap_nodes);
    PERFORM revaer_config.persist_engine_list(_id, 'dht_router_nodes', _dht_router_nodes);
    PERFORM revaer_config.persist_ip_filter_config(_id, _ip_filter);
    PERFORM revaer_config.persist_alt_speed_config(_id, _alt_speed);
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
        RAISE EXCEPTION 'fs_policy array field % is not supported', _column;
    END IF;

    PERFORM revaer_config.persist_fs_list(_id, _column, _value);
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.update_api_key_rate_limit(
    _key_id TEXT,
    _rate_limit JSONB
) RETURNS VOID AS
$$
DECLARE
    burst INTEGER;
    per_seconds BIGINT;
BEGIN
    IF _rate_limit IS NULL OR _rate_limit = 'null'::jsonb THEN
        burst := NULL;
        per_seconds := NULL;
    ELSIF jsonb_typeof(_rate_limit) = 'object' THEN
        IF jsonb_typeof(_rate_limit->'burst') = 'number' THEN
            burst := (_rate_limit->>'burst')::INTEGER;
        END IF;
        IF jsonb_typeof(_rate_limit->'per_seconds') = 'number' THEN
            per_seconds := (_rate_limit->>'per_seconds')::BIGINT;
        END IF;
    ELSE
        burst := NULL;
        per_seconds := NULL;
    END IF;

    UPDATE public.auth_api_keys
    SET rate_limit_burst = burst,
        rate_limit_per_seconds = per_seconds
    WHERE key_id = _key_id;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.insert_api_key(
    _key_id TEXT,
    _hash TEXT,
    _label TEXT,
    _enabled BOOLEAN,
    _rate_limit JSONB
) RETURNS VOID AS
$$
DECLARE
    burst INTEGER;
    per_seconds BIGINT;
BEGIN
    IF _rate_limit IS NULL OR _rate_limit = 'null'::jsonb THEN
        burst := NULL;
        per_seconds := NULL;
    ELSIF jsonb_typeof(_rate_limit) = 'object' THEN
        IF jsonb_typeof(_rate_limit->'burst') = 'number' THEN
            burst := (_rate_limit->>'burst')::INTEGER;
        END IF;
        IF jsonb_typeof(_rate_limit->'per_seconds') = 'number' THEN
            per_seconds := (_rate_limit->>'per_seconds')::BIGINT;
        END IF;
    ELSE
        burst := NULL;
        per_seconds := NULL;
    END IF;

    INSERT INTO public.auth_api_keys AS ak (key_id, hash, label, enabled, rate_limit_burst, rate_limit_per_seconds)
    VALUES (_key_id, _hash, _label, _enabled, burst, per_seconds)
    ON CONFLICT (key_id) DO UPDATE
    SET hash = EXCLUDED.hash,
        label = EXCLUDED.label,
        enabled = EXCLUDED.enabled,
        rate_limit_burst = EXCLUDED.rate_limit_burst,
        rate_limit_per_seconds = EXCLUDED.rate_limit_per_seconds;
END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION revaer_config.factory_reset()
RETURNS VOID AS
$$
DECLARE
    rec RECORD;
BEGIN
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
        '/var/lib/revaer/state',
        '/data/staging'
    );

    INSERT INTO public.fs_policy (id, library_root)
    VALUES (
        '00000000-0000-0000-0000-000000000003',
        '/data/library'
    );

    PERFORM revaer_config.persist_app_telemetry('00000000-0000-0000-0000-000000000001', '{}'::jsonb);
    PERFORM revaer_config.persist_app_features('00000000-0000-0000-0000-000000000001', '{}'::jsonb);
    PERFORM revaer_config.persist_app_immutable_keys('00000000-0000-0000-0000-000000000001', '[]'::jsonb);

    PERFORM revaer_config.persist_engine_list('00000000-0000-0000-0000-000000000002', 'listen_interfaces', '[]'::jsonb);
    PERFORM revaer_config.persist_engine_list('00000000-0000-0000-0000-000000000002', 'dht_bootstrap_nodes', '[]'::jsonb);
    PERFORM revaer_config.persist_engine_list('00000000-0000-0000-0000-000000000002', 'dht_router_nodes', '[]'::jsonb);
    PERFORM revaer_config.persist_ip_filter_config('00000000-0000-0000-0000-000000000002', '{}'::jsonb);
    PERFORM revaer_config.persist_alt_speed_config('00000000-0000-0000-0000-000000000002', '{}'::jsonb);

    PERFORM revaer_config.persist_fs_list('00000000-0000-0000-0000-000000000003', 'cleanup_keep', '[]'::jsonb);
    PERFORM revaer_config.persist_fs_list('00000000-0000-0000-0000-000000000003', 'cleanup_drop', '[]'::jsonb);
    PERFORM revaer_config.persist_fs_list(
        '00000000-0000-0000-0000-000000000003',
        'allow_paths',
        '["/data/staging", "/data/library"]'::jsonb
    );
END;
$$ LANGUAGE plpgsql;

-- END 0003_settings_normalization.sql
