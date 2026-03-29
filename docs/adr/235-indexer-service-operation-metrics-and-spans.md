# Indexer Service Operation Metrics And Spans

- Status: Accepted
- Date: 2026-03-08
- Context:
  - `ERD_INDEXERS_CHECKLIST.md` still had the observability item open for indexer-domain operations.
  - The API already emitted request spans for indexer endpoints, and the app-layer `IndexerService` already wrapped each domain operation in a stable tracing span.
  - What was missing was consistent per-operation metrics at the domain-service boundary so search, routing, policy, torznab, instance, and secret workflows all emitted a stable success/error signal and latency measurement.
- Decision:
  - Extend `revaer-telemetry` with indexer service operation counters and latency histograms labeled by operation and outcome.
  - Inject `Metrics` into `IndexerService` via bootstrap and test wiring instead of constructing telemetry inside the service.
  - Route every `IndexerFacade` operation through a single helper that records success/error outcomes and elapsed latency around the already-instrumented spans.
- Consequences:
  - Indexer-domain operations now emit stable metrics and spans from the API boundary through the app-service boundary without violating the DI rule.
  - Troubleshooting can distinguish success versus error rates per operation and correlate them with the existing tracing spans.
  - The metrics surface grows slightly, but only with bounded low-cardinality labels (`operation`, `outcome`).
- Follow-up:
  - Add dashboard panels and alerts for the new `indexer_operations_total` and `indexer_operation_latency_ms` series when the indexer health UI is built.
  - Keep new indexer-domain methods on the shared `run_operation` helper so observability coverage does not regress.
