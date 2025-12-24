# UI store, API coverage, and rate-limit retries

- Status: Accepted
- Date: 2025-12-24
- Context:
  - Shared UI state (theme, toasts, label/health caches) needed to live in the AppStore to match the yewdux architecture rule.
  - The API client needed coverage for health, metrics, and label list endpoints to unblock upcoming screens.
  - Rate-limit responses required user-visible backoff messaging and a safe retry path for idempotent fetches.
- Decision:
  - Move shell theme/toast/busy state into the AppStore and populate label/health caches from API calls.
  - Extend the UI API client with health/full, metrics, and label list endpoints, leaving option/selection/authoring calls for later UI wiring.
  - Handle 429 responses for torrent list/detail fetches with Retry-After backoff and a single retry.
- Consequences:
  - UI state is centralized and ready for labels/health screens without ad-hoc local state.
  - API coverage is aligned with the checklist endpoints, reducing future wiring churn.
  - Rate-limit retries add controlled delay behavior; repeated throttling still surfaces errors.
- Follow-up:
  - Remove demo-only list/detail fallback paths and add empty states.
  - Implement category/tag management screens and health viewer UI.
  - Wire per-torrent options/selection editing in the drawer and add torrent authoring UX.

## Motivation

- Keep shared UI state in yewdux and close API coverage gaps needed for Torrent UX.

## Design notes

- AppShell theme and toast lifecycles now flow through AppStore updates.
- Labels/health caches are populated from API calls and stored in dedicated slices.
- Added API client methods for remaining torrent and label endpoints.
- API client currently covers health/full, metrics, and label list endpoints; mutating endpoints await UI wiring.
- Rate-limit backoff uses Retry-After with a single retry for idempotent list/detail fetches.

## Test coverage summary

- just ci (fmt, lint, check-assets, udeps, audit, deny, ui-build, test, test-features-min, cov)
- llvm-cov reports: "warning: 40 functions have mismatched data"

## Observability updates

- No changes.

## Risk & rollback plan

- Risk: extra retry traffic on sustained 429 responses.
- Rollback: remove retry/backoff helpers and revert list/detail fetch handling.

## Dependency rationale

- No new dependencies.
