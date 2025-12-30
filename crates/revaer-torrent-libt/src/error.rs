//! # Design
//!
//! - Centralize libtorrent adapter error context without using `anyhow`.
//! - Keep error messages constant; store operational context in fields.
//! - Provide helpers to build `TorrentError` with structured sources.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use revaer_torrent_core::TorrentError;
use uuid::Uuid;

#[derive(Debug)]
/// Internal error details used by the libtorrent adapter.
pub enum LibtorrentError {
    /// A required field was missing from a request payload.
    MissingField {
        /// Field name that was missing.
        field: &'static str,
    },
    /// A request contained an invalid field value.
    InvalidInput {
        /// Field name with an invalid value.
        field: &'static str,
        /// Static reason describing the invalid value.
        reason: &'static str,
    },
    /// The libtorrent session was unavailable for the requested operation.
    SessionUnavailable {
        /// Operation that could not be serviced.
        operation: &'static str,
    },
    /// A native libtorrent call reported a failure.
    NativeFailure {
        /// Operation that triggered the failure.
        operation: &'static str,
        /// Native error message payload.
        message: String,
    },
    /// A fastresume store IO operation failed.
    StoreIo {
        /// Operation that triggered the IO failure.
        operation: &'static str,
        /// Path involved in the IO failure.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// A fastresume store parse operation failed.
    StoreParse {
        /// Operation that triggered the parse failure.
        operation: &'static str,
        /// Path involved in the parse failure.
        path: PathBuf,
        /// Underlying JSON parse error.
        source: serde_json::Error,
    },
}

impl Display for LibtorrentError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingField { field } => {
                let _ = field;
                formatter.write_str("required field missing")
            }
            Self::InvalidInput { field, reason } => {
                let _ = (field, reason);
                formatter.write_str("invalid torrent input")
            }
            Self::SessionUnavailable { operation } => {
                let _ = operation;
                formatter.write_str("libtorrent session unavailable")
            }
            Self::NativeFailure { operation, message } => {
                let _ = (operation, message);
                formatter.write_str("libtorrent native error")
            }
            Self::StoreIo {
                operation, path, ..
            } => {
                let _ = (operation, path);
                formatter.write_str("fastresume store IO failure")
            }
            Self::StoreParse {
                operation, path, ..
            } => {
                let _ = (operation, path);
                formatter.write_str("fastresume store parse failure")
            }
        }
    }
}

impl Error for LibtorrentError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::StoreIo { source, .. } => Some(source),
            Self::StoreParse { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Build a torrent error with structured operation context.
pub fn op_failed(
    operation: &'static str,
    torrent_id: Option<Uuid>,
    source: impl Error + Send + Sync + 'static,
) -> TorrentError {
    TorrentError::OperationFailed {
        operation,
        torrent_id,
        source: Box::new(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::Error as _;
    use std::error::Error;
    use std::io;

    fn json_error() -> serde_json::Error {
        match serde_json::from_str::<serde_json::Value>("invalid") {
            Ok(_) => serde_json::Error::custom("expected invalid json"),
            Err(err) => err,
        }
    }

    #[test]
    fn libtorrent_error_display_and_source() {
        let cases = vec![
            (
                LibtorrentError::MissingField { field: "path" },
                "required field missing",
                false,
            ),
            (
                LibtorrentError::InvalidInput {
                    field: "sample",
                    reason: "too large",
                },
                "invalid torrent input",
                false,
            ),
            (
                LibtorrentError::SessionUnavailable {
                    operation: "add_torrent",
                },
                "libtorrent session unavailable",
                false,
            ),
            (
                LibtorrentError::NativeFailure {
                    operation: "add_torrent",
                    message: "native error".to_string(),
                },
                "libtorrent native error",
                false,
            ),
            (
                LibtorrentError::StoreIo {
                    operation: "read",
                    path: PathBuf::from("store"),
                    source: io::Error::other("io"),
                },
                "fastresume store IO failure",
                true,
            ),
            (
                LibtorrentError::StoreParse {
                    operation: "parse",
                    path: PathBuf::from("store"),
                    source: json_error(),
                },
                "fastresume store parse failure",
                true,
            ),
        ];

        for (err, message, has_source) in cases {
            assert_eq!(err.to_string(), message);
            assert_eq!(err.source().is_some(), has_source);
        }
    }

    #[test]
    fn op_failed_wraps_torrent_error() -> Result<(), Box<dyn Error>> {
        let torrent_id = Uuid::nil();
        let err = op_failed("add", Some(torrent_id), io::Error::other("io"));
        match err {
            TorrentError::OperationFailed {
                operation,
                torrent_id: Some(id),
                source,
            } => {
                assert_eq!(operation, "add");
                assert_eq!(id, torrent_id);
                assert_eq!(source.to_string(), "io");
                Ok(())
            }
            _ => Err(io::Error::other("expected operation failed").into()),
        }
    }
}
