#![forbid(unsafe_code)]
#![deny(
    warnings,
    dead_code,
    unused,
    unused_imports,
    unused_must_use,
    unreachable_pub,
    clippy::all,
    clippy::pedantic,
    clippy::cargo,
    clippy::nursery,
    rustdoc::broken_intra_doc_links,
    rustdoc::bare_urls,
    missing_docs
)]

//! Telemetry primitives shared across the Revaer workspace.
//!
//! Layout: `init.rs` (logging setup), `context.rs` (request/app spans), `layers.rs`
//! (request-id middleware), `metrics.rs` (Prometheus registry), `openapi.rs`
//! (artifact persistence), `log_stream.rs` (live log broadcasting).

pub mod context;
pub mod error;
pub mod init;
pub mod layers;
pub mod log_stream;
pub mod metrics;
pub mod openapi;

pub use context::{
    GlobalContextGuard, current_request_id, current_route, record_app_mode, set_request_context,
    with_request_context,
};
pub use error::{Result as TelemetryResult, TelemetryError};
pub use init::{
    DEFAULT_LOG_LEVEL, LogFormat, LoggingConfig, OpenTelemetryConfig, OpenTelemetryGuard,
    build_sha, init_logging, init_logging_with_otel, log_format_from_config,
};
pub use layers::{propagate_request_id_layer, set_request_id_layer};
pub use log_stream::log_stream_receiver;
pub use metrics::{Metrics, MetricsSnapshot};
pub use openapi::persist_openapi;
