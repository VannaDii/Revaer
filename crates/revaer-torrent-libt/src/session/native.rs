use anyhow::{Result, anyhow};
use async_trait::async_trait;
use cxx::UniquePtr;
use uuid::Uuid;

use crate::convert::{map_native_event, map_priority};
use crate::ffi::ffi;
use crate::types::EngineRuntimeConfig;
use ffi::SourceKind;
use revaer_torrent_core::{
    AddTorrent, EngineEvent, FileSelectionUpdate, RemoveTorrent, TorrentRateLimit, TorrentSource,
};

use super::LibtSession;

pub(super) struct NativeSession {
    inner: UniquePtr<ffi::Session>,
}

pub(super) fn create_session() -> Result<Box<dyn LibtSession>> {
    let options = base_options();
    let inner = initialise_session(&options)?;
    Ok(Box::new(NativeSession { inner }))
}

impl NativeSession {
    fn map_error(message: String) -> Result<()> {
        if message.is_empty() {
            Ok(())
        } else {
            Err(anyhow!(message))
        }
    }
}

const fn base_options() -> ffi::SessionOptions {
    ffi::SessionOptions {
        download_root: String::new(),
        resume_dir: String::new(),
        enable_dht: false,
        sequential_default: false,
    }
}

fn initialise_session(options: &ffi::SessionOptions) -> Result<UniquePtr<ffi::Session>> {
    let inner = ffi::new_session(options);
    if inner.is_null() {
        Err(anyhow!("failed to initialise libtorrent session"))
    } else {
        Ok(inner)
    }
}

#[cfg(all(test, feature = "libtorrent"))]
fn create_native_session_for_tests() -> Result<NativeSession> {
    let options = base_options();
    let inner = initialise_session(&options)?;
    Ok(NativeSession { inner })
}

#[async_trait]
impl LibtSession for NativeSession {
    async fn add_torrent(&mut self, request: &AddTorrent) -> Result<()> {
        let mut add_request = ffi::AddTorrentRequest {
            id: request.id.to_string(),
            source_kind: match request.source {
                TorrentSource::Magnet { .. } => SourceKind::Magnet,
                TorrentSource::Metainfo { .. } => SourceKind::Metainfo,
            },
            magnet_uri: String::new(),
            metainfo: Vec::new(),
            download_dir: request.options.download_dir.clone().unwrap_or_default(),
            has_download_dir: request.options.download_dir.is_some(),
            sequential: request.options.sequential.unwrap_or_default(),
            has_sequential_override: request.options.sequential.is_some(),
            tags: request.options.tags.clone(),
        };

        match &request.source {
            TorrentSource::Magnet { uri } => add_request.magnet_uri.clone_from(uri),
            TorrentSource::Metainfo { bytes } => add_request.metainfo.clone_from(bytes),
        }

        let session = self.inner.pin_mut();
        let result = session.add_torrent(&add_request);
        Self::map_error(result)
    }

    async fn remove_torrent(&mut self, id: Uuid, options: &RemoveTorrent) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.remove_torrent(&key, options.with_data);
        Self::map_error(result)
    }

    async fn pause_torrent(&mut self, id: Uuid) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.pause_torrent(&key);
        Self::map_error(result)
    }

    async fn resume_torrent(&mut self, id: Uuid) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.resume_torrent(&key);
        Self::map_error(result)
    }

    async fn set_sequential(&mut self, id: Uuid, sequential: bool) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.set_sequential(&key, sequential);
        Self::map_error(result)
    }

    async fn load_fastresume(&mut self, id: Uuid, payload: &[u8]) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.load_fastresume(&key, payload);
        Self::map_error(result)
    }

    async fn update_limits(&mut self, id: Option<Uuid>, limits: &TorrentRateLimit) -> Result<()> {
        let request = ffi::LimitRequest {
            apply_globally: id.is_none(),
            id: id.map_or_else(String::new, |value| value.to_string()),
            download_bps: limits
                .download_bps
                .map_or(-1, |value| i64::try_from(value).unwrap_or(-1)),
            upload_bps: limits
                .upload_bps
                .map_or(-1, |value| i64::try_from(value).unwrap_or(-1)),
        };
        let session = self.inner.pin_mut();
        let result = session.update_limits(&request);
        Self::map_error(result)
    }

    async fn update_selection(&mut self, id: Uuid, rules: &FileSelectionUpdate) -> Result<()> {
        let priorities = rules
            .priorities
            .iter()
            .map(|override_rule| ffi::FilePriorityOverride {
                index: override_rule.index,
                priority: map_priority(override_rule.priority),
            })
            .collect::<Vec<_>>();

        let request = ffi::SelectionRules {
            id: id.to_string(),
            include: rules.include.clone(),
            exclude: rules.exclude.clone(),
            priorities,
            skip_fluff: rules.skip_fluff,
        };
        let session = self.inner.pin_mut();
        let result = session.update_selection(&request);
        Self::map_error(result)
    }

    async fn reannounce(&mut self, id: Uuid) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.reannounce(&key);
        Self::map_error(result)
    }

    async fn recheck(&mut self, id: Uuid) -> Result<()> {
        let key = id.to_string();
        let session = self.inner.pin_mut();
        let result = session.recheck(&key);
        Self::map_error(result)
    }

    async fn apply_config(&mut self, config: &EngineRuntimeConfig) -> Result<()> {
        let options = ffi::EngineOptions {
            listen_port: config.listen_port.unwrap_or_default(),
            set_listen_port: config.listen_port.is_some(),
            enable_dht: config.enable_dht,
            max_active: config.max_active.unwrap_or(-1),
            download_rate_limit: config.download_rate_limit.unwrap_or(-1),
            upload_rate_limit: config.upload_rate_limit.unwrap_or(-1),
            sequential_default: config.sequential_default,
            encryption_policy: config.encryption.as_u8(),
            download_root: config.download_root.clone(),
            resume_dir: config.resume_dir.clone(),
        };
        let session = self.inner.pin_mut();
        let result = session.apply_engine_profile(&options);
        Self::map_error(result)
    }

    async fn poll_events(&mut self) -> Result<Vec<EngineEvent>> {
        let session = self.inner.pin_mut();
        let raw_events = session.poll_events();
        let mut events = Vec::with_capacity(raw_events.len());

        for native in raw_events {
            let Ok(torrent_id) = Uuid::parse_str(&native.id) else {
                continue;
            };
            events.extend(map_native_event(torrent_id, native));
        }

        Ok(events)
    }
}

#[cfg(all(test, feature = "libtorrent"))]
mod tests {
    use super::*;
    use crate::ffi::ffi::{NativeEvent, NativeEventKind, NativeTorrentState};
    use crate::types::{EncryptionPolicy, EngineRuntimeConfig};
    use revaer_torrent_core::{AddTorrent, AddTorrentOptions, EngineEvent, TorrentSource};
    use tempfile::TempDir;
    use uuid::Uuid;

    #[tokio::test]
    async fn native_session_accepts_configuration_and_add() -> Result<()> {
        let download = TempDir::new()?;
        let resume_dir = TempDir::new()?;

        let mut session = create_native_session_for_tests()?;
        let config = EngineRuntimeConfig {
            download_root: download.path().to_string_lossy().into_owned(),
            resume_dir: resume_dir.path().to_string_lossy().into_owned(),
            enable_dht: false,
            sequential_default: false,
            listen_port: None,
            max_active: None,
            download_rate_limit: None,
            upload_rate_limit: None,
            encryption: EncryptionPolicy::Prefer,
        };

        session.apply_config(&config).await?;

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet(
                "magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567",
            ),
            options: AddTorrentOptions::default(),
        };

        session.add_torrent(&descriptor).await?;
        // Polling immediately should succeed even if no events are queued yet.
        let _ = session.poll_events().await?;
        Ok(())
    }

    #[tokio::test]
    async fn native_session_applies_rate_limits() -> Result<()> {
        let download = TempDir::new()?;
        let resume_dir = TempDir::new()?;

        let mut session = create_native_session_for_tests()?;
        let config = EngineRuntimeConfig {
            download_root: download.path().to_string_lossy().into_owned(),
            resume_dir: resume_dir.path().to_string_lossy().into_owned(),
            enable_dht: false,
            sequential_default: false,
            listen_port: Some(68_81),
            max_active: Some(2),
            download_rate_limit: Some(256_000),
            upload_rate_limit: Some(128_000),
            encryption: EncryptionPolicy::Prefer,
        };

        session.apply_config(&config).await?;

        let descriptor = AddTorrent {
            id: Uuid::new_v4(),
            source: TorrentSource::magnet(
                "magnet:?xt=urn:btih:fedcba98765432100123456789abcdef01234567",
            ),
            options: AddTorrentOptions::default(),
        };

        session.add_torrent(&descriptor).await?;

        session
            .update_limits(
                None,
                &TorrentRateLimit {
                    download_bps: Some(128_000),
                    upload_bps: Some(64_000),
                },
            )
            .await?;

        session
            .update_limits(
                Some(descriptor.id),
                &TorrentRateLimit {
                    download_bps: Some(64_000),
                    upload_bps: Some(32_000),
                },
            )
            .await?;

        Ok(())
    }

    #[test]
    fn native_event_translates_progress_and_resume_data() {
        let torrent_id = Uuid::new_v4();
        let events = map_native_event(
            torrent_id,
            NativeEvent {
                id: torrent_id.to_string(),
                kind: NativeEventKind::Progress,
                state: NativeTorrentState::Downloading,
                name: "demo".to_string(),
                download_dir: "/tmp/downloads".to_string(),
                library_path: String::new(),
                bytes_downloaded: 512,
                bytes_total: 1024,
                download_bps: 4096,
                upload_bps: 2048,
                ratio: 0.5,
                files: Vec::new(),
                resume_data: Vec::new(),
                message: String::new(),
            },
        );

        assert!(matches!(
            events.first(),
            Some(EngineEvent::Progress {
                progress,
                rates,
                torrent_id: id,
            }) if *id == torrent_id
                && progress.bytes_downloaded == 512
                && progress.bytes_total == 1024
                && rates.download_bps == 4096
                && rates.upload_bps == 2048
                && (rates.ratio - 0.5).abs() < f64::EPSILON
        ));

        let resume = map_native_event(
            torrent_id,
            NativeEvent {
                id: torrent_id.to_string(),
                kind: NativeEventKind::ResumeData,
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
                resume_data: vec![1, 2, 3, 4],
                message: String::new(),
            },
        );

        assert!(matches!(
            resume.first(),
            Some(EngineEvent::ResumeData {
                torrent_id: id,
                payload,
            }) if *id == torrent_id && payload == &vec![1, 2, 3, 4]
        ));
    }
}
