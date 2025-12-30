//! Engine and workflow traits implemented by torrent adapters.

use crate::error::{TorrentError, TorrentResult};
use crate::model::TorrentStatus;
use crate::model::{
    AddTorrent, FileSelectionUpdate, PeerSnapshot, PieceDeadline, RemoveTorrent, TorrentRateLimit,
    TorrentTrackersUpdate, TorrentWebSeedsUpdate,
};
use async_trait::async_trait;
use uuid::Uuid;

/// Primary engine trait implemented by adapters (e.g. libtorrent).
#[async_trait]
pub trait TorrentEngine: Send + Sync {
    /// Admit a new torrent into the underlying engine.
    async fn add_torrent(&self, request: AddTorrent) -> TorrentResult<()>;

    /// Author a new `.torrent` metainfo payload.
    async fn create_torrent(
        &self,
        _request: crate::model::TorrentAuthorRequest,
    ) -> TorrentResult<crate::model::TorrentAuthorResult> {
        Err(TorrentError::Unsupported {
            operation: "create_torrent",
        })
    }

    /// Remove a torrent from the engine, optionally deleting data.
    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> TorrentResult<()>;

    /// Pause a torrent; default implementation reports lack of support.
    async fn pause_torrent(&self, _id: Uuid) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "pause_torrent",
        })
    }

    /// Resume a torrent; default implementation reports lack of support.
    async fn resume_torrent(&self, _id: Uuid) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "resume_torrent",
        })
    }

    /// Toggle sequential download mode; default implementation reports lack of support.
    async fn set_sequential(&self, _id: Uuid, _sequential: bool) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "set_sequential",
        })
    }

    /// Update per-torrent or global rate limits.
    async fn update_limits(
        &self,
        _id: Option<Uuid>,
        _limits: TorrentRateLimit,
    ) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "update_limits",
        })
    }

    /// Adjust the file selection for a torrent.
    /// Adjust the file selection; default implementation reports lack of support.
    async fn update_selection(&self, _id: Uuid, _rules: FileSelectionUpdate) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "update_selection",
        })
    }

    /// Update per-torrent options after admission.
    async fn update_options(
        &self,
        _id: Uuid,
        _options: crate::model::TorrentOptionsUpdate,
    ) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "update_options",
        })
    }

    /// Update tracker configuration for a torrent.
    async fn update_trackers(
        &self,
        _id: Uuid,
        _trackers: TorrentTrackersUpdate,
    ) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "update_trackers",
        })
    }

    /// Update web seeds associated with a torrent.
    async fn update_web_seeds(
        &self,
        _id: Uuid,
        _web_seeds: TorrentWebSeedsUpdate,
    ) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "update_web_seeds",
        })
    }

    /// Re-announce to trackers; default implementation reports lack of support.
    async fn reannounce(&self, _id: Uuid) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "reannounce",
        })
    }

    /// Move torrent storage to a new download directory.
    async fn move_torrent(&self, _id: Uuid, _download_dir: String) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "move_torrent",
        })
    }

    /// Force a recheck of on-disk data; default implementation reports lack of support.
    async fn recheck(&self, _id: Uuid) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "recheck",
        })
    }

    /// Retrieve connected peers for a torrent.
    async fn peers(&self, _id: Uuid) -> TorrentResult<Vec<PeerSnapshot>> {
        Err(TorrentError::Unsupported { operation: "peers" })
    }

    /// Set or clear a streaming deadline for a piece.
    async fn set_piece_deadline(&self, _id: Uuid, _deadline: PieceDeadline) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "set_piece_deadline",
        })
    }
}

/// Workflow façade exposed to the API layer for torrent lifecycle control.
#[async_trait]
pub trait TorrentWorkflow: Send + Sync {
    /// Admit a new torrent via the workflow façade.
    async fn add_torrent(&self, request: AddTorrent) -> TorrentResult<()>;

    /// Author a new `.torrent` metainfo payload.
    async fn create_torrent(
        &self,
        _request: crate::model::TorrentAuthorRequest,
    ) -> TorrentResult<crate::model::TorrentAuthorResult> {
        Err(TorrentError::Unsupported {
            operation: "create_torrent",
        })
    }

    /// Remove a torrent via the workflow façade.
    async fn remove_torrent(&self, id: Uuid, options: RemoveTorrent) -> TorrentResult<()>;

    /// Pause a torrent; default implementation reports lack of support.
    async fn pause_torrent(&self, _id: Uuid) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "pause_torrent",
        })
    }

    /// Resume a torrent; default implementation reports lack of support.
    async fn resume_torrent(&self, _id: Uuid) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "resume_torrent",
        })
    }

    /// Toggle sequential download mode; default implementation reports lack of support.
    async fn set_sequential(&self, _id: Uuid, _sequential: bool) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "set_sequential",
        })
    }

    /// Update per-torrent or global rate limits via the workflow façade.
    async fn update_limits(
        &self,
        _id: Option<Uuid>,
        _limits: TorrentRateLimit,
    ) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "update_limits",
        })
    }

    /// Adjust the file selection via the workflow façade.
    /// Default implementation reports lack of support.
    async fn update_selection(&self, _id: Uuid, _rules: FileSelectionUpdate) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "update_selection",
        })
    }

    /// Update per-torrent options via the workflow façade.
    async fn update_options(
        &self,
        _id: Uuid,
        _options: crate::model::TorrentOptionsUpdate,
    ) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "update_options",
        })
    }

    /// Update tracker configuration for a torrent via the workflow façade.
    async fn update_trackers(
        &self,
        _id: Uuid,
        _trackers: TorrentTrackersUpdate,
    ) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "update_trackers",
        })
    }

    /// Update web seeds for a torrent via the workflow façade.
    async fn update_web_seeds(
        &self,
        _id: Uuid,
        _web_seeds: TorrentWebSeedsUpdate,
    ) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "update_web_seeds",
        })
    }

    /// Re-announce to trackers; default implementation reports lack of support.
    async fn reannounce(&self, _id: Uuid) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "reannounce",
        })
    }

    /// Move torrent storage to a new download directory; default implementation reports lack of
    /// support.
    async fn move_torrent(&self, _id: Uuid, _download_dir: String) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "move_torrent",
        })
    }

    /// Force a recheck of on-disk data; default implementation reports lack of support.
    async fn recheck(&self, _id: Uuid) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "recheck",
        })
    }

    /// Set or clear a streaming deadline for a piece.
    async fn set_piece_deadline(&self, _id: Uuid, _deadline: PieceDeadline) -> TorrentResult<()> {
        Err(TorrentError::Unsupported {
            operation: "set_piece_deadline",
        })
    }
}

/// Inspector trait used by API consumers to fetch torrent snapshots.
#[async_trait]
pub trait TorrentInspector: Send + Sync {
    /// Retrieve the full torrent status list.
    async fn list(&self) -> TorrentResult<Vec<TorrentStatus>>;

    /// Retrieve an individual torrent status snapshot.
    async fn get(&self, id: Uuid) -> TorrentResult<Option<TorrentStatus>>;

    /// Retrieve connected peers for a torrent.
    async fn peers(&self, id: Uuid) -> TorrentResult<Vec<PeerSnapshot>>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;

    struct StubEngine;

    #[async_trait]
    impl TorrentEngine for StubEngine {
        async fn add_torrent(&self, _request: AddTorrent) -> TorrentResult<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> TorrentResult<()> {
            Ok(())
        }
    }

    struct StubWorkflow;

    #[async_trait]
    impl TorrentWorkflow for StubWorkflow {
        async fn add_torrent(&self, _request: AddTorrent) -> TorrentResult<()> {
            Ok(())
        }

        async fn remove_torrent(&self, _id: Uuid, _options: RemoveTorrent) -> TorrentResult<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn engine_default_methods_error() -> Result<()> {
        let engine = StubEngine;
        let id = Uuid::new_v4();
        assert!(engine.pause_torrent(id).await.is_err());
        assert!(engine.resume_torrent(id).await.is_err());
        assert!(
            engine
                .create_torrent(crate::model::TorrentAuthorRequest::default())
                .await
                .is_err()
        );
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
                .update_limits(Some(id), TorrentRateLimit::default())
                .await
                .is_err()
        );
        assert!(
            engine
                .update_selection(id, FileSelectionUpdate::default())
                .await
                .is_err()
        );
        assert!(
            engine
                .update_options(id, crate::model::TorrentOptionsUpdate::default())
                .await
                .is_err()
        );
        assert!(
            engine
                .update_trackers(id, TorrentTrackersUpdate::default())
                .await
                .is_err()
        );
        assert!(
            engine
                .update_web_seeds(id, TorrentWebSeedsUpdate::default())
                .await
                .is_err()
        );
        let err = engine
            .set_sequential(id, true)
            .await
            .err()
            .ok_or_else(|| anyhow!("expected sequential error"))?;
        assert!(matches!(
            err,
            TorrentError::Unsupported { operation } if operation == "set_sequential"
        ));
        assert!(
            engine
                .set_piece_deadline(
                    id,
                    PieceDeadline {
                        piece: 0,
                        deadline_ms: Some(1_000),
                    },
                )
                .await
                .is_err()
        );
        Ok(())
    }

    #[tokio::test]
    async fn workflow_default_methods_error() -> Result<()> {
        let workflow = StubWorkflow;
        let id = Uuid::new_v4();
        assert!(workflow.pause_torrent(id).await.is_err());
        assert!(workflow.resume_torrent(id).await.is_err());
        assert!(
            workflow
                .create_torrent(crate::model::TorrentAuthorRequest::default())
                .await
                .is_err()
        );
        assert!(workflow.reannounce(id).await.is_err());
        assert!(workflow.recheck(id).await.is_err());
        assert!(
            workflow
                .move_torrent(id, "/tmp/downloads".into())
                .await
                .is_err()
        );
        assert!(
            workflow
                .update_limits(Some(id), TorrentRateLimit::default())
                .await
                .is_err()
        );
        assert!(
            workflow
                .update_selection(id, FileSelectionUpdate::default())
                .await
                .is_err()
        );
        assert!(
            workflow
                .update_options(id, crate::model::TorrentOptionsUpdate::default())
                .await
                .is_err()
        );
        assert!(
            workflow
                .update_trackers(id, TorrentTrackersUpdate::default())
                .await
                .is_err()
        );
        assert!(
            workflow
                .update_web_seeds(id, TorrentWebSeedsUpdate::default())
                .await
                .is_err()
        );
        let err = workflow
            .set_sequential(id, true)
            .await
            .err()
            .ok_or_else(|| anyhow!("expected sequential error"))?;
        assert!(matches!(
            err,
            TorrentError::Unsupported { operation } if operation == "set_sequential"
        ));
        assert!(
            workflow
                .set_piece_deadline(
                    id,
                    PieceDeadline {
                        piece: 0,
                        deadline_ms: Some(1_000),
                    },
                )
                .await
                .is_err()
        );
        Ok(())
    }
}
