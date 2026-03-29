//! Pure helpers for indexer admin forms.
//!
//! # Design
//! - Parse text-box driven admin inputs into strongly typed request values.
//! - Keep normalization deterministic so UI behavior stays testable.
//! - Return constant-style validation failures as plain strings for toast display.

use uuid::Uuid;

#[must_use]
pub(crate) fn split_csv(value: &str) -> Vec<String> {
    value
        .split([',', '\n'])
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

pub(crate) fn optional_uuid(value: &str) -> Result<Option<Uuid>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    Uuid::parse_str(trimmed)
        .map(Some)
        .map_err(|_| "invalid UUID".to_string())
}

pub(crate) fn required_uuid(value: &str) -> Result<Uuid, String> {
    optional_uuid(value)?.ok_or_else(|| "UUID is required".to_string())
}

pub(crate) fn optional_i32(value: &str) -> Result<Option<i32>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<i32>()
        .map(Some)
        .map_err(|_| "invalid integer".to_string())
}

pub(crate) fn required_i32(value: &str) -> Result<i32, String> {
    optional_i32(value)?.ok_or_else(|| "integer is required".to_string())
}

pub(crate) fn optional_i64(value: &str) -> Result<Option<i64>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<i64>()
        .map(Some)
        .map_err(|_| "invalid integer".to_string())
}

pub(crate) fn required_i64(value: &str) -> Result<i64, String> {
    optional_i64(value)?.ok_or_else(|| "integer is required".to_string())
}

pub(crate) fn optional_bool(value: &str) -> Result<Option<bool>, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    match trimmed {
        "true" => Ok(Some(true)),
        "false" => Ok(Some(false)),
        _ => Err("invalid bool".to_string()),
    }
}

#[must_use]
pub(crate) fn optional_string(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
}

pub(crate) fn uuid_list(value: &str) -> Result<Vec<Uuid>, String> {
    split_csv(value)
        .into_iter()
        .map(|entry| Uuid::parse_str(&entry).map_err(|_| "invalid UUID list".to_string()))
        .collect()
}

#[must_use]
pub(crate) fn string_list(value: &str) -> Vec<String> {
    split_csv(value)
}

#[must_use]
pub(crate) fn filtered_definitions<'a>(
    definitions: &'a [crate::models::IndexerDefinitionResponse],
    filter: &str,
) -> Vec<&'a crate::models::IndexerDefinitionResponse> {
    let needle = filter.trim().to_ascii_lowercase();
    if needle.is_empty() {
        return definitions.iter().collect();
    }
    definitions
        .iter()
        .filter(|definition| {
            definition
                .upstream_slug
                .to_ascii_lowercase()
                .contains(&needle)
                || definition
                    .display_name
                    .to_ascii_lowercase()
                    .contains(&needle)
                || definition.engine.to_ascii_lowercase().contains(&needle)
                || definition.protocol.to_ascii_lowercase().contains(&needle)
        })
        .collect()
}

#[must_use]
pub(crate) fn connectivity_status_badge_class(status: Option<&str>) -> &'static str {
    match status {
        Some("ok" | "healthy" | "passing") => "badge badge-success badge-outline",
        Some("degraded" | "challenged" | "cooldown") => "badge badge-warning badge-outline",
        Some("failing" | "quarantined" | "blocked") => "badge badge-error badge-outline",
        Some(_) => "badge badge-neutral badge-outline",
        None => "badge badge-ghost",
    }
}

#[must_use]
pub(crate) fn format_optional_percent(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".to_string(), |rate| format!("{:.1}%", rate * 100.0))
}

#[cfg(test)]
mod tests {
    use super::{
        connectivity_status_badge_class, filtered_definitions, format_optional_percent,
        optional_bool, split_csv, uuid_list,
    };
    use crate::models::IndexerDefinitionResponse;

    #[test]
    fn split_csv_trims_and_skips_empty_values() {
        assert_eq!(
            split_csv(" alpha, beta\n\n gamma "),
            vec!["alpha", "beta", "gamma"]
        );
    }

    #[test]
    fn uuid_list_rejects_invalid_values() {
        assert!(uuid_list("not-a-uuid").is_err());
    }

    #[test]
    fn optional_bool_accepts_known_values() {
        assert_eq!(optional_bool("true"), Ok(Some(true)));
        assert_eq!(optional_bool("false"), Ok(Some(false)));
        assert!(optional_bool("yes").is_err());
    }

    #[test]
    fn filtered_definitions_matches_multiple_fields() {
        let definitions = vec![IndexerDefinitionResponse {
            upstream_slug: "rarbg".to_string(),
            display_name: "RARBG".to_string(),
            protocol: "torrent".to_string(),
            engine: "torznab".to_string(),
            requires_login: false,
            supports_search: true,
            supports_tv_search: true,
            supports_movie_search: true,
            fields: Vec::new(),
        }];
        let filtered = filtered_definitions(&definitions, "torznab");
        assert_eq!(filtered.len(), 1);
    }

    #[test]
    fn connectivity_status_badge_class_maps_known_states() {
        assert_eq!(
            connectivity_status_badge_class(Some("healthy")),
            "badge badge-success badge-outline"
        );
        assert_eq!(
            connectivity_status_badge_class(Some("degraded")),
            "badge badge-warning badge-outline"
        );
        assert_eq!(
            connectivity_status_badge_class(Some("failing")),
            "badge badge-error badge-outline"
        );
        assert_eq!(connectivity_status_badge_class(None), "badge badge-ghost");
    }

    #[test]
    fn format_optional_percent_handles_missing_values() {
        assert_eq!(format_optional_percent(Some(0.975)), "97.5%");
        assert_eq!(format_optional_percent(None), "n/a");
    }
}
