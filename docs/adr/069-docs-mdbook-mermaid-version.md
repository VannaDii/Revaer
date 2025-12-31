# Docs: Pin mdbook-mermaid for just docs

- Status: Accepted
- Date: 2025-12-31
- Context:
  - Motivation: `just docs` failed because mdbook-mermaid 0.16.2 cannot parse under mdbook 0.5.2, even though docs are valid.
  - Constraints: Docs build must run via `just`, no manual tooling, avoid repo changes outside the justfile.
  - Test coverage summary: `just docs` run after change; no unit tests applicable.
  - Observability updates: None.
  - Dependency rationale: No new crates; pin existing mdbook-mermaid tool to 0.17.0 to match mdbook 0.5.x behavior.

- Decision:
  - Require mdbook-mermaid 0.17.0 in `just docs-install` and reinstall if mismatched.
  - Make `just docs` invoke `just docs-install` before build and index.
  - Alternatives considered: rely on user-managed tool versions; pin mdbook to 0.5.0; remove mermaid preprocessor.

- Consequences:
  - Positive outcomes: `just docs` consistently installs a compatible mermaid preprocessor and builds successfully.
  - Risks or trade-offs: Running `just docs` may reinstall mdbook-mermaid when versions differ; version pin may lag future mdbook releases.
  - Risk & rollback plan: If issues arise, revert the `justfile` change or update the pinned version and rerun `just docs`.

- Follow-up:
  - Implementation tasks: Update `justfile` and verify `just docs`.
  - Review checkpoints: Revisit the pin when mdbook or mdbook-mermaid releases require it.
