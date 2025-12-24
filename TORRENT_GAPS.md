# Torrent Gaps Checklist (implementation order)

-   [x] Precursor: schema/validation scaffolding

    -   [x] Introduce typed `EngineProfileConfig` module and a single stored-proc update path to avoid per-field drift.
    -   [x] Add shared validator to keep DB/API validation in lockstep as new fields are added.

-   [x] Precursor: FFI/bridge extensibility

    -   [x] Group related options into sub-structs (trackers, network, storage) to keep `EngineOptions` maintainable.
    -   [x] Add snapshot tests in Rust/C++ to lock struct layouts before expanding with new fields.

-   [x] Precursor: native testing harness

    -   [x] Ensure feature-gated native tests are easy to run locally; add helper to spin up a mocked session for config application.
    -   [x] Use the helper to speed iteration as new engine options are added.

-   [x] Precursor: safety guardrail and clamping

    -   [x] Add a central “safe defaults” function for `EngineProfile` → `EngineRuntimeConfig` → `EngineOptions` that clamps/normalizes values, ignores unknown/unsupported fields, and is unit-tested.
    -   [x] Ensure worker/bridge/native paths fail safe on bad inputs without destabilizing the session.

-   [x] Precursor: config/runtime parity and observability

    -   [x] Clamp/normalize before persistence; do not store insane values.
    -   [x] Expose all knobs via API: profile inspect shows stored + effective (post-clamp) values; torrent details expose current per-torrent settings (caps, limits, queue priority, PEX/super-seeding/seed-mode, storage/category/tag, seed ratio override, etc.).
    -   [x] Tests: assert API-reported effective values match applied runtime/libtorrent state after normalization.

-   [x] Tracker configuration flows end-to-end (profile → runtime → bridge → native)

    -   [x] Config/DB: introduce typed `TrackerConfig` (user_agent, default/extra trackers, replace flag, announce_ip/listen_interface/timeout, proxy refs), validate/normalize in stored proc, keep secrets in `settings_secret`.
    -   [x] Runtime/bridge: map `EngineProfile.tracker` into `EngineRuntimeConfig` and `EngineOptions`; include tracker fields in the FFI bridge structs.
    -   [x] Native: apply tracker settings in `session.cpp` via `lt::settings_pack` (user_agent, announce_ip, listen_interfaces, proxy host/port/type/creds, proxy_peers, timeouts, announce_to_all_trackers/replace_trackers); maintain default trackers and attach on add.
    -   [x] Per-torrent trackers: include trackers on `AddTorrentRequest`; append to `lt::add_torrent_params::trackers`, clearing when replace is set.
    -   [x] API/docs: ensure `TorrentCreateRequest.trackers` is validated and propagated; expose tracker profile fields if editable; update OpenAPI/docs.
    -   [x] Tests: config validation round-trip; Rust/bridge tests that `EngineOptions` carries tracker fields; native tests that `apply_engine_profile` and `add_torrent` honor user_agent/proxy/trackers.

-   [x] Client-supplied trackers reach libtorrent

    -   [x] API: validate/dedupe `TorrentCreateRequest.trackers`, thread through `to_options()` and responses.
    -   [x] Persistence: store per-torrent tracker lists with torrent metadata if not already kept.
    -   [x] Bridge/native: carry `trackers: Vec<String>` on `AddTorrentRequest`; apply to `lt::add_torrent_params::trackers`; stop discarding tags.
    -   [x] Worker: keep tracker lists intact from API → engine → bridge.
    -   [x] Tests: API parsing; unit test `AddTorrentRequest` includes trackers; native test that trackers reach `lt::add_torrent_params`.

-   [x] NAT traversal/local discovery toggles supported

    -   [x] Config/DB: add `lsd`/`upnp`/`natpmp` (and optional `peer_exchange`) flags with validation and default-off posture.
    -   [x] Runtime/bridge: add flags to `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: set `lt::settings_pack::enable_lsd`, `enable_upnp`, `enable_natpmp` (and optional `enable_outgoing_utp`/`enable_incoming_utp` if needed) from options instead of hard-coded false.
    -   [x] API/docs: expose/validate if profile edits are public; document security trade-offs.
    -   [x] Tests: config parsing; native test that toggles apply and defaults remain disabled.

-   [x] DHT bootstrap and router nodes configurable

    -   [x] Config/DB: add optional DHT bootstrap node list/router endpoints to `EngineProfile` (validated host:port entries); keep `enable_dht` as the gate.
    -   [x] Runtime/bridge: carry bootstrap/router lists in `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: apply via `lt::settings_pack::dht_bootstrap_nodes` during config application; allow updates on profile change.
    -   [x] API/docs: expose bootstrap/router configuration if engine profile is editable; document defaults and safe usage.
    -   [x] Tests: config validation; native (feature-gated) test that custom bootstrap/router nodes are applied without error and used when DHT is enabled.

-   [x] IP filtering/blocklists exposed

    -   [x] Config/DB: add optional IP filter/blocklist configuration to `EngineProfile` (inline CIDR list and/or URL to download and cache), with validation and revision bump; store last-updated metadata if remote fetch is supported.
    -   [x] Runtime/bridge: include IP filter config in `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: apply filters via `lt::ip_filter` / `session::set_ip_filter`; if remote blocklists are allowed, download (outside hot path) and load into the session, handling refresh intervals.
    -   [x] API/docs: expose filter settings if editable; document precedence with per-peer bans and security implications.
    -   [x] Tests: config validation; native (feature-gated) test that blocked CIDRs prevent peers; unit test filter parsing and application; if remote fetch is supported, add mocked fetch test.

-   [x] Multi-interface / IPv6 clarity

    -   [x] Config/DB: support multiple listen interfaces (`listen_interfaces: Vec<String>`) and IPv6 policy flags (enable/disable/prefer_v6).
    -   [x] Runtime/bridge: carry multi-interface and IPv6 prefs through `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: set `lt::settings_pack::listen_interfaces` with multiple entries; apply IPv6 prefs; ensure tracker/peer behavior matches policy.
    -   [x] Tests: config validation; native test that multiple interfaces and IPv6 policies apply without error.

-   [x] Privacy/anonymous mode and transport toggles

    -   [x] Config/DB: add privacy flags to `EngineProfile` (e.g., `anonymous_mode`, `force_proxy`, `prefer_rc4`, `allow_multiple_connections_per_ip`, explicit `utp_enabled` toggles) with validation and conservative defaults.
    -   [x] Runtime/bridge: include these flags in `EngineRuntimeConfig`/`EngineOptions`; validate combinations (e.g., anonymous_mode implies force_proxy when proxy is configured).
    -   [x] Native: apply via `lt::settings_pack` (`anonymous_mode`, `force_proxy`, `prefer_rc4`, `allow_multiple_connections_per_ip`, `enable_outgoing_utp`, `enable_incoming_utp`) and related privacy knobs.
    -   [x] API/docs: expose flags if profile edits are allowed; document trade-offs (reduced telemetry, tracker behaviors).
    -   [x] Tests: config validation; native (feature-gated) tests that toggles apply and defaults remain safe.

-   [x] Outgoing port range and DSCP/TOS markings configurable

    -   [x] Config/DB: add optional outgoing port range and DSCP/TOS fields to `EngineProfile`; validate ranges and DSCP values in stored procs.
    -   [x] Runtime/bridge: include these fields in `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: apply via `lt::settings_pack::outgoing_ports`, `peer_tos`, and `peer_socket_send_buffer_size`/`receive_buffer_size` if needed.
    -   [x] API/docs: expose with bounds; document network implications.
    -   [x] Tests: config validation; native test that settings apply without error.

-   [x] Connection/peer limits configurable

    -   [x] Config/DB: add global/per-torrent connection caps (e.g., `connections_limit`, `connections_per_torrent`, `unchoke_slots`, `half_open_limit`) with validation; set defaults to libtorrent defaults.
    -   [x] Runtime/bridge: extend `EngineRuntimeConfig`/`EngineOptions` and `AddTorrentOptions`/`AddTorrentRequest` with connection cap fields.
    -   [x] Native: apply via `lt::settings_pack` (`connections_limit`, `connections_per_torrent`, `unchoke_slots`, `half_open_limit`); per-torrent via `torrent_handle::set_max_connections` where applicable.
    -   [x] Worker: when per-torrent caps are provided on add, apply immediately; keep them in the per-torrent cache for verification/telemetry.
    -   [x] API/docs: expose optional per-torrent and profile-level connection limits with bounds; document effects on swarm health.
    -   [x] Tests: config validation; worker applies caps after add; native test confirms connection settings take effect without error.

-   [x] Per-torrent rate caps applied on admission

    -   [x] API: validate per-torrent `max_*_bps` and keep them in `AddTorrentOptions`; persist caps with torrent metadata if desired.
    -   [x] Worker: after successful add, issue `update_limits` for the torrent when caps are present; cache per-torrent caps for verification.
    -   [x] Bridge/native: allow per-torrent limits via `AddTorrentRequest` or immediate `update_limits`; ensure `NativeSession::update_limits` tolerates immediate calls.
    -   [x] Tests: API parsing; worker ensures immediate cap application; native test confirms per-torrent caps apply without errors.

-   [x] Alt-speed scheduling (global)

    -   [x] Config/DB: add alt speed caps and simple schedules (time-of-day/weekday) to `EngineProfile`; validate.
    -   [x] Runtime/bridge: include alt-speed caps/schedule in `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: set libtorrent alt-speed settings/ schedule fields; ensure transitions respected.
    -   [x] API/docs/tests: expose caps/schedule; document behavior; native test that schedule toggles caps as expected.

-   [x] Seeding stop criteria (ratio/time) supported

    -   [x] Config/DB: add optional `seed_ratio_limit`/`seed_time_limit` (and per-torrent defaults); validate non-negative numbers.
    -   [x] Runtime/bridge: add to `EngineRuntimeConfig`/`EngineOptions`; set `lt::settings_pack::share_ratio_limit`/`seed_time_limit`; per-torrent overrides enforced after add.
    -   [x] Worker: allow per-torrent overrides on add and apply immediately.
    -   [x] API/docs: expose optional per-torrent and profile defaults; document stop vs. pause behavior.
    -   [x] Tests: config validation; worker applies caps; native-side mapping exercised.

-   [x] Seed-mode / add-as-complete supported

    -   [x] API: add flag to admit torrents in seed mode (skip full recheck) with warnings; optional hash sample preflight.
    -   [x] Worker: carry flag in `EngineCommand::Add`; optionally perform sampled hash check; set seed mode when requested.
    -   [x] Native: allow `lt::torrent_flags::seed_mode` / `torrent_handle::set_seed_mode` when user opts in.
    -   [x] Tests: native test that seed-mode admission succeeds and does not recheck; safety tests around hash sample behavior.

-   [x] Add-paused / queued admission supported

    -   [x] API: add `paused`/`start_paused` flag to `TorrentCreateRequest`; validate.
    -   [x] Worker: honor paused on `EngineCommand::Add`, queuing torrent without starting; emit appropriate state.
    -   [x] Bridge/native: set `lt::torrent_flags::paused`/`auto_managed` on add per flag; avoid immediate start when paused.
    -   [x] Tests: API parsing; worker leaves torrent paused; native test that add-paused does not start transfers.

-   [x] Torrent queue priorities / auto-managed toggle

    -   [x] Config/DB: add defaults for auto-managed behavior and queue priority policy to `EngineProfile`; validate booleans/priority bounds.
    -   [x] Runtime/bridge: include flags in `EngineRuntimeConfig`/`EngineOptions`; allow per-torrent overrides in `AddTorrentOptions`.
    -   [x] Native: control `lt::torrent_flags::auto_managed` on add based on config; expose queue priority settings via `settings_pack` (e.g., `auto_manage_prefer_seeds`, `dont_count_slow_torrents`).
    -   [x] Worker: honor per-torrent auto-managed override when enqueuing adds.
    -   [x] API/docs/tests: expose per-torrent auto-managed flag if needed; add tests for managed vs manual admission.

-   [x] Choke/unchoke strategy configurable

    -   [x] Config/DB: add choke/unchoke strategy fields (e.g., `unchoke_algorithm`, `seed_choking_algorithm`, `strict_super_seeding`, `optimistic_unchoke_slots`) with validated enums/bounds.
    -   [x] Runtime/bridge: include these in `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: set `lt::settings_pack` equivalents (`unchoke_algorithm`, `seed_choking_algorithm`, `strict_super_seeding`, `num_optimistic_unchoke_slots`, `max_queued_disk_bytes`) accordingly.
    -   [x] API/docs/tests: expose only sane presets; document behavior; native tests verifying settings apply.

-   [x] Super-seeding (initial seeding) supported

    -   [x] Config/DB: add `super_seeding` default flag to `EngineProfile` and optional per-torrent override; validate booleans.
    -   [x] Runtime/bridge: carry flags in `EngineRuntimeConfig`/`EngineOptions` and `AddTorrentOptions`.
    -   [x] Native: toggle `torrent_handle::super_seeding` / `lt::torrent_flags::super_seeding` based on defaults/overrides on add.
    -   [x] API/docs/tests: expose per-torrent super-seeding where appropriate; add native test that super-seeding applies on add.

-   [x] Peer exchange (PEX) wired through

    -   [x] Config/DB: add a `pex_enabled` flag to `EngineProfile` with default-off and validation.
    -   [x] Runtime/bridge: include PEX toggle in `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: set `lt::settings_pack::enable_outgoing_utp`/`enable_incoming_utp` as needed and explicitly enable/disable PEX (extensions) on the session/torrent handles.
    -   [x] Worker: ensure per-torrent PEX behavior follows the profile flag; allow per-torrent override if required.
    -   [x] API/docs/tests: expose flag if profile edits are public; document swarm/priv tracker implications; native test that PEX toggling applies without errors.

-   [x] Post-add mutation symmetry for per-torrent knobs

    -   [x] API: add PATCH endpoints to update per-torrent options currently set on add (rate caps, connection limits, PEX, super-seeding, seed ratio/time caps, queue priority, add-paused state transitions, tracker/web seed updates).
        -   [x] Patch endpoint for connections/PEX/super-seeding/auto-managed/queue position/seed ratio and time caps.
        -   [x] Tracker and web seed updates.
        -   [x] Add-paused symmetry.
    -   [x] Worker: add/update commands to apply these changes post-add; ensure state and metadata stay in sync.
    -   [x] Bridge/native: expose libtorrent calls to update corresponding options on existing torrents; ignore/handle unsupported values safely.
    -   [x] Tests: API + worker + native tests to confirm post-add updates apply and persist; regression tests for symmetry.

-   [x] Tracker status and tracker ops surfaced

    -   [x] Domain/API: persist tracker list/status per torrent; add endpoints to list/add/remove trackers and show per-tracker messages.
    -   [x] Native: map tracker alerts into per-tracker status; support add/remove tracker operations via session/torrent handle.
    -   [x] Tests: API + native tests for tracker add/remove and status propagation.

-   [x] Tracker HTTP auth/cookies supported

    -   [x] Config/DB: add tracker auth profile (basic auth/cookie headers) with secret refs; validate.
    -   [x] Runtime/bridge: pass auth headers/cookies in `EngineRuntimeConfig`/`EngineOptions` and per-torrent overrides.
    -   [x] Native: set per-tracker auth/cookies via libtorrent settings/`add_torrent_params` fields; handle updates.
    -   [x] API/docs/tests: expose tracker auth configuration; native test that authenticated trackers connect successfully.

-   [x] Web seeds (HTTP/URL seeds) supported

    -   [x] API: accept web seed URLs on torrent create/update with validation (scheme/length).
    -   [x] Worker: carry web seeds through `EngineCommand::Add` and update commands when needed.
    -   [x] Bridge/native: include web seeds on `AddTorrentRequest` and apply via `add_torrent_params::url_seeds`; allow add/remove web seeds post-add.
    -   [x] Tests: API parsing; native test that web seeds are attached and used without error.

-   [x] Mid-download move/relocate supported

    -   [x] API: add move/relocate endpoint and request payload (new path).
    -   [x] Worker: introduce `EngineCommand::Move` to invoke storage relocation; integrate with FsOps expectations.
    -   [x] Bridge/native: expose `move_storage` (or equivalent) in FFI and call `torrent_handle::move_storage` with proper flags.
    -   [x] Tests: worker/native tests that move completes and events reflect new path; FsOps compatibility.

-   [x] Advanced storage options integrated with FsOps

    -   [x] Config/DB: add storage policy fields (e.g., sparse/allocation mode, partfile use, storage paths) to `EngineProfile`; validate choices.
    -   [x] Runtime/bridge: carry storage policy in `EngineRuntimeConfig`/`EngineOptions` and per-torrent overrides in `AddTorrentOptions`.
    -   [x] Native: apply storage options via `add_torrent_params` (e.g., `storage_mode_sparse`, partfile toggles) and coordinate storage location choices with FsOps expectations (paths, partfiles).
    -   [x] FsOps integration: ensure storage layout (partfiles/temp paths) is compatible with post-processing/moves; document expectations.
    -   [x] API/docs/tests: expose safe storage choices; tests to ensure storage options are honored and FsOps can operate on outputs.

-   [x] Peer class/priority tagging supported

    -   [x] Config/DB: allow defining peer classes and per-torrent class assignments; validate class IDs/ratios.
    -   [x] Runtime/bridge: add peer class info to `EngineRuntimeConfig`/`EngineOptions` and per-torrent options.
    -   [x] Native: use libtorrent peer classes (`peer_class_type_filter`, `set_peer_class_filter`, `set_peer_class_type_filter`) and `torrent_handle::set_peer_classes` to set per-peer/per-torrent priorities.
    -   [x] API/docs: expose only if needed; document complexity and defaults.
    -   [x] Tests: unit tests for class mapping; native test that class assignments apply without error.

-   [x] Peer view and diagnostics exposed

    -   [x] Domain/API: add peer listing (IP/flags/rates/client, progress) via inspector/endpoint.
    -   [x] Native: surface peer info through FFI (translate peer_info structs); ensure alert polling or snapshot supports peer views.
    -   [x] Tests: native tests that peers are returned; API tests for peer view responses.

-   [x] Alert coverage and error surfacing expanded

    -   [x] Bridge/native: map additional libtorrent alerts (tracker errors/warnings, listen/port binding failures, peer bans, storage errors, SSL verification issues) into `NativeEvent`/`EngineEvent` instead of dropping them.
    -   [x] Worker: translate new events into `EventBus` health/state changes with actionable messages; ensure degraded health captures session and tracker/storage faults.
    -   [x] API/UX: surface meaningful error messages in torrent status/details; document alert categories exposed.
    -   [x] Tests: native (feature-gated) tests that injected tracker/storage/listen failures produce expected events and health degradation; unit tests for alert mapping coverage.

-   [x] Session stats cadence tunable

    -   [x] Config/DB: add `stats_interval_ms` to `EngineProfile` with sane bounds.
    -   [x] Runtime/bridge: include in `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: set stats/tick interval to control alert cadence for stats; ensure worker can handle the volume.
    -   [x] API/docs: document effect on telemetry volume; keep default conservative.
    -   [x] Tests: config validation; native test that interval applies without error.

-   [x] Disk cache/memory knobs exposed

    -   [x] Config/DB: add cache-related fields (cache_size, cache_expiry, coalesce_reads, coalesce_writes, use_disk_cache_pool) to `EngineProfile`; validate bounds, including piece hash verification and disk I/O mode enums.
    -   [x] Runtime/bridge: thread cache_size/cache_expiry/coalesce flags and use_disk_cache_pool through `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: set `lt::settings_pack` cache knobs (`cache_size`, `cache_expiry`, `coalesce_reads`, `coalesce_writes`, `use_disk_cache_pool`) via name-based setters to avoid deprecated enums; include disk I/O modes and piece hash verification mapping.
    -   [x] API/docs: expose new cache/storage fields in settings and health surfaces; update examples.
    -   [x] Tests: add dedicated coverage that cache settings are applied and survive native round-trips (plus piece hash mode when available).
    -   [x] Piece hash verification mode and disk I/O mode mapping wired through.

-   [x] TLS/SSL tracker and client cert configuration

    -   [x] Config/DB: allow tracker SSL settings (trust store path, verify flags) and optional client cert/key references (stored as secrets).
    -   [x] Runtime/bridge: carry SSL tracker settings in `EngineRuntimeConfig`/`EngineOptions`.
    -   [x] Native: set `lt::settings_pack` SSL options (e.g., `ssl_cert`, `ssl_private_key`, `ssl_ca_cert`, `ssl_tracker_verify`) and apply per-tracker if supported.
    -   [x] API/docs/tests: expose only if needed; document security implications; native test that SSL settings apply and tracker connections succeed.

-   [x] BitTorrent v2/hybrid torrents supported

    -   [x] API: accept v2/hybrid metainfo and expose v2 fields where relevant; validate size.
    -   [x] Worker/bridge: ensure add path passes v2/hybrid payloads unchanged.
    -   [x] Native: confirm libtorrent is built with v2 support and accepts hybrid torrents; adjust event mapping if v2-specific metadata arises.
    -   [x] Tests: feature-gated native test adding a v2/hybrid torrent to ensure admission succeeds.

-   [x] Torrent creation (authoring) supported

    -   [x] API: add create-torrent endpoint to build `.torrent`/magnet from local files/dirs with options (piece size, private flag, trackers, web seeds, comment, source).
    -   [x] Worker/bridge: introduce command to invoke libtorrent create_torrent; marshal options; return metainfo/magnet.
    -   [x] Native: expose create-torrent path via FFI using `lt::create_torrent` + bencode; handle file traversal safely.
    -   [x] Tests: API + native tests creating torrents with various options; ensure outputs validate.

-   [x] Comments/source/private flag visibility and updates

    -   [x] API: surface per-torrent comment/source/private flag in details; allow updates where safe.
    -   [x] Worker/bridge: carry comment/source/private flag through add and update paths; for private flag, respect tracker requirements.
    -   [x] Native: apply comment/source/private settings via libtorrent structures on add and, where supported, updates.
    -   [x] Tests: API + native tests ensuring fields are exposed and updates apply where allowed.

-   [x] Categories/tags with policy and cleanup

    -   [x] Domain/API: support categories/tags/labels on torrents; endpoints to list/create/update categories/tags.
    -   [x] Policy: allow per-category/tag defaults (paths, rate limits, queue priority, auto-cleanup after ratio/time).
    -   [x] Worker/FsOps: apply category/tag-derived settings on add; integrate cleanup policy (remove after criteria) and FsOps paths.
    -   [x] Tests: API + worker tests for category/tag behavior; cleanup policy tested against ratio/time thresholds.

-   [x] qB-style API surface parity (if required)

    -   [x] Implement compatible endpoints for trackers (`/torrents/trackers` + ops), peers (`/sync/torrentPeers`), full torrent properties (`/torrents/properties`), categories/tags endpoints, aligned with our domain and security posture.
    -   [x] Tests: compatibility tests for the façade, ensuring responses match expected shapes/behaviors.

-   [x] Streaming/piece-deadline support exposed

    -   [x] API: optional streaming flag or piece deadlines for preview use-cases.
    -   [x] Worker: new command to set piece deadlines/read-ahead for a torrent.
    -   [x] Bridge/native: expose `set_piece_deadline`/`reset_piece_deadline` and read-ahead knobs through FFI; guard usage to streaming paths.
    -   [x] Tests: native test that deadlines apply without breaking transfers.

-   [x] Codex Task: Close audit findings and enforce “thin native wrapper” invariants

-   [x] Deliverables

    -   [x] 1. Apply fixes (code + tests + CI) so the audit becomes PASS.
    -   [x] 2. Regenerate report as: ARTIFACTS/TORRENT_FFI_AUDIT_REPORT.md with Status: PASS (or list remaining failures with exact diffs needed).

-   [x] Phase 1 — Fix the drift items (no Revaer-only semantics)

-   [x] 1. Reject unsupported metadata updates at the API layer

-   [x] Decision: thin wrapper; no Revaer-only updates.

-   [x] Action:

    -   [x] • In crates/revaer-api/src/models.rs (and any request parsing / validators used by PATCH):
    -   [x] • Reject comment, source, and private updates before the worker/FFI call.
    -   [x] • Error should be a clean 4xx with a deterministic message, e.g.:
    -   [x] • \"comment updates are not supported post-add\"
    -   [x] • \"source updates are not supported post-add\"
    -   [x] • \"private flag updates are not supported post-add\"
    -   [x] • Ensure worker does not mutate stored metadata for these fields in the update path.

-   [x] Verification:

    -   [x] • Add API tests asserting:
    -   [x] • PATCH including those fields returns 4xx.
    -   [x] • No native call is invoked (if you have a test seam; otherwise assert no state mutation).

-   [x] 2. Remove Rust-side seeding enforcement (native-only)

-   [x] Decision: only supported if libtorrent can enforce; otherwise reject per-torrent overrides.

-   [x] Action A — Delete policy enforcement

    -   [x] • Remove Rust worker seeding goal logic from:
    -   [x] • crates/revaer-torrent-libt/src/worker.rs
    -   [x] • Specifically anything like register_seeding_goal, evaluate_seeding_goal, PauseForSeedingGoal, etc.
    -   [x] • Ensure no “pause because ratio/time met” exists in Rust.

-   [x] Action B — Enforce via libtorrent

    -   [x] • Confirm global/profile seeding limits are applied via lt::settings_pack:
    -   [x] • share_ratio_limit
    -   [x] • seed_time_limit
    -   [x] • If your “per-torrent overrides” exist in Rust types:
    -   [x] • Only keep them if libtorrent has per-torrent handle APIs to enforce them.
    -   [x] • If libtorrent does not have per-torrent enforcement, then:
    -   [x] • Reject per-torrent seed ratio/time fields at API (POST add + PATCH) with 4xx
    -   [x] • Keep only global profile limits

-   [x] How Codex must decide per-torrent support:

    -   [x] • Search libtorrent headers in your build environment for per-torrent seed controls:
    -   [x] • torrent_handle methods related to ratio/time
    -   [x] • OR documented setting names usable via your name-based set\_\*\_setting(...) helpers
    -   [x] • If no real native mechanism exists, you must reject per-torrent overrides.

-   [x] Verification:

    -   [x] • Tests:
    -   [x] • A native integration test confirming global profile share_ratio_limit/seed_time_limit are applied (inspect runtime/effective config).
    -   [x] • If per-torrent overrides are rejected: API tests for POST/PATCH rejection.
    -   [x] • If per-torrent overrides are supported: native IT proving they change runtime behavior / applied state.

-   [x] 3. Fix HTTPS proxy support correctly (only if libtorrent actually supports it)

-   [x] Decision: support HTTPS proxy only if libtorrent has a distinct proxy type.

-   [x] Action:

    -   [x] • In crates/revaer-torrent-libt/src/ffi/session.cpp:
    -   [x] • Replace the “HTTPS coerced to HTTP” switch with correct mapping.
    -   [x] • Codex must prove support exists by locating the proxy enum(s) in libtorrent headers:
    -   [x] • If there is a distinct https proxy type in lt::settings_pack (or equivalent):
    -   [x] • Map your Https to it.
    -   [x] • If there is no distinct HTTPS proxy type:
    -   [x] • Do not silently coerce.
    -   [x] • Instead: fail validation earlier (config validator) with a clear error:
    -   [x] • \"Https proxy type is not supported by the linked libtorrent version\"
    -   [x] • Update docs/tests accordingly.

-   [x] Verification:

    -   [x] • Unit test mapping: Https never becomes Http.
    -   [x] • Native IT: apply profile with proxy kind Https:
    -   [x] • If supported: verify correct native setting is applied.
    -   [x] • If unsupported: API/config update must reject.

-   [x] 4. Apply proxy auth secrets end-to-end

-   [x] Decision: support proxy auth via libtorrent settings.

-   [x] Action:

    -   [x] • Ensure orchestrator resolves proxy auth secret refs:
    -   [x] • username_secret, password_secret for proxy config
    -   [x] • Do not confuse with tracker HTTP auth/cookies (separate)
    -   [x] • In crates/revaer-torrent-libt/src/ffi/session.cpp:
    -   [x] • Set:
    -   [x] • proxy_username
    -   [x] • proxy_password
    -   [x] • alongside proxy_hostname and proxy_port

-   [x] Verification:

    -   [x] • Unit test: secret resolution populates runtime plan fields.
    -   [x] • Native IT: apply profile with proxy creds and assert settings pack includes username/password (via your inspect or debug surface).

-   [x] 5. Implement ipv6_mode natively (only with clean mapping)

-   [x] Decision: implement ipv6_mode in native.

-   [x] Action:

    -   [x] • First determine the mapping target in libtorrent:
    -   [x] • Prefer name-based settings (consistent with your “avoid deprecated enums” posture).
    -   [x] • Codex must locate appropriate setting names or APIs for IPv6 preference/behavior.
    -   [x] • Update:
    -   [x] • crates/revaer-torrent-libt/src/ffi/bridge.rs to carry ipv6_mode in EngineNetworkOptions
    -   [x] • crates/revaer-torrent-libt/src/ffi/session.cpp to apply it
    -   [x] • If the only real control is “listen on v6 interfaces” (via listen_interfaces), then implement ipv6_mode as:
    -   [x] • A deterministic transform of listen interfaces (e.g., add/remove [::] bindings) in one place, and ensure inspect explains the derived behavior.
    -   [x] • Do not keep ipv6_mode as a “phantom effective field” that native ignores.

-   [x] Verification:

    -   [x] • Native IT: set ipv6_mode and assert applied native behavior (settings or derived listen interfaces) matches inspect/effective output.

-   [x] 6. DB alt_speed validation parity

-   [x] Action:

    -   [x] • In stored proc / SQL update path (e.g. crates/revaer-data/migrations/0001_db_init.sql or wherever proc lives):
    -   [x] • Validate/normalize alt_speed JSON exactly like Rust (sanitize_alt_speed semantics).
    -   [x] • If you already have a shared validator module pattern, route alt_speed through it.

-   [x] Verification:

    -   [x] • DB-level test (or migration test harness) that invalid alt_speed payload fails or is normalized identically to Rust.

-   [x] 7. Hard fail build if libtorrent version can’t be proven

-   [x] Decision: hard fail without verified >= MIN_VERSION.

-   [x] Action:

    -   [x] • In crates/revaer-torrent-libt/build.rs:
    -   [x] • Remove the unversioned fallback (libs.push(\"torrent-rasterbar\")) unless you have an explicit “vendored/bundled libtorrent” override path that also proves version.
    -   [x] • Build should fail with a clear error if pkg-config cannot confirm version.

-   [x] Verification:

    -   [x] • Add a build-script test if you have infrastructure; otherwise include a compile-time guard / error path message and ensure CI uses pkg-config path.

-   [x] 8. Run native integration tests in CI on every PR

-   [x] Decision: mandatory.

-   [x] Action:

    -   [x] • Update .github/workflows/ci.yml:
    -   [x] • Add a job/matrix leg that:
    -   [x] • installs libtorrent deps (libtorrent-rasterbar-dev, etc.)
    -   [x] • sets REVAER_NATIVE_IT=1
    -   [x] • runs the native integration suite
    -   [x] • Ensure Docker availability if your native IT depends on it (or remove that dependency).

-   [x] Verification:

    -   [x] • CI must fail if native IT fails.
    -   [x] • Codex must confirm the job actually executes (not skipped by conditions).

-   [x] Phase 2 — Re-run audit and ensure PASS

-   [x] After implementing the above, Codex must:

    -   [x] 1. Re-run ripgrep audits for:

    -   [x] • phantom fields (present in effective profile but never applied)
    -   [x] • silent coercions (Https→Http class bugs)
    -   [x] • Rust-side implementations of libtorrent semantics (especially seeding enforcement)

    -   [x] 2. Re-run full test suite:

    -   [x] • cargo test --workspace
    -   [x] • cargo test -p revaer-torrent-libt --features native-tests (or your actual feature flag)
    -   [x] • plus the integration suite invoked by REVAER_NATIVE_IT=1

    -   [x] 3. Re-emit ARTIFACTS/TORRENT_FFI_AUDIT_REPORT.md with:

    -   [x] • Status: PASS
    -   [x] • Counts all zero, or only MINOR with explicit approval rationale (but avoid that unless you’ve decided it’s acceptable).

-   [x] One explicit rule Codex must enforce in the report

-   [x] No “effective config” field may exist unless either:
    -   [x] • it is applied to libtorrent natively, OR
    -   [x] • it is explicitly derived from a native-applied field and clearly labeled as derived (not an independent knob)
