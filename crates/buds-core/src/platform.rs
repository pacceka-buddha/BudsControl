//! The seam between the platform-independent controller and each OS's Bluetooth stack.
//! Implementations live in `crates/buds-bt` (Windows, Linux, macOS) and in [`crate::fake`].

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use ts_rs::TS;

use crate::controller::Msg;

/// What a log line is: a frame we sent, a frame we received, or an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum LogDirection {
    Sent,
    Received,
    Info,
}

/// Sink for packet-log lines, handed to platform code so it can log connection steps.
pub type LogFn = Arc<dyn Fn(LogDirection, &str) + Send + Sync>;

/// A paired Bluetooth device as the OS reports it (any kind; the controller filters for buds).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BtDevice {
    pub address: [u8; 6],
    pub name: Option<String>,
    /// Currently connected to this computer (audio profiles up), not merely paired.
    pub connected: bool,
    /// The OS knows the device advertises the HeyMelody control service.
    pub has_control_service: bool,
}

impl BtDevice {
    /// `AA:BB:CC:DD:EE:FF`
    pub fn address_string(&self) -> String {
        format_address(&self.address)
    }
}

/// `AA:BB:CC:DD:EE:FF`
pub fn format_address(address: &[u8; 6]) -> String {
    address
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Name fragments of OnePlus, OPPO, realme (and DIZO) buds. A bare "buds" isn't enough, so other
/// makers' earbuds are left alone.
const BRAND_WORDS: [&str; 5] = ["oneplus", "oppo", "enco", "realme", "dizo"];

/// Whether a Bluetooth device name looks like OnePlus / OPPO / realme buds.
pub fn name_looks_like_buds(name: &str) -> bool {
    let name = name.to_lowercase();
    BRAND_WORDS.iter().any(|word| name.contains(word))
}

/// Opening the control channel failed (every candidate was tried). The text is for the log.
#[derive(Debug, Clone)]
pub struct OpenError(pub String);

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for OpenError {}

/// An open control channel to the buds.
pub trait Link: Send {
    /// RFCOMM channel number in use (shown in the Developer tab), if known.
    fn channel(&self) -> Option<u8>;
    /// Which candidate worked (stored in settings and tried first next time).
    fn connect_hint(&self) -> Option<String>;
    /// Writes all bytes. An error means the link is dead.
    fn write(&self, bytes: &[u8]) -> std::io::Result<()>;
    /// Closes the channel. Must be idempotent and must not block for long.
    fn close(&self);
}

/// Where a link delivers what it receives. Cheap to clone; give one to the reader thread.
#[derive(Clone)]
pub struct LinkSink {
    pub(crate) id: u64,
    pub(crate) tx: UnboundedSender<Msg>,
}

impl LinkSink {
    /// Bytes received from the buds (any chunking; the controller reassembles frames).
    pub fn data(&self, bytes: Vec<u8>) {
        let _ = self.tx.send(Msg::Data {
            link: self.id,
            bytes,
        });
    }

    /// The remote side closed or the socket failed.
    pub fn closed(&self) {
        let _ = self.tx.send(Msg::LinkClosed { link: self.id });
    }
}

/// One OS's Bluetooth backend.
pub trait Platform: Send + Sync + 'static {
    /// Paired devices (all kinds; the controller keeps the ones that look like buds). Must not
    /// start a radio inquiry; it runs every couple of seconds.
    fn paired_devices(&self) -> impl Future<Output = Vec<BtDevice>> + Send;

    /// Opens the control channel, trying this OS's candidate list. `hint` is the
    /// candidate that worked last time. The controller drops the returned future to cancel it, so
    /// implementations must release their socket when dropped.
    fn open(
        &self,
        device: &BtDevice,
        hint: Option<String>,
        sink: LinkSink,
        log: LogFn,
    ) -> impl Future<Output = Result<Box<dyn Link>, OpenError>> + Send;

    /// Asks buds that are connected to another device (the phone) to connect to this computer.
    fn page(
        &self,
        device: &BtDevice,
        log: LogFn,
    ) -> impl Future<Output = Result<(), String>> + Send;

    /// How long a connection attempt may sit in "Connecting…" before the controller starts over.
    fn connect_budget(&self) -> Duration;

    /// What to call this computer in the UI ("PC", "Mac", "computer").
    fn host_label(&self) -> &'static str;

    /// Wall-clock time; a big jump between two health ticks means the computer slept. Overridable
    /// so tests can simulate that.
    fn wall_time(&self) -> SystemTime {
        SystemTime::now()
    }
}
