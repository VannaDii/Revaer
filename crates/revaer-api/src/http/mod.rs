//! HTTP surface modules (routers, handlers, compatibility layers).

/// Authentication middleware and helpers.
pub mod auth;
/// Shared constants and header names for HTTP surfaces.
pub mod constants;
/// Request/response DTOs and error helpers.
pub mod dto;
/// Endpoint handlers grouped by feature.
pub mod handlers;
/// Rate limit helpers for HTTP responses.
pub mod rate_limit;
/// Router construction and server host.
pub mod router;
/// Metrics middleware for HTTP requests.
pub mod telemetry;

/// Error responses and `ProblemDetails` helpers.
pub use dto::errors;
#[cfg(feature = "compat-qb")]
/// qBittorrent compatibility handlers (feature-gated).
pub use handlers::compat_qb;
/// `OpenAPI` document handlers.
pub use handlers::docs;
/// Filesystem browser handlers.
pub use handlers::filesystem;
/// Health and diagnostics handlers.
pub use handlers::health;
/// Log streaming handlers.
pub use handlers::logs;
/// Settings/configuration handlers.
pub use handlers::settings;
/// Setup bootstrap handlers.
pub use handlers::setup;
/// Server-sent events handlers.
pub use handlers::sse;
/// API token refresh handlers.
pub use handlers::tokens;
/// Torrent-facing HTTP handlers and helpers.
pub use handlers::torrents;
