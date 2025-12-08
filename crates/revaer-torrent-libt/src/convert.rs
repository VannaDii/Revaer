//! Conversions between native libtorrent types and domain events.

use revaer_events::TorrentState as EventState;
use revaer_torrent_core::{EngineEvent, FilePriority};
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
