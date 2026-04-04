//! Input normalization helpers for indexer handlers.

use crate::http::errors::ApiError;

pub(super) fn trim_and_filter_empty(value: Option<&str>) -> Option<&str> {
    value.map(str::trim).filter(|trimmed| !trimmed.is_empty())
}

pub(super) fn normalize_required_str_field<'a>(
    value: &'a str,
    error_message: &'static str,
) -> Result<&'a str, ApiError> {
    let normalized = value.trim();
    if normalized.is_empty() {
        return Err(ApiError::bad_request(error_message));
    }
    Ok(normalized)
}
