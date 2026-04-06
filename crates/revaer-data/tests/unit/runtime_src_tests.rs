use super::*;
use chrono::Utc;

#[test]
fn round_trip_state_serialisation() {
    let variants = [
        TorrentState::Queued,
        TorrentState::FetchingMetadata,
        TorrentState::Downloading,
        TorrentState::Seeding,
        TorrentState::Completed,
        TorrentState::Stopped,
        TorrentState::Failed {
            message: "failure".to_string(),
        },
    ];

    for state in variants {
        let (label, message) = serialize_state(&state);
        let restored = deserialize_state(label, message);
        match (&state, &restored) {
            (
                TorrentState::Failed { message: original },
                TorrentState::Failed { message: round },
            ) => assert_eq!(original, round),
            _ => assert_eq!(format!("{state:?}"), format!("{restored:?}")),
        }
    }
}

#[test]
fn clamp_handles_large_values() {
    assert_eq!(clamp_i64(42), 42);
    assert_eq!(clamp_i64(i64::MAX as u64), i64::MAX);
    assert_eq!(clamp_i64(u64::MAX), i64::MAX);
}

#[test]
fn file_priority_labels_round_trip() {
    use revaer_torrent_core::FilePriority;

    let priorities = [
        FilePriority::Skip,
        FilePriority::Low,
        FilePriority::Normal,
        FilePriority::High,
    ];
    for priority in priorities {
        let label = file_priority_label(priority);
        let parsed = parse_file_priority(label);
        assert_eq!(parsed, priority);
    }
    assert_eq!(parse_file_priority("unknown"), FilePriority::Normal);
}

#[test]
fn deserialize_state_defaults_failed_message_and_unknown_labels() {
    assert_eq!(
        deserialize_state("failed", None),
        TorrentState::Failed {
            message: "unknown failure".to_string(),
        }
    );
    assert_eq!(deserialize_state("mystery", None), TorrentState::Stopped);
}

#[test]
fn fs_job_state_from_row_copies_runtime_fields() {
    let updated_at = Utc::now();
    let state = FsJobState::from(FsJobStateRow {
        status: "moved".to_string(),
        attempt: 2,
        src_path: "src".to_string(),
        dst_path: Some("dst".to_string()),
        transfer_mode: Some("copy".to_string()),
        last_error: Some("boom".to_string()),
        updated_at,
    });

    assert_eq!(state.status, "moved");
    assert_eq!(state.attempt, 2);
    assert_eq!(state.src_path, "src");
    assert_eq!(state.dst_path.as_deref(), Some("dst"));
    assert_eq!(state.transfer_mode.as_deref(), Some("copy"));
    assert_eq!(state.last_error.as_deref(), Some("boom"));
    assert_eq!(state.updated_at, updated_at);
}
