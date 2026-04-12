//! Torznab API endpoint handler.
//!
//! # Design
//! - Authenticate requests using the apikey query parameter only.
//! - Serve caps or empty search responses without committing DB writes.
//! - Return empty bodies for unauthorized or disabled instance access.

use std::collections::BTreeSet;
use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{StatusCode, header::CONTENT_TYPE},
    response::Response,
};
use serde::Deserialize;
use url::form_urlencoded;
use uuid::Uuid;

use crate::app::indexers::{
    SearchRequestCreateParams, SearchRequestServiceErrorKind, TorznabAccessErrorKind,
};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::models::{SearchPageItemResponse, SearchPageSummaryResponse};

use super::xml::{
    TorznabFeedItem, build_caps_response, build_empty_search_response, build_search_response,
};

const OTHER_CATEGORY_ID: i32 = 8000;
const DEFAULT_LIMIT: i32 = 50;
const MIN_LIMIT: i32 = 10;
const MAX_LIMIT: i32 = 200;

#[derive(Debug, Deserialize)]
/// Torznab query parameters.
pub(crate) struct TorznabQuery {
    t: Option<String>,
    apikey: Option<String>,
    q: Option<String>,
    cat: Option<String>,
    imdbid: Option<String>,
    tmdbid: Option<String>,
    tvdbid: Option<String>,
    season: Option<String>,
    ep: Option<String>,
    offset: Option<String>,
    limit: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TorznabSearchMode {
    Generic,
    Tv,
    Movie,
}

#[derive(Debug)]
enum TorznabSearchFailure {
    Invalid(&'static str),
    Internal,
}

#[derive(Debug)]
struct ParsedTorznabSearch {
    mode: TorznabSearchMode,
    query_text: String,
    category_ids: Vec<i32>,
    identifier_input: Option<(&'static str, String)>,
    season: Option<i32>,
    episode: Option<i32>,
    offset: i32,
    limit: i32,
}

/// Handle Torznab API requests.
///
/// # Errors
///
/// Returns an internal error if the response cannot be constructed.
#[tracing::instrument(
    name = "torznab.request",
    skip(state, query),
    fields(torznab_instance_public_id = %torznab_instance_public_id)
)]
pub(crate) async fn torznab_api(
    State(state): State<Arc<ApiState>>,
    Path(torznab_instance_public_id): Path<Uuid>,
    Query(query): Query<TorznabQuery>,
) -> Result<Response, ApiError> {
    let api_key = match query.apikey.as_deref() {
        Some(value) if !value.trim().is_empty() => value.trim(),
        _ => {
            state
                .telemetry
                .inc_torznab_invalid_request("missing_apikey");
            return empty_status(StatusCode::UNAUTHORIZED);
        }
    };

    let auth = match state
        .indexers
        .torznab_instance_authenticate(torznab_instance_public_id, api_key)
        .await
    {
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
                        .inc_torznab_invalid_request("instance_not_found");
                    empty_status(StatusCode::NOT_FOUND)
                }
                TorznabAccessErrorKind::Storage => empty_status(StatusCode::INTERNAL_SERVER_ERROR),
            };
        }
    };

    let query_target = query
        .t
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_ascii_lowercase();

    if query_target == "caps" {
        let Ok(categories) = state.indexers.torznab_category_list().await else {
            return empty_status(StatusCode::INTERNAL_SERVER_ERROR);
        };
        let xml = build_caps_response(&auth.display_name, &categories)?;
        return xml_response(StatusCode::OK, xml);
    }

    let parsed = match parse_torznab_search_request(state.as_ref(), &query, &query_target).await {
        Ok(value) => value,
        Err(TorznabSearchFailure::Invalid(reason)) => {
            state.telemetry.inc_torznab_invalid_request(reason);
            let xml = build_empty_search_response();
            return xml_response(StatusCode::OK, xml);
        }
        Err(TorznabSearchFailure::Internal) => {
            return empty_status(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    let xml =
        match execute_torznab_search(state.as_ref(), torznab_instance_public_id, api_key, parsed)
            .await
        {
            Ok(value) => value,
            Err(TorznabSearchFailure::Invalid(reason)) => {
                state.telemetry.inc_torznab_invalid_request(reason);
                build_empty_search_response()
            }
            Err(TorznabSearchFailure::Internal) => {
                return empty_status(StatusCode::INTERNAL_SERVER_ERROR);
            }
        };

    xml_response(StatusCode::OK, xml)
}

impl TorznabSearchMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Tv => "tv",
            Self::Movie => "movie",
        }
    }
}

fn parse_search_mode(query_target: &str) -> Option<TorznabSearchMode> {
    match query_target {
        "" | "search" => Some(TorznabSearchMode::Generic),
        "tvsearch" => Some(TorznabSearchMode::Tv),
        "movie" | "moviesearch" => Some(TorznabSearchMode::Movie),
        _ => None,
    }
}

fn parse_non_negative_i32(value: Option<&str>) -> Result<Option<i32>, ()> {
    let Some(raw) = value.map(str::trim) else {
        return Ok(None);
    };
    if raw.is_empty() {
        return Ok(None);
    }

    match raw.parse::<i32>() {
        Ok(parsed) if parsed >= 0 => Ok(Some(parsed)),
        _ => Err(()),
    }
}

fn parse_category_tokens(value: Option<&str>) -> Vec<&str> {
    value
        .map(str::trim)
        .filter(|raw| !raw.is_empty())
        .map(|raw| {
            raw.split(',')
                .map(str::trim)
                .filter(|token| !token.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn parse_identifier_input(
    query: &TorznabQuery,
) -> Result<Option<(&'static str, String)>, &'static str> {
    let mut identifiers = Vec::new();

    if let Some(imdb_id) = normalize_imdb(query.imdbid.as_deref()) {
        identifiers.push(("imdb", imdb_id));
    } else if query.imdbid.as_deref().is_some() {
        return Err("invalid_identifier_combo");
    }

    if let Some(tmdb_id) = normalize_positive_number(query.tmdbid.as_deref()) {
        identifiers.push(("tmdb", tmdb_id));
    } else if query.tmdbid.as_deref().is_some() {
        return Err("invalid_identifier_combo");
    }

    if let Some(tvdb_id) = normalize_positive_number(query.tvdbid.as_deref()) {
        identifiers.push(("tvdb", tvdb_id));
    } else if query.tvdbid.as_deref().is_some() {
        return Err("invalid_identifier_combo");
    }

    if identifiers.len() > 1 {
        return Err("invalid_identifier_combo");
    }
    if let Some(identifier) = identifiers.pop() {
        return Ok(Some(identifier));
    }

    parse_identifier_from_query(query.q.as_deref())
}

fn normalize_imdb(value: Option<&str>) -> Option<String> {
    let raw = value?.trim().to_ascii_lowercase();
    if raw.is_empty() {
        return None;
    }

    let normalized = if raw.starts_with("tt") {
        raw
    } else {
        let mut prefixed = String::with_capacity(raw.len() + 2);
        prefixed.push_str("tt");
        prefixed.push_str(&raw);
        prefixed
    };

    let digits = normalized.strip_prefix("tt")?;
    let is_valid = (7..=9).contains(&digits.len()) && digits.chars().all(|ch| ch.is_ascii_digit());
    is_valid.then_some(normalized)
}

fn normalize_positive_number(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }

    let parsed = raw.parse::<i32>().ok()?;
    (parsed > 0).then(|| parsed.to_string())
}

#[derive(Default)]
struct IdentifierCandidates {
    imdb: Vec<String>,
    tmdb: Vec<String>,
    tvdb: Vec<String>,
}

impl IdentifierCandidates {
    fn push_token(&mut self, token: &str) {
        let lowered = token.to_ascii_lowercase();
        if let Some(imdb) = normalize_imdb(Some(&lowered)) {
            self.imdb.push(imdb);
            return;
        }

        if let Some(tmdb) = lowered
            .strip_prefix("tmdb:")
            .and_then(|suffix| normalize_positive_number(Some(suffix)))
        {
            self.tmdb.push(tmdb);
            return;
        }

        if let Some(tvdb) = lowered
            .strip_prefix("tvdb:")
            .and_then(|suffix| normalize_positive_number(Some(suffix)))
        {
            self.tvdb.push(tvdb);
        }
    }

    fn is_invalid(&self) -> bool {
        self.non_empty_count() > 1 || self.has_multiple_candidates()
    }

    fn into_identifier(self) -> Option<(&'static str, String)> {
        self.imdb
            .into_iter()
            .next()
            .map(|value| ("imdb", value))
            .or_else(|| self.tmdb.into_iter().next().map(|value| ("tmdb", value)))
            .or_else(|| self.tvdb.into_iter().next().map(|value| ("tvdb", value)))
    }

    fn non_empty_count(&self) -> usize {
        [
            !self.imdb.is_empty(),
            !self.tmdb.is_empty(),
            !self.tvdb.is_empty(),
        ]
        .into_iter()
        .filter(|present| *present)
        .count()
    }

    fn has_multiple_candidates(&self) -> bool {
        [self.imdb.len(), self.tmdb.len(), self.tvdb.len()]
            .into_iter()
            .any(|count| count > 1)
    }
}

fn parse_identifier_from_query(
    value: Option<&str>,
) -> Result<Option<(&'static str, String)>, &'static str> {
    let raw = value.map(str::trim).unwrap_or_default();
    if raw.is_empty() {
        return Ok(None);
    }

    if raw.split_whitespace().count() != 1 {
        return Ok(None);
    }

    let mut candidates = IdentifierCandidates::default();
    candidates.push_token(raw);

    if candidates.is_invalid() {
        return Err("invalid_identifier_combo");
    }

    Ok(candidates.into_identifier())
}

fn is_invalid_combo(
    mode: TorznabSearchMode,
    query_text: &str,
    season: Option<i32>,
    episode: Option<i32>,
    identifier: Option<&(&'static str, String)>,
) -> bool {
    match mode {
        TorznabSearchMode::Generic => season.is_some() || episode.is_some(),
        TorznabSearchMode::Movie => {
            season.is_some()
                || episode.is_some()
                || matches!(identifier, Some((identifier_type, _)) if *identifier_type == "tvdb")
        }
        TorznabSearchMode::Tv => {
            if episode.is_some() && season.is_none() {
                return true;
            }
            season.is_some() && query_text.is_empty() && identifier.is_none()
        }
    }
}

async fn parse_torznab_search_request(
    state: &ApiState,
    query: &TorznabQuery,
    query_target: &str,
) -> Result<ParsedTorznabSearch, TorznabSearchFailure> {
    let mode = parse_search_mode(query_target)
        .ok_or(TorznabSearchFailure::Invalid("unsupported_query_type"))?;

    let limit = parse_non_negative_i32(query.limit.as_deref())
        .map_err(|()| TorznabSearchFailure::Invalid("invalid_limit"))?
        .map_or(DEFAULT_LIMIT, |value| value.clamp(MIN_LIMIT, MAX_LIMIT));

    let offset = parse_non_negative_i32(query.offset.as_deref())
        .map_err(|()| TorznabSearchFailure::Invalid("invalid_offset"))?
        .unwrap_or(0);

    let season = parse_non_negative_i32(query.season.as_deref())
        .map_err(|()| TorznabSearchFailure::Invalid("invalid_season_episode_combo"))?;
    let episode = parse_non_negative_i32(query.ep.as_deref())
        .map_err(|()| TorznabSearchFailure::Invalid("invalid_season_episode_combo"))?;

    let category_ids = parse_and_sanitize_categories(state, query.cat.as_deref()).await?;
    let identifier_input = parse_identifier_input(query).map_err(TorznabSearchFailure::Invalid)?;
    let query_text = query
        .q
        .as_deref()
        .map(str::trim)
        .unwrap_or_default()
        .to_string();

    if is_invalid_combo(
        mode,
        &query_text,
        season,
        episode,
        identifier_input.as_ref(),
    ) {
        return Err(TorznabSearchFailure::Invalid(
            "invalid_season_episode_combo",
        ));
    }

    Ok(ParsedTorznabSearch {
        mode,
        query_text,
        category_ids,
        identifier_input,
        season,
        episode,
        offset,
        limit,
    })
}

async fn parse_and_sanitize_categories(
    state: &ApiState,
    cat: Option<&str>,
) -> Result<Vec<i32>, TorznabSearchFailure> {
    let raw_tokens = parse_category_tokens(cat);
    if raw_tokens.is_empty() {
        return Ok(Vec::new());
    }

    let categories = state
        .indexers
        .torznab_category_list()
        .await
        .map_err(|_| TorznabSearchFailure::Internal)?;

    let known_ids = categories
        .into_iter()
        .map(|category| category.torznab_cat_id)
        .collect::<BTreeSet<_>>();

    let mut category_ids = Vec::new();
    for token in &raw_tokens {
        if let Ok(category_id) = token.parse::<i32>()
            && known_ids.contains(&category_id)
        {
            category_ids.push(category_id);
        }
    }
    category_ids.sort_unstable();
    category_ids.dedup();

    if category_ids.is_empty() {
        return Err(TorznabSearchFailure::Invalid("invalid_category_filter"));
    }

    Ok(category_ids)
}

async fn execute_torznab_search(
    state: &ApiState,
    torznab_instance_public_id: Uuid,
    api_key: &str,
    search: ParsedTorznabSearch,
) -> Result<String, TorznabSearchFailure> {
    let limit_usize = usize::try_from(search.limit).unwrap_or(0);
    let offset_usize = usize::try_from(search.offset).unwrap_or(usize::MAX);
    let identifier_types = search
        .identifier_input
        .as_ref()
        .map(|(identifier_type, _)| vec![(*identifier_type).to_string()]);
    let identifier_values = search
        .identifier_input
        .as_ref()
        .map(|(_, value)| vec![value.clone()]);

    let params = SearchRequestCreateParams {
        actor_user_public_id: None,
        query_text: &search.query_text,
        query_type: search
            .identifier_input
            .as_ref()
            .map_or("free_text", |(identifier_type, _)| *identifier_type),
        torznab_mode: Some(search.mode.as_str()),
        requested_media_domain_key: None,
        page_size: Some(search.limit),
        search_profile_public_id: None,
        request_policy_set_public_id: None,
        season_number: search.season,
        episode_number: search.episode,
        identifier_types: identifier_types.as_deref(),
        identifier_values: identifier_values.as_deref(),
        torznab_cat_ids: (!search.category_ids.is_empty())
            .then_some(search.category_ids.as_slice()),
    };

    let search_request = state
        .indexers
        .search_request_create(params)
        .await
        .map_err(|error| map_search_request_failure(error.kind()))?;

    let page_summaries = state
        .indexers
        .search_page_list(
            SYSTEM_ACTOR_PUBLIC_ID,
            search_request.search_request_public_id,
        )
        .await
        .map_err(|_| TorznabSearchFailure::Internal)?
        .pages;

    let total = search_result_total(&page_summaries);
    let page_numbers = page_numbers_for_window(&page_summaries, offset_usize, limit_usize);
    let mut items = Vec::new();
    for page_number in page_numbers {
        let Ok(page) = state
            .indexers
            .search_page_fetch(
                SYSTEM_ACTOR_PUBLIC_ID,
                search_request.search_request_public_id,
                page_number,
            )
            .await
        else {
            return Err(TorznabSearchFailure::Internal);
        };
        items.extend(page.items);
    }

    let feed_items = map_feed_items(state, items, torznab_instance_public_id, api_key).await?;
    let start = if limit_usize == 0 {
        0
    } else {
        (offset_usize % limit_usize).min(feed_items.len())
    };
    let end = start.saturating_add(limit_usize).min(feed_items.len());

    build_search_response(&feed_items[start..end], search.offset, total)
        .map_err(|_| TorznabSearchFailure::Internal)
}

fn search_result_total(page_summaries: &[SearchPageSummaryResponse]) -> i32 {
    let exact_total = page_summaries.iter().fold(0usize, |total, summary| {
        total.saturating_add(usize::try_from(summary.item_count).unwrap_or(0))
    });
    i32::try_from(exact_total).unwrap_or(i32::MAX)
}

fn page_numbers_for_window(
    page_summaries: &[SearchPageSummaryResponse],
    offset: usize,
    limit: usize,
) -> Vec<i32> {
    if page_summaries.is_empty() || limit == 0 {
        return Vec::new();
    }

    let start = offset;
    let end = offset.saturating_add(limit);
    let mut page_start = 0usize;
    let mut selected = Vec::new();
    for summary in page_summaries {
        let item_count = usize::try_from(summary.item_count).unwrap_or(0);
        let page_end = page_start.saturating_add(item_count);
        if page_end > start && page_start < end {
            selected.push(summary.page_number);
        }
        page_start = page_end;
    }

    selected
}

const fn map_search_request_failure(kind: SearchRequestServiceErrorKind) -> TorznabSearchFailure {
    match kind {
        SearchRequestServiceErrorKind::Invalid
        | SearchRequestServiceErrorKind::NotFound
        | SearchRequestServiceErrorKind::Unauthorized => {
            TorznabSearchFailure::Invalid("invalid_query")
        }
        SearchRequestServiceErrorKind::Storage => TorznabSearchFailure::Internal,
    }
}

async fn map_feed_items(
    state: &ApiState,
    items: Vec<SearchPageItemResponse>,
    torznab_instance_public_id: Uuid,
    api_key: &str,
) -> Result<Vec<TorznabFeedItem>, TorznabSearchFailure> {
    let mut feed_items = Vec::new();

    for item in items {
        let Some(source_id) = item.canonical_torrent_source_public_id else {
            continue;
        };

        let categories = resolve_feed_categories(state, torznab_instance_public_id, &item).await?;

        feed_items.push(TorznabFeedItem {
            guid: source_id,
            title: item.title_display,
            size_bytes: item.size_bytes,
            published_at: item.published_at,
            categories,
            seeders: item.seeders.unwrap_or(0),
            leechers: item.leechers.unwrap_or(0),
            download_volume_factor: 1,
            infohash: item.infohash_v2.or(item.infohash_v1),
            download_link: build_download_link(torznab_instance_public_id, source_id, api_key),
        });
    }

    Ok(feed_items)
}

async fn resolve_feed_categories(
    state: &ApiState,
    torznab_instance_public_id: Uuid,
    item: &SearchPageItemResponse,
) -> Result<Vec<i32>, TorznabSearchFailure> {
    let Some(indexer_instance_public_id) = item.indexer_instance_public_id else {
        return Ok(vec![OTHER_CATEGORY_ID]);
    };

    let category_ids = state
        .indexers
        .torznab_feed_category_ids(
            torznab_instance_public_id,
            indexer_instance_public_id,
            item.tracker_category,
            item.tracker_subcategory,
        )
        .await
        .map_err(|_| TorznabSearchFailure::Internal)?;

    Ok(expand_torznab_categories(&category_ids))
}

fn expand_torznab_categories(category_ids: &[i32]) -> Vec<i32> {
    let mut expanded = BTreeSet::new();

    for &category_id in category_ids {
        if category_id <= 0 {
            continue;
        }

        let parent_category_id = (category_id / 1000) * 1000;
        if parent_category_id > 0 {
            expanded.insert(parent_category_id);
        }
        expanded.insert(category_id);
    }

    if expanded.is_empty() {
        return vec![OTHER_CATEGORY_ID];
    }

    expanded.into_iter().collect()
}

fn build_download_link(torznab_instance_public_id: Uuid, source_id: Uuid, api_key: &str) -> String {
    let query = form_urlencoded::Serializer::new(String::new())
        .append_pair("apikey", api_key)
        .finish();
    format!("/torznab/{torznab_instance_public_id}/download/{source_id}?{query}")
}

fn xml_response(status: StatusCode, body: String) -> Result<Response, ApiError> {
    Response::builder()
        .status(status)
        .header(CONTENT_TYPE, "application/xml; charset=utf-8")
        .body(Body::from(body))
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
    use crate::app::indexers::{
        SearchRequestServiceError, SearchRequestServiceErrorKind, TorznabCategory,
        TorznabInstanceAuth,
    };
    use crate::http::handlers::indexers::test_support::{RecordingIndexers, indexer_test_state};
    use crate::models::{
        SearchPageListResponse, SearchPageResponse, SearchRequestExplainabilityResponse,
    };
    use chrono::{TimeZone, Utc};
    use std::sync::Arc;

    fn torznab_query() -> TorznabQuery {
        TorznabQuery {
            t: None,
            apikey: None,
            q: None,
            cat: None,
            imdbid: None,
            tmdbid: None,
            tvdbid: None,
            season: None,
            ep: None,
            offset: None,
            limit: None,
        }
    }

    fn auth(display_name: &str) -> TorznabInstanceAuth {
        TorznabInstanceAuth {
            torznab_instance_id: 1,
            search_profile_id: 2,
            display_name: display_name.to_string(),
        }
    }

    fn explainability() -> SearchRequestExplainabilityResponse {
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

    async fn response_body(response: Response) -> String {
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("response body");
        String::from_utf8(body.to_vec()).expect("utf8 response body")
    }

    #[test]
    fn torznab_query_parses_optional_values() {
        let query = TorznabQuery {
            t: Some("caps".to_string()),
            apikey: Some("key".to_string()),
            q: None,
            cat: None,
            imdbid: None,
            tmdbid: None,
            tvdbid: None,
            season: None,
            ep: None,
            offset: None,
            limit: None,
        };
        assert_eq!(query.t.as_deref(), Some("caps"));
        assert_eq!(query.apikey.as_deref(), Some("key"));
    }

    #[test]
    fn empty_status_preserves_status_code() {
        let response = empty_status(StatusCode::UNAUTHORIZED).expect("response should build");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response.headers().get(CONTENT_TYPE).is_none());
    }

    #[test]
    fn xml_response_sets_content_type() {
        let response =
            xml_response(StatusCode::OK, "<caps/>".to_string()).expect("response should build");
        assert_eq!(
            response
                .headers()
                .get(CONTENT_TYPE)
                .and_then(|h| h.to_str().ok()),
            Some("application/xml; charset=utf-8")
        );
    }

    #[test]
    fn parse_non_negative_i32_handles_blank_valid_and_invalid_inputs() {
        assert_eq!(parse_non_negative_i32(None), Ok(None));
        assert_eq!(parse_non_negative_i32(Some("   ")), Ok(None));
        assert_eq!(parse_non_negative_i32(Some(" 42 ")), Ok(Some(42)));
        assert_eq!(parse_non_negative_i32(Some("0")), Ok(Some(0)));
        assert_eq!(parse_non_negative_i32(Some("-1")), Err(()));
        assert_eq!(parse_non_negative_i32(Some("abc")), Err(()));
    }

    #[test]
    fn parse_category_tokens_trims_and_discards_empty_entries() {
        let tokens = parse_category_tokens(Some(" 2000, 2040 ,, 500 "));
        assert_eq!(tokens, vec!["2000", "2040", "500"]);
        assert!(parse_category_tokens(Some("   ")).is_empty());
        assert!(parse_category_tokens(None).is_empty());
    }

    #[test]
    fn parse_search_mode_accepts_supported_values() {
        assert_eq!(
            parse_search_mode("search"),
            Some(TorznabSearchMode::Generic)
        );
        assert_eq!(parse_search_mode("tvsearch"), Some(TorznabSearchMode::Tv));
        assert_eq!(parse_search_mode("movie"), Some(TorznabSearchMode::Movie));
        assert_eq!(
            parse_search_mode("moviesearch"),
            Some(TorznabSearchMode::Movie)
        );
        assert_eq!(parse_search_mode("caps"), None);
    }

    #[test]
    fn normalize_imdb_accepts_prefixed_and_unprefixed_values() {
        assert_eq!(
            normalize_imdb(Some("tt1234567")),
            Some("tt1234567".to_string())
        );
        assert_eq!(
            normalize_imdb(Some("12345678")),
            Some("tt12345678".to_string())
        );
        assert_eq!(normalize_imdb(Some("tt12")), None);
        assert_eq!(normalize_imdb(Some("tt1234abc")), None);
        assert_eq!(normalize_imdb(Some("   ")), None);
    }

    #[test]
    fn normalize_positive_number_accepts_only_positive_integers() {
        assert_eq!(
            normalize_positive_number(Some(" 15 ")),
            Some("15".to_string())
        );
        assert_eq!(normalize_positive_number(Some("0")), None);
        assert_eq!(normalize_positive_number(Some("-3")), None);
        assert_eq!(normalize_positive_number(Some("abc")), None);
        assert_eq!(normalize_positive_number(Some("   ")), None);
    }

    #[test]
    fn parse_identifier_input_prefers_explicit_identifiers_and_rejects_conflicts() {
        let imdb_query = TorznabQuery {
            t: None,
            apikey: None,
            q: Some("ignored".to_string()),
            cat: None,
            imdbid: Some("1234567".to_string()),
            tmdbid: None,
            tvdbid: None,
            season: None,
            ep: None,
            offset: None,
            limit: None,
        };
        assert_eq!(
            parse_identifier_input(&imdb_query),
            Ok(Some(("imdb", "tt1234567".to_string())))
        );

        let conflicting_query = TorznabQuery {
            tmdbid: Some("42".to_string()),
            tvdbid: Some("9".to_string()),
            ..imdb_query
        };
        assert_eq!(
            parse_identifier_input(&conflicting_query),
            Err("invalid_identifier_combo")
        );
    }

    #[test]
    fn parse_identifier_input_falls_back_to_query_text() {
        let query = TorznabQuery {
            t: None,
            apikey: None,
            q: Some("tmdb:42".to_string()),
            cat: None,
            imdbid: None,
            tmdbid: None,
            tvdbid: None,
            season: None,
            ep: None,
            offset: None,
            limit: None,
        };
        assert_eq!(
            parse_identifier_input(&query),
            Ok(Some(("tmdb", "42".to_string())))
        );
    }

    #[test]
    fn parse_identifier_from_query_rejects_mixed_types() {
        let parsed = parse_identifier_from_query(Some("tt1234567 tmdb:42"));
        assert_eq!(parsed, Ok(None));
    }

    #[test]
    fn parse_identifier_from_query_parses_single_identifier() {
        let parsed = parse_identifier_from_query(Some("tt1234567")).expect("parsed");
        assert_eq!(parsed, Some(("imdb", "tt1234567".to_string())));
    }

    #[test]
    fn parse_identifier_from_query_ignores_identifier_plus_text() {
        let parsed = parse_identifier_from_query(Some("tt1234567 dune")).expect("parsed");
        assert_eq!(parsed, None);
    }

    #[test]
    fn parse_identifier_from_query_parses_prefixed_numeric_identifiers() {
        assert_eq!(
            parse_identifier_from_query(Some("tmdb:42")),
            Ok(Some(("tmdb", "42".to_string())))
        );
        assert_eq!(
            parse_identifier_from_query(Some("tvdb:77")),
            Ok(Some(("tvdb", "77".to_string())))
        );
        assert_eq!(
            parse_identifier_from_query(Some("tmdb:42 tmdb:43")),
            Ok(None)
        );
    }

    #[test]
    fn invalid_combo_flags_tv_without_anchor() {
        let invalid = is_invalid_combo(TorznabSearchMode::Tv, "", Some(1), None, None);
        assert!(invalid);
    }

    #[test]
    fn invalid_combo_rejects_generic_or_movie_with_tv_only_inputs() {
        assert!(is_invalid_combo(
            TorznabSearchMode::Generic,
            "query",
            Some(1),
            None,
            None,
        ));
        assert!(is_invalid_combo(
            TorznabSearchMode::Movie,
            "query",
            None,
            None,
            Some(&("tvdb", "55".to_string())),
        ));
        assert!(!is_invalid_combo(
            TorznabSearchMode::Tv,
            "query",
            Some(1),
            Some(2),
            None,
        ));
    }

    #[test]
    fn expand_torznab_categories_includes_parent_and_subcategory() {
        let categories = expand_torznab_categories(&[2010]);
        assert_eq!(categories, vec![2000, 2010]);
    }

    #[test]
    fn expand_torznab_categories_does_not_insert_zero_parent() {
        let categories = expand_torznab_categories(&[500]);
        assert_eq!(categories, vec![500]);
    }

    #[test]
    fn expand_torznab_categories_deduplicates_existing_parent() {
        let categories = expand_torznab_categories(&[2000, 2010, 2010]);
        assert_eq!(categories, vec![2000, 2010]);
    }

    #[test]
    fn expand_torznab_categories_falls_back_to_other() {
        let categories = expand_torznab_categories(&[]);
        assert_eq!(categories, vec![OTHER_CATEGORY_ID]);
    }

    #[test]
    fn expand_torznab_categories_ignores_non_positive_values() {
        let categories = expand_torznab_categories(&[0, -1, 5040]);
        assert_eq!(categories, vec![5000, 5040]);
    }

    #[test]
    fn build_download_link_url_encodes_api_key() {
        let link = build_download_link(Uuid::from_u128(1), Uuid::from_u128(2), "a&b=1+2%");
        assert_eq!(
            link,
            "/torznab/00000000-0000-0000-0000-000000000001/download/00000000-0000-0000-0000-000000000002?apikey=a%26b%3D1%2B2%25"
        );
    }

    #[test]
    fn search_result_total_sums_page_counts() {
        let pages = vec![
            SearchPageSummaryResponse {
                page_number: 1,
                item_count: 50,
                sealed_at: None,
            },
            SearchPageSummaryResponse {
                page_number: 2,
                item_count: 10,
                sealed_at: None,
            },
        ];

        assert_eq!(search_result_total(&pages), 60);
    }

    #[test]
    fn page_numbers_for_window_limits_fetches_to_requested_slice() {
        let pages = vec![
            SearchPageSummaryResponse {
                page_number: 1,
                item_count: 50,
                sealed_at: None,
            },
            SearchPageSummaryResponse {
                page_number: 2,
                item_count: 50,
                sealed_at: None,
            },
            SearchPageSummaryResponse {
                page_number: 3,
                item_count: 10,
                sealed_at: None,
            },
        ];

        let selected = page_numbers_for_window(&pages, 55, 20);
        assert_eq!(selected, vec![2]);
    }

    #[test]
    fn page_numbers_for_window_handles_empty_and_cross_page_ranges() {
        let pages = vec![
            SearchPageSummaryResponse {
                page_number: 1,
                item_count: 10,
                sealed_at: None,
            },
            SearchPageSummaryResponse {
                page_number: 2,
                item_count: 10,
                sealed_at: None,
            },
            SearchPageSummaryResponse {
                page_number: 3,
                item_count: 10,
                sealed_at: None,
            },
        ];

        assert!(page_numbers_for_window(&pages, 0, 0).is_empty());
        assert!(page_numbers_for_window(&[], 0, 10).is_empty());
        assert_eq!(page_numbers_for_window(&pages, 8, 10), vec![1, 2]);
    }

    #[test]
    fn map_search_request_failure_maps_invalid_and_storage_cases() {
        assert!(matches!(
            map_search_request_failure(SearchRequestServiceErrorKind::Invalid),
            TorznabSearchFailure::Invalid("invalid_query")
        ));
        assert!(matches!(
            map_search_request_failure(SearchRequestServiceErrorKind::NotFound),
            TorznabSearchFailure::Invalid("invalid_query")
        ));
        assert!(matches!(
            map_search_request_failure(SearchRequestServiceErrorKind::Unauthorized),
            TorznabSearchFailure::Invalid("invalid_query")
        ));
        assert!(matches!(
            map_search_request_failure(SearchRequestServiceErrorKind::Storage),
            TorznabSearchFailure::Internal
        ));
    }

    #[tokio::test]
    async fn torznab_api_returns_unauthorized_when_apikey_is_missing() {
        let indexers = RecordingIndexers::default();
        let state =
            indexer_test_state(Arc::new(indexers)).expect("torznab test state should initialize");

        let response = torznab_api(
            State(state),
            Path(Uuid::new_v4()),
            Query(TorznabQuery {
                t: Some("search".to_string()),
                ..torznab_query()
            }),
        )
        .await
        .expect("missing apikey response");

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(response_body(response).await.is_empty());
    }

    #[tokio::test]
    async fn torznab_api_returns_caps_xml_for_authenticated_instance() {
        let indexers = RecordingIndexers::default();
        indexers.set_torznab_auth_result(Ok(auth("Library")));
        indexers.set_torznab_category_list_result(Ok(vec![
            TorznabCategory {
                torznab_cat_id: 2000,
                name: "Movies".to_string(),
            },
            TorznabCategory {
                torznab_cat_id: 5000,
                name: "TV".to_string(),
            },
        ]));
        let state =
            indexer_test_state(Arc::new(indexers)).expect("torznab test state should initialize");

        let response = torznab_api(
            State(state),
            Path(Uuid::new_v4()),
            Query(TorznabQuery {
                t: Some("caps".to_string()),
                apikey: Some("secret".to_string()),
                ..torznab_query()
            }),
        )
        .await
        .expect("caps response");

        let body = response_body(response).await;
        assert!(body.contains("Library"));
        assert!(body.contains("Movies"));
        assert!(body.contains("5000"));
    }

    #[tokio::test]
    async fn torznab_api_returns_empty_search_for_invalid_category_filter() {
        let indexers = RecordingIndexers::default();
        indexers.set_torznab_auth_result(Ok(auth("Library")));
        indexers.set_torznab_category_list_result(Ok(vec![TorznabCategory {
            torznab_cat_id: 2000,
            name: "Movies".to_string(),
        }]));
        let state = indexer_test_state(Arc::new(indexers.clone()))
            .expect("torznab test state should initialize");

        let response = torznab_api(
            State(state),
            Path(Uuid::new_v4()),
            Query(TorznabQuery {
                t: Some("search".to_string()),
                apikey: Some("secret".to_string()),
                q: Some("dune".to_string()),
                cat: Some("9999".to_string()),
                ..torznab_query()
            }),
        )
        .await
        .expect("invalid category response");

        let body = response_body(response).await;
        assert!(body.contains("total=\"0\""));
        assert!(indexers.search_request_snapshots().is_empty());
    }

    #[tokio::test]
    async fn torznab_api_executes_tv_search_and_renders_feed_items() {
        let indexers = RecordingIndexers::default();
        let torznab_instance_public_id = Uuid::from_u128(1);
        let source_public_id = Uuid::from_u128(2);
        let indexer_instance_public_id = Uuid::from_u128(3);

        indexers.set_torznab_auth_result(Ok(auth("Library")));
        indexers.set_torznab_category_list_result(Ok(vec![TorznabCategory {
            torznab_cat_id: 2040,
            name: "TV HD".to_string(),
        }]));
        indexers.set_search_page_list_response(SearchPageListResponse {
            pages: vec![SearchPageSummaryResponse {
                page_number: 1,
                item_count: 1,
                sealed_at: None,
            }],
            explainability: explainability(),
        });
        indexers.set_search_page_fetch_response(SearchPageResponse {
            page_number: 1,
            sealed_at: None,
            item_count: 1,
            items: vec![SearchPageItemResponse {
                position: 1,
                canonical_torrent_public_id: Uuid::from_u128(4),
                title_display: "Example Episode".to_string(),
                size_bytes: Some(1234),
                infohash_v1: Some("a".repeat(40)),
                infohash_v2: None,
                magnet_hash: None,
                canonical_torrent_source_public_id: Some(source_public_id),
                indexer_instance_public_id: Some(indexer_instance_public_id),
                indexer_display_name: Some("Indexer".to_string()),
                seeders: Some(12),
                leechers: Some(3),
                published_at: Some(Utc.with_ymd_and_hms(2026, 4, 1, 0, 0, 0).unwrap()),
                download_url: None,
                magnet_uri: None,
                details_url: None,
                tracker_name: Some("Example".to_string()),
                tracker_category: Some(2),
                tracker_subcategory: Some(40),
            }],
        });
        indexers.set_torznab_feed_category_result(Ok(vec![2040]));
        let state = indexer_test_state(Arc::new(indexers.clone()))
            .expect("torznab test state should initialize");

        let response = torznab_api(
            State(state),
            Path(torznab_instance_public_id),
            Query(TorznabQuery {
                t: Some("tvsearch".to_string()),
                apikey: Some("secret".to_string()),
                q: Some("  Example  ".to_string()),
                cat: Some("2040".to_string()),
                tvdbid: Some("42".to_string()),
                season: Some("1".to_string()),
                ep: Some("2".to_string()),
                offset: Some("0".to_string()),
                limit: Some("5".to_string()),
                ..torznab_query()
            }),
        )
        .await
        .expect("search response");

        let body = response_body(response).await;
        assert!(body.contains("Example Episode"));
        assert!(body.contains("download/00000000-0000-0000-0000-000000000002"));
        assert!(body.contains("<category>2000</category>"));
        assert!(body.contains("<category>2040</category>"));

        let calls = indexers.search_request_snapshots();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].query_text, "Example");
        assert_eq!(calls[0].query_type, "tvdb");
        assert_eq!(calls[0].torznab_mode.as_deref(), Some("tv"));
        assert_eq!(calls[0].page_size, Some(10));
        assert_eq!(calls[0].season_number, Some(1));
        assert_eq!(calls[0].episode_number, Some(2));
        assert_eq!(
            calls[0].identifier_types.as_deref(),
            Some(&["tvdb".to_string()][..])
        );
        assert_eq!(
            calls[0].identifier_values.as_deref(),
            Some(&["42".to_string()][..])
        );
        assert_eq!(calls[0].torznab_cat_ids.as_deref(), Some(&[2040][..]));
    }

    #[tokio::test]
    async fn torznab_api_returns_server_error_when_search_page_fetch_fails() {
        let indexers = RecordingIndexers::default();
        indexers.set_torznab_auth_result(Ok(auth("Library")));
        indexers.set_search_page_list_response(SearchPageListResponse {
            pages: vec![SearchPageSummaryResponse {
                page_number: 1,
                item_count: 1,
                sealed_at: None,
            }],
            explainability: explainability(),
        });
        indexers.set_search_page_fetch_error(SearchRequestServiceError::new(
            SearchRequestServiceErrorKind::Storage,
        ));
        let state =
            indexer_test_state(Arc::new(indexers)).expect("torznab test state should initialize");

        let response = torznab_api(
            State(state),
            Path(Uuid::new_v4()),
            Query(TorznabQuery {
                t: Some("search".to_string()),
                apikey: Some("secret".to_string()),
                q: Some("dune".to_string()),
                ..torznab_query()
            }),
        )
        .await
        .expect("storage error response");

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
        assert!(response_body(response).await.is_empty());
    }
}
