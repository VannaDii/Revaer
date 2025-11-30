//! HTTP surface modules (routers, handlers, compatibility layers).

/// Authentication middleware and helpers.
pub mod auth;
#[cfg(feature = "compat-qb")]
pub mod compat_qb;
/// Shared constants and header names for HTTP surfaces.
pub mod constants;
/// OpenAPI document publishing.
pub mod docs;
/// Problem response helpers and error types.
pub mod errors;
/// Health and diagnostics endpoints.
pub mod health;
/// Router construction and server host.
pub mod router;
/// Settings/configuration handlers.
pub mod settings;
/// Setup bootstrap handlers.
pub mod setup;
/// Server-sent events filters and streaming utilities.
pub mod sse;
/// Metrics middleware for HTTP requests.
pub mod telemetry;
/// Torrent-facing HTTP helpers (pagination, filtering, metadata views).
pub mod torrents;
