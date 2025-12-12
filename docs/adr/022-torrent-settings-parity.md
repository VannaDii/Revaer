# Torrent settings parity and observability

- Status: Accepted
- Date: 2025-12-12
- Context:
  - TORRENT_GAPS precursor calls for API/runtime parity and observability of the knobs we already support.
  - Torrent details previously lacked a single place to inspect applied settings (rate caps, selection rules, tags/trackers) and metadata drifted after rate/selection updates.
  - Engine profile parity (stored vs effective) is already exposed via the config snapshot, but per-torrent settings needed an equivalent surface.
- Decision:
  - Expose a `TorrentSettingsView` on torrent detail responses covering download_dir/sequential status from the inspector plus tags/trackers/rate caps and the latest selection rules captured in API state.
  - Record selection rules and rate limit updates in `TorrentMetadata` on creation and after rate/selection actions so the API surface reflects current requests.
  - Added tests to lock the settings/selection projection alongside the existing effective engine profile check; no new dependencies introduced.
  - Alternatives: keep only rate limits visible (rejected—missing parity for other knobs); fetch selection from the worker each time (rejected—no transport yet and higher coupling).
- Consequences:
  - Clients can now observe per-torrent knobs in a single payload, and metadata stays in sync when limits or selection change.
  - Provides a scaffold to extend settings as new torrent options land (queue priority, PEX, etc.) without reshaping the API again.
  - Risk: settings reflect API-side intent; if runtime diverges we must extend inspector reporting or add additional reconciliation hooks.
- Follow-up:
  - Thread future torrent options into `TorrentMetadata`/settings and surface runtime-effective values when the inspector can supply them.
  - Regenerate OpenAPI when torrent surfaces change and keep UI/CLI renderers updated if they need to show the new fields.
