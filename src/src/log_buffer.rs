use log::{Level, LevelFilter, Metadata, Record};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum number of log records kept in memory for the in-app log viewer.
const MAX_ENTRIES: usize = 1000;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    pub seq: u64,
    pub ts_ms: u64,
    pub level: String,
    pub target: String,
    pub message: String,
}

#[derive(Debug, Default)]
pub struct LogBuffer {
    entries: Mutex<VecDeque<LogEntry>>,
    next_seq: AtomicU64,
}

impl LogBuffer {
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(VecDeque::with_capacity(256)),
            next_seq: AtomicU64::new(1),
        }
    }

    pub fn push(&self, level: Level, target: &str, message: String) {
        let seq = self.next_seq.fetch_add(1, Ordering::SeqCst);
        let ts_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let mut entries = self.entries.lock();
        if entries.len() >= MAX_ENTRIES {
            entries.pop_front();
        }
        entries.push_back(LogEntry {
            seq,
            ts_ms,
            level: level.as_str().to_string(),
            target: target.to_string(),
            message,
        });
    }

    /// Returns all entries with `seq` greater than `since_seq` (or all when `None`).
    pub fn tail_since(&self, since_seq: Option<u64>) -> Vec<LogEntry> {
        let entries = self.entries.lock();
        match since_seq {
            Some(since) => entries.iter().filter(|e| e.seq > since).cloned().collect(),
            None => entries.iter().cloned().collect(),
        }
    }

    pub fn clear(&self) {
        self.entries.lock().clear();
    }

    pub fn len(&self) -> usize {
        self.entries.lock().len()
    }
}

struct FanoutLogger {
    buffer: Arc<LogBuffer>,
    level: LevelFilter,
}

impl log::Log for FanoutLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // Same concise shape as the previous env_logger setup so existing
        // terminal tooling keeps working (UTC clock, no date clutter).
        let day_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() % 86_400)
            .unwrap_or(0);
        eprintln!(
            "[{:02}:{:02}:{:02} {} {}] {}",
            day_secs / 3600,
            (day_secs % 3600) / 60,
            day_secs % 60,
            record.level(),
            record.target(),
            record.args()
        );
        self.buffer.push(
            record.level(),
            record.target(),
            format!("{}", record.args()),
        );
    }

    fn flush(&self) {}
}

fn level_from_env() -> LevelFilter {
    // Honor RUST_LOG (e.g. `RUST_LOG=audiover=debug,info`), defaulting to info.
    // Parses the first level-like token; full env_logger directive syntax is
    // intentionally not required here.
    if let Ok(spec) = std::env::var("RUST_LOG") {
        for token in spec.split([',', '=', ':']) {
            match token.trim().to_lowercase().as_str() {
                "off" => return LevelFilter::Off,
                "error" => return LevelFilter::Error,
                "warn" | "warning" => return LevelFilter::Warn,
                "info" => return LevelFilter::Info,
                "debug" => return LevelFilter::Debug,
                "trace" => return LevelFilter::Trace,
                _ => continue,
            }
        }
    }
    LevelFilter::Info
}

/// Installs the combined stderr + in-memory logger. Must be called before
/// any `log!` macro fires (first line of `main`).
pub fn install_logger(buffer: Arc<LogBuffer>) {
    let level = level_from_env();
    let logger = FanoutLogger { buffer, level };
    let _ = log::set_boxed_logger(Box::new(logger));
    log::set_max_level(level);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buffer_caps_and_filters_by_seq() {
        let buf = LogBuffer::new();
        buf.push(Level::Info, "t", "a".to_string());
        buf.push(Level::Error, "t", "b".to_string());
        let all = buf.tail_since(None);
        assert_eq!(all.len(), 2);
        let rest = buf.tail_since(Some(all[0].seq));
        assert_eq!(rest.len(), 1);
        assert_eq!(rest[0].message, "b");
        buf.clear();
        assert_eq!(buf.len(), 0);
    }
}
