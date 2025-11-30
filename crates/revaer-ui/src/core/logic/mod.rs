//! Pure UI helpers extracted from components for non-wasm testing.

use crate::features::torrents::state::TorrentRow;
use std::collections::BTreeSet;
use std::fmt::Write;

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
}

/// Toggle the presence of an id in the selection set.
#[must_use]
pub fn toggle_selection(selected: &BTreeSet<String>, id: &str) -> BTreeSet<String> {
    let mut next = selected.clone();
    if !next.remove(id) {
        next.insert(id.to_string());
    }
    next
}

/// Select all rows or clear when already fully selected.
#[must_use]
pub fn select_all_or_clear(selected: &BTreeSet<String>, rows: &[TorrentRow]) -> BTreeSet<String> {
    if selected.len() == rows.len() {
        BTreeSet::new()
    } else {
        rows.iter().map(|row| row.id.clone()).collect()
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
    file_present: bool,
) -> Result<AddPayload, AddInputError> {
    validate_add_input(value, file_present)?;
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

/// Build the SSE URL with optional `api_key` query.
#[must_use]
pub fn build_sse_url(base_url: &str, api_key: &Option<String>) -> String {
    let mut url = format!("{}/v1/events/stream", base_url.trim_end_matches('/'));
    if let Some(key) = api_key {
        let _ = write!(url, "?api_key={key}");
    }
    url
}

/// Build the torrents list path from search/regex flags.
#[must_use]
pub fn build_torrents_path(search: &Option<String>, regex: bool) -> String {
    search.as_ref().filter(|s| !s.is_empty()).map_or_else(
        || {
            if regex {
                "/v1/torrents?regex=true".to_string()
            } else {
                "/v1/torrents".to_string()
            }
        },
        |query| {
            let encoded = urlencoding::encode(query);
            format!(
                "/v1/torrents?search={encoded}{}",
                if regex { "&regex=true" } else { "" }
            )
        },
    )
}

/// Column planning for responsive tables. Returns visible vs overflow columns.
#[must_use]
pub fn plan_columns(width: u16) -> (Vec<&'static str>, Vec<&'static str>) {
    const REQUIRED: [&str; 5] = ["name", "status", "progress", "down", "up"];
    const OPTIONAL: [&str; 5] = ["eta", "ratio", "size", "tags", "path"];
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
    use crate::features::torrents::state::TorrentRow;

    fn row(id: &str) -> TorrentRow {
        TorrentRow {
            id: id.to_string(),
            name: "n".into(),
            status: "s".into(),
            progress: 0.0,
            eta: None,
            ratio: 0.0,
            tags: vec![],
            tracker: String::new(),
            path: String::new(),
            category: String::new(),
            size_bytes: 0,
            upload_bps: 0,
            download_bps: 0,
        }
    }

    #[test]
    fn toggle_selection_adds_and_removes() {
        let set = BTreeSet::new();
        let added = toggle_selection(&set, "1");
        assert!(added.contains("1"));
        let removed = toggle_selection(&added, "1");
        assert!(removed.is_empty());
    }

    #[test]
    fn select_all_clears_when_full() {
        let rows = vec![row("1"), row("2")];
        let empty = BTreeSet::new();
        let all = select_all_or_clear(&empty, &rows);
        assert_eq!(all.len(), 2);
        let cleared = select_all_or_clear(&all, &rows);
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
    fn build_add_payload_parses_tags_and_fields() {
        let payload =
            build_add_payload("magnet:?xt=urn:btih:abc", "tv", "4k, hevc", "/data", false).unwrap();
        assert_eq!(payload.category.as_deref(), Some("tv"));
        assert_eq!(payload.save_path.as_deref(), Some("/data"));
        assert_eq!(
            payload.tags,
            Some(vec!["4k".to_string(), "hevc".to_string()])
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
        assert_eq!(
            build_torrents_path(&None, false),
            "/v1/torrents".to_string()
        );
        assert_eq!(
            build_torrents_path(&Some("abc".into()), true),
            "/v1/torrents?search=abc&regex=true"
        );
    }

    #[test]
    fn sse_url_includes_key_when_present() {
        assert_eq!(
            build_sse_url("http://x", &None),
            "http://x/v1/events/stream"
        );
        assert_eq!(
            build_sse_url("http://x/", &Some("k".into())),
            "http://x/v1/events/stream?api_key=k"
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
    fn plan_columns_collapses_optionals() {
        let (xs_visible, xs_overflow) = plan_columns(crate::breakpoints::XS.max_width.unwrap());
        assert!(xs_visible.contains(&"status"));
        assert!(xs_overflow.contains(&"eta"));
        let (lg_visible, lg_overflow) = plan_columns(crate::breakpoints::LG.min_width);
        assert!(lg_visible.contains(&"size"));
        assert!(lg_overflow.is_empty());
    }
}
