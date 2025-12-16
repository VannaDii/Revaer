//! Conversions between native libtorrent types and domain events.

use revaer_events::TorrentState as EventState;
use revaer_torrent_core::{EngineEvent, FilePriority, model::TrackerStatus};
use tracing::debug;
use uuid::Uuid;

use crate::ffi::ffi::{NativeEvent, NativeEventKind, NativeTorrentState};

#[must_use]
pub(crate) fn map_native_event(id: Uuid, event: NativeEvent) -> Vec<EngineEvent> {
    let NativeEvent {
        kind,
        state,
        name,
        download_dir,
        library_path,
        bytes_downloaded,
        bytes_total,
        download_bps,
        upload_bps,
        ratio,
        files,
        resume_data,
        message,
        tracker_statuses,
        ..
    } = event;

    match kind {
        NativeEventKind::FilesDiscovered => {
            if files.is_empty() {
                Vec::new()
            } else {
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
        }
        NativeEventKind::Progress => {
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
            vec![EngineEvent::Progress {
                torrent_id: id,
                progress,
                rates,
            }]
        }
        NativeEventKind::StateChanged => vec![EngineEvent::StateChanged {
            torrent_id: id,
            state: map_state(state),
        }],
        NativeEventKind::Completed => vec![EngineEvent::Completed {
            torrent_id: id,
            library_path,
        }],
        NativeEventKind::MetadataUpdated => {
            let name = (!name.is_empty()).then_some(name);
            let download_dir = (!download_dir.is_empty()).then_some(download_dir);
            vec![EngineEvent::MetadataUpdated {
                torrent_id: id,
                name,
                download_dir,
            }]
        }
        NativeEventKind::ResumeData => vec![EngineEvent::ResumeData {
            torrent_id: id,
            payload: resume_data,
        }],
        NativeEventKind::Error => vec![EngineEvent::Error {
            torrent_id: id,
            message,
        }],
        NativeEventKind::TrackerUpdate => {
            let trackers = tracker_statuses
                .into_iter()
                .map(|status| TrackerStatus {
                    url: status.url,
                    status: (!status.status.is_empty()).then_some(status.status),
                    message: (!status.message.is_empty()).then_some(status.message),
                })
                .collect();
            vec![EngineEvent::TrackerStatus {
                torrent_id: id,
                trackers,
            }]
        }
        other => {
            debug!(?other, torrent_id = %id, "ignored unsupported libtorrent event");
            Vec::new()
        }
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
        };

        let events = map_native_event(uuid::Uuid::nil(), native);
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
