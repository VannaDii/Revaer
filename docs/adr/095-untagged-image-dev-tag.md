# Untagged images use dev tag

- Status: Accepted
- Date: 2026-01-23
- Context:
  - What problem are we solving?
    - Untagged builds currently publish to a separate `-dev` image name and still apply a `latest` tag, making it harder to discover the intended development tag.
  - What constraints or forces shape the decision?
    - Keep tagging logic in the GitHub workflow without altering the build artifacts or Dockerfile.
- Decision:
  - Summary of the choice made.
    - Publish untagged builds to the primary image name with a `dev` tag, while tagged builds retain `latest`.
  - Alternatives considered.
    - Keep the `-dev` image suffix and add an extra `dev` alias tag.
- Consequences:
  - Positive outcomes.
    - Untagged images are clearly labeled as development artifacts in the primary repository.
  - Risks or trade-offs.
    - Development images now share the same repository name as releases, requiring clear tag usage.
- Follow-up:
  - Implementation tasks.
    - Monitor downstream consumers for any references to the previous `-dev` image name.
  - Review checkpoints.
    - Reassess if consumers need both `dev` and `latest` tags for untagged builds.

## Task record

- Motivation: Align untagged image naming with a `dev` tag instead of a separate `-dev` repository and `latest`.
- Design notes: Use a workflow alias tag that switches between `latest` and `dev` based on ref type.
- Test coverage summary: `just ci` and `just ui-e2e` passed.
- Observability updates: None (workflow-only change).
- Risk & rollback plan: Revert the alias tag logic in `.github/workflows/ci.yml` if consumers depend on `revaer-dev` or `latest`.
- Dependency rationale: No new dependencies introduced.
