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
                comment: (!event.comment.is_empty()).then_some(event.comment),
                source: (!event.source.is_empty()).then_some(event.source),
                private: event.has_private.then_some(event.private_flag),
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

const fn map_progress_event(
    id: Uuid,
    bytes_downloaded: u64,
    bytes_total: u64,
    download_bps: u64,
    upload_bps: u64,
    ratio: f64,
) -> EngineEvent {
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
    use anyhow::{Result, anyhow};

    #[test]
    fn session_errors_without_ids_are_emitted() -> Result<()> {
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
            comment: String::new(),
            source: String::new(),
            private_flag: false,
            has_private: false,
        };

        let events = map_native_event(None, native);
        assert_eq!(events.len(), 1);
        match &events[0] {
            EngineEvent::SessionError {
                component, message, ..
            } => {
                assert_eq!(component.as_deref(), Some("network"));
                assert_eq!(message, "listen failed");
                Ok(())
            }
            _ => Err(anyhow!("unexpected event kind")),
        }
    }

    #[test]
    fn tracker_update_is_mapped() -> Result<()> {
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
            comment: String::new(),
            source: String::new(),
            private_flag: false,
            has_private: false,
        };

        let events = map_native_event(Some(uuid::Uuid::nil()), native);
        assert_eq!(events.len(), 1);
        match &events[0] {
            EngineEvent::TrackerStatus { trackers, .. } => {
                assert_eq!(trackers.len(), 1);
                assert_eq!(trackers[0].url, "https://tracker.example/announce");
                assert_eq!(trackers[0].status.as_deref(), Some("error"));
                assert_eq!(trackers[0].message.as_deref(), Some("failed to resolve"));
                Ok(())
            }
            _ => Err(anyhow!("unexpected event kind")),
        }
    }

    fn test_native_event(
        torrent_id: uuid::Uuid,
        kind: NativeEventKind,
        state: NativeTorrentState,
    ) -> NativeEvent {
        NativeEvent {
            id: torrent_id.to_string(),
            kind,
            state,
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
            tracker_statuses: Vec::new(),
            component: String::new(),
            comment: String::new(),
            source: String::new(),
            private_flag: false,
            has_private: false,
        }
    }

    #[test]
    fn files_discovered_event_is_mapped() {
        let torrent_id = uuid::Uuid::new_v4();
        let mut native = test_native_event(
            torrent_id,
            NativeEventKind::FilesDiscovered,
            NativeTorrentState::Downloading,
        );
        native.files = vec![crate::ffi::ffi::NativeFile {
            index: 7,
            path: "season/episode.mkv".to_string(),
            size_bytes: 42,
        }];

        let files = map_native_event(Some(torrent_id), native);
        assert!(matches!(
            files.first(),
            Some(EngineEvent::FilesDiscovered { torrent_id: id, files })
                if *id == torrent_id
                    && files.len() == 1
                    && files[0].index == 7
                    && files[0].path == "season/episode.mkv"
                    && files[0].size_bytes == 42
                    && files[0].selected
                    && files[0].priority == FilePriority::Normal
        ));
    }

    #[test]
    fn progress_event_is_mapped() {
        let torrent_id = uuid::Uuid::new_v4();
        let mut native = test_native_event(
            torrent_id,
            NativeEventKind::Progress,
            NativeTorrentState::Downloading,
        );
        native.bytes_downloaded = 128;
        native.bytes_total = 512;
        native.download_bps = 4096;
        native.upload_bps = 2048;
        native.ratio = 1.5;

        let progress = map_native_event(Some(torrent_id), native);
        assert!(matches!(
            progress.first(),
            Some(EngineEvent::Progress { torrent_id: id, progress, rates })
                if *id == torrent_id
                    && progress.bytes_downloaded == 128
                    && progress.bytes_total == 512
                    && rates.download_bps == 4096
                    && rates.upload_bps == 2048
                    && (rates.ratio - 1.5).abs() < f64::EPSILON
        ));
    }

    #[test]
    fn state_changed_event_is_mapped() {
        let torrent_id = uuid::Uuid::new_v4();
        let state = map_native_event(
            Some(torrent_id),
            test_native_event(
                torrent_id,
                NativeEventKind::StateChanged,
                NativeTorrentState::Completed,
            ),
        );
        assert!(matches!(
            state.first(),
            Some(EngineEvent::StateChanged { torrent_id: id, state: EventState::Completed })
                if *id == torrent_id
        ));
    }

    #[test]
    fn completed_event_is_mapped() {
        let torrent_id = uuid::Uuid::new_v4();
        let mut native = test_native_event(
            torrent_id,
            NativeEventKind::Completed,
            NativeTorrentState::Completed,
        );
        native.library_path = "/library/demo".to_string();

        let completed = map_native_event(Some(torrent_id), native);
        assert!(matches!(
            completed.first(),
            Some(EngineEvent::Completed { torrent_id: id, library_path }) if *id == torrent_id && library_path == "/library/demo"
        ));
    }

    #[test]
    fn metadata_updated_event_is_mapped() {
        let torrent_id = uuid::Uuid::new_v4();
        let mut native = test_native_event(
            torrent_id,
            NativeEventKind::MetadataUpdated,
            NativeTorrentState::Downloading,
        );
        native.name = "demo".to_string();
        native.download_dir = "/downloads".to_string();
        native.comment = "comment".to_string();
        native.source = "source".to_string();
        native.private_flag = true;
        native.has_private = true;

        let metadata = map_native_event(Some(torrent_id), native);
        assert!(matches!(
            metadata.first(),
            Some(EngineEvent::MetadataUpdated {
                torrent_id: id,
                name,
                download_dir,
                comment,
                source,
                private,
            }) if *id == torrent_id
                && name.as_deref() == Some("demo")
                && download_dir.as_deref() == Some("/downloads")
                && comment.as_deref() == Some("comment")
                && source.as_deref() == Some("source")
                && *private == Some(true)
        ));
    }

    #[test]
    fn resume_data_event_is_mapped() {
        let torrent_id = uuid::Uuid::new_v4();
        let mut native = test_native_event(
            torrent_id,
            NativeEventKind::ResumeData,
            NativeTorrentState::Downloading,
        );
        native.resume_data = vec![1, 2, 3];

        let resume = map_native_event(Some(torrent_id), native);
        assert!(matches!(
            resume.first(),
            Some(EngineEvent::ResumeData { torrent_id: id, payload }) if *id == torrent_id && payload == &vec![1, 2, 3]
        ));
    }

    #[test]
    fn error_event_is_mapped() {
        let torrent_id = uuid::Uuid::new_v4();
        let mut native = test_native_event(
            torrent_id,
            NativeEventKind::Error,
            NativeTorrentState::Failed,
        );
        native.message = "disk error".to_string();

        let error = map_native_event(Some(torrent_id), native);
        assert!(matches!(
            error.first(),
            Some(EngineEvent::Error { torrent_id: id, message }) if *id == torrent_id && message == "disk error"
        ));
    }

    #[test]
    fn helpers_drop_missing_ids_and_cover_state_priority_variants() {
        let unsupported = map_native_event(
            None,
            NativeEvent {
                id: String::new(),
                kind: NativeEventKind::FilesDiscovered,
                state: NativeTorrentState::Downloading,
                name: String::new(),
                download_dir: String::new(),
                library_path: String::new(),
                bytes_downloaded: 0,
                bytes_total: 0,
                download_bps: 0,
                upload_bps: 0,
                ratio: 0.0,
                files: vec![crate::ffi::ffi::NativeFile {
                    index: 0,
                    path: "ignored".to_string(),
                    size_bytes: 1,
                }],
                resume_data: Vec::new(),
                message: String::new(),
                tracker_statuses: Vec::new(),
                component: String::new(),
                comment: String::new(),
                source: String::new(),
                private_flag: false,
                has_private: false,
            },
        );
        assert!(unsupported.is_empty());
        assert!(map_files_event(uuid::Uuid::nil(), Vec::new()).is_empty());

        assert!(matches!(
            map_state(NativeTorrentState::Queued),
            EventState::Queued
        ));
        assert!(matches!(
            map_state(NativeTorrentState::FetchingMetadata),
            EventState::FetchingMetadata
        ));
        assert!(matches!(
            map_state(NativeTorrentState::Downloading),
            EventState::Downloading
        ));
        assert!(matches!(
            map_state(NativeTorrentState::Seeding),
            EventState::Seeding
        ));
        assert!(matches!(
            map_state(NativeTorrentState::Completed),
            EventState::Completed
        ));
        assert!(matches!(
            map_state(NativeTorrentState::Failed),
            EventState::Failed { .. }
        ));
        assert!(matches!(
            map_state(NativeTorrentState::Stopped),
            EventState::Stopped
        ));

        assert_eq!(map_priority(FilePriority::Skip), 0);
        assert_eq!(map_priority(FilePriority::Low), 1);
        assert_eq!(map_priority(FilePriority::Normal), 4);
        assert_eq!(map_priority(FilePriority::High), 7);

        let tracker_event = map_tracker_update(
            uuid::Uuid::nil(),
            vec![NativeTrackerStatus {
                url: "https://tracker.example/announce".to_string(),
                status: String::new(),
                message: String::new(),
            }],
        );
        assert!(matches!(
            tracker_event,
            EngineEvent::TrackerStatus { trackers, .. }
                if trackers.len() == 1
                    && trackers[0].status.is_none()
                    && trackers[0].message.is_none()
        ));
    }
}
