# Local network auth ranges and settings validation

- Status: Accepted
- Date: 2026-01-17
- Context:
  - Local auth bypass must work for recovery even when API key state is broken.
  - Local-only checks must handle reverse proxies (k3s/docker) that rewrite the peer IP.
  - Operators need to adjust what counts as local without locking themselves out.
- Decision:
  - Persist app profile local network CIDRs and enforce them for no-auth and recovery flows.
  - Trust forwarded client IP headers only when the peer is already within a local range.
  - Validate local network updates against the saving client address before applying.
- Consequences:
  - Anonymous access is now scoped to configured local networks.
  - Factory reset remains possible from local clients even when API key inventory queries fail.
  - Misconfigured local ranges can block access until corrected or reset.
- Follow-up:
  - Keep OpenAPI and UI fields in sync with `app_profile.local_networks`.
  - Monitor any proxy deployments for forwarded header quirks.

## Motivation

- Provide a safe recovery path when auth state or API key inventory is broken.
- Allow common local topologies (LAN device -> k3s/docker service) without false negatives.
- Prevent settings updates that would immediately disconnect the caller.

## Design notes

- Added `app_profile.local_networks` as a normalized list of CIDR strings with defaults for
  loopback, RFC1918, and link-local ranges.
- API auth middleware now derives client IP from ConnectInfo and trusted forwarded headers,
  enforcing local-only access for no-auth and factory reset fallbacks.
- Settings patch validates that the updated local network list still includes the caller IP
  before persisting.

## Test coverage summary

- Auth middleware tests cover anonymous local access, remote rejection, and factory reset
  allowance when API key inventory checks fail.
- Config validation tests cover CIDR normalization and invalid prefixes.

## Observability updates

- Auth middleware logs when local network parsing fails or when recovery paths are used.

## Risk & rollback plan

- Risk: misconfigured local CIDRs can block anonymous access or factory reset.
  - Mitigation: validation rejects updates that exclude the saving client.
- Rollback: revert migration 0009 and remove local network enforcement in auth middleware,
  then restore the previous auth behavior.

## Dependency rationale

- No new dependencies. CIDR parsing reuses std-based helpers in `revaer-config`.
