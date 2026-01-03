//! Default identifiers and retention policies for configuration records.
//!
//! # Design
//! - Centralize default IDs so database/config expectations stay consistent.
//! - Keep time-based defaults explicit for auditability.

/// Static ID for the singleton application profile row.
pub(crate) const APP_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000001";
/// Static ID for the singleton engine profile row.
pub(crate) const ENGINE_PROFILE_ID: &str = "00000000-0000-0000-0000-000000000002";
/// Static ID for the singleton filesystem policy row.
pub(crate) const FS_POLICY_ID: &str = "00000000-0000-0000-0000-000000000003";
/// Default API key TTL window in days.
pub(crate) const API_KEY_TTL_DAYS: i64 = 14;
