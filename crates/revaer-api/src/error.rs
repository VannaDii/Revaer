//! # Design
//!
//! - Provide a single crate-level error type for API server bootstrap/serve failures.
//! - Keep error messages constant; capture operational context in structured fields.
//! - Preserve sources for diagnostics without double-logging.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::net::SocketAddr;
use std::path::PathBuf;

use revaer_config::AppMode;
use revaer_telemetry::TelemetryError;

/// Result alias for API server operations.
pub type ApiServerResult<T> = std::result::Result<T, ApiServerError>;

/// Errors raised while bootstrapping or serving the API.
#[derive(Debug)]
pub enum ApiServerError {
    /// Persisting the `OpenAPI` artifact failed.
    OpenApiPersist {
        /// Target path for the artifact.
        path: PathBuf,
        /// Underlying telemetry error.
        source: TelemetryError,
    },
    /// Binding the API listener failed.
    Bind {
        /// Address attempted.
        addr: SocketAddr,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Serving the API failed.
    Serve {
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Bind address is invalid for the current app mode.
    InvalidBindAddr {
        /// Current app mode.
        mode: AppMode,
        /// Address attempted.
        addr: SocketAddr,
    },
}

impl Display for ApiServerError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::OpenApiPersist { .. } => formatter.write_str("failed to persist openapi"),
            Self::Bind { .. } => formatter.write_str("failed to bind api listener"),
            Self::Serve { .. } => formatter.write_str("api server terminated unexpectedly"),
            Self::InvalidBindAddr { .. } => {
                formatter.write_str("bind address is invalid for the current app mode")
            }
        }
    }
}

impl Error for ApiServerError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::OpenApiPersist { source, .. } => Some(source),
            Self::Bind { source, .. } | Self::Serve { source } => Some(source),
            Self::InvalidBindAddr { .. } => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::Error as _;
    use std::io;

    #[test]
    fn api_server_error_display_and_source() -> Result<(), Box<dyn Error>> {
        let json_error = match serde_json::from_str::<serde_json::Value>("invalid") {
            Ok(_) => serde_json::Error::custom("expected invalid json"),
            Err(err) => err,
        };
        let openapi = ApiServerError::OpenApiPersist {
            path: PathBuf::from("openapi.json"),
            source: TelemetryError::OpenApiSerialize { source: json_error },
        };
        assert_eq!(openapi.to_string(), "failed to persist openapi");
        assert!(openapi.source().is_some());

        let bind = ApiServerError::Bind {
            addr: "127.0.0.1:7070".parse()?,
            source: io::Error::new(io::ErrorKind::AddrInUse, "busy"),
        };
        assert_eq!(bind.to_string(), "failed to bind api listener");
        assert!(bind.source().is_some());

        let serve = ApiServerError::Serve {
            source: io::Error::new(io::ErrorKind::BrokenPipe, "lost"),
        };
        assert_eq!(serve.to_string(), "api server terminated unexpectedly");
        assert!(serve.source().is_some());

        let invalid = ApiServerError::InvalidBindAddr {
            mode: AppMode::Setup,
            addr: "0.0.0.0:7070".parse()?,
        };
        assert_eq!(
            invalid.to_string(),
            "bind address is invalid for the current app mode"
        );
        assert!(invalid.source().is_none());
        Ok(())
    }
}
