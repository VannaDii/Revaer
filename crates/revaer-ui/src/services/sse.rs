//! SSE parser helpers (transport-only).
//!
//! # Design
//! - Accept partial chunks and emit complete SSE frames when a blank line is received.
//! - Keep this module DOM-free so it can run in tests and non-wasm contexts.
//! - Decode JSON payloads into typed envelopes when possible, otherwise keep raw text.

use crate::core::events::{UiEvent, UiEventEnvelope};
use revaer_events::{Event as CoreEvent, TorrentState};
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

/// Parsed SSE frame with decoded metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SseFrame {
    /// Optional event name.
    pub event: Option<String>,
    /// Optional event id.
    pub id: Option<String>,
    /// Optional retry hint in milliseconds.
    pub retry: Option<u64>,
    /// Concatenated data payload.
    pub data: String,
}

impl SseFrame {
    fn is_empty(&self) -> bool {
        self.event.is_none() && self.id.is_none() && self.retry.is_none() && self.data.is_empty()
    }
}

/// Incremental SSE parser for streamed chunks.
#[derive(Default)]
pub(crate) struct SseParser {
    line: String,
    pending_cr: bool,
    builder: FrameBuilder,
}

impl SseParser {
    pub(crate) fn push(&mut self, chunk: &str) -> Vec<SseFrame> {
        let mut frames = Vec::new();
        for ch in chunk.chars() {
            if self.pending_cr {
                self.pending_cr = false;
                if ch == '\n' {
                    continue;
                }
            }
            match ch {
                '\n' => self.finish_line(&mut frames),
                '\r' => {
                    self.pending_cr = true;
                    self.finish_line(&mut frames);
                }
                _ => self.line.push(ch),
            }
        }
        frames
    }

    pub(crate) fn finish(&mut self) -> Option<SseFrame> {
        if !self.line.is_empty() {
            self.finish_line(&mut Vec::new());
        }
        self.builder.take_frame()
    }

    fn finish_line(&mut self, frames: &mut Vec<SseFrame>) {
        let line = self.line.clone();
        self.line.clear();
        if line.is_empty() {
            if let Some(frame) = self.builder.take_frame() {
                frames.push(frame);
            }
            return;
        }
        if line.starts_with(':') {
            return;
        }
        let (field, value) = line
            .split_once(':')
            .map(|(field, value)| (field, value.strip_prefix(' ').unwrap_or(value)))
            .unwrap_or((line.as_str(), ""));
        self.builder.apply_field(field, value);
    }
}

/// Legacy kind/data payload used by the dummy SSE stream.
#[derive(Clone, Debug, Deserialize, PartialEq)]
pub(crate) struct LegacyPayload {
    pub kind: String,
    pub data: Value,
}

/// Decode failures produced by SSE payload parsing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SseDecodeError {
    /// Optional event name for the frame.
    pub event: Option<String>,
    /// Optional event id for the frame.
    pub id: Option<String>,
    /// Raw payload data.
    pub data: String,
}

/// Decode an SSE frame into a typed envelope.
pub(crate) fn decode_frame(frame: &SseFrame) -> Result<UiEventEnvelope, SseDecodeError> {
    let data = frame.data.trim();
    if data.is_empty() {
        return Err(SseDecodeError {
            event: frame.event.clone(),
            id: frame.id.clone(),
            data: String::new(),
        });
    }
    if let Ok(envelope) = serde_json::from_str::<revaer_events::EventEnvelope>(data) {
        return Ok(UiEventEnvelope::from_core(envelope));
    }
    if let Ok(legacy) = serde_json::from_str::<LegacyPayload>(data) {
        if let Some(mapped) = map_legacy_payload(&legacy, frame.id.as_deref()) {
            return Ok(mapped);
        }
    }
    Err(SseDecodeError {
        event: frame.event.clone(),
        id: frame.id.clone(),
        data: frame.data.clone(),
    })
}

fn map_legacy_payload(payload: &LegacyPayload, frame_id: Option<&str>) -> Option<UiEventEnvelope> {
    let id = frame_id.and_then(|value| value.parse::<u64>().ok());
    let event = match payload.kind.as_str() {
        "system_rates" => {
            #[derive(Deserialize)]
            struct SystemRates {
                download_bps: u64,
                upload_bps: u64,
            }
            let rates: SystemRates = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::SystemRates {
                download_bps: rates.download_bps,
                upload_bps: rates.upload_bps,
            }
        }
        "torrent_added" => {
            #[derive(Deserialize)]
            struct Added {
                id: Uuid,
                name: String,
            }
            let added: Added = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::TorrentAdded {
                torrent_id: added.id,
                name: added.name,
            })
        }
        "files_discovered" => {
            #[derive(Deserialize)]
            struct Files {
                torrent_id: Uuid,
                files: Vec<revaer_events::DiscoveredFile>,
            }
            let files: Files = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::FilesDiscovered {
                torrent_id: files.torrent_id,
                files: files.files,
            })
        }
        "progress" => {
            #[derive(Deserialize)]
            struct ProgressData {
                id: Uuid,
                progress: ProgressMetrics,
                #[serde(default)]
                rates: ProgressRates,
            }
            #[derive(Deserialize)]
            struct ProgressMetrics {
                bytes_downloaded: u64,
                bytes_total: u64,
                #[serde(default)]
                eta_seconds: Option<u64>,
            }
            #[derive(Deserialize, Default)]
            struct ProgressRates {
                #[serde(default)]
                download_bps: u64,
                #[serde(default)]
                upload_bps: u64,
                #[serde(default)]
                ratio: f64,
            }
            let progress: ProgressData = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::Progress {
                torrent_id: progress.id,
                bytes_downloaded: progress.progress.bytes_downloaded,
                bytes_total: progress.progress.bytes_total,
                eta_seconds: progress.progress.eta_seconds,
                download_bps: progress.rates.download_bps,
                upload_bps: progress.rates.upload_bps,
                ratio: progress.rates.ratio,
            })
        }
        "state_changed" => {
            #[derive(Deserialize)]
            struct StateChanged {
                id: Uuid,
                state: Value,
            }
            let state: StateChanged = serde_json::from_value(payload.data.clone()).ok()?;
            let parsed = parse_state(&state.state)?;
            UiEvent::Core(CoreEvent::StateChanged {
                torrent_id: state.id,
                state: parsed,
            })
        }
        "completed" => {
            #[derive(Deserialize)]
            struct Completed {
                id: Uuid,
                library_path: String,
            }
            let completed: Completed = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::Completed {
                torrent_id: completed.id,
                library_path: completed.library_path,
            })
        }
        "torrent_removed" => {
            #[derive(Deserialize)]
            struct Removed {
                id: Uuid,
            }
            let removed: Removed = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::TorrentRemoved {
                torrent_id: removed.id,
            })
        }
        "fsops_started" => {
            #[derive(Deserialize)]
            struct FsopsStarted {
                torrent_id: Uuid,
            }
            let started: FsopsStarted = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::FsopsStarted {
                torrent_id: started.torrent_id,
            })
        }
        "fsops_progress" => {
            #[derive(Deserialize)]
            struct FsopsProgress {
                torrent_id: Uuid,
                status: String,
            }
            let progress: FsopsProgress = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::FsopsProgress {
                torrent_id: progress.torrent_id,
                step: progress.status,
            })
        }
        "fsops_completed" => {
            #[derive(Deserialize)]
            struct FsopsCompleted {
                torrent_id: Uuid,
            }
            let completed: FsopsCompleted = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::FsopsCompleted {
                torrent_id: completed.torrent_id,
            })
        }
        "fsops_failed" => {
            #[derive(Deserialize)]
            struct FsopsFailed {
                torrent_id: Uuid,
                message: String,
            }
            let failed: FsopsFailed = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::FsopsFailed {
                torrent_id: failed.torrent_id,
                message: failed.message,
            })
        }
        "metadata_updated" => {
            #[derive(Deserialize)]
            struct MetadataUpdated {
                torrent_id: Uuid,
                name: Option<String>,
                download_dir: Option<String>,
                comment: Option<String>,
                source: Option<String>,
                private: Option<bool>,
            }
            let metadata: MetadataUpdated = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::MetadataUpdated {
                torrent_id: metadata.torrent_id,
                name: metadata.name,
                download_dir: metadata.download_dir,
                comment: metadata.comment,
                source: metadata.source,
                private: metadata.private,
            })
        }
        "selection_reconciled" => {
            #[derive(Deserialize)]
            struct Selection {
                torrent_id: Uuid,
                reason: String,
            }
            let selection: Selection = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::SelectionReconciled {
                torrent_id: selection.torrent_id,
                reason: selection.reason,
            })
        }
        "settings_changed" => {
            #[derive(Deserialize)]
            struct SettingsChanged {
                description: String,
            }
            let settings: SettingsChanged = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::SettingsChanged {
                description: settings.description,
            })
        }
        "health_changed" => {
            #[derive(Deserialize)]
            struct HealthChanged {
                degraded: Vec<String>,
            }
            let health: HealthChanged = serde_json::from_value(payload.data.clone()).ok()?;
            UiEvent::Core(CoreEvent::HealthChanged {
                degraded: health.degraded,
            })
        }
        _ => return None,
    };
    Some(UiEventEnvelope::legacy(event, id))
}

fn parse_state(value: &Value) -> Option<TorrentState> {
    serde_json::from_value(value.clone()).ok()
}

#[derive(Default)]
struct FrameBuilder {
    event: Option<String>,
    id: Option<String>,
    retry: Option<u64>,
    data: String,
}

impl FrameBuilder {
    fn apply_field(&mut self, field: &str, value: &str) {
        match field {
            "event" => self.event = Some(value.to_string()),
            "id" => self.id = Some(value.to_string()),
            "retry" => self.retry = value.parse::<u64>().ok(),
            "data" => {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(value);
            }
            _ => {}
        }
    }

    fn take_frame(&mut self) -> Option<SseFrame> {
        let frame = SseFrame {
            event: self.event.take(),
            id: self.id.take(),
            retry: self.retry.take(),
            data: std::mem::take(&mut self.data),
        };
        if frame.is_empty() { None } else { Some(frame) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_emits_frames_on_blank_lines() {
        let mut parser = SseParser::default();
        let input = "event: test\ndata: hello\n\nid: 42\ndata: world\n\n";
        let frames = parser.push(input);
        assert_eq!(frames.len(), 2);
        assert_eq!(frames[0].event.as_deref(), Some("test"));
        assert_eq!(frames[0].data, "hello");
        assert_eq!(frames[1].id.as_deref(), Some("42"));
        assert_eq!(frames[1].data, "world");
    }

    #[test]
    fn parser_joins_multi_line_data() {
        let mut parser = SseParser::default();
        let input = "data: line1\ndata: line2\n\n";
        let frames = parser.push(input);
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data, "line1\nline2");
    }
}
