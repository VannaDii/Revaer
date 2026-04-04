# Indexer Migration Rollback

Revaer’s indexer migration path is designed to be reversible.

## Coexistence

- Revaer can run alongside Prowlarr because Revaer only exposes its own Torznab endpoints and import surfaces.
- Revaer does not push configuration into Sonarr, Radarr, Lidarr, or Readarr.
- Existing Arr and Prowlarr configuration stays outside Revaer-managed state.

## Rollback

Rollback is URL-only:

1. Switch each Arr client’s Torznab URL back from Revaer to the prior Prowlarr URL.
2. Keep the previous API key or credentials in the Arr client as needed for the old endpoint.
3. Leave Revaer import jobs, search profiles, and Torznab instances in place for inspection or later retry.

No cleanup is required in Revaer to restore the previous Arr behavior because Revaer does not mutate downstream Arr configuration.

## Operational Notes

- Dry-run import jobs are safe to execute while Prowlarr is still active.
- Revaer Torznab instances can coexist with imported indexer management flows.
- If you need to compare behavior during migration, keep both Revaer and Prowlarr Torznab endpoints available and move one Arr client at a time.
