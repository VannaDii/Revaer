//! HTTP handler modules for the API surface.
//!
//! # Design
//! - Keep handlers scoped to a single feature/route group.
//! - Delegate shared concerns (auth, rate limits) to middleware modules.

#[cfg(feature = "compat-qb")]
pub mod compat_qb;
pub mod docs;
pub mod filesystem;
pub mod health;
pub mod logs;
pub mod settings;
pub mod setup;
pub mod sse;
pub mod tokens;
pub mod torrents;
