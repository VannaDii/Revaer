//! Manual search UI state.
//!
//! # Design
//! - Keep form fields as strings until validation to preserve user intent.
//! - Store selected result keys separately from DTOs to keep toggles deterministic.
//! - Model transient UI messages explicitly instead of hiding them in component locals.

#[cfg(target_arch = "wasm32")]
use std::collections::BTreeSet;

#[cfg(target_arch = "wasm32")]
use crate::models::SearchRequestExplainabilityResponse;
#[cfg(target_arch = "wasm32")]
use crate::models::{SearchPageListResponse, SearchPageResponse, SearchRequestCreateResponse};
#[cfg(target_arch = "wasm32")]
use uuid::Uuid;

/// Manual search form fields owned by the UI.
#[derive(Clone, PartialEq, Eq, Default)]
pub(crate) struct SearchFormState {
    pub(crate) query_text: String,
    pub(crate) query_type: String,
    pub(crate) torznab_mode: String,
    pub(crate) requested_media_domain_key: String,
    pub(crate) page_size: String,
    pub(crate) season_number: String,
    pub(crate) episode_number: String,
    pub(crate) identifier_types: String,
    pub(crate) identifier_values: String,
    pub(crate) torznab_cat_ids: String,
}

impl SearchFormState {
    /// Build the default search form.
    #[must_use]
    pub(crate) fn with_defaults() -> Self {
        Self {
            query_type: "free_text".to_string(),
            page_size: "50".to_string(),
            ..Self::default()
        }
    }
}

#[cfg(target_arch = "wasm32")]
/// Snapshot of a manual search run shown in the UI.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SearchRunState {
    pub(crate) search_request_public_id: Uuid,
    pub(crate) request_policy_set_public_id: Uuid,
    pub(crate) pages: SearchPageListResponse,
    pub(crate) current_page: Option<SearchPageResponse>,
    pub(crate) selected_page_number: Option<i32>,
    pub(crate) selected_result_keys: BTreeSet<String>,
}

#[cfg(target_arch = "wasm32")]
impl SearchRunState {
    /// Start a new run from the create response.
    #[must_use]
    pub(crate) fn new(response: SearchRequestCreateResponse) -> Self {
        Self {
            search_request_public_id: response.search_request_public_id,
            request_policy_set_public_id: response.request_policy_set_public_id,
            pages: SearchPageListResponse {
                pages: Vec::new(),
                explainability: empty_explainability(),
            },
            current_page: None,
            selected_page_number: None,
            selected_result_keys: BTreeSet::new(),
        }
    }
}

#[cfg(target_arch = "wasm32")]
/// Empty explainability payload used before the first refresh.
#[must_use]
pub(crate) fn empty_explainability() -> SearchRequestExplainabilityResponse {
    SearchRequestExplainabilityResponse {
        zero_runnable_indexers: false,
        skipped_canceled_indexers: 0,
        skipped_failed_indexers: 0,
        blocked_results: 0,
        blocked_rule_public_ids: Vec::new(),
        rate_limited_indexers: 0,
        retrying_indexers: 0,
    }
}
