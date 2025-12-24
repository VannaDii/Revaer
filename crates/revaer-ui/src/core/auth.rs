//! Authentication primitives shared across the UI.
//!
//! # Design
//! - Keep auth state as simple data so callers can store/clear it without side effects.
//! - Treat empty credentials as unauthenticated at the call site.
//! - Leave header encoding to transport clients to keep core DOM-free.

/// Supported authentication modes for the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthMode {
    /// API key header authentication.
    ApiKey,
    /// Local username/password authentication.
    Local,
}

/// Local username/password credential pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LocalAuth {
    /// Local username value.
    pub username: String,
    /// Local password value.
    pub password: String,
}

/// Active authentication state for outbound requests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AuthState {
    /// API key-based authentication.
    ApiKey(String),
    /// Local basic authentication.
    Local(LocalAuth),
    /// Explicit anonymous access (no headers).
    Anonymous,
}
