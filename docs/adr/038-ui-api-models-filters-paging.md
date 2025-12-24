# UI shared API models and torrent query paging state

- Status: Accepted
- Date: 2025-12-24
- Context:
  - The UI duplicated API DTOs, causing drift from backend shapes and blocking checklist compliance.
  - Torrent list fetching needed a real query/paging model to align with the API list response.
  - SSE fsops events required a stable store cache separate from row state.
- Decision:
  - Extract shared API DTOs into a new `revaer-api-models` crate and re-export from `revaer-api`.
  - Update the UI to consume shared DTOs, map list/detail views from API shapes, and parse list responses with `next` cursors.
  - Add `TorrentsQueryModel`, `TorrentsPaging`, and `fsops_by_id` to the torrent store and update SSE to fill fsops state.
- Consequences:
  - API DTOs are now single-source across API/CLI/UI consumers.
  - UI list fetching can track cursor paging and filter parameters in state.
  - Detail views now map from API DTOs with placeholder metadata until richer fields are available.
- Follow-up:
  - Wire filter fields into URL/query state and implement load-more pagination.
  - Replace add-torrent payloads with `TorrentCreateRequest` + client UUIDs.
  - Populate health and label caches from API endpoints.

## Motivation

- Eliminate duplicated API DTOs in the UI and align list fetching with backend paging semantics.

## Design notes

- Introduced `revaer-api-models` as the canonical DTO crate and re-exported it from `revaer-api`.
- `TorrentSummary` and `TorrentDetail` conversions now map from shared DTOs into UI row/detail views.
- `TorrentsQueryModel` and `TorrentsPaging` feed `build_torrents_path` for list requests.
- SSE fsops events update `fsops_by_id` without mutating row state.

## Test coverage summary

- just ci (fmt, lint, check-assets, udeps, audit, deny, ui-build, test, test-features-min, cov)
- llvm-cov reports: "warning: 40 functions have mismatched data"

## Observability updates

- No changes.

## Risk & rollback plan

- Risk: mapping differences between API DTOs and UI view models could hide fields.
- Rollback: revert to the previous UI DTO definitions and list fetch logic.

## Dependency rationale

- Added `revaer-api-models` to share API DTOs across crates.
- Added `chrono` as a UI dev-dependency for DTO construction in tests.
