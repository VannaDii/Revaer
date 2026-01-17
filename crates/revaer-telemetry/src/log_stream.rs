//! Log stream broadcaster for SSE consumers.
//!
//! # Design
//! - Reuse the formatted log output to avoid duplicating formatting logic.
//! - Broadcast newline-delimited log lines via a bounded channel.
//! - Retain a rolling buffer so new SSE subscribers can see recent lines.
//! - Keep the writer lightweight and non-blocking for the hot logging path.

use std::collections::VecDeque;
use std::io::{self, Write};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use tokio::sync::broadcast;
use tracing_subscriber::fmt::MakeWriter;

const LOG_STREAM_CAPACITY: usize = 1024;
const LOG_STREAM_RETENTION: Duration = Duration::from_secs(120);

static LOG_STREAM: OnceLock<broadcast::Sender<String>> = OnceLock::new();
static LOG_STREAM_BUFFER: OnceLock<Mutex<VecDeque<LogEntry>>> = OnceLock::new();

/// Subscribe to the log stream as newline-delimited messages.
#[must_use]
pub fn log_stream_receiver() -> broadcast::Receiver<String> {
    log_stream_sender().subscribe()
}

/// Snapshot recent log lines retained in memory.
#[must_use]
pub fn log_stream_snapshot() -> Vec<String> {
    let mut buffer = match log_stream_buffer().lock() {
        Ok(guard) => guard,
        Err(err) => err.into_inner(),
    };
    let now = Instant::now();
    prune_log_buffer(&mut buffer, now);
    buffer.iter().map(|entry| entry.line.clone()).collect()
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

fn log_stream_buffer() -> &'static Mutex<VecDeque<LogEntry>> {
    LOG_STREAM_BUFFER.get_or_init(|| Mutex::new(VecDeque::new()))
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

#[derive(Clone)]
struct LogEntry {
    at: Instant,
    line: String,
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
        if lines.is_empty() {
            return;
        }
        record_lines(&lines);
        if self.sender.receiver_count() == 0 {
            return;
        }
        for line in lines {
            if line.is_empty() {
                continue;
            }
            if self.sender.send(line).is_err() {
                break;
            }
        }
    }
}

fn record_lines(lines: &[String]) {
    let Ok(mut buffer) = log_stream_buffer().try_lock() else {
        return;
    };
    let now = Instant::now();
    for line in lines {
        if line.is_empty() {
            continue;
        }
        buffer.push_back(LogEntry {
            at: now,
            line: line.clone(),
        });
    }
    prune_log_buffer(&mut buffer, now);
}

fn prune_log_buffer(buffer: &mut VecDeque<LogEntry>, now: Instant) {
    let cutoff = now.checked_sub(LOG_STREAM_RETENTION).unwrap_or(now);
    while matches!(buffer.front(), Some(entry) if entry.at < cutoff) {
        buffer.pop_front();
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
            self.emit_lines(vec![line]);
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
    use super::{
        LineBuffer, LogEntry, log_stream_buffer, log_stream_receiver, log_stream_snapshot,
        log_stream_writer, prune_log_buffer,
    };
    use std::io::Write;
    use std::time::{Duration, Instant};
    use tracing_subscriber::fmt::MakeWriter;

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

    #[test]
    fn log_stream_writer_emits_lines_and_flushes_on_drop() -> std::io::Result<()> {
        if let Ok(mut buffer) = log_stream_buffer().lock() {
            buffer.clear();
        }
        let mut log_receiver = log_stream_receiver();
        let make_writer = log_stream_writer();
        {
            let mut writer = make_writer.make_writer();
            writer.write_all(b"alpha\nbeta\npartial")?;
        }

        let mut lines = Vec::new();
        for _ in 0..5 {
            if let Ok(line) = log_receiver.try_recv() {
                lines.push(line);
            }
        }

        assert!(lines.contains(&"alpha".to_string()));
        assert!(lines.contains(&"beta".to_string()));
        assert!(lines.contains(&"partial".to_string()));
        Ok(())
    }

    #[test]
    fn log_stream_snapshot_returns_recent_lines() {
        if let Ok(mut buffer) = log_stream_buffer().lock() {
            buffer.clear();
        }
        let now = Instant::now();
        if let Ok(mut buffer) = log_stream_buffer().lock() {
            buffer.push_back(LogEntry {
                at: now,
                line: "recent".to_string(),
            });
        }
        assert_eq!(log_stream_snapshot(), vec!["recent".to_string()]);
    }

    #[test]
    fn prune_log_buffer_discards_old_entries() {
        let now = Instant::now();
        let old = now.checked_sub(Duration::from_secs(300)).unwrap_or(now);
        let mut buffer = std::collections::VecDeque::from([
            LogEntry {
                at: old,
                line: "old".to_string(),
            },
            LogEntry {
                at: now,
                line: "fresh".to_string(),
            },
        ]);
        prune_log_buffer(&mut buffer, now);
        assert_eq!(buffer.len(), 1);
        assert_eq!(
            buffer.front().map(|entry| entry.line.as_str()),
            Some("fresh")
        );
    }
}
