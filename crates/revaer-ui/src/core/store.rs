//! App-wide yewdux store slices.
//!
//! # Design
//! - Keep shared UI state in one store to avoid ad-hoc contexts.
//! - Use small, focused slices so reducers stay predictable.

use crate::core::auth::{AuthMode, AuthState};
#[cfg(target_arch = "wasm32")]
use crate::core::events::{UiEvent, UiEventEnvelope};
use crate::features::torrents::state::TorrentsState;
#[cfg(target_arch = "wasm32")]
use crate::features::torrents::state::{ProgressPatch, remove_row, update_metadata, update_status};
use crate::models::SseState;
#[cfg(target_arch = "wasm32")]
use revaer_events::{Event as CoreEvent, TorrentState as CoreTorrentState};
#[cfg(target_arch = "wasm32")]
use uuid::Uuid;
use yewdux::store::Store;

/// Global application store for shared state.
#[derive(Clone, Debug, PartialEq, Store, Default)]
pub struct AppStore {
    /// Authentication + setup flow state.
    pub auth: AuthSlice,
    /// Torrent list/detail state.
    pub torrents: TorrentsState,
    /// System/SSE connection state.
    pub system: SystemState,
}

/// Shared authentication state for the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthSlice {
    /// Preferred auth mode (API key or local auth).
    pub mode: AuthMode,
    /// Active auth state.
    pub state: Option<AuthState>,
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

impl Default for AuthSlice {
    fn default() -> Self {
        Self {
            mode: AuthMode::ApiKey,
            state: None,
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
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemState {
    /// SSE connection status.
    pub sse_state: SseState,
}

impl Default for SystemState {
    fn default() -> Self {
        Self {
            sse_state: SseState::Reconnecting {
                retry_in_secs: 3,
                last_event: "initial".to_string(),
                reason: "connecting".to_string(),
            },
        }
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
    match envelope.event {
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
            } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                SseApplyOutcome::Progress(ProgressPatch {
                    id: torrent_id,
                    progress: progress_ratio(bytes_downloaded, bytes_total),
                    eta_seconds: None,
                    download_bps: None,
                    upload_bps: None,
                })
            }
            CoreEvent::StateChanged { torrent_id, state } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                update_status(&mut store.torrents, torrent_id, format_state(&state));
                SseApplyOutcome::Applied
            }
            CoreEvent::Completed { torrent_id, .. } => {
                if !torrent_exists(&store.torrents, torrent_id) {
                    return SseApplyOutcome::Refresh;
                }
                update_status(&mut store.torrents, torrent_id, "completed".to_string());
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
                update_metadata(&mut store.torrents, torrent_id, name, download_dir);
                SseApplyOutcome::Applied
            }
            CoreEvent::TorrentRemoved { torrent_id } => {
                remove_row(&mut store.torrents, torrent_id);
                SseApplyOutcome::Applied
            }
            CoreEvent::TorrentAdded { .. }
            | CoreEvent::FilesDiscovered { .. }
            | CoreEvent::FsopsStarted { .. }
            | CoreEvent::FsopsProgress { .. }
            | CoreEvent::FsopsCompleted { .. }
            | CoreEvent::FsopsFailed { .. }
            | CoreEvent::SelectionReconciled { .. }
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
fn progress_ratio(downloaded: u64, total: u64) -> f32 {
    if total == 0 {
        0.0
    } else {
        let scaled = downloaded.saturating_mul(10_000) / total;
        let scaled = u16::try_from(scaled).unwrap_or(u16::MAX);
        f32::from(scaled) / 10_000.0
    }
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
