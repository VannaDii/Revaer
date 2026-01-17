# OpenAPI Coverage Gaps

This document lists API routes present in `crates/revaer-api/src/http/router.rs` that are missing from `docs/api/openapi.json`.

## Summary

- The OpenAPI spec is aligned with the current router surface; no gaps remain for the default feature set.

## Missing admin routes

- None.

## Missing v1 routes

- None.

## Notes

- Feature-gated compat-qb routes are excluded because they are not mounted unless the `compat-qb` feature is enabled.
