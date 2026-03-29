//! Torznab download redirect handler.
//!
//! # Design
//! - Authenticate requests using the apikey query parameter only.
//! - Record acquisition attempts via stored procedures before redirecting.
//! - Return empty bodies for unauthorized, missing, or invalid downloads.

use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header::LOCATION},
    response::Response,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::app::indexers::TorznabAccessErrorKind;
use crate::app::state::ApiState;
use crate::http::errors::ApiError;

#[derive(Debug, Deserialize)]
/// Torznab download query parameters.
pub(crate) struct TorznabDownloadQuery {
    apikey: Option<String>,
}

/// Handle Torznab download redirects.
///
/// # Errors
///
/// Returns an internal error if the response cannot be constructed.
#[tracing::instrument(
    name = "torznab.download",
    skip(state, query),
    fields(
        torznab_instance_public_id = %torznab_instance_public_id,
        canonical_torrent_source_public_id = %canonical_torrent_source_public_id
    )
)]
pub(crate) async fn torznab_download(
    State(state): State<Arc<ApiState>>,
    Path((torznab_instance_public_id, canonical_torrent_source_public_id)): Path<(Uuid, Uuid)>,
    Query(query): Query<TorznabDownloadQuery>,
) -> Result<Response, ApiError> {
    let api_key = match query.apikey {
        Some(value) if !value.trim().is_empty() => value,
        _ => {
            state
                .telemetry
                .inc_torznab_invalid_request("missing_apikey");
            return empty_status(StatusCode::UNAUTHORIZED);
        }
    };

    let auth = state
        .indexers
        .torznab_instance_authenticate(torznab_instance_public_id, &api_key)
        .await;
    if let Err(error) = auth {
        return match error.kind() {
            TorznabAccessErrorKind::Unauthorized => {
                state.telemetry.inc_torznab_invalid_request("unauthorized");
                empty_status(StatusCode::UNAUTHORIZED)
            }
            TorznabAccessErrorKind::NotFound => {
                state
                    .telemetry
                    .inc_torznab_invalid_request("instance_not_found");
                empty_status(StatusCode::NOT_FOUND)
            }
            TorznabAccessErrorKind::Storage => empty_status(StatusCode::INTERNAL_SERVER_ERROR),
        };
    }

    let redirect = state
        .indexers
        .torznab_download_prepare(
            torznab_instance_public_id,
            canonical_torrent_source_public_id,
        )
        .await;
    let redirect = match redirect {
        Ok(value) => value,
        Err(error) => {
            return match error.kind() {
                TorznabAccessErrorKind::Unauthorized => {
                    state.telemetry.inc_torznab_invalid_request("unauthorized");
                    empty_status(StatusCode::UNAUTHORIZED)
                }
                TorznabAccessErrorKind::NotFound => {
                    state
                        .telemetry
                        .inc_torznab_invalid_request("source_not_found");
                    empty_status(StatusCode::NOT_FOUND)
                }
                TorznabAccessErrorKind::Storage => empty_status(StatusCode::INTERNAL_SERVER_ERROR),
            };
        }
    };

    let Some(url) = redirect else {
        state
            .telemetry
            .inc_torznab_invalid_request("source_not_found");
        return empty_status(StatusCode::NOT_FOUND);
    };

    redirect_response(StatusCode::FOUND, url)
}

fn redirect_response(status: StatusCode, location: String) -> Result<Response, ApiError> {
    Response::builder()
        .status(status)
        .header(LOCATION, location)
        .body(Body::empty())
        .map_err(|_| ApiError::internal("failed to build torznab response"))
}

fn empty_status(status: StatusCode) -> Result<Response, ApiError> {
    Response::builder()
        .status(status)
        .body(Body::empty())
        .map_err(|_| ApiError::internal("failed to build torznab response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn torznab_download_query_parses_key() {
        let query = TorznabDownloadQuery {
            apikey: Some("abc".to_string()),
        };
        assert_eq!(query.apikey.as_deref(), Some("abc"));
    }

    #[test]
    fn redirect_response_sets_location() {
        let response =
            redirect_response(StatusCode::FOUND, "https://example.com".to_string()).unwrap();
        assert_eq!(
            response
                .headers()
                .get(LOCATION)
                .and_then(|h| h.to_str().ok()),
            Some("https://example.com")
        );
    }
}
