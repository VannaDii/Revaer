# 006 – Unified API & CLI Contract

- Status: Accepted
- Date: 2025-10-17

## Context
- Phase One requires parity between the public HTTP interface and the administrative CLI so operators can automate without reverse engineering payloads.
- Prior iterations lacked shared DTOs, consistent Problem+JSON responses, and stable pagination/SSE semantics across API and CLI.
- New rate limiting and telemetry features must surface identically on both surfaces to satisfy observability and security requirements.

## Decision
- Shared request/response models live in `revaer-api::models` and are re-exported to the CLI, ensuring identical JSON encoding/decoding paths.
- All routes return RFC9457 Problem+JSON payloads on validation/runtime errors, including `invalid_params` pointers for user-correctable mistakes; the CLI pretty-prints these problems and maps validation to exit code `2`.
- Cursor pagination, filter semantics, and SSE replay (`Last-Event-ID`) are implemented once in the API and exercised by dedicated CLI commands (`ls`, `status`, `tail`).
- The CLI propagates `x-request-id` headers, emits structured telemetry events to `REVAER_TELEMETRY_ENDPOINT`, and redacts secrets in logs; runtime failures exit with code `3` to distinguish from validation issues.

## Consequences
- Changes to the API contract require updates in a single module (`revaer-api::models`), reducing the risk of CLI drift.
- Downstream tooling can rely on deterministic exit codes and Problem+JSON payloads, simplifying automation.
- Telemetry pipelines receive consistent trace identifiers regardless of whether requests originate from the CLI or other clients.

## Verification
- Integration tests cover pagination, filter validation, SSE replay, and CLI HTTP interactions via `httpmock`, ensuring behaviour remains in lockstep.
- `just api-export` regenerates `docs/api/openapi.json`, and CI asserts the CLI uses the shared DTOs by compiling with the workspace feature set.
