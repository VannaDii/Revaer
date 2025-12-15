# Queue auto-managed defaults and PEX threading

- Status: Accepted
- Date: 2025-03-16
- Context:
  - TORRENT_GAPS called out missing support for queue management toggles (auto-managed defaults, prefer-seed/don’t-count-slow policies) and peer exchange enable/disable paths.
  - Config/runtime needed a single source of truth so workers and the native bridge don’t drift, and per-torrent overrides had to survive restarts via metadata.
- Decision:
  - Added engine profile fields `auto_managed`, `auto_manage_prefer_seeds`, and `dont_count_slow_torrents` with validation/normalization and a unified stored-proc update, plus a migration to persist them.
  - Extended runtime/FFI to carry the queue policy flags; native now sets libtorrent’s `auto_manage_prefer_seeds`/`dont_count_slow_torrents`, tracks the default auto-managed posture, and applies per-torrent overrides (including queue position) when adding torrents.
  - Threaded `pex_enabled` through add options with a native toggle that maps to `disable_pex`, allowing profile-level defaults and per-torrent overrides.
  - API accepts/validates the new per-torrent knobs (auto-managed, queue position, PEX) and exposes them through OpenAPI; metadata persistence caches the flags for resume.
- Consequences:
  - New migration and stored-proc signature; engines built on old schemas must run migrations before updating.
  - Native add paths now branch on override/default auto-managed flags; queue positions imply manual management to align with libtorrent expectations.
  - Added coverage for option mapping and request validation; stub/native harnesses record the new metadata for symmetry tests.
- Follow-up:
  - Extend torrent detail/inspect surfaces to surface auto-managed/PEX state where useful.
  - Evaluate whether additional queue policy knobs (e.g., priority clamping) are needed for future gaps.
