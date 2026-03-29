# Cross-platform allocation safety probe

- Status: Accepted
- Date: 2026-02-01
- Context:
  - Motivation: Allocation safety relied on `/proc/meminfo`, which is Linux-only. We need a
    cross-platform source of *live* available memory so we do not lock into Linux.
  - Constraints: Keep error messages constant; avoid unsafe code; preserve minimal dependencies.
- Decision:
  - Use `systemstat` to fetch live memory statistics on all platforms.
  - Prefer Linux `MemAvailable` when present, otherwise fall back to the live free-memory value
    returned by `systemstat`.
  - Keep the 80% available-memory guard and fail closed when memory cannot be determined.
  - Dependency rationale: `systemstat` provides cross-platform live memory data without adding
    unsafe code in Revaer. Alternatives considered: OS-specific FFI (requires unsafe) or estimates
    (rejected).
- Consequences:
  - Positive outcomes: Allocation guard works on macOS/Windows/Linux; no platform lock-in.
  - Risks or trade-offs: Adds a small dependency footprint; relies on OS-reported statistics.
- Follow-up:
  - Implementation tasks: update allocation helper to use `systemstat`; add docs entry.
  - Test coverage summary: allocation guard unit tests remain; live-memory probe is exercised via
    API/CLI/E2E.
  - Observability updates: none.
