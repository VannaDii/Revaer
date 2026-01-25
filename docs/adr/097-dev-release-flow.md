# Dev prereleases and PR image previews

- Status: Accepted
- Date: 2026-01-24
- Context:
  - What problem are we solving?
    - Main should publish dev prereleases and dev-tagged images without displacing stable “latest” artifacts.
    - PRs need preview images without exposing secrets to forks.
  - What constraints or forces shape the decision?
    - CI must run via `just`, releases must be semver-based from Conventional Commits, and stable releases/images stay version-tagged.
- Decision:
  - Summary of the choice made.
    - Use semantic-release on main to publish `-dev.N` prereleases with attached artifacts, tag dev images with the prerelease tag plus `dev`, and publish PR preview images for non-fork PRs using `pr-<num>` and `pr-<num>-<sha>` tags only.
  - Alternatives considered.
    - Continue tag-only releases (no dev prereleases).
    - Publish dev images under a separate repository name.
- Consequences:
  - Positive outcomes.
    - Main builds produce versioned dev releases and dev images without changing the stable “latest” artifacts.
    - Non-fork PRs get preview images with consistent tags.
  - Risks or trade-offs.
    - Adds release tooling dependencies and requires Conventional Commit discipline for every main merge.
- Follow-up:
  - Implementation tasks.
    - Monitor semantic-release output and adjust release rules if release cadence is too strict or too noisy.
  - Review checkpoints.
    - Revisit tag patterns if GitHub tag filters or image consumers need additional aliases.

## Task record

- Motivation: Publish dev prereleases and PR preview images without displacing stable releases or `latest` images.
- Design notes: Semantic-release prereleases on main drive version tags; PR images are tagged `pr-<num>` and `pr-<num>-<sha>` only.
- Test coverage summary: `just ci`, `just ui-e2e`.
- Observability updates: None (workflow-only change).
- Risk & rollback plan: Remove release-dev and PR image jobs and revert to tag-only releases if prereleases cause instability.
- Dependency rationale: Add semantic-release tooling in `release/` to analyze Conventional Commits and publish prereleases with assets.
