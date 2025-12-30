-- Development seed data for Revaer.
-- Safe to run repeatedly; uses upserts where applicable.

-- Ensure the app profile is active and bound for local development.
INSERT INTO app_profile (id, mode, instance_name, bind_addr, http_port)
VALUES (
    '00000000-0000-0000-0000-000000000001',
    'active',
    'revaer-dev',
    '0.0.0.0',
    7070
)
ON CONFLICT (id) DO UPDATE
SET mode = EXCLUDED.mode,
    instance_name = EXCLUDED.instance_name,
    bind_addr = EXCLUDED.bind_addr,
    http_port = EXCLUDED.http_port;

-- Default engine profile paths for local runs.
UPDATE engine_profile
SET
    resume_dir = '/var/lib/revaer/state',
    download_root = '/data/staging'
WHERE id = '00000000-0000-0000-0000-000000000002';

-- Default filesystem policy and allowed paths.
UPDATE fs_policy
SET
    library_root = '/data/library'
WHERE id = '00000000-0000-0000-0000-000000000003';

SELECT revaer_config.set_fs_list(
    '00000000-0000-0000-0000-000000000003',
    'allow_paths',
    ARRAY['/data/staging', '/data/library']::TEXT[]
);

-- Development API key: key_id=dev, secret=revaer_dev
-- Argon2id hash generated with default parameters.
INSERT INTO auth_api_keys (key_id, hash, label, enabled)
VALUES (
    'dev',
    '$argon2id$v=19$m=65536,t=3,p=4$iSyvOFZDidd/MMd8uzVZhA$h5FHZ1bjsApa1rSgmHW//WDblqCyqOKAnCuxs963log',
    'Development key',
    TRUE
)
ON CONFLICT (key_id) DO UPDATE
SET hash = EXCLUDED.hash,
    label = EXCLUDED.label,
    enabled = EXCLUDED.enabled;
