-- Resolve ambiguity between output column names and table columns.
CREATE OR REPLACE FUNCTION revaer_config.fetch_api_keys()
RETURNS TABLE (
    key_id TEXT,
    label TEXT,
    enabled BOOLEAN,
    rate_limit_burst INTEGER,
    rate_limit_per_seconds BIGINT,
    expires_at TIMESTAMPTZ
) AS
$$
BEGIN
    RETURN QUERY
    SELECT ak.key_id,
           ak.label,
           ak.enabled,
           ak.rate_limit_burst,
           ak.rate_limit_per_seconds,
           ak.expires_at
    FROM public.auth_api_keys AS ak
    WHERE ak.enabled = TRUE
      AND (ak.expires_at IS NULL OR ak.expires_at > now())
    ORDER BY ak.created_at;
END;
$$ LANGUAGE plpgsql STABLE;
