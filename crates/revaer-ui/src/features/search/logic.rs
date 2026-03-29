//! Manual search helpers.
//!
//! # Design
//! - Keep request normalization pure so invalid input is rejected before transport.
//! - Reuse API DTOs to avoid duplicate schemas in the UI layer.
//! - Prefer stable, deterministic formatting for selectable search results.

use crate::features::search::state::SearchFormState;
use crate::models::{SearchPageItemResponse, SearchRequestCreateRequest};

/// Build an API request from the search form.
///
/// # Errors
///
/// Returns an error when required fields are missing or numeric fields are invalid.
pub(crate) fn build_search_request(
    form: &SearchFormState,
) -> Result<SearchRequestCreateRequest, String> {
    let query_type = required_value(&form.query_type, "query type")?;

    Ok(SearchRequestCreateRequest {
        query_text: form.query_text.trim().to_string(),
        query_type,
        torznab_mode: optional_value(&form.torznab_mode),
        requested_media_domain_key: optional_value(&form.requested_media_domain_key),
        page_size: parse_optional_i32(&form.page_size, "page size")?,
        search_profile_public_id: None,
        request_policy_set_public_id: None,
        season_number: parse_optional_i32(&form.season_number, "season number")?,
        episode_number: parse_optional_i32(&form.episode_number, "episode number")?,
        identifier_types: csv_values(&form.identifier_types),
        identifier_values: csv_values(&form.identifier_values),
        torznab_cat_ids: csv_i32_values(&form.torznab_cat_ids)?,
    })
}

/// Return a stable selection key for a result row.
#[must_use]
pub(crate) fn selection_key(item: &SearchPageItemResponse) -> String {
    item.canonical_torrent_source_public_id.map_or_else(
        || format!("{}:{}", item.canonical_torrent_public_id, item.position),
        |value| value.to_string(),
    )
}

/// Return the preferred source to push to the download client.
#[must_use]
pub(crate) fn preferred_source(item: &SearchPageItemResponse) -> Option<String> {
    item.magnet_uri
        .clone()
        .or_else(|| item.download_url.clone())
}

/// Human-readable size text.
#[must_use]
pub(crate) fn format_size(size_bytes: Option<i64>) -> String {
    let Some(raw_size) = size_bytes else {
        return "-".to_string();
    };
    if raw_size <= 0 {
        return "0 B".to_string();
    }
    let units = ["B", "KiB", "MiB", "GiB", "TiB"];
    let mut whole = raw_size;
    let mut unit_index = 0usize;
    while whole >= 1024 && unit_index + 1 < units.len() {
        whole /= 1024;
        unit_index += 1;
    }
    if unit_index == 0 {
        format!("{raw_size} {}", units[unit_index])
    } else {
        let divisor = 1024_i64.pow(u32::try_from(unit_index).unwrap_or(0));
        let major = raw_size / divisor;
        let remainder = raw_size % divisor;
        let decimal = (remainder * 10) / divisor;
        format!("{major}.{decimal} {}", units[unit_index])
    }
}

/// Short row subtitle assembled from tracker and swarm hints.
#[must_use]
pub(crate) fn result_meta(item: &SearchPageItemResponse) -> String {
    let mut parts = Vec::new();
    if let Some(indexer) = item.indexer_display_name.as_deref() {
        parts.push(indexer.to_string());
    }
    if let Some(tracker) = item.tracker_name.as_deref() {
        parts.push(tracker.to_string());
    }
    if let Some(seeders) = item.seeders {
        parts.push(format!("S:{seeders}"));
    }
    if let Some(leechers) = item.leechers {
        parts.push(format!("L:{leechers}"));
    }
    if parts.is_empty() {
        "-".to_string()
    } else {
        parts.join(" • ")
    }
}

fn required_value(value: &str, field_name: &str) -> Result<String, String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        Err(format!("{field_name} is required"))
    } else {
        Ok(normalized.to_string())
    }
}

fn optional_value(value: &str) -> Option<String> {
    let normalized = value.trim();
    (!normalized.is_empty()).then(|| normalized.to_string())
}

fn csv_values(value: &str) -> Option<Vec<String>> {
    let values: Vec<String> = value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToString::to_string)
        .collect();
    (!values.is_empty()).then_some(values)
}

fn parse_optional_i32(value: &str, field_name: &str) -> Result<Option<i32>, String> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Ok(None);
    }
    normalized
        .parse::<i32>()
        .map(Some)
        .map_err(|_| format!("{field_name} must be a valid integer"))
}

fn csv_i32_values(value: &str) -> Result<Option<Vec<i32>>, String> {
    let values = csv_values(value)
        .unwrap_or_default()
        .into_iter()
        .map(|entry| {
            entry
                .parse::<i32>()
                .map_err(|_| "Torznab categories must be comma-separated integers".to_string())
        })
        .collect::<Result<Vec<i32>, String>>()?;
    Ok((!values.is_empty()).then_some(values))
}

#[cfg(test)]
mod tests {
    use super::{build_search_request, format_size, preferred_source, result_meta, selection_key};
    use crate::features::search::state::SearchFormState;
    use crate::models::SearchPageItemResponse;
    use chrono::{TimeZone, Utc};
    use uuid::Uuid;

    fn sample_item() -> SearchPageItemResponse {
        SearchPageItemResponse {
            position: 2,
            canonical_torrent_public_id: Uuid::from_u128(10),
            title_display: "Dune".to_string(),
            size_bytes: Some(2_147_483_648),
            infohash_v1: None,
            infohash_v2: None,
            magnet_hash: None,
            canonical_torrent_source_public_id: Some(Uuid::from_u128(20)),
            indexer_instance_public_id: Some(Uuid::from_u128(30)),
            indexer_display_name: Some("Alpha".to_string()),
            seeders: Some(12),
            leechers: Some(4),
            published_at: Some(Utc.with_ymd_and_hms(2026, 3, 15, 0, 0, 0).unwrap()),
            download_url: Some("https://example.test/file.torrent".to_string()),
            magnet_uri: Some("magnet:?xt=urn:btih:demo".to_string()),
            details_url: None,
            tracker_name: Some("Tracker".to_string()),
            tracker_category: Some(2000),
            tracker_subcategory: Some(2010),
        }
    }

    #[test]
    fn build_search_request_parses_optional_fields() {
        let form = SearchFormState {
            query_text: " dune ".to_string(),
            query_type: " free_text ".to_string(),
            torznab_mode: " movie ".to_string(),
            requested_media_domain_key: " movies ".to_string(),
            page_size: "25".to_string(),
            season_number: String::new(),
            episode_number: String::new(),
            identifier_types: "imdb, tmdb".to_string(),
            identifier_values: "tt123, 456".to_string(),
            torznab_cat_ids: "2000, 2010".to_string(),
        };

        let request = build_search_request(&form).expect("request");
        assert_eq!(request.query_text, "dune");
        assert_eq!(request.query_type, "free_text");
        assert_eq!(request.torznab_mode.as_deref(), Some("movie"));
        assert_eq!(
            request.requested_media_domain_key.as_deref(),
            Some("movies")
        );
        assert_eq!(request.page_size, Some(25));
        assert_eq!(
            request.identifier_types,
            Some(vec!["imdb".to_string(), "tmdb".to_string()])
        );
        assert_eq!(request.torznab_cat_ids, Some(vec![2000, 2010]));
    }

    #[test]
    fn build_search_request_rejects_bad_category_ids() {
        let form = SearchFormState {
            query_type: "free_text".to_string(),
            torznab_cat_ids: "2000, nope".to_string(),
            ..SearchFormState::with_defaults()
        };

        let error = build_search_request(&form).expect_err("invalid");
        assert!(error.contains("Torznab categories"));
    }

    #[test]
    fn selection_key_prefers_source_public_id() {
        let item = sample_item();
        assert_eq!(selection_key(&item), Uuid::from_u128(20).to_string());
    }

    #[test]
    fn preferred_source_prefers_magnet() {
        let item = sample_item();
        assert_eq!(
            preferred_source(&item).as_deref(),
            Some("magnet:?xt=urn:btih:demo")
        );
    }

    #[test]
    fn format_size_scales_binary_units() {
        assert_eq!(format_size(Some(1_536)), "1.5 KiB");
        assert_eq!(format_size(None), "-");
    }

    #[test]
    fn result_meta_compacts_known_fields() {
        let item = sample_item();
        assert_eq!(result_meta(&item), "Alpha • Tracker • S:12 • L:4");
    }
}
