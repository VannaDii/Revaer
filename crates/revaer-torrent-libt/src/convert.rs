//! Conversions between native libtorrent types and domain events.

use revaer_events::TorrentState as EventState;
use revaer_torrent_core::{EngineEvent, FilePriority, model::TrackerStatus};
use tracing::debug;
use uuid::Uuid;

use crate::ffi::ffi::{NativeEvent, NativeEventKind, NativeTorrentState, NativeTrackerStatus};

#[must_use]
pub(crate) fn map_native_event(id: Option<Uuid>, event: NativeEvent) -> Vec<EngineEvent> {
    match event.kind {
        NativeEventKind::FilesDiscovered => {
            let Some(torrent_id) = id else {
                debug!("dropped files event without torrent id");
                return Vec::new();
            };
            map_files_event(torrent_id, event.files)
        }
        NativeEventKind::Progress => {
            let Some(torrent_id) = id else {
                debug!("dropped progress event without torrent id");
                return Vec::new();
            };
            vec![map_progress_event(
                torrent_id,
                event.bytes_downloaded,
                event.bytes_total,
                event.download_bps,
                event.upload_bps,
                event.ratio,
            )]
        }
        NativeEventKind::StateChanged => {
            let Some(torrent_id) = id else {
                debug!("dropped state change without torrent id");
                return Vec::new();
            };
            vec![EngineEvent::StateChanged {
                torrent_id,
                state: map_state(event.state),
            }]
        }
        NativeEventKind::Completed => {
            let Some(torrent_id) = id else {
                debug!("dropped completion without torrent id");
                return Vec::new();
            };
            vec![EngineEvent::Completed {
                torrent_id,
                library_path: event.library_path,
            }]
        }
        NativeEventKind::MetadataUpdated => {
            let Some(torrent_id) = id else {
                debug!("dropped metadata update without torrent id");
                return Vec::new();
            };
            vec![EngineEvent::MetadataUpdated {
                torrent_id,
                name: (!event.name.is_empty()).then_some(event.name),
                download_dir: (!event.download_dir.is_empty()).then_some(event.download_dir),
            }]
        }
        NativeEventKind::ResumeData => {
            let Some(torrent_id) = id else {
                debug!("dropped resume data without torrent id");
                return Vec::new();
            };
            vec![EngineEvent::ResumeData {
                torrent_id,
                payload: event.resume_data,
            }]
        }
        NativeEventKind::Error => {
            let Some(torrent_id) = id else {
                debug!("dropped error without torrent id");
                return Vec::new();
            };
            vec![EngineEvent::Error {
                torrent_id,
                message: event.message,
            }]
        }
        NativeEventKind::TrackerUpdate => {
            let Some(torrent_id) = id else {
                debug!("dropped tracker update without torrent id");
                return Vec::new();
            };
            vec![map_tracker_update(torrent_id, event.tracker_statuses)]
        }
        NativeEventKind::SessionError => vec![EngineEvent::SessionError {
            component: (!event.component.is_empty()).then_some(event.component),
            message: event.message,
        }],
        other => {
            debug!(?other, torrent_id = ?id, "ignored unsupported libtorrent event");
            Vec::new()
        }
    }
}

fn map_files_event(id: Uuid, files: Vec<crate::ffi::ffi::NativeFile>) -> Vec<EngineEvent> {
    if files.is_empty() {
        return Vec::new();
    }
    let files = files
        .into_iter()
        .map(|file| revaer_torrent_core::TorrentFile {
            index: file.index,
            path: file.path,
            size_bytes: file.size_bytes,
            bytes_completed: 0,
            priority: FilePriority::Normal,
            selected: true,
        })
        .collect();
    vec![EngineEvent::FilesDiscovered {
        torrent_id: id,
        files,
    }]
}

fn map_progress_event(
    id: Uuid,
    bytes_downloaded: u64,
    bytes_total: u64,
    download_bps: u64,
    upload_bps: u64,
    ratio: f64,
) -> EngineEvent {
    debug_assert!(
        !id.is_nil(),
        "progress events should always carry a valid torrent id"
    );
    let progress = revaer_torrent_core::TorrentProgress {
        bytes_downloaded,
        bytes_total,
        eta_seconds: None,
    };
    let rates = revaer_torrent_core::TorrentRates {
        download_bps,
        upload_bps,
        ratio,
    };
    EngineEvent::Progress {
        torrent_id: id,
        progress,
        rates,
    }
}

fn map_tracker_update(id: Uuid, tracker_statuses: Vec<NativeTrackerStatus>) -> EngineEvent {
    let trackers = tracker_statuses
        .into_iter()
        .map(|status| TrackerStatus {
            url: status.url,
            status: (!status.status.is_empty()).then_some(status.status),
            message: (!status.message.is_empty()).then_some(status.message),
        })
        .collect();
    EngineEvent::TrackerStatus {
        torrent_id: id,
        trackers,
    }
}

#[must_use]
pub(crate) fn map_state(state: NativeTorrentState) -> EventState {
    match state {
        NativeTorrentState::Queued => EventState::Queued,
        NativeTorrentState::FetchingMetadata => EventState::FetchingMetadata,
        NativeTorrentState::Downloading => EventState::Downloading,
        NativeTorrentState::Seeding => EventState::Seeding,
        NativeTorrentState::Completed => EventState::Completed,
        NativeTorrentState::Failed => EventState::Failed {
            message: "engine reported failure".to_string(),
        },
        NativeTorrentState::Stopped => EventState::Stopped,
        other => {
            debug!(
                ?other,
                "unknown native torrent state reported by libtorrent"
            );
            EventState::Stopped
        }
    }
}

#[must_use]
pub(crate) const fn map_priority(priority: FilePriority) -> u8 {
    match priority {
        FilePriority::Skip => 0,
        FilePriority::Low => 1,
        FilePriority::Normal => 4,
        FilePriority::High => 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ffi::ffi::{NativeEvent, NativeEventKind, NativeTorrentState, NativeTrackerStatus};

    #[test]
    fn session_errors_without_ids_are_emitted() {
        let native = NativeEvent {
            id: String::new(),
            kind: NativeEventKind::SessionError,
            state: NativeTorrentState::Failed,
            name: String::new(),
            download_dir: String::new(),
            library_path: String::new(),
            bytes_downloaded: 0,
            bytes_total: 0,
            download_bps: 0,
            upload_bps: 0,
            ratio: 0.0,
            files: Vec::new(),
            resume_data: Vec::new(),
            message: "listen failed".to_string(),
            tracker_statuses: Vec::new(),
            component: "network".to_string(),
        };

        let events = map_native_event(None, native);
        assert_eq!(events.len(), 1);
        match &events[0] {
            EngineEvent::SessionError {
                component, message, ..
            } => {
                assert_eq!(component.as_deref(), Some("network"));
                assert_eq!(message, "listen failed");
            }
            other => panic!("unexpected event {other:?}"),
        }
    }

    #[test]
    fn tracker_update_is_mapped() {
        let native = NativeEvent {
            id: uuid::Uuid::nil().to_string(),
            kind: NativeEventKind::TrackerUpdate,
            state: NativeTorrentState::Downloading,
            name: String::new(),
            download_dir: String::new(),
            library_path: String::new(),
            bytes_downloaded: 0,
            bytes_total: 0,
            download_bps: 0,
            upload_bps: 0,
            ratio: 0.0,
            files: Vec::new(),
            resume_data: Vec::new(),
            message: String::new(),
            tracker_statuses: vec![NativeTrackerStatus {
                url: "https://tracker.example/announce".to_string(),
                status: "error".to_string(),
                message: "failed to resolve".to_string(),
            }],
            component: String::new(),
        };

        let events = map_native_event(Some(uuid::Uuid::nil()), native);
        assert_eq!(events.len(), 1);
        match &events[0] {
            EngineEvent::TrackerStatus { trackers, .. } => {
                assert_eq!(trackers.len(), 1);
                assert_eq!(trackers[0].url, "https://tracker.example/announce");
                assert_eq!(trackers[0].status.as_deref(), Some("error"));
                assert_eq!(trackers[0].message.as_deref(), Some("failed to resolve"));
            }
            other => panic!("unexpected event: {other:?}"),
        }
    }
}
