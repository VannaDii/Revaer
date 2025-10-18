use revaer_torrent_core::{AddTorrent, FileSelectionUpdate, RemoveTorrent, TorrentRateLimit};
use uuid::Uuid;

#[derive(Debug)]
pub enum EngineCommand {
    Add(AddTorrent),
    Remove {
        id: Uuid,
        options: RemoveTorrent,
    },
    Pause {
        id: Uuid,
    },
    Resume {
        id: Uuid,
    },
    SetSequential {
        id: Uuid,
        sequential: bool,
    },
    UpdateLimits {
        id: Option<Uuid>,
        limits: TorrentRateLimit,
    },
    UpdateSelection {
        id: Uuid,
        rules: FileSelectionUpdate,
    },
    Reannounce {
        id: Uuid,
    },
    Recheck {
        id: Uuid,
    },
}
