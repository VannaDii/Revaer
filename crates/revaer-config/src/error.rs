//! Error types for configuration operations.

use std::io;

use argon2::password_hash::Error as PasswordHashError;
use thiserror::Error;

/// Primary error type for configuration operations.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Attempted to modify a field marked as immutable.
    #[error("immutable configuration field")]
    ImmutableField {
        /// Section containing the immutable field.
        section: String,
        /// Name of the immutable field.
        field: String,
    },
    /// Field contained an invalid value.
    #[error("invalid configuration field")]
    InvalidField {
        /// Section that failed validation.
        section: String,
        /// Field that failed validation.
        field: String,
        /// Offending value when available.
        value: Option<String>,
        /// Machine-readable reason for the failure.
        reason: &'static str,
    },
    /// Field did not exist in the target section.
    #[error("unknown configuration field")]
    UnknownField {
        /// Section where the unknown field was encountered.
        section: String,
        /// Name of the unexpected field.
        field: String,
    },
    /// Label kind value was invalid.
    #[error("invalid label kind")]
    InvalidLabelKind {
        /// Label kind payload provided by the caller.
        value: String,
    },
    /// App mode value was invalid.
    #[error("invalid app mode")]
    InvalidAppMode {
        /// App mode payload provided by the caller.
        value: String,
    },
    /// UUID value was invalid.
    #[error("invalid UUID")]
    InvalidUuid {
        /// UUID payload provided by the caller.
        value: String,
    },
    /// Bind address value was invalid.
    #[error("invalid bind address")]
    InvalidBindAddr {
        /// Bind address payload provided by the caller.
        value: String,
    },
    /// A setup token is missing.
    #[error("setup token missing")]
    SetupTokenMissing,
    /// A setup token has expired.
    #[error("setup token expired")]
    SetupTokenExpired,
    /// A setup token is invalid.
    #[error("setup token invalid")]
    SetupTokenInvalid,
    /// Failed to hash secret material.
    #[error("failed to hash secret material")]
    SecretHashFailed {
        /// Hashing error detail.
        detail: PasswordHashError,
    },
    /// Stored secret hash payload was invalid.
    #[error("invalid stored hash")]
    StoredHashInvalid {
        /// Hash parsing error detail.
        detail: PasswordHashError,
    },
    /// Secret verification failed.
    #[error("failed to verify secret")]
    SecretVerifyFailed {
        /// Verification error detail.
        detail: PasswordHashError,
    },
    /// Configuration change notification payload was invalid.
    #[error("invalid notification payload")]
    NotificationPayloadInvalid,
    /// Configuration change notification payload missing revision.
    #[error("missing revision in notification payload")]
    NotificationPayloadMissingRevision,
    /// Underlying database operation failed.
    #[cfg(not(target_arch = "wasm32"))]
    #[error("database operation failed")]
    Database {
        /// Operation identifier.
        operation: &'static str,
        /// Source database error.
        source: sqlx::Error,
    },
    /// Data layer operation failed.
    #[cfg(not(target_arch = "wasm32"))]
    #[error("data access failed")]
    DataAccess {
        /// Operation identifier.
        operation: &'static str,
        /// Source data-layer error.
        source: revaer_data::DataError,
    },
    /// File system operation failed.
    #[error("filesystem operation failed")]
    Io {
        /// Operation identifier.
        operation: &'static str,
        /// Source IO error.
        source: io::Error,
    },
}

/// Convenience alias for configuration results.
pub type ConfigResult<T> = Result<T, ConfigError>;
