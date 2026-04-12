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
    use crate::app::indexers::{TorznabAccessError, TorznabAccessErrorKind, TorznabInstanceAuth};
    use crate::http::handlers::indexers::test_support::{RecordingIndexers, indexer_test_state};
    use axum::extract::{Path, Query, State};
    use std::sync::Arc;
    use uuid::Uuid;

    fn auth(display_name: &str) -> TorznabInstanceAuth {
        TorznabInstanceAuth {
            torznab_instance_id: 1,
            search_profile_id: 2,
            display_name: display_name.to_string(),
        }
    }

    fn ids() -> (Uuid, Uuid) {
        (Uuid::from_u128(1), Uuid::from_u128(2))
    }

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

    #[tokio::test]
    async fn torznab_download_rejects_missing_or_blank_api_key() -> Result<(), ApiError> {
        for apikey in [None, Some("   ".to_string())] {
            let indexers = Arc::new(RecordingIndexers::default());
            let state = indexer_test_state(indexers.clone())?;
            let response = torznab_download(
                State(state),
                Path(ids()),
                Query(TorznabDownloadQuery { apikey }),
            )
            .await?;

            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert!(response.headers().get(LOCATION).is_none());
            assert!(indexers.torznab_auth_calls().is_empty());
        }
        Ok(())
    }

    #[tokio::test]
    async fn torznab_download_maps_auth_failures() -> Result<(), ApiError> {
        let cases = [
            (
                TorznabAccessErrorKind::Unauthorized,
                StatusCode::UNAUTHORIZED,
            ),
            (TorznabAccessErrorKind::NotFound, StatusCode::NOT_FOUND),
            (
                TorznabAccessErrorKind::Storage,
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
        ];

        for (kind, expected_status) in cases {
            let indexers = Arc::new(RecordingIndexers::default());
            indexers.set_torznab_auth_result(Err(TorznabAccessError::new(kind)));
            let state = indexer_test_state(indexers.clone())?;
            let (torznab_instance_public_id, canonical_torrent_source_public_id) = ids();

            let response = torznab_download(
                State(state),
                Path((
                    torznab_instance_public_id,
                    canonical_torrent_source_public_id,
                )),
                Query(TorznabDownloadQuery {
                    apikey: Some("secret".to_string()),
                }),
            )
            .await?;

            assert_eq!(response.status(), expected_status);
            assert!(response.headers().get(LOCATION).is_none());
            let calls = indexers.torznab_auth_calls();
            assert_eq!(
                calls,
                vec![(torznab_instance_public_id, "secret".to_string())]
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn torznab_download_maps_prepare_failures_and_missing_sources() -> Result<(), ApiError> {
        let error_cases = [
            (
                Err(TorznabAccessError::new(
                    TorznabAccessErrorKind::Unauthorized,
                )),
                StatusCode::UNAUTHORIZED,
            ),
            (
                Err(TorznabAccessError::new(TorznabAccessErrorKind::NotFound)),
                StatusCode::NOT_FOUND,
            ),
            (
                Err(TorznabAccessError::new(TorznabAccessErrorKind::Storage)),
                StatusCode::INTERNAL_SERVER_ERROR,
            ),
            (Ok(None), StatusCode::NOT_FOUND),
        ];

        for (result, expected_status) in error_cases {
            let indexers = Arc::new(RecordingIndexers::default());
            indexers.set_torznab_auth_result(Ok(auth("Primary")));
            indexers.set_torznab_download_prepare_result(result);
            let state = indexer_test_state(indexers.clone())?;
            let (torznab_instance_public_id, canonical_torrent_source_public_id) = ids();

            let response = torznab_download(
                State(state),
                Path((
                    torznab_instance_public_id,
                    canonical_torrent_source_public_id,
                )),
                Query(TorznabDownloadQuery {
                    apikey: Some("secret".to_string()),
                }),
            )
            .await?;

            assert_eq!(response.status(), expected_status);
            assert!(response.headers().get(LOCATION).is_none());
            let calls = indexers.torznab_download_prepare_calls();
            assert_eq!(
                calls,
                vec![(
                    torznab_instance_public_id,
                    canonical_torrent_source_public_id
                )]
            );
        }
        Ok(())
    }

    #[tokio::test]
    async fn torznab_download_redirects_to_prepared_source() -> Result<(), ApiError> {
        let indexers = Arc::new(RecordingIndexers::default());
        indexers.set_torznab_auth_result(Ok(auth("Primary")));
        indexers.set_torznab_download_prepare_result(Ok(Some(
            "https://downloads.example.invalid/file.torrent".to_string(),
        )));
        let state = indexer_test_state(indexers.clone())?;
        let (torznab_instance_public_id, canonical_torrent_source_public_id) = ids();

        let response = torznab_download(
            State(state),
            Path((
                torznab_instance_public_id,
                canonical_torrent_source_public_id,
            )),
            Query(TorznabDownloadQuery {
                apikey: Some("secret".to_string()),
            }),
        )
        .await?;

        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response
                .headers()
                .get(LOCATION)
                .and_then(|value| value.to_str().ok()),
            Some("https://downloads.example.invalid/file.torrent")
        );
        let auth_calls = indexers.torznab_auth_calls();
        assert_eq!(
            auth_calls,
            vec![(torznab_instance_public_id, "secret".to_string())]
        );
        let download_calls = indexers.torznab_download_prepare_calls();
        assert_eq!(
            download_calls,
            vec![(
                torznab_instance_public_id,
                canonical_torrent_source_public_id
            )]
        );
        Ok(())
    }
}
