//! API bootstrap and wiring.
use crate::http::compat_qb;
use crate::{ApiServer, ApiState, AppMode, EventBus, Metrics, TorrentHandles};
use axum::Router;
use revaer_config::ConfigService;
use std::net::SocketAddr;
use std::sync::Arc;

/// Build the API server with provided dependencies.
pub fn build_api(
    config: ConfigService,
    events: EventBus,
    torrent_handles: Option<TorrentHandles>,
    metrics: Metrics,
) -> anyhow::Result<ApiServer> {
    ApiServer::new(config, events, torrent_handles, metrics)
}

/// Mount compatibility routes when the feature is enabled.
#[cfg(feature = "compat-qb")]
pub fn mount_compat(router: Router<ApiState>) -> Router<ApiState> {
    compat_qb::mount(router)
}

#[cfg(not(feature = "compat-qb"))]
pub fn mount_compat(router: Router<ApiState>) -> Router<ApiState> {
    router
}

/// Validate bind addr and mode before serving.
pub fn validate_bind(mode: &AppMode, addr: &SocketAddr) -> anyhow::Result<()> {
    if matches!(mode, AppMode::Setup) && !addr.ip().is_loopback() {
        anyhow::bail!("setup mode requires loopback bind");
    }
    Ok(())
}
