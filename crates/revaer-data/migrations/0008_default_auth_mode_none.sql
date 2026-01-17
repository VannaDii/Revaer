-- Default new app profiles to no-auth for local recovery.

ALTER TABLE public.app_profile
    ALTER COLUMN auth_mode SET DEFAULT 'none';
