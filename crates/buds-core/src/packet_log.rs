//! Packet log for the Developer tab, plus an optional log file for diagnosing connection problems
//! after the fact. Kept apart from the controller state so logging doesn't redraw the flyout.

use std::collections::VecDeque;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use ts_rs::TS;

use crate::platform::{LogDirection, LogFn};

/// How many entries the in-memory ring keeps.
pub const LOG_RING: usize = 600;
/// A log file bigger than this is started fresh when opened.
pub const LOG_FILE_MAX: u64 = 2_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LogEntry {
    /// Milliseconds since the Unix epoch.
    #[ts(type = "number")]
    pub time_ms: u64,
    pub direction: LogDirection,
    pub text: String,
}

#[derive(Clone)]
pub struct PacketLog {
    inner: Arc<Mutex<Inner>>,
    tx: broadcast::Sender<LogEntry>,
}

struct Inner {
    entries: VecDeque<LogEntry>,
    paused: bool,
    file: Option<File>,
}

impl Default for PacketLog {
    fn default() -> Self {
        Self::new()
    }
}

impl PacketLog {
    /// In-memory only.
    pub fn new() -> PacketLog {
        let (tx, _) = broadcast::channel(1024);
        PacketLog {
            inner: Arc::new(Mutex::new(Inner {
                entries: VecDeque::new(),
                paused: false,
                file: None,
            })),
            tx,
        }
    }

    /// Also appends every line to `path` (truncated first if it has grown past [`LOG_FILE_MAX`]).
    pub fn with_file(path: &Path) -> PacketLog {
        let log = PacketLog::new();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let too_big = std::fs::metadata(path).is_ok_and(|m| m.len() > LOG_FILE_MAX);
        let mut options = OpenOptions::new();
        options.create(true);
        if too_big {
            options.write(true).truncate(true);
        } else {
            options.append(true);
        }
        log.inner.lock().unwrap().file = options.open(path).ok();
        log
    }

    pub fn append(&self, direction: LogDirection, text: &str) {
        let entry = LogEntry {
            time_ms: now_ms(),
            direction,
            text: text.to_string(),
        };
        let mut inner = self.inner.lock().unwrap();
        // The file always gets the line; Pause only freezes the on-screen list.
        if let Some(file) = inner.file.as_mut() {
            let arrow = match direction {
                LogDirection::Sent => "→ ",
                LogDirection::Received => "← ",
                LogDirection::Info => "",
            };
            let stamp = format_time(entry.time_ms, "%Y-%m-%d %H:%M:%S%.3f");
            let _ = writeln!(file, "{stamp} {arrow}{text}");
        }
        if inner.paused {
            return;
        }
        inner.entries.push_back(entry.clone());
        while inner.entries.len() > LOG_RING {
            inner.entries.pop_front();
        }
        drop(inner);
        let _ = self.tx.send(entry);
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.inner.lock().unwrap().entries.iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.inner.lock().unwrap().entries.clear();
    }

    pub fn paused(&self) -> bool {
        self.inner.lock().unwrap().paused
    }

    pub fn set_paused(&self, paused: bool) {
        self.inner.lock().unwrap().paused = paused;
    }

    /// Live feed of new entries (for the UI's batching task and the CLI echo).
    pub fn subscribe(&self) -> broadcast::Receiver<LogEntry> {
        self.tx.subscribe()
    }

    /// A closure for platform code to log through.
    pub fn log_fn(&self) -> LogFn {
        let log = self.clone();
        Arc::new(move |direction, text| log.append(direction, text))
    }

    /// The list as text for the Copy button: `HH:mm:ss.SSS → AA 0B ...`.
    pub fn export_text(&self) -> String {
        self.entries()
            .iter()
            .map(|e| {
                let tag = match e.direction {
                    LogDirection::Sent => "→",
                    LogDirection::Received => "←",
                    LogDirection::Info => "·",
                };
                format!(
                    "{} {tag} {}",
                    format_time(e.time_ms, "%H:%M:%S%.3f"),
                    e.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}

fn format_time(ms: u64, pattern: &str) -> String {
    DateTime::from_timestamp_millis(ms as i64)
        .map(|utc| utc.with_timezone(&Local).format(pattern).to_string())
        .unwrap_or_default()
}
