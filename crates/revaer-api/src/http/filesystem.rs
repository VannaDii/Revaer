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

    let metadata = fs::metadata(&path)
        .await
        .map_err(|err| ApiError::bad_request(format!("{path_string}: {err}")))?;
    if !metadata.is_dir() {
        return Err(ApiError::bad_request(format!(
            "{path_string}: not a directory"
        )));
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
    let mut reader = fs::read_dir(path)
        .await
        .map_err(|err| ApiError::bad_request(format!("{}: {err}", path.display())))?;
    let mut entries = Vec::new();

    while let Some(entry) = reader
        .next_entry()
        .await
        .map_err(|err| ApiError::bad_request(format!("{}: {err}", path.display())))?
    {
        let file_type = entry
            .file_type()
            .await
            .map_err(|err| ApiError::bad_request(format!("{}: {err}", path.display())))?;
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
