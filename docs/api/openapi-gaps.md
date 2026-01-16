# OpenAPI Coverage Gaps

This document lists API routes present in `crates/revaer-api/src/http/router.rs` that are missing from `docs/api/openapi.json`.

## Summary

- The current OpenAPI spec omits several admin torrent endpoints and multiple v1 utility/streaming routes.
- The missing routes below should be added to the OpenAPI spec to fully reflect the HTTP surface area.

## Missing admin routes

| Path | Methods |
| --- | --- |
| `/admin/factory-reset` | `POST` |
| `/admin/torrents` | `GET`, `POST` |
| `/admin/torrents/categories` | `GET` |
| `/admin/torrents/categories/{name}` | `PUT` |
| `/admin/torrents/tags` | `GET` |
| `/admin/torrents/tags/{name}` | `PUT` |
| `/admin/torrents/create` | `POST` |
| `/admin/torrents/{id}` | `GET`, `DELETE` |
| `/admin/torrents/{id}/peers` | `GET` |

## Missing v1 routes

| Path | Methods |
| --- | --- |
| `/v1/dashboard` | `GET` |
| `/v1/fs/browse` | `GET` |
| `/v1/logs/stream` | `GET` |
| `/v1/torrents/{id}/trackers` | `GET`, `PATCH`, `DELETE` |
| `/v1/torrents/{id}/peers` | `GET` |
| `/v1/torrents/{id}/web_seeds` | `PATCH` |
| `/v1/events` | `GET` |
| `/v1/events/stream` | `GET` |

## Notes

- Feature-gated compat-qb routes are excluded because they are not mounted unless the `compat-qb` feature is enabled.
