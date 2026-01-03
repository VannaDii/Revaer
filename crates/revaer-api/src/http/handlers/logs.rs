//! Live log streaming endpoint.
//!
//! # Design
//! - Bridge structured tracing output to SSE without extra formatting layers.
//! - Allow clients to reconnect without holding server state.
//! - Emit keep-alive frames to keep proxies from closing idle streams.

use std::convert::Infallible;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    extract::State,
    response::sse::{self, Sse},
};
use futures_util::StreamExt;
use revaer_telemetry::log_stream_receiver;
use tokio::sync::broadcast;
use tokio_stream::wrappers::{BroadcastStream, errors::BroadcastStreamRecvError};

use crate::app::state::ApiState;
use crate::http::constants::SSE_KEEP_ALIVE_SECS;
use crate::http::errors::ApiError;

pub(crate) async fn stream_logs(
    State(_state): State<Arc<ApiState>>,
) -> Result<Sse<impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send>, ApiError>
{
    let stream = build_log_stream(log_stream_receiver());

    Ok(Sse::new(stream).keep_alive(
        sse::KeepAlive::new()
            .interval(Duration::from_secs(SSE_KEEP_ALIVE_SECS))
            .text("keep-alive"),
    ))
}

fn build_log_stream(
    receiver: broadcast::Receiver<String>,
) -> impl futures_core::Stream<Item = Result<sse::Event, Infallible>> + Send {
    BroadcastStream::new(receiver).filter_map(|result| async move {
        let event = match result {
            Ok(line) => sse::Event::default().event("log").data(line),
            Err(err) => sse::Event::default()
                .event("log_status")
                .data(log_status_message(&err)),
        };
        Some(Ok(event))
    })
}

fn log_status_message(err: &BroadcastStreamRecvError) -> String {
    match err {
        BroadcastStreamRecvError::Lagged(count) => {
            format!("log stream lagged; dropped {count} lines")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigFacade;
    use async_trait::async_trait;
    use axum::response::IntoResponse;
    use revaer_config::{
        ApiKeyAuth, AppProfile, AppliedChanges, ConfigError, ConfigResult, ConfigSnapshot,
        SettingsChangeset, SetupToken,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use std::error::Error;
    use std::io;
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn log_status_message_formats_lagged_count() {
        let err = BroadcastStreamRecvError::Lagged(3);
        let message = log_status_message(&err);
        assert_eq!(message, "log stream lagged; dropped 3 lines");
    }

    #[derive(Clone, Default)]
    struct StubConfig;

    fn stub_error() -> ConfigError {
        ConfigError::Io {
            operation: "logs.stub",
            source: io::Error::other("stub"),
        }
    }

    #[async_trait]
    impl ConfigFacade for StubConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            Err(stub_error())
        }

        async fn issue_setup_token(
            &self,
            _ttl: Duration,
            _issued_by: &str,
        ) -> ConfigResult<SetupToken> {
            Err(stub_error())
        }

        async fn validate_setup_token(&self, _token: &str) -> ConfigResult<()> {
            Err(stub_error())
        }

        async fn consume_setup_token(&self, _token: &str) -> ConfigResult<()> {
            Err(stub_error())
        }

        async fn apply_changeset(
            &self,
            _actor: &str,
            _reason: &str,
            _changeset: SettingsChangeset,
        ) -> ConfigResult<AppliedChanges> {
            Err(stub_error())
        }

        async fn snapshot(&self) -> ConfigResult<ConfigSnapshot> {
            Err(stub_error())
        }

        async fn authenticate_api_key(
            &self,
            _key_id: &str,
            _secret: &str,
        ) -> ConfigResult<Option<ApiKeyAuth>> {
            Err(stub_error())
        }

        async fn has_api_keys(&self) -> ConfigResult<bool> {
            Err(stub_error())
        }

        async fn factory_reset(&self) -> ConfigResult<()> {
            Err(stub_error())
        }
    }

    #[tokio::test]
    async fn build_log_stream_emits_events_for_lines_and_lagged() -> Result<(), Box<dyn Error>> {
        let (sender, receiver) = broadcast::channel(1);
        let mut stream = Box::pin(build_log_stream(receiver));

        sender
            .send("alpha".to_string())
            .map_err(|_| io::Error::other("send failed"))?;
        sender
            .send("beta".to_string())
            .map_err(|_| io::Error::other("send failed"))?;
        assert!(matches!(stream.next().await, Some(Ok(_))));

        sender
            .send("gamma".to_string())
            .map_err(|_| io::Error::other("send failed"))?;
        assert!(matches!(stream.next().await, Some(Ok(_))));
        Ok(())
    }

    #[tokio::test]
    async fn stream_logs_builds_sse_response() -> Result<(), ApiError> {
        let config: Arc<dyn ConfigFacade> = Arc::new(StubConfig);
        let telemetry = Metrics::new().map_err(|err| ApiError::internal(err.to_string()))?;
        let state = Arc::new(ApiState::new(
            config,
            telemetry,
            Arc::new(serde_json::json!({})),
            EventBus::new(),
            None,
        ));

        let response = stream_logs(State(state)).await?.into_response();
        assert_eq!(response.status(), axum::http::StatusCode::OK);
        Ok(())
    }

    #[tokio::test]
    async fn stub_config_returns_errors() {
        let config = StubConfig;
        assert!(config.get_app_profile().await.is_err());
        assert!(
            config
                .issue_setup_token(Duration::from_secs(1), "tester")
                .await
                .is_err()
        );
        assert!(config.validate_setup_token("token").await.is_err());
        assert!(config.consume_setup_token("token").await.is_err());
        assert!(
            config
                .apply_changeset("actor", "reason", SettingsChangeset::default())
                .await
                .is_err()
        );
        assert!(config.snapshot().await.is_err());
        assert!(config.authenticate_api_key("id", "secret").await.is_err());
        assert!(config.has_api_keys().await.is_err());
        assert!(config.factory_reset().await.is_err());
    }
}
