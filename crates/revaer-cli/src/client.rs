//! Shared client utilities, error types, and telemetry wiring for the CLI.

use std::fmt::{self, Display, Formatter};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::anyhow;
use rand::{Rng, distr::Alphanumeric};
use reqwest::header::{HeaderMap, HeaderValue};
use reqwest::{Client, StatusCode, Url};
use serde::Serialize;

use crate::cli::Cli;

pub(crate) const HEADER_SETUP_TOKEN: &str = "x-revaer-setup-token";
pub(crate) const HEADER_API_KEY: &str = "x-revaer-api-key";
pub(crate) const HEADER_REQUEST_ID: &str = "x-request-id";
pub(crate) const HEADER_LAST_EVENT_ID: &str = "Last-Event-ID";

/// CLI-level error type to distinguish validation from operational failures.
#[derive(Debug)]
pub(crate) enum CliError {
    Validation(String),
    Failure(anyhow::Error),
}

/// Convenience alias for functions returning a `CliError`.
pub(crate) type CliResult<T> = Result<T, CliError>;

impl CliError {
    pub(crate) fn validation(message: impl Into<String>) -> Self {
        Self::Validation(message.into())
    }

    pub(crate) fn failure(error: impl Into<anyhow::Error>) -> Self {
        Self::Failure(error.into())
    }

    pub(crate) const fn exit_code(&self) -> i32 {
        match self {
            Self::Validation(_) => 2,
            Self::Failure(_) => 3,
        }
    }

    pub(crate) fn display_message(&self) -> String {
        match self {
            Self::Validation(message) => message.clone(),
            Self::Failure(error) => format!("{error:#}"),
        }
    }
}

impl Display for CliError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter.write_str("cli error")
    }
}

impl std::error::Error for CliError {}

/// Dependencies constructed from environment flags and CLI options.
#[derive(Clone)]
pub(crate) struct CliDependencies {
    pub(crate) client: Client,
    pub(crate) telemetry: Option<TelemetryEmitter>,
}

impl CliDependencies {
    /// Construct a configured HTTP client and optional telemetry emitter.
    pub(crate) fn from_env(cli: &Cli, trace_id: &str) -> CliResult<Self> {
        let mut default_headers = HeaderMap::new();
        let request_id = HeaderValue::from_str(trace_id).map_err(|_| {
            CliError::failure(anyhow!("trace identifier contains invalid characters"))
        })?;
        default_headers.insert(HEADER_REQUEST_ID, request_id);

        let client = Client::builder()
            .timeout(Duration::from_secs(cli.timeout))
            .default_headers(default_headers)
            .build()
            .map_err(|err| CliError::failure(anyhow!("failed to build HTTP client: {err}")))?;

        Ok(Self {
            client,
            telemetry: TelemetryEmitter::from_env(),
        })
    }
}

/// Application context passed to command handlers.
#[derive(Clone)]
pub(crate) struct AppContext {
    pub(crate) client: Client,
    pub(crate) base_url: Url,
    pub(crate) api_key: Option<ApiKeyCredential>,
}

/// API key credential parsed from CLI flags or environment variables.
#[derive(Debug, Clone)]
pub(crate) struct ApiKeyCredential {
    pub(crate) key_id: String,
    pub(crate) secret: String,
}

impl ApiKeyCredential {
    #[must_use]
    pub(crate) fn header_value(&self) -> String {
        format!("{}:{}", self.key_id, self.secret)
    }
}

/// Telemetry emitter used to forward CLI outcomes.
#[derive(Clone)]
pub(crate) struct TelemetryEmitter {
    pub(crate) client: Client,
    pub(crate) endpoint: Url,
}

impl TelemetryEmitter {
    #[must_use]
    pub(crate) fn from_env() -> Option<Self> {
        let endpoint = std::env::var("REVAER_TELEMETRY_ENDPOINT").ok()?;
        let endpoint = endpoint.parse().ok()?;
        let client = Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .ok()?;
        Some(Self { client, endpoint })
    }

    pub(crate) async fn emit(
        &self,
        trace_id: &str,
        command: &str,
        outcome: &str,
        exit_code: i32,
        message: Option<&str>,
    ) {
        let event = TelemetryEvent {
            command,
            outcome,
            trace_id,
            exit_code,
            message,
            timestamp_ms: timestamp_now_ms(),
        };

        if let Err(err) = self
            .client
            .post(self.endpoint.clone())
            .json(&event)
            .send()
            .await
        {
            tracing::debug!(error = %err, "telemetry emit failed");
        }
    }
}

#[derive(Serialize)]
struct TelemetryEvent<'a> {
    command: &'a str,
    outcome: &'a str,
    trace_id: &'a str,
    exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<&'a str>,
    timestamp_ms: u64,
}

/// Parse the API URL provided to the CLI.
pub(crate) fn parse_url(input: &str) -> Result<Url, String> {
    input
        .parse::<Url>()
        .map_err(|err| format!("invalid URL '{input}': {err}"))
}

/// Parse the API key provided to the CLI.
pub(crate) fn parse_api_key(input: Option<String>) -> CliResult<Option<ApiKeyCredential>> {
    let Some(raw) = input else {
        return Ok(None);
    };

    let trimmed = raw.trim();
    let (key_id, secret) = trimmed
        .split_once(':')
        .ok_or_else(|| CliError::validation("API key must be provided as key_id:secret"))?;

    if key_id.trim().is_empty() || secret.trim().is_empty() {
        return Err(CliError::validation(
            "API key components cannot be empty strings",
        ));
    }

    Ok(Some(ApiKeyCredential {
        key_id: key_id.trim().to_string(),
        secret: secret.trim().to_string(),
    }))
}

/// Generate a random alphanumeric string of the requested length.
#[must_use]
pub(crate) fn random_string(len: usize) -> String {
    let mut rng = rand::rng();
    std::iter::repeat_with(|| rng.sample(Alphanumeric) as char)
        .take(len)
        .collect()
}

/// Millisecond timestamp helper for telemetry.
#[must_use]
pub(crate) fn timestamp_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Classify an HTTP response into a CLI error.
pub(crate) async fn classify_problem(response: reqwest::Response) -> CliError {
    let status = response.status();
    let bytes = response.bytes().await.unwrap_or_default();

    let body_text = String::from_utf8_lossy(&bytes).to_string();
    let problem = serde_json::from_slice::<revaer_api::models::ProblemDetails>(&bytes).ok();

    let message = problem
        .as_ref()
        .and_then(|p| p.detail.clone())
        .unwrap_or_else(|| {
            problem
                .as_ref()
                .map_or_else(|| body_text.trim().to_string(), |p| p.title.clone())
        });

    if matches!(
        status,
        StatusCode::BAD_REQUEST | StatusCode::CONFLICT | StatusCode::UNPROCESSABLE_ENTITY
    ) {
        CliError::validation(message)
    } else {
        let detail = if let Some(problem) = problem {
            format!("{} (status {})", message, problem.status)
        } else if !body_text.is_empty() {
            format!("{message} (status {status})")
        } else {
            format!("request failed with status {status}")
        };
        CliError::failure(anyhow!(detail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use httpmock::MockServer;
    use httpmock::prelude::*;

    #[test]
    fn random_string_produces_expected_length() {
        let generated = random_string(16);
        assert_eq!(generated.len(), 16);
        assert!(generated.chars().all(|ch| ch.is_ascii_alphanumeric()));
    }

    #[tokio::test]
    async fn telemetry_emitter_emits_event() -> Result<()> {
        let server = MockServer::start_async().await;
        let mock = server.mock(|when, then| {
            when.method(POST).path("/telemetry");
            then.status(200);
        });

        let emitter = TelemetryEmitter {
            client: Client::new(),
            endpoint: format!("{}/telemetry", server.base_url())
                .parse()
                .map_err(|_| anyhow::anyhow!("invalid URL"))?,
        };

        emitter
            .emit("trace", "command", "success", 0, Some("message"))
            .await;

        mock.assert();
        Ok(())
    }
}
