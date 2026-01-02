//! # Design
//!
//! - Centralize application-level errors for bootstrap and orchestration.
//! - Keep error messages constant while carrying context fields for debugging.
//! - Preserve source errors without re-logging at call sites.

use std::io;
use std::path::PathBuf;

use thiserror::Error;

/// Result alias for application operations.
pub type AppResult<T> = Result<T, AppError>;

/// Application-level error type.
#[derive(Debug, Error)]
pub enum AppError {
    /// Environment configuration was missing.
    #[error("missing environment configuration")]
    MissingEnv {
        /// Name of the missing environment variable.
        name: &'static str,
    },
    /// Configuration operations failed.
    #[error("configuration operation failed")]
    Config {
        /// Operation identifier.
        operation: &'static str,
        /// Source configuration error.
        source: revaer_config::ConfigError,
    },
    /// API server operations failed.
    #[error("api server operation failed")]
    ApiServer {
        /// Operation identifier.
        operation: &'static str,
        /// Source API server error.
        source: revaer_api::ApiServerError,
    },
    /// Telemetry operations failed.
    #[error("telemetry operation failed")]
    Telemetry {
        /// Operation identifier.
        operation: &'static str,
        /// Source telemetry error.
        source: revaer_telemetry::TelemetryError,
    },
    /// Torrent workflow operations failed.
    #[error("torrent operation failed")]
    Torrent {
        /// Operation identifier.
        operation: &'static str,
        /// Source torrent error.
        source: revaer_torrent_core::TorrentError,
    },
    /// Filesystem post-processing operations failed.
    #[error("filesystem post-processing failed")]
    FsOps {
        /// Operation identifier.
        operation: &'static str,
        /// Source fsops error.
        source: revaer_fsops::FsOpsError,
    },
    /// Runtime persistence operations failed.
    #[error("runtime persistence failed")]
    Runtime {
        /// Operation identifier.
        operation: &'static str,
        /// Source runtime data error.
        source: revaer_data::DataError,
    },
    /// HTTP client operations failed.
    #[error("http operation failed")]
    Http {
        /// Operation identifier.
        operation: &'static str,
        /// URL used for the request.
        url: String,
        /// Source HTTP client error.
        source: reqwest::Error,
    },
    /// HTTP response returned a non-success status.
    #[error("http response status error")]
    HttpStatus {
        /// Operation identifier.
        operation: &'static str,
        /// URL used for the request.
        url: String,
        /// HTTP status code returned by the server.
        status: u16,
    },
    /// IO operations failed.
    #[error("io operation failed")]
    Io {
        /// Operation identifier.
        operation: &'static str,
        /// Optional path involved in the failure.
        path: Option<PathBuf>,
        /// Source IO error.
        source: io::Error,
    },
    /// Configuration values were invalid.
    #[error("invalid configuration")]
    InvalidConfig {
        /// Field name that failed validation.
        field: &'static str,
        /// Machine-readable reason for the failure.
        reason: &'static str,
        /// Optional value associated with the failure.
        value: Option<String>,
    },
    /// Required runtime state was missing.
    #[error("missing state")]
    MissingState {
        /// State field that was missing.
        field: &'static str,
        /// Optional value associated with the missing state.
        value: Option<String>,
    },
    /// Required dependency was missing.
    #[error("missing dependency")]
    MissingDependency {
        /// Name of the missing dependency.
        name: &'static str,
    },
}

impl AppError {
    pub(crate) const fn config(
        operation: &'static str,
        source: revaer_config::ConfigError,
    ) -> Self {
        Self::Config { operation, source }
    }

    pub(crate) const fn api_server(
        operation: &'static str,
        source: revaer_api::ApiServerError,
    ) -> Self {
        Self::ApiServer { operation, source }
    }

    pub(crate) const fn telemetry(
        operation: &'static str,
        source: revaer_telemetry::TelemetryError,
    ) -> Self {
        Self::Telemetry { operation, source }
    }

    #[cfg(feature = "libtorrent")]
    pub(crate) const fn torrent(
        operation: &'static str,
        source: revaer_torrent_core::TorrentError,
    ) -> Self {
        Self::Torrent { operation, source }
    }

    #[cfg(feature = "libtorrent")]
    pub(crate) const fn fsops(operation: &'static str, source: revaer_fsops::FsOpsError) -> Self {
        Self::FsOps { operation, source }
    }

    #[cfg(feature = "libtorrent")]
    pub(crate) const fn runtime(operation: &'static str, source: revaer_data::DataError) -> Self {
        Self::Runtime { operation, source }
    }

    #[cfg(feature = "libtorrent")]
    pub(crate) const fn http(operation: &'static str, url: String, source: reqwest::Error) -> Self {
        Self::Http {
            operation,
            url,
            source,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::io;
    #[cfg(feature = "libtorrent")]
    use uuid::Uuid;

    #[test]
    fn app_error_helpers_build_variants() -> Result<(), Box<dyn Error>> {
        let Err(json_error) = serde_json::from_str::<serde_json::Value>("invalid") else {
            return Err(io::Error::other("expected invalid json").into());
        };
        let config = AppError::config(
            "load",
            revaer_config::ConfigError::InvalidAppMode {
                value: "bad".to_string(),
            },
        );
        assert!(matches!(config, AppError::Config { .. }));

        let api = AppError::api_server(
            "serve",
            revaer_api::ApiServerError::Serve {
                source: io::Error::other("io"),
            },
        );
        assert!(matches!(api, AppError::ApiServer { .. }));

        let telemetry = AppError::telemetry(
            "init",
            revaer_telemetry::TelemetryError::OpenApiSerialize { source: json_error },
        );
        assert!(matches!(telemetry, AppError::Telemetry { .. }));
        Ok(())
    }

    #[cfg(feature = "libtorrent")]
    #[test]
    fn app_error_helpers_build_libtorrent_variants() {
        let torrent = AppError::torrent(
            "add",
            revaer_torrent_core::TorrentError::NotFound {
                torrent_id: Uuid::nil(),
            },
        );
        assert!(matches!(torrent, AppError::Torrent { .. }));

        let fsops = AppError::fsops(
            "cleanup",
            revaer_fsops::FsOpsError::Unsupported {
                operation: "cleanup",
                value: None,
            },
        );
        assert!(matches!(fsops, AppError::FsOps { .. }));

        let runtime = AppError::runtime(
            "save",
            revaer_data::DataError::PathNotUtf8 {
                field: "path",
                path: PathBuf::from(".server_root/revaer"),
            },
        );
        assert!(matches!(runtime, AppError::Runtime { .. }));
    }
}
