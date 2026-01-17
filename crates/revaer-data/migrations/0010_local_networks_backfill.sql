-- Backfill local network defaults for existing app profiles.

INSERT INTO public.app_profile_local_networks (profile_id, cidr, ord)
SELECT ap.id,
       defaults.cidr,
       defaults.ord
FROM public.app_profile AS ap
CROSS JOIN LATERAL (
    SELECT cidr, ord
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
) AS defaults
WHERE NOT EXISTS (
    SELECT 1
    FROM public.app_profile_local_networks AS ln
    WHERE ln.profile_id = ap.id
)
ON CONFLICT (profile_id, cidr) DO NOTHING;
