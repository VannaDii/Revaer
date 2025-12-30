//! Error types for telemetry operations.

use std::error::Error;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;

use prometheus::Error as PrometheusError;

/// Result alias for telemetry operations.
pub type Result<T> = std::result::Result<T, TelemetryError>;

/// Errors raised by telemetry helpers.
#[derive(Debug)]
pub enum TelemetryError {
    /// Installing the tracing subscriber failed.
    SubscriberInstall {
        /// Underlying tracing subscriber error.
        source: tracing_subscriber::util::TryInitError,
    },
    /// Building a Prometheus collector failed.
    MetricsCollector {
        /// Metric identifier tied to the failure.
        name: &'static str,
        /// Underlying Prometheus error.
        source: PrometheusError,
    },
    /// Registering a Prometheus collector failed.
    MetricsRegister {
        /// Metric identifier tied to the failure.
        name: &'static str,
        /// Underlying Prometheus error.
        source: PrometheusError,
    },
    /// Encoding Prometheus metrics failed.
    MetricsEncode {
        /// Underlying Prometheus error.
        source: PrometheusError,
    },
    /// Rendered metrics output was not valid UTF-8.
    MetricsUtf8 {
        /// Underlying UTF-8 conversion error.
        source: std::string::FromUtf8Error,
    },
    /// Serialising the `OpenAPI` document failed.
    OpenApiSerialize {
        /// Underlying serde error.
        source: serde_json::Error,
    },
    /// Creating the `OpenAPI` output directory failed.
    OpenApiCreateDir {
        /// Directory path that could not be created.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
    /// Writing the `OpenAPI` artifact failed.
    OpenApiWrite {
        /// File path that could not be written.
        path: PathBuf,
        /// Underlying IO error.
        source: std::io::Error,
    },
}

impl Display for TelemetryError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::SubscriberInstall { .. } => {
                formatter.write_str("failed to install tracing subscriber")
            }
            Self::MetricsCollector { .. } => {
                formatter.write_str("failed to build metrics collector")
            }
            Self::MetricsRegister { .. } => {
                formatter.write_str("failed to register metrics collector")
            }
            Self::MetricsEncode { .. } => formatter.write_str("failed to encode metrics"),
            Self::MetricsUtf8 { .. } => formatter.write_str("metrics output was not valid utf-8"),
            Self::OpenApiSerialize { .. } => {
                formatter.write_str("failed to serialize openapi document")
            }
            Self::OpenApiCreateDir { .. } => {
                formatter.write_str("failed to create openapi output directory")
            }
            Self::OpenApiWrite { .. } => formatter.write_str("failed to write openapi artifact"),
        }
    }
}

impl Error for TelemetryError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::SubscriberInstall { source } => Some(source),
            Self::MetricsCollector { source, .. }
            | Self::MetricsRegister { source, .. }
            | Self::MetricsEncode { source } => Some(source),
            Self::MetricsUtf8 { source } => Some(source),
            Self::OpenApiSerialize { source } => Some(source),
            Self::OpenApiCreateDir { source, .. } | Self::OpenApiWrite { source, .. } => {
                Some(source)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::Error as _;
    use std::error::Error;
    use std::io;
    use tracing_subscriber::util::SubscriberInitExt;

    fn try_init_error()
    -> std::result::Result<tracing_subscriber::util::TryInitError, Box<dyn Error>> {
        match tracing_subscriber::registry().try_init() {
            Ok(()) => match tracing_subscriber::registry().try_init() {
                Ok(()) => Err(io::Error::other("expected init error").into()),
                Err(err) => Ok(err),
            },
            Err(err) => Ok(err),
        }
    }

    fn json_error() -> serde_json::Error {
        match serde_json::from_str::<serde_json::Value>("invalid") {
            Ok(_) => serde_json::Error::custom("expected invalid json"),
            Err(err) => err,
        }
    }

    #[test]
    fn telemetry_error_display_and_source() -> std::result::Result<(), Box<dyn Error>> {
        let init_error = try_init_error()?;
        let utf8_error = String::from_utf8(vec![0, 159])
            .err()
            .ok_or_else(|| io::Error::other("expected utf8 error"))?;
        let cases = vec![
            (
                TelemetryError::SubscriberInstall { source: init_error },
                "failed to install tracing subscriber",
            ),
            (
                TelemetryError::MetricsCollector {
                    name: "metric",
                    source: prometheus::Error::Msg("metrics".to_string()),
                },
                "failed to build metrics collector",
            ),
            (
                TelemetryError::MetricsRegister {
                    name: "metric",
                    source: prometheus::Error::Msg("metrics".to_string()),
                },
                "failed to register metrics collector",
            ),
            (
                TelemetryError::MetricsEncode {
                    source: prometheus::Error::Msg("metrics".to_string()),
                },
                "failed to encode metrics",
            ),
            (
                TelemetryError::MetricsUtf8 { source: utf8_error },
                "metrics output was not valid utf-8",
            ),
            (
                TelemetryError::OpenApiSerialize {
                    source: json_error(),
                },
                "failed to serialize openapi document",
            ),
            (
                TelemetryError::OpenApiCreateDir {
                    path: PathBuf::from("openapi"),
                    source: io::Error::other("io"),
                },
                "failed to create openapi output directory",
            ),
            (
                TelemetryError::OpenApiWrite {
                    path: PathBuf::from("openapi.json"),
                    source: io::Error::other("io"),
                },
                "failed to write openapi artifact",
            ),
        ];

        for (err, message) in cases {
            assert_eq!(err.to_string(), message);
            assert!(err.source().is_some());
        }
        Ok(())
    }
}
