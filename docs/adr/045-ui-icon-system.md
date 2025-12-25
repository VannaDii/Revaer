# UI Icon System and Icon Buttons

- Status: Accepted
- Date: 2025-12-24
- Context:
  - Motivation: eliminate inline SVGs and standardize icon usage per the dashboard checklist.
  - Constraints: reuse Nexus/DaisyUI styling, avoid new dependencies, keep accessibility consistent.
- Decision:
  - Summary: add a shared icon module under `components/atoms/icons` and a reusable `IconButton` component for icon-only actions.
  - Design notes: provide `IconProps` (size, class, optional title) and `IconVariant` for outline/solid arrows; reuse existing `.icon-btn` styles for consistent hover/focus behavior.
  - Alternatives considered: keep inline SVGs or introduce an external icon crate; rejected to avoid duplication and dependencies.
- Consequences:
  - Positive outcomes: centralized icon rendering, consistent sizing, and cleaner shell/dashboard markup.
  - Risks/trade-offs: visual regressions if CSS assumptions about SVG sizing shift.
  - Observability updates: none.
- Follow-up:
  - Implementation tasks: keep new icons in the shared module; replace any future inline SVGs with components.
  - Test coverage summary: UI component wiring only; no new tests added (llvm-cov still warns about mismatched data).
  - Dependency rationale: no new dependencies introduced.
  - Risk & rollback plan: revert icon module changes and restore inline SVGs if styling regresses.
