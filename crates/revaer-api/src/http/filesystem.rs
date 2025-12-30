//! Filesystem inspection endpoints.
//!
//! # Design
//! - Provide a read-only directory browser for remote settings forms.
//! - Validate path existence on the server before returning listings.
//! - Keep responses small and deterministic (sorted entries).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    Json,
    extract::{Query, State},
};
use serde::Deserialize;
use tokio::fs;

use crate::app::state::ApiState;
use crate::http::errors::ApiError;
use crate::models::{FsBrowseResponse, FsEntry, FsEntryKind};

#[derive(Debug, Deserialize)]
pub(crate) struct FsBrowseQuery {
    pub(crate) path: Option<String>,
}

pub(crate) async fn browse_filesystem(
    State(_state): State<Arc<ApiState>>,
    Query(query): Query<FsBrowseQuery>,
) -> Result<Json<FsBrowseResponse>, ApiError> {
    let path_string = query.path.unwrap_or_else(|| "/".to_string());
    let path = PathBuf::from(&path_string);

    let metadata = fs::metadata(&path).await.map_err(|err| {
        ApiError::bad_request("filesystem path lookup failed")
            .with_context_field("path", path_string.clone())
            .with_context_field("error", err.to_string())
    })?;
    if !metadata.is_dir() {
        return Err(ApiError::bad_request("filesystem path is not a directory")
            .with_context_field("path", path_string));
    }

    let mut entries = list_directory(&path).await?;
    entries.sort_by(|a, b| {
        let kind_order = kind_rank(&a.kind).cmp(&kind_rank(&b.kind));
        kind_order.then_with(|| a.name.cmp(&b.name))
    });

    let parent = path.parent().map(path_to_string);

    Ok(Json(FsBrowseResponse {
        path: path_to_string(&path),
        parent,
        entries,
    }))
}

async fn list_directory(path: &Path) -> Result<Vec<FsEntry>, ApiError> {
    let mut reader = fs::read_dir(path).await.map_err(|err| {
        ApiError::bad_request("filesystem directory read failed")
            .with_context_field("path", path_to_string(path))
            .with_context_field("error", err.to_string())
    })?;
    let mut entries = Vec::new();

    while let Some(entry) = reader.next_entry().await.map_err(|err| {
        ApiError::bad_request("filesystem directory entry read failed")
            .with_context_field("path", path_to_string(path))
            .with_context_field("error", err.to_string())
    })? {
        let file_type = entry.file_type().await.map_err(|err| {
            ApiError::bad_request("filesystem entry type lookup failed")
                .with_context_field("path", path_to_string(&entry.path()))
                .with_context_field("error", err.to_string())
        })?;
        let kind = if file_type.is_dir() {
            FsEntryKind::Directory
        } else if file_type.is_file() {
            FsEntryKind::File
        } else if file_type.is_symlink() {
            FsEntryKind::Symlink
        } else {
            FsEntryKind::Other
        };

        let name = entry.file_name().to_string_lossy().to_string();
        let entry_path = path_to_string(&entry.path());
        entries.push(FsEntry {
            name,
            path: entry_path,
            kind,
        });
    }

    Ok(entries)
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

const fn kind_rank(kind: &FsEntryKind) -> u8 {
    match kind {
        FsEntryKind::Directory => 0,
        FsEntryKind::File => 1,
        FsEntryKind::Symlink => 2,
        FsEntryKind::Other => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigFacade;
    use anyhow::{Result, anyhow};
    use async_trait::async_trait;
    use axum::http::StatusCode;
    use revaer_config::{
        ApiKeyAuth, AppMode, AppProfile, AppliedChanges, ConfigError, ConfigResult, ConfigSnapshot,
        SettingsChangeset, SetupToken, TelemetryConfig,
    };
    use revaer_events::EventBus;
    use revaer_telemetry::Metrics;
    use serde_json::json;
    use std::sync::Arc;
    use std::time::Duration;
    use uuid::Uuid;

    #[derive(Clone)]
    struct StubConfig;

    #[async_trait]
    impl ConfigFacade for StubConfig {
        async fn get_app_profile(&self) -> ConfigResult<AppProfile> {
            let bind_addr = std::net::IpAddr::from([127, 0, 0, 1]);
            Ok(AppProfile {
                id: Uuid::new_v4(),
                instance_name: "test".to_string(),
                mode: AppMode::Active,
                auth_mode: revaer_config::AppAuthMode::ApiKey,
                version: 1,
                http_port: 8080,
                bind_addr,
                telemetry: TelemetryConfig::default(),
                label_policies: Vec::new(),
                immutable_keys: Vec::new(),
            })
        }

        async fn issue_setup_token(&self, _: Duration, _: &str) -> ConfigResult<SetupToken> {
            Err(ConfigError::Io {
                operation: "filesystem.issue_setup_token",
                source: std::io::Error::other("stubbed config failure"),
            })
        }

        async fn validate_setup_token(&self, _: &str) -> ConfigResult<()> {
            Err(ConfigError::Io {
                operation: "filesystem.validate_setup_token",
                source: std::io::Error::other("stubbed config failure"),
            })
        }

        async fn consume_setup_token(&self, _: &str) -> ConfigResult<()> {
            Err(ConfigError::Io {
                operation: "filesystem.consume_setup_token",
                source: std::io::Error::other("stubbed config failure"),
            })
        }

        async fn apply_changeset(
            &self,
            _: &str,
            _: &str,
            _: SettingsChangeset,
        ) -> ConfigResult<AppliedChanges> {
            Err(ConfigError::Io {
                operation: "filesystem.apply_changeset",
                source: std::io::Error::other("stubbed config failure"),
            })
        }

        async fn snapshot(&self) -> ConfigResult<ConfigSnapshot> {
            Err(ConfigError::Io {
                operation: "filesystem.snapshot",
                source: std::io::Error::other("stubbed config failure"),
            })
        }

        async fn authenticate_api_key(&self, _: &str, _: &str) -> ConfigResult<Option<ApiKeyAuth>> {
            Ok(None)
        }

        async fn has_api_keys(&self) -> ConfigResult<bool> {
            Ok(false)
        }

        async fn factory_reset(&self) -> ConfigResult<()> {
            Err(ConfigError::Io {
                operation: "filesystem.factory_reset",
                source: std::io::Error::other("stubbed config failure"),
            })
        }
    }

    fn test_state() -> Result<Arc<ApiState>> {
        Ok(Arc::new(ApiState::new(
            Arc::new(StubConfig),
            Metrics::new()?,
            Arc::new(json!({})),
            EventBus::with_capacity(4),
            None,
        )))
    }

    #[tokio::test]
    async fn browse_filesystem_lists_directory_entries() -> Result<()> {
        let root = std::env::temp_dir().join(format!("revaer-fs-{}", Uuid::new_v4()));
        let dir_path = root.join("child");
        let file_path = root.join("file.txt");
        std::fs::create_dir_all(&dir_path)?;
        std::fs::write(&file_path, "data")?;

        let query = FsBrowseQuery {
            path: Some(root.to_string_lossy().to_string()),
        };
        let response = browse_filesystem(State(test_state()?), Query(query)).await?;
        let Json(payload) = response;
        assert_eq!(payload.entries.len(), 2);
        assert_eq!(payload.entries[0].kind, FsEntryKind::Directory);
        assert_eq!(payload.entries[1].kind, FsEntryKind::File);
        assert_eq!(payload.path, root.to_string_lossy().to_string());
        assert_eq!(
            payload.parent,
            root.parent().map(|path| path.to_string_lossy().to_string())
        );

        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[tokio::test]
    async fn browse_filesystem_rejects_file_path() -> Result<()> {
        let root = std::env::temp_dir().join(format!("revaer-fs-file-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root)?;
        let file_path = root.join("file.txt");
        std::fs::write(&file_path, "data")?;

        let query = FsBrowseQuery {
            path: Some(file_path.to_string_lossy().to_string()),
        };
        let err = browse_filesystem(State(test_state()?), Query(query))
            .await
            .err()
            .ok_or_else(|| anyhow!("expected file path rejection"))?;
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
        assert_eq!(err.detail(), Some("filesystem path is not a directory"));

        std::fs::remove_dir_all(&root)?;
        Ok(())
    }

    #[tokio::test]
    async fn browse_filesystem_rejects_missing_path() -> Result<()> {
        let root = std::env::temp_dir().join(format!("revaer-fs-missing-{}", Uuid::new_v4()));
        let missing = root.join("missing");

        let query = FsBrowseQuery {
            path: Some(missing.to_string_lossy().to_string()),
        };
        let err = browse_filesystem(State(test_state()?), Query(query))
            .await
            .err()
            .ok_or_else(|| anyhow!("expected missing path rejection"))?;
        assert_eq!(err.status(), StatusCode::BAD_REQUEST);
        assert_eq!(err.detail(), Some("filesystem path lookup failed"));
        Ok(())
    }
}
