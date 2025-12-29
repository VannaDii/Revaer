//! Log stream broadcaster for SSE consumers.
//!
//! # Design
//! - Reuse the formatted log output to avoid duplicating formatting logic.
//! - Broadcast newline-delimited log lines via a bounded channel.
//! - Keep the writer lightweight and non-blocking for the hot logging path.

use std::io::{self, Write};
use std::sync::OnceLock;

use tokio::sync::broadcast;
use tracing_subscriber::fmt::MakeWriter;

const LOG_STREAM_CAPACITY: usize = 1024;

static LOG_STREAM: OnceLock<broadcast::Sender<String>> = OnceLock::new();

/// Subscribe to the log stream as newline-delimited messages.
#[must_use]
pub fn log_stream_receiver() -> broadcast::Receiver<String> {
    log_stream_sender().subscribe()
}

pub(crate) fn log_stream_writer() -> LogStreamMakeWriter {
    LogStreamMakeWriter {
        sender: log_stream_sender(),
    }
}

fn log_stream_sender() -> broadcast::Sender<String> {
    LOG_STREAM
        .get_or_init(|| broadcast::channel(LOG_STREAM_CAPACITY).0)
        .clone()
}

/// `tracing_subscriber` writer that mirrors output and broadcasts log lines.
#[derive(Clone)]
pub(crate) struct LogStreamMakeWriter {
    sender: broadcast::Sender<String>,
}

impl<'a> MakeWriter<'a> for LogStreamMakeWriter {
    type Writer = LogStreamWriter;

    fn make_writer(&'a self) -> Self::Writer {
        LogStreamWriter::new(self.sender.clone())
    }
}

pub(crate) struct LogStreamWriter {
    sender: broadcast::Sender<String>,
    stdout: io::Stdout,
    buffer: LineBuffer,
}

impl LogStreamWriter {
    fn new(sender: broadcast::Sender<String>) -> Self {
        Self {
            sender,
            stdout: io::stdout(),
            buffer: LineBuffer::default(),
        }
    }

    fn emit_lines(&self, lines: Vec<String>) {
        for line in lines {
            if line.is_empty() {
                continue;
            }
            let _ = self.sender.send(line);
        }
    }
}

impl Write for LogStreamWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.stdout.write_all(buf)?;
        let lines = self.buffer.push(buf);
        self.emit_lines(lines);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.stdout.flush()
    }
}

impl Drop for LogStreamWriter {
    fn drop(&mut self) {
        if let Some(line) = self.buffer.finish() {
            let _ = self.sender.send(line);
        }
    }
}

#[derive(Default)]
struct LineBuffer {
    buffer: Vec<u8>,
}

impl LineBuffer {
    fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);
        self.drain_complete_lines()
    }

    fn finish(&mut self) -> Option<String> {
        if self.buffer.is_empty() {
            return None;
        }
        let line = String::from_utf8_lossy(&self.buffer).to_string();
        self.buffer.clear();
        Some(trim_line(&line))
    }

    fn drain_complete_lines(&mut self) -> Vec<String> {
        let mut lines = Vec::new();
        let mut start = 0usize;
        let mut idx = 0usize;
        while idx < self.buffer.len() {
            if self.buffer[idx] == b'\n' {
                let slice = &self.buffer[start..idx];
                let line = String::from_utf8_lossy(slice).to_string();
                lines.push(trim_line(&line));
                start = idx.saturating_add(1);
            }
            idx = idx.saturating_add(1);
        }
        if start > 0 {
            self.buffer.drain(0..start);
        }
        lines
    }
}

fn trim_line(line: &str) -> String {
    line.trim_end_matches(['\r', '\n']).to_string()
}

#[cfg(test)]
mod tests {
    use super::LineBuffer;

    #[test]
    fn line_buffer_splits_on_newlines() {
        let mut buffer = LineBuffer::default();
        let lines = buffer.push(b"alpha\nbeta\n");
        assert_eq!(lines, vec!["alpha".to_string(), "beta".to_string()]);
        assert!(buffer.finish().is_none());
    }

    #[test]
    fn line_buffer_keeps_partial_line() {
        let mut buffer = LineBuffer::default();
        let lines = buffer.push(b"alpha");
        assert!(lines.is_empty());
        assert_eq!(buffer.finish(), Some("alpha".to_string()));
    }

    #[test]
    fn line_buffer_trims_crlf() {
        let mut buffer = LineBuffer::default();
        let lines = buffer.push(b"alpha\r\n");
        assert_eq!(lines, vec!["alpha".to_string()]);
    }
}
