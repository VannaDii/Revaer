-- Local network CIDR ranges for anonymous access.

CREATE TABLE IF NOT EXISTS public.app_profile_local_networks (
    profile_id UUID NOT NULL REFERENCES public.app_profile(id) ON DELETE CASCADE,
    cidr TEXT NOT NULL,
    ord INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (profile_id, cidr)
);

CREATE UNIQUE INDEX IF NOT EXISTS app_profile_local_networks_order
    ON public.app_profile_local_networks (profile_id, ord);

DROP TRIGGER IF EXISTS app_profile_local_networks_touch_updated_at
    ON public.app_profile_local_networks;
CREATE TRIGGER app_profile_local_networks_touch_updated_at
BEFORE UPDATE ON public.app_profile_local_networks
FOR EACH ROW
EXECUTE FUNCTION revaer_touch_updated_at();

CREATE OR REPLACE FUNCTION revaer_config.update_app_local_networks(
    _profile_id UUID,
    _cidrs TEXT[]
) RETURNS VOID AS
$$
BEGIN
    DELETE FROM public.app_profile_local_networks WHERE profile_id = _profile_id;

    INSERT INTO public.app_profile_local_networks (profile_id, cidr, ord)
    SELECT _profile_id,
           btrim(value),
           ord
    FROM unnest(COALESCE(_cidrs, ARRAY[]::TEXT[])) WITH ORDINALITY AS t(value, ord)
    WHERE btrim(value) <> '';
END;
$$ LANGUAGE plpgsql;

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
    local_networks TEXT[],
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
           COALESCE(
               (
                   SELECT array_agg(cidr ORDER BY ord)
                   FROM public.app_profile_local_networks
                   WHERE profile_id = ap.id
               ),
               ARRAY[]::TEXT[]
           ),
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

INSERT INTO public.app_profile_local_networks (profile_id, cidr, ord)
SELECT '00000000-0000-0000-0000-000000000001',
       cidr,
       ord
FROM unnest(
    ARRAY[
        '127.0.0.0/8',
        '10.0.0.0/8',
        '172.16.0.0/12',
        '192.168.0.0/16',
        '169.254.0.0/16',
        '::1/128',
        'fe80::/10',
        'fd00::/8'
    ]::TEXT[]
) WITH ORDINALITY AS t(cidr, ord)
ON CONFLICT (profile_id, cidr) DO NOTHING;

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
    PERFORM revaer_config.update_app_local_networks(
        '00000000-0000-0000-0000-000000000001',
        ARRAY[
            '127.0.0.0/8',
            '10.0.0.0/8',
            '172.16.0.0/12',
            '192.168.0.0/16',
            '169.254.0.0/16',
            '::1/128',
            'fe80::/10',
            'fd00::/8'
        ]::TEXT[]
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
