# Reusable image build workflow

- Status: Accepted
- Date: 2026-01-24
- Context:
  - What problem are we solving?
    - Image build logic is duplicated across CI and PR workflows, and CI was failing to load due to invalid tag filters.
  - What constraints or forces shape the decision?
    - Keep CI driven by `just`, avoid dev tag releases updating stable artifacts, and reduce workflow duplication.
- Decision:
  - Summary of the choice made.
    - Introduce a reusable workflow for multi-arch image build/manifest creation and use it from both CI and PR workflows, while gating CI roots to skip dev tag pushes.
  - Alternatives considered.
    - Keep duplicated image steps in each workflow.
    - Split tag builds into a separate workflow without reuse.
- Consequences:
  - Positive outcomes.
    - Consistent image build behavior across workflows with less duplication and clear tag policies.
  - Risks or trade-offs.
    - Reusable workflows add indirection when tracing failures.
- Follow-up:
  - Implementation tasks.
    - Monitor build images runs for any tag mismatches or manifest issues.
  - Review checkpoints.
    - Revisit tag gating if GitHub tag filters expand to support exclusion patterns.

## Task record

- Motivation: Fix CI failures and share image build logic between CI and PR workflows.
- Design notes: Use a reusable workflow with parameterized tags and checkout refs to drive both dev and PR image builds.
- Test coverage summary: `just ci`, `just ui-e2e`.
- Observability updates: None (workflow-only change).
- Risk & rollback plan: Revert to inline workflow steps if reuse introduces instability.
- Dependency rationale: No new dependencies introduced.
