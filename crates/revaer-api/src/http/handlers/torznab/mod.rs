//! Torznab endpoint handlers.
//!
//! # Design
//! - Authenticate using the query-string API key only (no headers in v1).
//! - Delegate data access to stored-procedure-backed services.
//! - Return deterministic XML responses and avoid DB writes for invalid requests.

pub mod api;
pub mod download;
mod xml;

pub(crate) use api::torznab_api;
pub(crate) use download::torznab_download;
