//! Manual search transport helpers.
//!
//! # Design
//! - Keep feature-specific transport concerns out of view components.
//! - Translate shared client errors into UI-safe strings at the feature boundary.
//! - Add selected results sequentially so partial failures remain attributable.

use crate::features::search::logic::{build_search_request, preferred_source};
use crate::features::search::state::SearchFormState;
use crate::models::{
    AddTorrentInput, SearchPageItemResponse, SearchPageListResponse, SearchPageResponse,
    SearchRequestCreateResponse,
};
use crate::services::api::ApiClient;
use uuid::Uuid;

/// Submit a manual search request.
///
/// # Errors
///
/// Returns an error string when validation or transport fails.
pub(crate) async fn submit_search(
    client: &ApiClient,
    form: &SearchFormState,
) -> Result<SearchRequestCreateResponse, String> {
    let request = build_search_request(form)?;
    client
        .create_search_request(&request)
        .await
        .map_err(|error| error.to_string())
}

/// Fetch page summaries and explainability for a search run.
///
/// # Errors
///
/// Returns an error string when transport fails.
pub(crate) async fn fetch_search_pages(
    client: &ApiClient,
    search_request_public_id: Uuid,
) -> Result<SearchPageListResponse, String> {
    client
        .fetch_search_pages(search_request_public_id)
        .await
        .map_err(|error| error.to_string())
}

/// Fetch a specific search page.
///
/// # Errors
///
/// Returns an error string when transport fails.
pub(crate) async fn fetch_search_page(
    client: &ApiClient,
    search_request_public_id: Uuid,
    page_number: i32,
) -> Result<SearchPageResponse, String> {
    client
        .fetch_search_page(search_request_public_id, page_number)
        .await
        .map_err(|error| error.to_string())
}

/// Push selected results to the configured download client.
///
/// # Errors
///
/// Returns an error string on the first invalid or failed add.
pub(crate) async fn add_selected_results(
    client: &ApiClient,
    items: &[SearchPageItemResponse],
) -> Result<usize, String> {
    if items.is_empty() {
        return Err("Select at least one result to add".to_string());
    }
    for item in items {
        let Some(source) = preferred_source(item) else {
            return Err("Selected result is missing a magnet or download URL".to_string());
        };
        client
            .add_torrent(AddTorrentInput {
                value: Some(source),
                file: None,
                category: None,
                tags: None,
                save_path: None,
                max_download_bps: None,
                max_upload_bps: None,
            })
            .await
            .map_err(|error| error.to_string())?;
    }
    Ok(items.len())
}
