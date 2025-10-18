use std::collections::HashMap;
use std::ffi::OsStr;
use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use revaer_torrent_core::{FilePriorityOverride, FileSelectionRules};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

const META_SUFFIX: &str = ".meta.json";
const FASTRESUME_SUFFIX: &str = ".fastresume";

/// Persisted metadata companion alongside libtorrent fastresume files.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct StoredTorrentMetadata {
    #[serde(default)]
    pub selection: FileSelectionRules,
    #[serde(default)]
    pub priorities: Vec<FilePriorityOverride>,
    #[serde(default)]
    pub download_dir: Option<String>,
    #[serde(default)]
    pub sequential: bool,
    #[serde(default)]
    pub updated_at: DateTime<Utc>,
}

/// Combined view of resume payload + metadata for a torrent.
#[derive(Debug, Clone, Default)]
pub struct StoredTorrentState {
    pub torrent_id: Uuid,
    pub fastresume: Option<Vec<u8>>,
    pub metadata: Option<StoredTorrentMetadata>,
}

impl StoredTorrentState {
    const fn new(torrent_id: Uuid) -> Self {
        Self {
            torrent_id,
            fastresume: None,
            metadata: None,
        }
    }
}

/// Service responsible for persisting fast-resume data and selection metadata.
#[derive(Clone, Debug)]
pub struct FastResumeStore {
    base_dir: PathBuf,
}

impl FastResumeStore {
    /// Construct a store rooted at the provided directory.
    #[must_use]
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    /// Ensure the underlying directory exists.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn ensure_initialized(&self) -> Result<()> {
        if !self.base_dir.exists() {
            fs::create_dir_all(&self.base_dir)
                .with_context(|| format!("failed to create resume_dir {}", self.display()))?;
        }
        Ok(())
    }

    /// Load all known torrents from disk.
    ///
    /// # Errors
    ///
    /// Returns an error if a fastresume payload or metadata file cannot be read or decoded.
    pub fn load_all(&self) -> Result<Vec<StoredTorrentState>> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut map: HashMap<Uuid, StoredTorrentState> = HashMap::new();
        for entry in fs::read_dir(&self.base_dir)
            .with_context(|| format!("failed to read resume_dir {}", self.display()))?
        {
            let entry = entry?;
            let path = entry.path();
            let Some(file_name) = path.file_name().and_then(OsStr::to_str) else {
                continue;
            };

            if let Some(id) = strip_suffix(file_name, FASTRESUME_SUFFIX) {
                let payload = fs::read(&path).with_context(|| {
                    format!(
                        "failed to read fastresume payload for torrent {id} at {}",
                        path.display()
                    )
                })?;
                map.entry(id)
                    .or_insert_with(|| StoredTorrentState::new(id))
                    .fastresume = Some(payload);
            } else if let Some(id) = strip_suffix(file_name, META_SUFFIX) {
                let data = fs::read_to_string(&path).with_context(|| {
                    format!(
                        "failed to read metadata for torrent {id} at {}",
                        path.display()
                    )
                })?;
                let metadata: StoredTorrentMetadata = serde_json::from_str(&data)
                    .with_context(|| format!("failed to parse metadata JSON for torrent {id}"))?;
                map.entry(id)
                    .or_insert_with(|| StoredTorrentState::new(id))
                    .metadata = Some(metadata);
            }
        }

        Ok(map.into_values().collect())
    }

    /// Persist the fastresume payload for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload cannot be written.
    pub fn write_fastresume(&self, torrent_id: Uuid, payload: &[u8]) -> Result<()> {
        self.ensure_initialized()?;
        fs::write(self.fastresume_path(&torrent_id), payload).with_context(|| {
            format!(
                "failed to persist fastresume for torrent {torrent_id} at {}",
                self.fastresume_path(&torrent_id).display()
            )
        })?;
        Ok(())
    }

    /// Persist the outbound metadata for a torrent (selection/priorities/etc.).
    ///
    /// # Errors
    ///
    /// Returns an error if the metadata cannot be encoded or written.
    pub fn write_metadata(&self, torrent_id: Uuid, metadata: &StoredTorrentMetadata) -> Result<()> {
        self.ensure_initialized()?;
        let mut metadata = metadata.clone();
        metadata.updated_at = Utc::now();
        let json = serde_json::to_string_pretty(&metadata)
            .with_context(|| format!("failed to encode metadata for torrent {torrent_id}"))?;
        fs::write(self.metadata_path(&torrent_id), json).with_context(|| {
            format!(
                "failed to persist metadata for torrent {torrent_id} at {}",
                self.metadata_path(&torrent_id).display()
            )
        })?;
        Ok(())
    }

    /// Remove persisted state for a torrent.
    ///
    /// # Errors
    ///
    /// Returns an error if the stored files cannot be deleted.
    pub fn remove(&self, torrent_id: Uuid) -> Result<()> {
        let fastresume_path = self.fastresume_path(&torrent_id);
        if fastresume_path.exists() {
            fs::remove_file(&fastresume_path).with_context(|| {
                format!(
                    "failed to remove fastresume for torrent {torrent_id} at {}",
                    fastresume_path.display()
                )
            })?;
        }

        let metadata_path = self.metadata_path(&torrent_id);
        if metadata_path.exists() {
            fs::remove_file(&metadata_path).with_context(|| {
                format!(
                    "failed to remove metadata for torrent {torrent_id} at {}",
                    metadata_path.display()
                )
            })?;
        }

        Ok(())
    }

    fn fastresume_path(&self, torrent_id: &Uuid) -> PathBuf {
        self.base_dir
            .join(format!("{torrent_id}{FASTRESUME_SUFFIX}"))
    }

    fn metadata_path(&self, torrent_id: &Uuid) -> PathBuf {
        self.base_dir.join(format!("{torrent_id}{META_SUFFIX}"))
    }

    fn display(&self) -> String {
        self.base_dir.display().to_string()
    }
}

fn strip_suffix(file_name: &str, suffix: &str) -> Option<Uuid> {
    file_name
        .strip_suffix(suffix)
        .and_then(|value| Uuid::parse_str(value).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn sample_metadata() -> StoredTorrentMetadata {
        StoredTorrentMetadata {
            selection: FileSelectionRules {
                include: vec!["**/*.mkv".into()],
                exclude: vec!["**/extras/**".into()],
                skip_fluff: true,
            },
            priorities: vec![FilePriorityOverride {
                index: 0,
                priority: revaer_torrent_core::FilePriority::High,
            }],
            download_dir: Some("/data/downloads".into()),
            sequential: true,
            updated_at: Utc::now(),
        }
    }

    #[test]
    fn strip_suffix_extracts_uuid() {
        let id = Uuid::new_v4();
        assert_eq!(
            strip_suffix(&format!("{id}{META_SUFFIX}"), META_SUFFIX),
            Some(id)
        );
        assert_eq!(
            strip_suffix(&format!("{id}{FASTRESUME_SUFFIX}"), FASTRESUME_SUFFIX),
            Some(id)
        );
        assert!(strip_suffix("invalid.txt", META_SUFFIX).is_none());
    }

    #[test]
    fn ensure_initialized_creates_directory() -> Result<()> {
        let temp = TempDir::new()?;
        let target = temp.path().join("resume");
        let store = FastResumeStore::new(&target);
        store.ensure_initialized()?;
        assert!(target.exists());
        Ok(())
    }

    #[test]
    fn write_and_load_round_trip() -> Result<()> {
        let temp = TempDir::new()?;
        let store = FastResumeStore::new(temp.path());
        let torrent_id = Uuid::new_v4();

        let metadata = sample_metadata();
        let resume_blob = vec![0_u8, 1, 2, 3];

        store.write_metadata(torrent_id, &metadata)?;
        store.write_fastresume(torrent_id, &resume_blob)?;

        let mut loaded = store.load_all()?;
        assert_eq!(loaded.len(), 1);
        let state = loaded.pop().expect("state missing");
        assert_eq!(state.torrent_id, torrent_id);
        assert_eq!(state.fastresume, Some(resume_blob));
        assert!(state.metadata.is_some());
        let stored_meta = state.metadata.unwrap();
        assert_eq!(stored_meta.selection.include.len(), 1);
        assert_eq!(stored_meta.priorities.len(), 1);
        assert!(stored_meta.sequential);
        assert!(stored_meta.updated_at <= Utc::now());

        store.remove(torrent_id)?;
        assert!(store.load_all()?.is_empty());

        Ok(())
    }
}
