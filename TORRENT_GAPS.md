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

-   [ ] Outgoing port range and DSCP/TOS markings configurable

    -   [ ] Config/DB: add optional outgoing port range and DSCP/TOS fields to `EngineProfile`; validate ranges and DSCP values in stored procs.
    -   [ ] Runtime/bridge: include these fields in `EngineRuntimeConfig`/`EngineOptions`.
    -   [ ] Native: apply via `lt::settings_pack::outgoing_ports`, `peer_tos`, and `peer_socket_send_buffer_size`/`receive_buffer_size` if needed.
    -   [ ] API/docs: expose with bounds; document network implications.
    -   [ ] Tests: config validation; native test that settings apply without error.

-   [ ] Connection/peer limits configurable

    -   [ ] Config/DB: add global/per-torrent connection caps (e.g., `connections_limit`, `connections_per_torrent`, `unchoke_slots`, `half_open_limit`) with validation; set defaults to libtorrent defaults.
    -   [ ] Runtime/bridge: extend `EngineRuntimeConfig`/`EngineOptions` and `AddTorrentOptions`/`AddTorrentRequest` with connection cap fields.
    -   [ ] Native: apply via `lt::settings_pack` (`connections_limit`, `connections_per_torrent`, `unchoke_slots`, `half_open_limit`); per-torrent via `torrent_handle::set_max_connections` where applicable.
    -   [ ] Worker: when per-torrent caps are provided on add, apply immediately; keep them in the per-torrent cache for verification/telemetry.
    -   [ ] API/docs: expose optional per-torrent and profile-level connection limits with bounds; document effects on swarm health.
    -   [ ] Tests: config validation; worker applies caps after add; native test confirms connection settings take effect without error.

-   [ ] Per-torrent rate caps applied on admission

    -   [ ] API: validate per-torrent `max_*_bps` and keep them in `AddTorrentOptions`; persist caps with torrent metadata if desired.
    -   [ ] Worker: after successful add, issue `update_limits` for the torrent when caps are present; cache per-torrent caps for verification.
    -   [ ] Bridge/native: allow per-torrent limits via `AddTorrentRequest` or immediate `update_limits`; ensure `NativeSession::update_limits` tolerates immediate calls.
    -   [ ] Tests: API parsing; worker ensures immediate cap application; native test confirms per-torrent caps apply without errors.

-   [ ] Alt-speed scheduling (global)

    -   [ ] Config/DB: add alt speed caps and simple schedules (time-of-day/weekday) to `EngineProfile`; validate.
    -   [ ] Runtime/bridge: include alt-speed caps/schedule in `EngineRuntimeConfig`/`EngineOptions`.
    -   [ ] Native: set libtorrent alt-speed settings/ schedule fields; ensure transitions respected.
    -   [ ] API/docs/tests: expose caps/schedule; document behavior; native test that schedule toggles caps as expected.

-   [ ] Seeding stop criteria (ratio/time) supported

    -   [ ] Config/DB: add optional `seed_ratio_limit`/`seed_time_limit` (and per-torrent defaults); validate non-negative numbers.
    -   [ ] Runtime/bridge: add to `EngineRuntimeConfig`/`EngineOptions`; set `lt::settings_pack::share_ratio_limit`/`seed_time_limit`; per-torrent via `torrent_handle` after add.
    -   [ ] Worker: allow per-torrent overrides on add and apply immediately.
    -   [ ] API/docs: expose optional per-torrent and profile defaults; document stop vs. pause behavior.
    -   [ ] Tests: config validation; worker applies caps; native test that ratio/time limits are honored.

-   [ ] Seed-mode / add-as-complete supported

    -   [ ] API: add flag to admit torrents in seed mode (skip full recheck) with warnings; optional hash sample preflight.
    -   [ ] Worker: carry flag in `EngineCommand::Add`; optionally perform sampled hash check; set seed mode when requested.
    -   [ ] Native: allow `lt::torrent_flags::seed_mode` / `torrent_handle::set_seed_mode` when user opts in.
    -   [ ] Tests: native test that seed-mode admission succeeds and does not recheck; safety tests around hash sample behavior.

-   [ ] Add-paused / queued admission supported

    -   [ ] API: add `paused`/`start_paused` flag to `TorrentCreateRequest`; validate.
    -   [ ] Worker: honor paused on `EngineCommand::Add`, queuing torrent without starting; emit appropriate state.
    -   [ ] Bridge/native: set `lt::torrent_flags::paused`/`auto_managed` on add per flag; avoid immediate start when paused.
    -   [ ] Tests: API parsing; worker leaves torrent paused; native test that add-paused does not start transfers.

-   [ ] Torrent queue priorities / auto-managed toggle

    -   [ ] Config/DB: add defaults for auto-managed behavior and queue priority policy to `EngineProfile`; validate booleans/priority bounds.
    -   [ ] Runtime/bridge: include flags in `EngineRuntimeConfig`/`EngineOptions`; allow per-torrent overrides in `AddTorrentOptions`.
    -   [ ] Native: control `lt::torrent_flags::auto_managed` on add based on config; expose queue priority settings via `settings_pack` (e.g., `auto_manage_prefer_seeds`, `dont_count_slow_torrents`).
    -   [ ] Worker: honor per-torrent auto-managed override when enqueuing adds.
    -   [ ] API/docs/tests: expose per-torrent auto-managed flag if needed; add tests for managed vs manual admission.

-   [ ] Choke/unchoke strategy configurable

    -   [ ] Config/DB: add choke/unchoke strategy fields (e.g., `unchoke_algorithm`, `seed_choking_algorithm`, `strict_super_seeding`, `optimistic_unchoke_slots`) with validated enums/bounds.
    -   [ ] Runtime/bridge: include these in `EngineRuntimeConfig`/`EngineOptions`.
    -   [ ] Native: set `lt::settings_pack` equivalents (`unchoke_algorithm`, `seed_choking_algorithm`, `strict_super_seeding`, `num_optimistic_unchoke_slots`, `max_queued_disk_bytes`) accordingly.
    -   [ ] API/docs/tests: expose only sane presets; document behavior; native tests verifying settings apply.

-   [ ] Super-seeding (initial seeding) supported

    -   [ ] Config/DB: add `super_seeding` default flag to `EngineProfile` and optional per-torrent override; validate booleans.
    -   [ ] Runtime/bridge: carry flags in `EngineRuntimeConfig`/`EngineOptions` and `AddTorrentOptions`.
    -   [ ] Native: toggle `torrent_handle::super_seeding` / `lt::torrent_flags::super_seeding` based on defaults/overrides on add.
    -   [ ] API/docs/tests: expose per-torrent super-seeding where appropriate; add native test that super-seeding applies on add.

-   [ ] Peer exchange (PEX) wired through

    -   [ ] Config/DB: add a `pex_enabled` flag to `EngineProfile` with default-off and validation.
    -   [ ] Runtime/bridge: include PEX toggle in `EngineRuntimeConfig`/`EngineOptions`.
    -   [ ] Native: set `lt::settings_pack::enable_outgoing_utp`/`enable_incoming_utp` as needed and explicitly enable/disable PEX (extensions) on the session/torrent handles.
    -   [ ] Worker: ensure per-torrent PEX behavior follows the profile flag; allow per-torrent override if required.
    -   [ ] API/docs/tests: expose flag if profile edits are public; document swarm/priv tracker implications; native test that PEX toggling applies without errors.

-   [ ] Post-add mutation symmetry for per-torrent knobs

    -   [ ] API: add PATCH endpoints to update per-torrent options currently set on add (rate caps, connection limits, PEX, super-seeding, seed ratio/time caps, queue priority, add-paused state transitions, tracker/web seed updates).
    -   [ ] Worker: add/update commands to apply these changes post-add; ensure state and metadata stay in sync.
    -   [ ] Bridge/native: expose libtorrent calls to update corresponding options on existing torrents; ignore/handle unsupported values safely.
    -   [ ] Tests: API + worker + native tests to confirm post-add updates apply and persist; regression tests for symmetry.

-   [ ] Tracker status and tracker ops surfaced

    -   [ ] Domain/API: persist tracker list/status per torrent; add endpoints to list/add/remove trackers and show per-tracker messages.
    -   [ ] Native: map tracker alerts into per-tracker status; support add/remove tracker operations via session/torrent handle.
    -   [ ] Tests: API + native tests for tracker add/remove and status propagation.

-   [ ] Tracker HTTP auth/cookies supported

    -   [ ] Config/DB: add tracker auth profile (basic auth/cookie headers) with secret refs; validate.
    -   [ ] Runtime/bridge: pass auth headers/cookies in `EngineRuntimeConfig`/`EngineOptions` and per-torrent overrides.
    -   [ ] Native: set per-tracker auth/cookies via libtorrent settings/`add_torrent_params` fields; handle updates.
    -   [ ] API/docs/tests: expose tracker auth configuration; native test that authenticated trackers connect successfully.

-   [ ] Web seeds (HTTP/URL seeds) supported

    -   [ ] API: accept web seed URLs on torrent create/update with validation (scheme/length).
    -   [ ] Worker: carry web seeds through `EngineCommand::Add` and update commands when needed.
    -   [ ] Bridge/native: include web seeds on `AddTorrentRequest` and apply via `add_torrent_params::url_seeds`; allow add/remove web seeds post-add.
    -   [ ] Tests: API parsing; native test that web seeds are attached and used without error.

-   [ ] Mid-download move/relocate supported

    -   [ ] API: add move/relocate endpoint and request payload (new path).
    -   [ ] Worker: introduce `EngineCommand::Move` to invoke storage relocation; integrate with FsOps expectations.
    -   [ ] Bridge/native: expose `move_storage` (or equivalent) in FFI and call `torrent_handle::move_storage` with proper flags.
    -   [ ] Tests: worker/native tests that move completes and events reflect new path; FsOps compatibility.

-   [ ] Advanced storage options integrated with FsOps

    -   [ ] Config/DB: add storage policy fields (e.g., sparse/allocation mode, partfile use, storage paths) to `EngineProfile`; validate choices.
    -   [ ] Runtime/bridge: carry storage policy in `EngineRuntimeConfig`/`EngineOptions` and per-torrent overrides in `AddTorrentOptions`.
    -   [ ] Native: apply storage options via `add_torrent_params` (e.g., `storage_mode_sparse`, partfile toggles) and coordinate storage location choices with FsOps expectations (paths, partfiles).
    -   [ ] FsOps integration: ensure storage layout (partfiles/temp paths) is compatible with post-processing/moves; document expectations.
    -   [ ] API/docs/tests: expose safe storage choices; tests to ensure storage options are honored and FsOps can operate on outputs.

-   [ ] Peer class/priority tagging supported

    -   [ ] Config/DB: allow defining peer classes and per-torrent class assignments; validate class IDs/ratios.
    -   [ ] Runtime/bridge: add peer class info to `EngineRuntimeConfig`/`EngineOptions` and per-torrent options.
    -   [ ] Native: use libtorrent peer classes (`peer_class_type_filter`, `set_peer_class_filter`, `set_peer_class_type_filter`) and `torrent_handle::set_peer_classes` to set per-peer/per-torrent priorities.
    -   [ ] API/docs: expose only if needed; document complexity and defaults.
    -   [ ] Tests: unit tests for class mapping; native test that class assignments apply without error.

-   [ ] Peer view and diagnostics exposed

    -   [ ] Domain/API: add peer listing (IP/flags/rates/client, progress) via inspector/endpoint.
    -   [ ] Native: surface peer info through FFI (translate peer_info structs); ensure alert polling or snapshot supports peer views.
    -   [ ] Tests: native tests that peers are returned; API tests for peer view responses.

-   [ ] Alert coverage and error surfacing expanded

    -   [ ] Bridge/native: map additional libtorrent alerts (tracker errors/warnings, listen/port binding failures, peer bans, storage errors, SSL verification issues) into `NativeEvent`/`EngineEvent` instead of dropping them.
    -   [ ] Worker: translate new events into `EventBus` health/state changes with actionable messages; ensure degraded health captures session and tracker/storage faults.
    -   [ ] API/UX: surface meaningful error messages in torrent status/details; document alert categories exposed.
    -   [ ] Tests: native (feature-gated) tests that injected tracker/storage/listen failures produce expected events and health degradation; unit tests for alert mapping coverage.

-   [ ] Session stats cadence tunable

    -   [ ] Config/DB: add `stats_interval_ms` to `EngineProfile` with sane bounds.
    -   [ ] Runtime/bridge: include in `EngineRuntimeConfig`/`EngineOptions`.
    -   [ ] Native: set `lt::settings_pack::stats_interval` to control alert cadence for stats; ensure worker can handle the volume.
    -   [ ] API/docs: document effect on telemetry volume; keep default conservative.
    -   [ ] Tests: config validation; native test that interval applies without error.

-   [ ] Disk cache/memory knobs exposed

    -   [ ] Config/DB: add cache-related fields (cache_size, cache_expiry, coalesce_reads, coalesce_writes, piece_hashes verification mode) to `EngineProfile`; validate bounds.
    -   [ ] Runtime/bridge: thread through `EngineRuntimeConfig`/`EngineOptions`.
    -   [ ] Native: set `lt::settings_pack` cache knobs (`cache_size`, `cache_expiry`, `coalesce_reads`, `coalesce_writes`, `use_disk_cache_pool`, `disk_io_write_mode`, `disk_io_read_mode`, piece hash verification settings) as needed.
    -   [ ] API/docs: expose only safe subset; document performance trade-offs.
    -   [ ] Tests: config validation; native test that cache settings apply and do not error.

-   [ ] TLS/SSL tracker and client cert configuration

    -   [ ] Config/DB: allow tracker SSL settings (trust store path, verify flags) and optional client cert/key references (stored as secrets).
    -   [ ] Runtime/bridge: carry SSL tracker settings in `EngineRuntimeConfig`/`EngineOptions`.
    -   [ ] Native: set `lt::settings_pack` SSL options (e.g., `ssl_cert`, `ssl_private_key`, `ssl_ca_cert`, `ssl_tracker_verify`) and apply per-tracker if supported.
    -   [ ] API/docs/tests: expose only if needed; document security implications; native test that SSL settings apply and tracker connections succeed.

-   [ ] BitTorrent v2/hybrid torrents supported

    -   [ ] API: accept v2/hybrid metainfo and expose v2 fields where relevant; validate size.
    -   [ ] Worker/bridge: ensure add path passes v2/hybrid payloads unchanged.
    -   [ ] Native: confirm libtorrent is built with v2 support and accepts hybrid torrents; adjust event mapping if v2-specific metadata arises.
    -   [ ] Tests: feature-gated native test adding a v2/hybrid torrent to ensure admission succeeds.

-   [ ] Torrent creation (authoring) supported

    -   [ ] API: add create-torrent endpoint to build `.torrent`/magnet from local files/dirs with options (piece size, private flag, trackers, web seeds, comment, source).
    -   [ ] Worker/bridge: introduce command to invoke libtorrent create_torrent; marshal options; return metainfo/magnet.
    -   [ ] Native: expose create-torrent path via FFI using `lt::create_torrent` + bencode; handle file traversal safely.
    -   [ ] Tests: API + native tests creating torrents with various options; ensure outputs validate.

-   [ ] Comments/source/private flag visibility and updates

    -   [ ] API: surface per-torrent comment/source/private flag in details; allow updates where safe.
    -   [ ] Worker/bridge: carry comment/source/private flag through add and update paths; for private flag, respect tracker requirements.
    -   [ ] Native: apply comment/source/private settings via libtorrent structures on add and, where supported, updates.
    -   [ ] Tests: API + native tests ensuring fields are exposed and updates apply where allowed.

-   [ ] Categories/tags with policy and cleanup

    -   [ ] Domain/API: support categories/tags/labels on torrents; endpoints to list/create/update categories/tags.
    -   [ ] Policy: allow per-category/tag defaults (paths, rate limits, queue priority, auto-cleanup after ratio/time).
    -   [ ] Worker/FsOps: apply category/tag-derived settings on add; integrate cleanup policy (remove after criteria) and FsOps paths.
    -   [ ] Tests: API + worker tests for category/tag behavior; cleanup policy tested against ratio/time thresholds.

-   [ ] qB-style API surface parity (if required)

    -   [ ] Implement compatible endpoints for trackers (`/torrents/trackers` + ops), peers (`/sync/torrentPeers`), full torrent properties (`/torrents/properties`), categories/tags endpoints, aligned with our domain and security posture.
    -   [ ] Tests: compatibility tests for the façade, ensuring responses match expected shapes/behaviors.

-   [ ] Streaming/piece-deadline support exposed

    -   [ ] API: optional streaming flag or piece deadlines for preview use-cases.
    -   [ ] Worker: new command to set piece deadlines/read-ahead for a torrent.
    -   [ ] Bridge/native: expose `set_piece_deadline`/`reset_piece_deadline` and read-ahead knobs through FFI; guard usage to streaming paths.
    -   [ ] Tests: native test that deadlines apply and do not break normal transfers.
