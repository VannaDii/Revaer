//! Engine and workflow traits implemented by torrent adapters.

use crate::model::TorrentStatus;
use crate::model::{
    AddTorrent, FileSelectionUpdate, PeerSnapshot, RemoveTorrent, TorrentRateLimit,
    TorrentTrackersUpdate, TorrentWebSeedsUpdate,
};
use anyhow::bail;
use async_trait::async_trait;
use uuid::Uuid;

/// Primary engine trait implemented by adapters (e.g. libtorrent).
#[async_trait]
pub trait TorrentEngine: Send + Sync {
    /// Admit a new torrent into the underlying engine.
    async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()>;

    /// Remove a torrent from the engine, optionally deleting data.
    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()>;

    /// Pause a torrent; default implementation reports lack of support.
    async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("pause operation not supported by this engine");
    }

    /// Resume a torrent; default implementation reports lack of support.
    async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("resume operation not supported by this engine");
    }

    /// Toggle sequential download mode; default implementation reports lack of support.
    async fn set_sequential(&self, id: Uuid, sequential: bool) -> anyhow::Result<()> {
        let _ = (id, sequential);
        bail!("sequential toggle not supported by this engine");
    }

    /// Update per-torrent or global rate limits.
    async fn update_limits(
        &self,
        id: Option<Uuid>,
        limits: TorrentRateLimit,
    ) -> anyhow::Result<()> {
        let _ = (id, limits);
        bail!("rate limit updates not supported by this engine");
    }

    /// Adjust the file selection for a torrent.
    /// Adjust the file selection; default implementation reports lack of support.
    async fn update_selection(&self, id: Uuid, rules: FileSelectionUpdate) -> anyhow::Result<()> {
        let _ = (id, rules);
        bail!("file selection updates not supported by this engine");
    }

    /// Update per-torrent options after admission.
    async fn update_options(
        &self,
        id: Uuid,
        options: crate::model::TorrentOptionsUpdate,
    ) -> anyhow::Result<()> {
        let _ = (id, options);
        bail!("option updates not supported by this engine");
    }

    /// Update tracker configuration for a torrent.
    async fn update_trackers(
        &self,
        id: Uuid,
        trackers: TorrentTrackersUpdate,
    ) -> anyhow::Result<()> {
        let _ = (id, trackers);
        bail!("tracker updates not supported by this engine");
    }

    /// Update web seeds associated with a torrent.
    async fn update_web_seeds(
        &self,
        id: Uuid,
        web_seeds: TorrentWebSeedsUpdate,
    ) -> anyhow::Result<()> {
        let _ = (id, web_seeds);
        bail!("web seed updates not supported by this engine");
    }

    /// Re-announce to trackers; default implementation reports lack of support.
    async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("reannounce not supported by this engine");
    }

    /// Move torrent storage to a new download directory.
    async fn move_torrent(&self, id: Uuid, download_dir: String) -> anyhow::Result<()> {
        let _ = (id, download_dir);
        bail!("move not supported by this engine");
    }

    /// Force a recheck of on-disk data; default implementation reports lack of support.
    async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("recheck not supported by this engine");
    }

    /// Retrieve connected peers for a torrent.
    async fn peers(&self, id: Uuid) -> anyhow::Result<Vec<PeerSnapshot>> {
        let _ = id;
        bail!("peer inspection not supported by this engine");
    }
}

/// Workflow façade exposed to the API layer for torrent lifecycle control.
#[async_trait]
pub trait TorrentWorkflow: Send + Sync {
    /// Admit a new torrent via the workflow façade.
    async fn add_torrent(&self, request: AddTorrent) -> anyhow::Result<()>;

    /// Remove a torrent via the workflow façade.
    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> anyhow::Result<()>;

    /// Pause a torrent; default implementation reports lack of support.
    async fn pause_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("pause operation not supported");
    }

    /// Resume a torrent; default implementation reports lack of support.
    async fn resume_torrent(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("resume operation not supported");
    }

    /// Toggle sequential download mode; default implementation reports lack of support.
    async fn set_sequential(&self, id: Uuid, sequential: bool) -> anyhow::Result<()> {
        let _ = (id, sequential);
        bail!("sequential toggle not supported");
    }

    /// Update per-torrent or global rate limits via the workflow façade.
    async fn update_limits(
        &self,
        id: Option<Uuid>,
        limits: TorrentRateLimit,
    ) -> anyhow::Result<()> {
        let _ = (id, limits);
        bail!("rate limit updates not supported");
    }

    /// Adjust the file selection via the workflow façade.
    /// Default implementation reports lack of support.
    async fn update_selection(&self, id: Uuid, rules: FileSelectionUpdate) -> anyhow::Result<()> {
        let _ = (id, rules);
        bail!("file selection updates not supported");
    }

    /// Update per-torrent options via the workflow façade.
    async fn update_options(
        &self,
        id: Uuid,
        options: crate::model::TorrentOptionsUpdate,
    ) -> anyhow::Result<()> {
        let _ = (id, options);
        bail!("option updates not supported");
    }

    /// Update tracker configuration for a torrent via the workflow façade.
    async fn update_trackers(
        &self,
        id: Uuid,
        trackers: TorrentTrackersUpdate,
    ) -> anyhow::Result<()> {
        let _ = (id, trackers);
        bail!("tracker updates not supported");
    }

    /// Update web seeds for a torrent via the workflow façade.
    async fn update_web_seeds(
        &self,
        id: Uuid,
        web_seeds: TorrentWebSeedsUpdate,
    ) -> anyhow::Result<()> {
        let _ = (id, web_seeds);
        bail!("web seed updates not supported");
    }

    /// Re-announce to trackers; default implementation reports lack of support.
    async fn reannounce(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("reannounce not supported");
    }

    /// Move torrent storage to a new download directory; default implementation reports lack of
    /// support.
    async fn move_torrent(&self, id: Uuid, download_dir: String) -> anyhow::Result<()> {
        let _ = (id, download_dir);
        bail!("move not supported");
    }

    /// Force a recheck of on-disk data; default implementation reports lack of support.
    async fn recheck(&self, id: Uuid) -> anyhow::Result<()> {
        let _ = id;
        bail!("recheck not supported");
    }
}

/// Inspector trait used by API consumers to fetch torrent snapshots.
#[async_trait]
pub trait TorrentInspector: Send + Sync {
    /// Retrieve the full torrent status list.
    async fn list(&self) -> anyhow::Result<Vec<TorrentStatus>>;

    /// Retrieve an individual torrent status snapshot.
    async fn get(&self, id: Uuid) -> anyhow::Result<Option<TorrentStatus>>;

    /// Retrieve connected peers for a torrent.
    async fn peers(&self, id: Uuid) -> anyhow::Result<Vec<PeerSnapshot>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct StubEngine;

    #[async_trait]
    impl TorrentEngine for StubEngine {
        async fn add_torrent(&self, _request: AddTorrent) -> anyhow::Result<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> anyhow::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn engine_default_methods_error() {
        let engine = StubEngine;
        let id = Uuid::new_v4();
        assert!(engine.pause_torrent(id).await.is_err());
        assert!(engine.resume_torrent(id).await.is_err());
        assert!(engine.reannounce(id).await.is_err());
        assert!(engine.recheck(id).await.is_err());
        assert!(engine.peers(id).await.is_err());
        assert!(
            engine
                .move_torrent(id, "/tmp/downloads".into())
                .await
                .is_err()
        );
        assert!(
            engine
                .set_sequential(id, true)
                .await
                .expect_err("sequential should error")
                .to_string()
                .contains("sequential")
        );
    }
}
