# AGENT.md Compliance Gaps

- [x] Pin toolchain to workspace MSRV in `rust-toolchain.toml` (channel `1.91.0`).
- [x] Align `just lint` to AGENT recipe (`cargo clippy --workspace --all-targets --all-features -- -D warnings`).
- [x] Standardize crate lint prelude to the AGENT set (`clippy::cargo` added across crates; unsafe isolation applied to `revaer-torrent-libt`).
- [x] Remove banned `#[allow(...)]` on the FFI surface and isolate unsafe to `ffi.rs` with `#![forbid(unsafe_code)]` in `revaer-torrent-libt`.
- [x] Reorganize `revaer-api` to the service/daemon layout (`config/`, `app/`, `http/`, `infra/`, thin `lib.rs`, scoped tests).
- [x] Reorganize `revaer-config` into modules with a thin `lib.rs` (models/validate/service split; DB facade moved out of `lib.rs`).
- [x] Split `revaer-fsops` so `lib.rs` is thin and delegates to `service.rs` (pipeline + IO); further model/policy extraction remains a future refinement.
- [x] Split `revaer-torrent-core` into `model/` (DTOs) and `service/` (traits) with a thin `lib.rs` re-export surface.
- [x] Align `revaer-torrent-libt` crate layout (`ffi.rs`, `types.rs`, `adapter.rs`, `convert.rs`, thin `lib.rs`) and remove the non-send allowance (unsafe isolated to `ffi`, `clippy::non_send_fields_in_send_ty` allowance removed).
- [x] Lift coverage back above the 80% gate (added adapter and API stub config tests to satisfy `just cov`).
- [x] Apply the prescribed UI structure (app/*, core/* logic, services/api+sse, features/* with view/state/logic/api, components presentational only).
- [x] Restructure CLI/runtime crates to match archetypes (commands/, client.rs, output.rs; runtime bootstrap separated) (runtime crate now follows thin module layout; CLI now split into cli.rs, client.rs, commands/, output.rs with coverage restored).
- [x] Expand CI: add MSRV job (1.91.0), feature-matrix runs for flags, and release artifacts (OpenAPI, SBOM/license report) driven by `just` (toolchain pinned; MSRV + feature-matrix jobs added; release job emits OpenAPI/SBOM/license artifacts).
- [x] Add an ADR covering `.secignore`/`deny.toml` advisory ignores (e.g., RUSTSEC-2024-0370) with remediation plan (`docs/adr/019-advisory-rustsec-2024-0370.md`).
