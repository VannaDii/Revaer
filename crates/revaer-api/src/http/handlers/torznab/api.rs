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
use uuid::Uuid;

use crate::app::indexers::{
    SearchRequestCreateParams, SearchRequestServiceErrorKind, TorznabAccessErrorKind,
};
use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::http::handlers::indexers::SYSTEM_ACTOR_PUBLIC_ID;
use crate::models::SearchPageItemResponse;

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

    let mut candidates = IdentifierCandidates::default();
    for token in raw.split_whitespace() {
        candidates.push_token(token);
    }

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

    let mut items = Vec::new();
    for summary in page_summaries {
        let Ok(page) = state
            .indexers
            .search_page_fetch(
                SYSTEM_ACTOR_PUBLIC_ID,
                search_request.search_request_public_id,
                summary.page_number,
            )
            .await
        else {
            return Err(TorznabSearchFailure::Internal);
        };
        items.extend(page.items);
    }

    let feed_items = map_feed_items(state, items, torznab_instance_public_id, api_key).await?;
    let total = i32::try_from(feed_items.len()).unwrap_or(i32::MAX);
    let start = usize::try_from(search.offset)
        .ok()
        .map_or(feed_items.len(), |value| value.min(feed_items.len()));
    let limit_usize = usize::try_from(search.limit).unwrap_or(0);
    let end = start.saturating_add(limit_usize).min(feed_items.len());

    build_search_response(&feed_items[start..end], search.offset, total)
        .map_err(|_| TorznabSearchFailure::Internal)
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
            download_link: format!(
                "/torznab/{torznab_instance_public_id}/download/{source_id}?apikey={api_key}"
            ),
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
        expanded.insert(parent_category_id);
        expanded.insert(category_id);
    }

    if expanded.is_empty() {
        return vec![OTHER_CATEGORY_ID];
    }

    expanded.into_iter().collect()
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
    fn parse_identifier_from_query_rejects_mixed_types() {
        let parsed = parse_identifier_from_query(Some("tt1234567 tmdb:42"));
        assert_eq!(parsed, Err("invalid_identifier_combo"));
    }

    #[test]
    fn parse_identifier_from_query_parses_single_identifier() {
        let parsed = parse_identifier_from_query(Some("tt1234567")).expect("parsed");
        assert_eq!(parsed, Some(("imdb", "tt1234567".to_string())));
    }

    #[test]
    fn invalid_combo_flags_tv_without_anchor() {
        let invalid = is_invalid_combo(TorznabSearchMode::Tv, "", Some(1), None, None);
        assert!(invalid);
    }

    #[test]
    fn expand_torznab_categories_includes_parent_and_subcategory() {
        let categories = expand_torznab_categories(&[2010]);
        assert_eq!(categories, vec![2000, 2010]);
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
}
