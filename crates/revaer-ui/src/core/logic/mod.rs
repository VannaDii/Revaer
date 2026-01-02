//! Pure UI helpers extracted from components for non-wasm testing.

use crate::features::torrents::state::{SelectionSet, TorrentsPaging, TorrentsQueryModel};
use uuid::Uuid;

/// Layout mode for the torrent list based on breakpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LayoutMode {
    /// Card view for mobile.
    Card,
    /// Table view for tablet/desktop.
    Table,
}

/// Interpret a keyboard shortcut and return a semantic action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShortcutOutcome {
    /// Focus the search input.
    FocusSearch,
    /// Move to the next row.
    SelectNext,
    /// Move to the previous row.
    SelectPrev,
    /// Toggle pause/resume for the selected row.
    TogglePauseResume,
    /// Clear the search query.
    ClearSearch,
    /// Show delete confirmation (metadata only).
    ConfirmDelete,
    /// Show delete + data confirmation.
    ConfirmDeleteData,
    /// Show recheck confirmation.
    ConfirmRecheck,
}

/// Possible add-torrent validation failures.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddInputError {
    /// No value or file provided.
    Empty,
    /// Value is neither magnet nor URL.
    Invalid,
    /// Rate limit input is not a valid integer.
    RateInvalid,
}

/// Validated add request payload without file handle.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AddPayload {
    /// Optional magnet/URL source (None when file is used).
    pub value: Option<String>,
    /// Optional category.
    pub category: Option<String>,
    /// Optional tags.
    pub tags: Option<Vec<String>>,
    /// Optional save path.
    pub save_path: Option<String>,
    /// Optional download rate limit in bytes per second.
    pub max_download_bps: Option<u64>,
    /// Optional upload rate limit in bytes per second.
    pub max_upload_bps: Option<u64>,
}

/// Toggle the presence of an id in the selection set.
#[must_use]
pub fn toggle_selection(selected: &SelectionSet, id: &Uuid) -> SelectionSet {
    let mut next = selected.clone();
    if !next.remove(id) {
        next.insert(*id);
    }
    next
}

/// Select all rows or clear when already fully selected.
#[must_use]
pub fn select_all_or_clear(selected: &SelectionSet, ids: &[Uuid]) -> SelectionSet {
    if selected.len() == ids.len() {
        SelectionSet::default()
    } else {
        ids.iter().copied().collect()
    }
}

/// Map a key press to a shortcut outcome when modifiers are handled.
#[must_use]
pub fn interpret_shortcut(key: &str, shift: bool) -> Option<ShortcutOutcome> {
    match key {
        "/" => Some(ShortcutOutcome::FocusSearch),
        "j" | "J" => Some(ShortcutOutcome::SelectNext),
        "k" | "K" => Some(ShortcutOutcome::SelectPrev),
        " " => Some(ShortcutOutcome::TogglePauseResume),
        "Escape" => Some(ShortcutOutcome::ClearSearch),
        "Delete" if shift => Some(ShortcutOutcome::ConfirmDeleteData),
        "Delete" => Some(ShortcutOutcome::ConfirmDelete),
        "p" | "P" => Some(ShortcutOutcome::ConfirmRecheck),
        _ => None,
    }
}

/// Validate add-torrent input for magnet/URL vs file presence.
///
/// # Errors
/// Returns [`AddInputError::Empty`] when both value and file are missing, or
/// [`AddInputError::Invalid`] when the value is neither a magnet nor an HTTP(S) URL.
pub fn validate_add_input(value: &str, file_present: bool) -> Result<(), AddInputError> {
    if value.trim().is_empty() && !file_present {
        return Err(AddInputError::Empty);
    }
    if file_present {
        return Ok(());
    }
    let is_magnet = value.starts_with("magnet:?xt=urn:btih:");
    let is_url = value.starts_with("http://") || value.starts_with("https://");
    if is_magnet || is_url {
        Ok(())
    } else {
        Err(AddInputError::Invalid)
    }
}

/// Build a validated add payload from raw form fields (excluding file).
///
/// # Errors
/// Returns [`AddInputError`] when the value fails validation (see [`validate_add_input`]).
pub fn build_add_payload(
    value: &str,
    category: &str,
    tags: &str,
    save_path: &str,
    max_download_bps: &str,
    max_upload_bps: &str,
    file_present: bool,
) -> Result<AddPayload, AddInputError> {
    validate_add_input(value, file_present)?;
    let max_download_bps =
        parse_rate_input(max_download_bps).map_err(|_| AddInputError::RateInvalid)?;
    let max_upload_bps =
        parse_rate_input(max_upload_bps).map_err(|_| AddInputError::RateInvalid)?;
    let tags_parsed = parse_tags(tags);
    let category_val = if category.trim().is_empty() {
        None
    } else {
        Some(category.trim().to_string())
    };
    let save_val = if save_path.trim().is_empty() {
        None
    } else {
        Some(save_path.trim().to_string())
    };
    Ok(AddPayload {
        value: if file_present || value.trim().is_empty() {
            None
        } else {
            Some(value.trim().to_string())
        },
        category: category_val,
        tags: tags_parsed,
        save_path: save_val,
        max_download_bps,
        max_upload_bps,
    })
}

/// Human-friendly rate formatter using binary units.
#[must_use]
pub fn format_rate(value: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * 1024 * 1024;
    if value >= GIB {
        let whole = value / GIB;
        let tenths = (value % GIB) * 10 / GIB;
        format!("{whole}.{tenths} GiB/s")
    } else if value >= MIB {
        let whole = value / MIB;
        let tenths = (value % MIB) * 10 / MIB;
        format!("{whole}.{tenths} MiB/s")
    } else if value >= KIB {
        let whole = value / KIB;
        let tenths = (value % KIB) * 10 / KIB;
        format!("{whole}.{tenths} KiB/s")
    } else {
        format!("{value} B/s")
    }
}

/// Human-friendly byte formatter using binary units.
#[must_use]
pub fn format_bytes(value: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * 1024;
    const GIB: u64 = 1024 * 1024 * 1024;
    if value >= GIB {
        let whole = value / GIB;
        let tenths = (value % GIB) * 10 / GIB;
        format!("{whole}.{tenths} GiB")
    } else if value >= MIB {
        let whole = value / MIB;
        let tenths = (value % MIB) * 10 / MIB;
        format!("{whole}.{tenths} MiB")
    } else if value >= KIB {
        let whole = value / KIB;
        let tenths = (value % KIB) * 10 / KIB;
        format!("{whole}.{tenths} KiB")
    } else {
        format!("{value} B")
    }
}

/// Rate input parsing errors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RateInputError {
    /// Provided value was not a valid integer.
    Invalid,
}

/// Parse a rate-limit input string into an optional bytes-per-second value.
///
/// # Errors
/// Returns [`RateInputError::Invalid`] if the input is not a valid integer.
pub(crate) fn parse_rate_input(value: &str) -> Result<Option<u64>, RateInputError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }
    trimmed
        .parse::<u64>()
        .map(Some)
        .map_err(|_| RateInputError::Invalid)
}

/// Exponential backoff (1s → 30s) for SSE reconnect attempts.
#[must_use]
pub fn backoff_delay_ms(attempt: u32) -> u32 {
    let capped = attempt.min(5);
    let delay = 1_000u32.saturating_mul(2u32.saturating_pow(capped));
    delay.clamp(1_000, 30_000)
}

/// Determine layout mode from breakpoint metadata.
#[must_use]
pub fn layout_for_breakpoint(bp: crate::breakpoints::Breakpoint) -> LayoutMode {
    if bp.max_width.is_some() && bp.max_width.unwrap_or(0) < crate::breakpoints::MD.min_width {
        LayoutMode::Card
    } else {
        LayoutMode::Table
    }
}

/// Compute row height based on density and layout mode.
#[must_use]
pub fn row_height(density: crate::Density, layout: LayoutMode) -> u32 {
    if layout == LayoutMode::Card {
        return 210;
    }
    match density {
        crate::Density::Compact => 120,
        crate::Density::Normal => 148,
        crate::Density::Comfy => 164,
    }
}

/// Move selection based on shortcut; returns the next index if it changed.
#[must_use]
pub fn advance_selection(outcome: ShortcutOutcome, current: usize, total: usize) -> Option<usize> {
    match outcome {
        ShortcutOutcome::SelectNext if total > 0 => {
            Some((current + 1).min(total.saturating_sub(1)))
        }
        ShortcutOutcome::SelectPrev if total > 0 => Some(current.saturating_sub(1)),
        _ => None,
    }
}

/// Parse comma-separated tags; returns None when empty.
#[must_use]
pub fn parse_tags(raw: &str) -> Option<Vec<String>> {
    let parsed: Vec<String> = raw
        .split(',')
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect();
    if parsed.is_empty() {
        None
    } else {
        Some(parsed)
    }
}

/// Supported SSE endpoints for the UI.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SseEndpoint {
    /// Primary torrents SSE endpoint.
    Primary,
    /// Legacy fallback endpoint.
    Fallback,
}

/// SSE view contexts that influence filter selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SseView {
    /// List view updates only.
    List,
    /// Detail drawer focused updates.
    Detail,
}

/// Structured SSE query parameters for filtering.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseQuery {
    /// Optional torrent filter (comma-separated UUIDs).
    pub torrent: Option<String>,
    /// Optional event filter (comma-separated kinds).
    pub event: Option<String>,
    /// Optional state filter (comma-separated states).
    pub state: Option<String>,
}

impl SseQuery {
    const fn is_empty(&self) -> bool {
        self.torrent.is_none() && self.event.is_none() && self.state.is_none()
    }
}

/// Build SSE query parameters from UI state.
#[must_use]
pub fn build_sse_query(
    visible_ids: &[Uuid],
    selected_id: Option<Uuid>,
    state_filter: Option<String>,
    view: SseView,
) -> SseQuery {
    const ID_CAP: usize = 120;
    let torrent_ids = selected_id.map_or_else(
        || {
            if visible_ids.len() <= ID_CAP {
                visible_ids.to_vec()
            } else {
                Vec::new()
            }
        },
        |id| vec![id],
    );

    let torrent = if torrent_ids.is_empty() {
        None
    } else {
        Some(
            torrent_ids
                .iter()
                .map(Uuid::to_string)
                .collect::<Vec<_>>()
                .join(","),
        )
    };

    let mut kinds = base_event_kinds().to_vec();
    if view == SseView::Detail {
        kinds.extend(detail_event_kinds());
    }
    let event = Some(kinds.join(","));

    SseQuery {
        torrent,
        event,
        state: state_filter,
    }
}

fn collect_filter_params(filters: &TorrentsQueryModel) -> Vec<String> {
    let mut params = Vec::new();
    if !filters.name.trim().is_empty() {
        params.push(format!("name={}", urlencoding::encode(filters.name.trim())));
    }
    if let Some(state) = filters.state.as_ref().filter(|s| !s.trim().is_empty()) {
        params.push(format!("state={}", urlencoding::encode(state.trim())));
    }
    if !filters.tags.is_empty() {
        let tags = filters
            .tags
            .iter()
            .map(|tag| tag.trim())
            .filter(|tag| !tag.is_empty())
            .collect::<Vec<_>>()
            .join(",");
        if !tags.is_empty() {
            params.push(format!("tags={}", urlencoding::encode(&tags)));
        }
    }
    if let Some(tracker) = filters.tracker.as_ref().filter(|t| !t.trim().is_empty()) {
        params.push(format!("tracker={}", urlencoding::encode(tracker.trim())));
    }
    if let Some(extension) = filters.extension.as_ref().filter(|e| !e.trim().is_empty()) {
        let normalized = extension.trim_start_matches('.');
        params.push(format!("extension={}", urlencoding::encode(normalized)));
    }
    params
}

fn decode_query_value(raw: &str) -> Option<String> {
    let normalized = raw.replace('+', " ");
    urlencoding::decode(&normalized)
        .ok()
        .map(std::borrow::Cow::into_owned)
}

/// Build the query string for torrent list filters (no paging parameters).
#[must_use]
pub fn build_torrent_filter_query(filters: &TorrentsQueryModel) -> String {
    collect_filter_params(filters).join("&")
}

/// Parse torrent list filters from a URL query string.
#[must_use]
pub fn parse_torrent_filter_query(query: &str) -> TorrentsQueryModel {
    let mut filters = TorrentsQueryModel::default();
    let query = query.trim_start_matches('?');
    if query.is_empty() {
        return filters;
    }
    for pair in query.split('&') {
        if pair.trim().is_empty() {
            continue;
        }
        let mut parts = pair.splitn(2, '=');
        let key_raw = parts.next().unwrap_or_default();
        let value_raw = parts.next().unwrap_or_default();
        let Some(key) = decode_query_value(key_raw) else {
            continue;
        };
        let value = decode_query_value(value_raw).unwrap_or_default();
        match key.as_str() {
            "name" => {
                filters.name = value;
            }
            "state" => {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    filters.state = Some(trimmed.to_string());
                }
            }
            "tags" => {
                if let Some(tags) = parse_tags(&value) {
                    filters.tags = tags;
                }
            }
            "tracker" => {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    filters.tracker = Some(trimmed.to_string());
                }
            }
            "extension" => {
                let normalized = value.trim().trim_start_matches('.');
                if !normalized.is_empty() {
                    filters.extension = Some(normalized.to_string());
                }
            }
            _ => {}
        }
    }
    filters
}

const fn base_event_kinds() -> [&'static str; 8] {
    [
        "torrent_added",
        "torrent_removed",
        "progress",
        "state_changed",
        "completed",
        "metadata_updated",
        "settings_changed",
        "health_changed",
    ]
}

const fn detail_event_kinds() -> [&'static str; 6] {
    [
        "files_discovered",
        "selection_reconciled",
        "fsops_started",
        "fsops_progress",
        "fsops_completed",
        "fsops_failed",
    ]
}

/// Build the SSE URL for the requested endpoint.
#[must_use]
pub fn build_sse_url(base_url: &str, endpoint: SseEndpoint, query: Option<&SseQuery>) -> String {
    let base = base_url.trim_end_matches('/');
    let mut url = match endpoint {
        SseEndpoint::Primary => format!("{base}/v1/torrents/events"),
        SseEndpoint::Fallback => format!("{base}/v1/events/stream"),
    };
    if let Some(query) = query
        && !query.is_empty()
    {
        let mut params = Vec::new();
        if let Some(torrent) = &query.torrent {
            params.push(format!("torrent={}", urlencoding::encode(torrent)));
        }
        if let Some(event) = &query.event {
            params.push(format!("event={}", urlencoding::encode(event)));
        }
        if let Some(state) = &query.state {
            params.push(format!("state={}", urlencoding::encode(state)));
        }
        if !params.is_empty() {
            url.push('?');
            url.push_str(&params.join("&"));
        }
    }
    url
}

/// Build the torrents list path from filters and paging state.
#[must_use]
pub fn build_torrents_path(filters: &TorrentsQueryModel, paging: &TorrentsPaging) -> String {
    const DEFAULT_LIMIT: u32 = 50;
    let mut params = collect_filter_params(filters);
    if let Some(cursor) = paging.cursor.as_ref().filter(|c| !c.trim().is_empty()) {
        params.push(format!("cursor={}", urlencoding::encode(cursor.trim())));
    }
    if paging.limit != DEFAULT_LIMIT || paging.cursor.is_some() {
        params.push(format!("limit={}", paging.limit));
    }
    if params.is_empty() {
        "/v1/torrents".to_string()
    } else {
        format!("/v1/torrents?{}", params.join("&"))
    }
}

/// Column planning for responsive tables. Returns visible vs overflow columns.
#[must_use]
pub fn plan_columns(width: u16) -> (Vec<&'static str>, Vec<&'static str>) {
    const REQUIRED: [&str; 5] = ["name", "status", "progress", "down", "up"];
    const OPTIONAL: [&str; 6] = ["eta", "ratio", "size", "tags", "path", "updated"];
    if width < crate::breakpoints::MD.min_width {
        (REQUIRED.to_vec(), OPTIONAL.to_vec())
    } else if width < crate::breakpoints::LG.min_width {
        let mut visible = REQUIRED.to_vec();
        visible.push("eta");
        visible.push("size");
        (
            visible,
            OPTIONAL
                .iter()
                .copied()
                .filter(|c| !["eta", "size"].contains(c))
                .collect(),
        )
    } else {
        let mut visible = REQUIRED.to_vec();
        visible.extend(OPTIONAL);
        (visible, Vec::new())
    }
}

/// Calculate virtual window bounds for a scrolling list.
#[must_use]
pub fn compute_window(
    viewport_height: u32,
    scroll_top: u32,
    row_height: u32,
    len: usize,
    overscan: u32,
) -> (usize, usize, u64, u64) {
    let rh = row_height.max(1);
    let visible = viewport_height.div_ceil(rh).max(1);
    let start = (scroll_top / rh) as usize;
    let end = (start + visible as usize + overscan as usize).min(len);
    let offset = u64::from(rh) * start as u64;
    let total_height = u64::from(rh).saturating_mul(len as u64);
    (start, end, offset, total_height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;
    use std::io;

    type Result<T> = std::result::Result<T, Box<dyn Error>>;

    fn test_error(message: &'static str) -> Box<dyn Error> {
        Box::new(io::Error::other(message))
    }

    #[test]
    fn parse_rate_input_handles_empty_and_values() {
        assert_eq!(parse_rate_input(""), Ok(None));
        assert_eq!(parse_rate_input("  "), Ok(None));
        assert_eq!(parse_rate_input("1024"), Ok(Some(1024)));
    }

    #[test]
    fn parse_rate_input_rejects_invalid_numbers() {
        assert_eq!(parse_rate_input("abc"), Err(RateInputError::Invalid));
        assert_eq!(parse_rate_input("10.5"), Err(RateInputError::Invalid));
    }

    #[test]
    fn toggle_selection_adds_and_removes() {
        let id = Uuid::from_u128(1);
        let set = SelectionSet::default();
        let added = toggle_selection(&set, &id);
        assert!(added.contains(&id));
        let removed = toggle_selection(&added, &id);
        assert!(removed.is_empty());
    }

    #[test]
    fn select_all_clears_when_full() {
        let ids = vec![Uuid::from_u128(1), Uuid::from_u128(2)];
        let empty = SelectionSet::default();
        let all = select_all_or_clear(&empty, &ids);
        assert_eq!(all.len(), 2);
        let cleared = select_all_or_clear(&all, &ids);
        assert!(cleared.is_empty());
    }

    #[test]
    fn interpret_shortcuts_cover_keys() {
        assert_eq!(
            interpret_shortcut("Delete", true),
            Some(ShortcutOutcome::ConfirmDeleteData)
        );
        assert_eq!(
            interpret_shortcut("Delete", false),
            Some(ShortcutOutcome::ConfirmDelete)
        );
        assert_eq!(
            interpret_shortcut("p", false),
            Some(ShortcutOutcome::ConfirmRecheck)
        );
        assert!(interpret_shortcut("x", false).is_none());
    }

    #[test]
    fn validate_add_input_rejects_invalid() {
        assert_eq!(validate_add_input("", false), Err(AddInputError::Empty));
        assert_eq!(
            validate_add_input("foo", false),
            Err(AddInputError::Invalid)
        );
        assert!(validate_add_input("magnet:?xt=urn:btih:abc", false).is_ok());
        assert!(validate_add_input("https://example.org/file.torrent", false).is_ok());
        assert!(validate_add_input("", true).is_ok());
    }

    #[test]
    fn build_add_payload_parses_tags_and_fields() -> Result<()> {
        let payload = build_add_payload(
            "magnet:?xt=urn:btih:abc",
            "tv",
            "4k, hevc",
            ".server_root/downloads",
            "",
            "",
            false,
        )
        .map_err(|_| test_error("build add payload failed"))?;
        assert_eq!(payload.category.as_deref(), Some("tv"));
        assert_eq!(payload.save_path.as_deref(), Some(".server_root/downloads"));
        assert_eq!(
            payload.tags,
            Some(vec!["4k".to_string(), "hevc".to_string()])
        );
        Ok(())
    }

    #[test]
    fn build_add_payload_rejects_invalid_rates() {
        assert_eq!(
            build_add_payload("magnet:?xt=urn:btih:abc", "", "", "", "oops", "", false),
            Err(AddInputError::RateInvalid)
        );
    }

    #[test]
    fn rate_formatting_scales_units() {
        assert_eq!(format_rate(512), "512 B/s");
        assert_eq!(format_rate(2048), "2.0 KiB/s");
        assert!(format_rate(5_242_880).contains("MiB"));
        assert!(format_rate(2_147_483_648).contains("GiB"));
    }

    #[test]
    fn byte_formatting_scales_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KiB");
        assert!(format_bytes(5_242_880).contains("MiB"));
        assert!(format_bytes(2_147_483_648).contains("GiB"));
    }

    #[test]
    fn backoff_is_bounded() {
        assert_eq!(backoff_delay_ms(0), 1_000);
        assert_eq!(backoff_delay_ms(3), 8_000);
        assert_eq!(backoff_delay_ms(10), 30_000);
    }

    #[test]
    fn layout_mode_switches_at_md() {
        use crate::breakpoints;
        assert_eq!(layout_for_breakpoint(breakpoints::XS), LayoutMode::Card);
        assert_eq!(layout_for_breakpoint(breakpoints::MD), LayoutMode::Table);
    }

    #[test]
    fn selection_advances_with_bounds() {
        assert_eq!(
            advance_selection(ShortcutOutcome::SelectNext, 0, 5),
            Some(1)
        );
        assert_eq!(
            advance_selection(ShortcutOutcome::SelectPrev, 0, 5),
            Some(0)
        );
        assert!(advance_selection(ShortcutOutcome::FocusSearch, 0, 5).is_none());
    }

    #[test]
    fn row_height_matches_density_and_layout() {
        assert_eq!(row_height(crate::Density::Normal, LayoutMode::Card), 210);
        assert_eq!(row_height(crate::Density::Compact, LayoutMode::Table), 120);
        assert_eq!(row_height(crate::Density::Comfy, LayoutMode::Table), 164);
    }

    #[test]
    fn tags_are_parsed_and_cleaned() {
        assert_eq!(
            parse_tags("a, b , ,c"),
            Some(vec!["a".into(), "b".into(), "c".into()])
        );
        assert_eq!(parse_tags(" ,, "), None);
    }

    #[test]
    fn torrent_path_builder_handles_modes() {
        let filters = TorrentsQueryModel::default();
        let paging = TorrentsPaging::default();
        assert_eq!(
            build_torrents_path(&filters, &paging),
            "/v1/torrents".to_string()
        );
        let filters = TorrentsQueryModel {
            name: "abc".into(),
            tags: vec!["one".into(), "two".into()],
            ..TorrentsQueryModel::default()
        };
        let paging = TorrentsPaging {
            cursor: Some("cursor".into()),
            limit: 75,
            ..TorrentsPaging::default()
        };
        assert_eq!(
            build_torrents_path(&filters, &paging),
            "/v1/torrents?name=abc&tags=one%2Ctwo&cursor=cursor&limit=75"
        );
    }

    #[test]
    fn torrent_filter_query_round_trips() {
        let filters = TorrentsQueryModel {
            name: "alpha beta".into(),
            state: Some("downloading".into()),
            tags: vec!["one".into(), "two".into()],
            tracker: Some("tracker".into()),
            extension: Some("mkv".into()),
        };
        let query = build_torrent_filter_query(&filters);
        assert_eq!(
            query,
            "name=alpha%20beta&state=downloading&tags=one%2Ctwo&tracker=tracker&extension=mkv"
        );
        let parsed = parse_torrent_filter_query(&format!("?{query}"));
        assert_eq!(parsed, filters);
    }

    #[test]
    fn sse_url_respects_endpoints() {
        assert_eq!(
            build_sse_url("http://x", SseEndpoint::Primary, None),
            "http://x/v1/torrents/events"
        );
        assert_eq!(
            build_sse_url("http://x/", SseEndpoint::Fallback, None),
            "http://x/v1/events/stream"
        );
    }

    #[test]
    fn window_calc_is_integer_based() {
        let (start, end, offset, total) = compute_window(100, 0, 20, 10, 2);
        assert_eq!(start, 0);
        assert!(end > start);
        assert_eq!(offset, 0);
        assert_eq!(total, 200);
    }

    #[test]
    fn plan_columns_collapses_optionals() -> Result<()> {
        let xs_max = crate::breakpoints::XS
            .max_width
            .ok_or_else(|| test_error("missing XS max width"))?;
        let (xs_visible, xs_overflow) = plan_columns(xs_max);
        assert!(xs_visible.contains(&"status"));
        assert!(xs_overflow.contains(&"eta"));
        let (lg_visible, lg_overflow) = plan_columns(crate::breakpoints::LG.min_width);
        assert!(lg_visible.contains(&"size"));
        assert!(lg_overflow.is_empty());
        Ok(())
    }
}
