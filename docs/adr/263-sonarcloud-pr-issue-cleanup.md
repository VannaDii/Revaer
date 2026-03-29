# SonarCloud PR issue cleanup and scope alignment

- Status: Accepted
- Date: 2026-03-29
- Context:
  - PR `#6` introduced live SonarCloud failures on reliability, coverage, duplication, and security hotspots.
  - The fresh SonarCloud API issue list showed that most findings came from PostgreSQL migration SQL being analyzed with generic PL/SQL rules, plus generated Playwright API schema output and repetitive contract-style test files being counted in duplication and coverage gates.
- Decision:
  - Fix the actionable Rust and test findings directly in code.
  - Add checked-in Sonar scope configuration so PostgreSQL migration SQL is excluded from Sonar issue, duplication, and coverage gating, generated API schema output is excluded from naming-rule noise, repetitive Playwright contract files do not dominate duplication metrics, and Rust coverage remains enforced by the repository's existing `just cov` gate rather than a second Sonar coverage gate with different long-lived-branch semantics.
  - Alternatives considered: refactor every migration and generated artifact to satisfy Sonar’s non-PostgreSQL rules, or leave the gate failing. Both were rejected because they would create noise without improving runtime safety.
- Consequences:
  - Positive: SonarCloud quality gates stay focused on application code and actionable regressions.
  - Trade-off: Sonar scope must be kept aligned if migration, generated-file, or Rust source layouts move.
- Follow-up:
  - Re-run SonarCloud after pushing the branch and verify the PR issue list reflects the new scope.
  - Revisit exclusions if SonarCloud adds PostgreSQL-aware analysis that can replace the current PL/SQL false positives.

## Task record

- Motivation:
  - Clear the live SonarCloud PR gate using the fresh API issue list instead of stale screenshots.
- Design notes:
  - Keep real behavior fixes in code, and record scope adjustments in repository-owned Sonar config rather than ad-hoc CI arguments only.
  - Use repository-local `just cov` as the authoritative Rust coverage gate and let Sonar focus on issue, duplication, and hotspot feedback for the PR.
- Test coverage summary:
  - Validate with `just ci` and `just ui-e2e` after the Sonar cleanup changes.
- Observability updates:
  - No runtime telemetry changes required.
- Risk & rollback plan:
  - Risk is hiding meaningful future findings if exclusions are too broad; rollback is removing or narrowing the Sonar scope entries and re-running the scan.
- Dependency rationale:
  - No new dependencies added. Alternatives considered: none required.
