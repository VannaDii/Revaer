//! API bootstrap and wiring.

use std::net::SocketAddr;

use revaer_config::{AppMode, ConfigService};
use revaer_events::EventBus;
use revaer_telemetry::Metrics;

use crate::error::{ApiServerError, ApiServerResult};
use crate::{ApiServer, TorrentHandles};

/// Build the API server with provided dependencies.
///
/// # Errors
///
/// Returns an error if API server initialization fails.
pub fn build_api(
    config: ConfigService,
    events: EventBus,
    torrent_handles: Option<TorrentHandles>,
    metrics: Metrics,
) -> ApiServerResult<ApiServer> {
    ApiServer::new(config, events, torrent_handles, metrics)
}

/// Validate bind addr and mode before serving.
///
/// # Errors
///
/// Returns `ApiServerError::InvalidBindAddr` when setup mode is bound to a non-loopback address.
pub fn validate_bind(mode: &AppMode, addr: &SocketAddr) -> ApiServerResult<()> {
    if matches!(mode, AppMode::Setup) && !addr.ip().is_loopback() {
        return Err(ApiServerError::InvalidBindAddr {
            mode: mode.clone(),
            addr: *addr,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn validate_bind_rejects_setup_non_loopback() -> Result<(), Box<dyn Error>> {
        let addr: SocketAddr = "0.0.0.0:7070".parse()?;
        assert!(matches!(
            validate_bind(&AppMode::Setup, &addr),
            Err(ApiServerError::InvalidBindAddr { .. })
        ));
        Ok(())
    }

    #[test]
    fn validate_bind_allows_loopback_and_active() -> Result<(), Box<dyn Error>> {
        let loopback: SocketAddr = "127.0.0.1:7070".parse()?;
        validate_bind(&AppMode::Setup, &loopback)?;

        let public: SocketAddr = "0.0.0.0:7070".parse()?;
        validate_bind(&AppMode::Active, &public)?;
        Ok(())
    }
}
