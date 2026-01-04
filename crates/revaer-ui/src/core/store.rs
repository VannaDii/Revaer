//! App-wide yewdux store slices.
//!
//! # Design
//! - Keep shared UI state in one store to avoid ad-hoc contexts.
//! - Use small, focused slices so reducers stay predictable.

use crate::core::auth::{AuthMode, AuthState};
#[cfg(target_arch = "wasm32")]
use crate::core::events::{UiEvent, UiEventEnvelope};
use crate::core::theme::ThemeMode;
use crate::core::ui::{Density, UiMode};
use crate::features::torrents::state::TorrentsState;
#[cfg(target_arch = "wasm32")]
use crate::features::torrents::state::{
    ProgressPatch, remove_row, update_fsops_completed, update_fsops_failed, update_fsops_progress,
    update_fsops_started, update_metadata, update_status,
};
use crate::i18n::{DEFAULT_LOCALE, LocaleCode};
use crate::models::{Toast, TorrentLabelEntry};
#[cfg(target_arch = "wasm32")]
use chrono::{DateTime, Utc};
#[cfg(target_arch = "wasm32")]
use revaer_events::{Event as CoreEvent, TorrentState as CoreTorrentState};
#[cfg(target_arch = "wasm32")]
use uuid::Uuid;
#[cfg(target_arch = "wasm32")]
use yewdux::dispatch::Dispatch;
use yewdux::store::Store;

/// Global application store for shared state.
#[derive(Clone, Debug, PartialEq, Store, Default)]
pub struct AppStore {
    /// Authentication + setup flow state.
    pub auth: AuthSlice,
    /// Shared UI shell state.
    pub ui: UiSlice,
    /// Torrent list/detail state.
    pub torrents: TorrentsState,
    /// Cached category/tag policy state.
    pub labels: LabelsSlice,
    /// Health snapshot cache.
    pub health: HealthSlice,
    /// System/SSE connection state.
    pub system: SystemState,
}

#[cfg(target_arch = "wasm32")]
#[must_use]
pub(crate) fn app_dispatch() -> Dispatch<AppStore> {
    Dispatch::global()
}

/// Shared authentication state for the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthSlice {
    /// Preferred auth mode (API key or local auth).
    pub mode: AuthMode,
    /// Active auth state.
    pub state: Option<AuthState>,
    /// Whether to prefer API key auth in the prompt.
    pub bypass_local: bool,
    /// Current setup gating state.
    pub app_mode: AppModeState,
    /// Setup token returned by the API.
    pub setup_token: Option<String>,
    /// Setup token expiry timestamp.
    pub setup_expires_at: Option<String>,
    /// Setup flow error message.
    pub setup_error: Option<String>,
    /// Setup flow busy flag.
    pub setup_busy: bool,
}

/// Shared UI shell state for modals/toasts/navigation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UiSlice {
    /// Current theme selection.
    pub theme: ThemeMode,
    /// Current UI mode (simple/advanced).
    pub mode: UiMode,
    /// Density selection for list layouts.
    pub density: Density,
    /// Active locale selection.
    pub locale: LocaleCode,
    /// Active toast notifications.
    pub toasts: Vec<Toast>,
    /// Modal/drawer/FAB open states.
    pub panels: UiPanels,
    /// Busy flags for UI operations.
    pub busy: UiBusyState,
}

impl Default for UiSlice {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            mode: UiMode::Simple,
            density: Density::Normal,
            locale: DEFAULT_LOCALE,
            toasts: Vec::new(),
            panels: UiPanels::default(),
            busy: UiBusyState::default(),
        }
    }
}

/// UI panel open/closed flags.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct UiPanels {
    /// Whether a blocking modal is open.
    pub modal_open: bool,
    /// Whether a drawer panel is open.
    pub drawer_open: bool,
    /// Whether the main FAB is expanded.
    pub fab_open: bool,
}

/// Busy flags for the UI shell.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct UiBusyState {
    /// True while an add-torrent request is in flight.
    pub add_torrent: bool,
    /// True while a create-torrent request is in flight.
    pub create_torrent: bool,
}

/// Cached category/tag label policies.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct LabelsSlice {
    /// Label policies for categories.
    pub categories: std::collections::HashMap<String, TorrentLabelEntry>,
    /// Label policies for tags.
    pub tags: std::collections::HashMap<String, TorrentLabelEntry>,
}

/// Cached health snapshots.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct HealthSlice {
    /// Basic health snapshot.
    pub basic: Option<HealthSnapshot>,
    /// Full health snapshot.
    pub full: Option<FullHealthSnapshot>,
    /// Raw metrics text (Prometheus format) when fetched.
    pub metrics_text: Option<String>,
}

/// Basic health response cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthSnapshot {
    /// Overall status ("ok", "degraded").
    pub status: String,
    /// Application mode string.
    pub mode: String,
    /// Database component status.
    pub database_status: Option<String>,
    /// Database schema revision, when known.
    pub database_revision: Option<i64>,
}

/// Full health response cache.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullHealthSnapshot {
    /// Overall status ("ok", "degraded").
    pub status: String,
    /// Application mode string.
    pub mode: String,
    /// Schema revision identifier.
    pub revision: i64,
    /// Build identifier.
    pub build: String,
    /// Degraded component list.
    pub degraded: Vec<String>,
    /// Metrics snapshot for config and guardrails.
    pub metrics: HealthMetricsSnapshot,
    /// Torrent health snapshot for queue sizing.
    pub torrent: TorrentHealthSnapshot,
}

/// Health metrics response snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HealthMetricsSnapshot {
    /// Config watch latency in milliseconds.
    pub config_watch_latency_ms: i64,
    /// Config apply latency in milliseconds.
    pub config_apply_latency_ms: i64,
    /// Total count of config update failures.
    pub config_update_failures_total: u64,
    /// Total count of slow config watches.
    pub config_watch_slow_total: u64,
    /// Total count of guardrail violations.
    pub guardrail_violations_total: u64,
    /// Total count of rate-limit throttles.
    pub rate_limit_throttled_total: u64,
}

/// Torrent-level health snapshot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TorrentHealthSnapshot {
    /// Count of active torrents.
    pub active: i64,
    /// Queue depth snapshot.
    pub queue_depth: i64,
}

/// High-level connectivity states for the SSE client.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SseConnectionState {
    /// Connected and receiving events.
    Connected,
    /// Disconnected without an active retry timer.
    Disconnected,
    /// Waiting to retry with backoff.
    Reconnecting,
}

/// Detailed SSE error information.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseError {
    /// Human-readable reason for the latest failure.
    pub message: String,
    /// Optional HTTP status code, when available.
    pub status_code: Option<u16>,
}

/// Detailed SSE connection status for UI diagnostics.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SseStatus {
    /// Current connectivity state.
    pub state: SseConnectionState,
    /// Current backoff delay in milliseconds.
    pub backoff_ms: Option<u64>,
    /// Timestamp (ms since epoch) for the next retry.
    pub next_retry_at_ms: Option<u64>,
    /// Last numeric event id observed.
    pub last_event_id: Option<u64>,
    /// Last observed error details.
    pub last_error: Option<SseError>,
    /// Optional auth mode label for diagnostics.
    pub auth_mode: Option<String>,
}

impl Default for SseStatus {
    fn default() -> Self {
        Self {
            state: SseConnectionState::Reconnecting,
            backoff_ms: None,
            next_retry_at_ms: None,
            last_event_id: None,
            last_error: Some(SseError {
                message: "connecting".to_string(),
                status_code: None,
            }),
            auth_mode: None,
        }
    }
}

/// Minimal SSE status slice for the connectivity indicator.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SseStatusSummary {
    /// Current connectivity state.
    pub state: SseConnectionState,
    /// Timestamp (ms since epoch) for the next retry.
    pub next_retry_at_ms: Option<u64>,
    /// Whether a last error is present.
    pub has_error: bool,
}

impl Default for AuthSlice {
    fn default() -> Self {
        Self {
            mode: AuthMode::ApiKey,
            state: None,
            bypass_local: false,
            app_mode: AppModeState::Loading,
            setup_token: None,
            setup_expires_at: None,
            setup_error: None,
            setup_busy: false,
        }
    }
}

/// Current high-level app mode used for setup gating.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppModeState {
    /// Initial loading state.
    Loading,
    /// Setup flow is required.
    Setup,
    /// App is active and ready for auth.
    Active,
}

/// System-level state, including SSE connection status.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct SystemState {
    /// Aggregate transfer rates.
    pub rates: SystemRates,
    /// SSE connection status.
    pub sse_status: SseStatus,
}

/// Aggregate transfer rates reported by SSE or polling.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct SystemRates {
    /// Aggregate download rate in bytes per second.
    pub download_bps: u64,
    /// Aggregate upload rate in bytes per second.
    pub upload_bps: u64,
}

/// Read the aggregate system rates from the store.
#[must_use]
pub const fn select_system_rates(store: &AppStore) -> SystemRates {
    store.system.rates
}

/// Read the current SSE connection status.
#[must_use]
pub fn select_sse_status(store: &AppStore) -> SseStatus {
    store.system.sse_status.clone()
}

/// Read the SSE status summary used by the connectivity indicator.
#[must_use]
pub const fn select_sse_status_summary(store: &AppStore) -> SseStatusSummary {
    let status = &store.system.sse_status;
    SseStatusSummary {
        state: status.state,
        next_retry_at_ms: status.next_retry_at_ms,
        has_error: status.last_error.is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_store_seeds_auth_and_ui_state() {
        let store = AppStore::default();
        assert_eq!(store.ui.theme, ThemeMode::Dark);
        assert_eq!(store.ui.mode, UiMode::Simple);
        assert_eq!(store.ui.density, Density::Normal);
        assert_eq!(store.ui.locale, DEFAULT_LOCALE);
        assert!(store.ui.toasts.is_empty());

        assert_eq!(store.auth.mode, AuthMode::ApiKey);
        assert_eq!(store.auth.app_mode, AppModeState::Loading);
        assert!(!store.auth.bypass_local);
        assert!(store.auth.state.is_none());
    }

    #[test]
    fn default_sse_status_marks_connecting() {
        let status = SseStatus::default();
        assert_eq!(status.state, SseConnectionState::Reconnecting);
        assert_eq!(
            status.last_error.as_ref().map(|err| err.message.as_str()),
            Some("connecting")
        );
    }

    #[test]
    fn selectors_return_expected_snapshots() {
        let mut store = AppStore::default();
        store.system.rates = SystemRates {
            download_bps: 12,
            upload_bps: 34,
        };
        store.system.sse_status = SseStatus {
            state: SseConnectionState::Connected,
            backoff_ms: Some(1_000),
            next_retry_at_ms: Some(5_000),
            last_event_id: Some(9),
            last_error: None,
            auth_mode: Some("api_key".to_string()),
        };

        assert_eq!(
            select_system_rates(&store),
            SystemRates {
                download_bps: 12,
                upload_bps: 34,
            }
        );
        let summary = select_sse_status_summary(&store);
        assert_eq!(summary.state, SseConnectionState::Connected);
        assert_eq!(summary.next_retry_at_ms, Some(5_000));
        assert!(!summary.has_error);
    }
}

/// Result of applying a normalized SSE envelope.
#[cfg(target_arch = "wasm32")]
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum SseApplyOutcome {
    /// Applied directly to store state.
    Applied,
    /// Progress update to coalesce before applying.
    Progress(ProgressPatch),
    /// Requires a targeted refresh of list/detail data.
    Refresh,
    /// Refresh a single torrent detail payload to update list metadata.
    RefreshTorrent {
        /// Torrent identifier to refresh.
        id: Uuid,
    },
    /// System rates update for dashboard-only state.
    SystemRates {
        /// Aggregate download rate in bytes per second.
        download_bps: u64,
        /// Aggregate upload rate in bytes per second.
        upload_bps: u64,
    },
}

/// Apply a normalized SSE envelope to the app store.
#[cfg(target_arch = "wasm32")]
pub(crate) fn apply_sse_envelope(
    store: &mut AppStore,
    envelope: UiEventEnvelope,
) -> SseApplyOutcome {
    let UiEventEnvelope {
        timestamp, event, ..
    } = envelope;
    match event {
        UiEvent::SystemRates {
            download_bps,
            upload_bps,
        } => SseApplyOutcome::SystemRates {
            download_bps,
            upload_bps,
        },
        UiEvent::Core(event) => match event {
            CoreEvent::Progress {
                torrent_id,
                bytes_downloaded,
                bytes_total,
                eta_seconds,
                download_bps,
                upload_bps,
                ratio,
            } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                SseApplyOutcome::Progress(ProgressPatch {
                    id: torrent_id,
                    progress: progress_ratio(bytes_downloaded, bytes_total),
                    size_bytes: Some(bytes_total),
                    eta_seconds,
                    download_bps: Some(download_bps),
                    upload_bps: Some(upload_bps),
                    ratio: Some(ratio),
                    updated: format_updated_timestamp(&timestamp),
                })
            }
            CoreEvent::StateChanged { torrent_id, state } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                update_status(
                    &mut store.torrents,
                    torrent_id,
                    format_state(&state),
                    format_updated_timestamp(&timestamp),
                );
                SseApplyOutcome::Applied
            }
            CoreEvent::Completed { torrent_id, .. } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                update_status(
                    &mut store.torrents,
                    torrent_id,
                    "completed".to_string(),
                    format_updated_timestamp(&timestamp),
                );
                SseApplyOutcome::Applied
            }
            CoreEvent::MetadataUpdated {
                torrent_id,
                name,
                download_dir,
                ..
            } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                update_metadata(
                    &mut store.torrents,
                    torrent_id,
                    name,
                    download_dir,
                    format_updated_timestamp(&timestamp),
                );
                SseApplyOutcome::RefreshTorrent { id: torrent_id }
            }
            CoreEvent::TorrentRemoved { torrent_id } => {
                remove_row(&mut store.torrents, torrent_id);
                SseApplyOutcome::Applied
            }
            CoreEvent::TorrentAdded { .. } | CoreEvent::FilesDiscovered { .. } => {
                SseApplyOutcome::Refresh
            }
            CoreEvent::FsopsStarted { torrent_id } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                update_fsops_started(&mut store.torrents, torrent_id);
                SseApplyOutcome::Applied
            }
            CoreEvent::FsopsProgress { torrent_id, step } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                update_fsops_progress(&mut store.torrents, torrent_id, step);
                SseApplyOutcome::Applied
            }
            CoreEvent::FsopsCompleted { torrent_id } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                update_fsops_completed(&mut store.torrents, torrent_id);
                SseApplyOutcome::Applied
            }
            CoreEvent::FsopsFailed {
                torrent_id,
                message,
            } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                let status = format!("failed: {message}");
                update_fsops_failed(&mut store.torrents, torrent_id, message);
                update_status(
                    &mut store.torrents,
                    torrent_id,
                    status,
                    format_updated_timestamp(&timestamp),
                );
                SseApplyOutcome::Applied
            }
            CoreEvent::SelectionReconciled { .. }
            | CoreEvent::SettingsChanged { .. }
            | CoreEvent::HealthChanged { .. } => SseApplyOutcome::Refresh,
        },
    }
}

#[cfg(target_arch = "wasm32")]
fn torrent_exists(state: &TorrentsState, id: Uuid) -> bool {
    state.by_id.contains_key(&id)
}

#[cfg(target_arch = "wasm32")]
fn progress_ratio(downloaded: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        u64_to_f64(downloaded) / u64_to_f64(total)
    }
}

#[cfg(target_arch = "wasm32")]
fn u64_to_f64(value: u64) -> f64 {
    const TWO_POW_32: f64 = 4_294_967_296.0;
    let high = u32::try_from(value >> 32).unwrap_or(0);
    let low = u32::try_from(value & 0xFFFF_FFFF).unwrap_or(0);
    (f64::from(high) * TWO_POW_32) + f64::from(low)
}

#[cfg(target_arch = "wasm32")]
fn format_updated_timestamp(timestamp: &str) -> Option<String> {
    let parsed = DateTime::parse_from_rfc3339(timestamp).ok()?;
    Some(
        parsed
            .with_timezone(&Utc)
            .format("%Y-%m-%d %H:%M UTC")
            .to_string(),
    )
}

#[cfg(target_arch = "wasm32")]
fn format_state(state: &CoreTorrentState) -> String {
    match state {
        CoreTorrentState::Queued => "queued".to_string(),
        CoreTorrentState::FetchingMetadata => "fetching_metadata".to_string(),
        CoreTorrentState::Downloading => "downloading".to_string(),
        CoreTorrentState::Seeding => "seeding".to_string(),
        CoreTorrentState::Completed => "completed".to_string(),
        CoreTorrentState::Failed { message } => format!("failed: {message}"),
        CoreTorrentState::Stopped => "stopped".to_string(),
    }
}
