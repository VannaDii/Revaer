//! Error types for torrent core services.

use std::error::Error;

use thiserror::Error;
use uuid::Uuid;

/// Primary error type for torrent operations.
#[derive(Debug, Error)]
pub enum TorrentError {
    /// Operation is not supported by the underlying engine.
    #[error("torrent operation not supported")]
    Unsupported {
        /// Operation identifier.
        operation: &'static str,
    },
    /// Operation failed in the underlying engine.
    #[error("torrent operation failed")]
    OperationFailed {
        /// Operation identifier.
        operation: &'static str,
        /// Torrent identifier when available.
        torrent_id: Option<Uuid>,
        /// Underlying failure.
        #[source]
        source: Box<dyn Error + Send + Sync>,
    },
    /// Torrent was not found.
    #[error("torrent not found")]
    NotFound {
        /// Missing torrent identifier.
        torrent_id: Uuid,
    },
}

/// Convenience alias for torrent operation results.
pub type TorrentResult<T> = Result<T, TorrentError>;
