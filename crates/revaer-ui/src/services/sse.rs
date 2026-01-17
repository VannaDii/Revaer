//! SSE parser helpers (transport-only).
//!
//! # Design
//! - Accept partial chunks and emit complete SSE frames when a blank line is received.
//! - Keep this module DOM-free so it can run in tests and non-wasm contexts.
//! - Decode JSON payloads into typed envelopes when possible, otherwise keep raw text.

use crate::core::events::UiEventEnvelope;

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
    Err(SseDecodeError {
        event: frame.event.clone(),
        id: frame.id.clone(),
        data: frame.data.clone(),
    })
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
