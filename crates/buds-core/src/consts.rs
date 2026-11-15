//! Timing constants for the controller.

use std::time::Duration;

/// Delays between connection retries; the last one repeats.
pub const RETRY_DELAYS: [Duration; 5] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(3),
    Duration::from_secs(5),
    Duration::from_secs(8),
];
/// Periodic battery / noise refresh.
pub const POLL_INTERVAL: Duration = Duration::from_secs(30);
/// Device poll and safety net (the original app's health check ran every 3 s; this one also polls the device list).
pub const HEALTH_INTERVAL: Duration = Duration::from_secs(2);
/// Wait after the buds connect to this computer before opening RFCOMM: at 1.5 s the open hangs,
/// by ~3 s it succeeds in under 0.2 s.
pub const CONNECT_AFTER_DEVICE_UP: Duration = Duration::from_secs(3);
/// A request with no reply for this long means the link is dead.
pub const REPLY_WATCHDOG: Duration = Duration::from_secs(3);
/// An EQ change the buds haven't confirmed after this long is re-read and the buds' answer taken.
pub const EQ_CONFIRM_TIMEOUT: Duration = Duration::from_millis(1200);
/// After the buds acknowledge an EQ change, read it back once after this long if still unconfirmed.
pub const EQ_RECHECK_DELAY: Duration = Duration::from_millis(500);
/// Control-channel opens that fail this many times in a row start backing off.
pub const OPEN_BACKOFF_AFTER: u32 = 2;
/// First back-off wait; it doubles with each further failure.
pub const OPEN_BACKOFF_BASE: Duration = Duration::from_secs(3);
/// Longest wait between opens while backing off.
pub const OPEN_BACKOFF_MAX: Duration = Duration::from_secs(300);
/// Wait for the product ID before sending a mode requested while disconnected.
pub const APPLY_QUEUED_MODE_DELAY: Duration = Duration::from_millis(1500);
/// The find-my-earbuds sound stops by itself after this long.
pub const FIND_AUTO_STOP: Duration = Duration::from_secs(30);
/// At most one page (ask the buds to connect here) this often.
pub const PAGE_MIN_INTERVAL: Duration = Duration::from_secs(15);
/// A page that never reported back stops blocking new ones after this long.
pub const PAGE_INFLIGHT_EXPIRY: Duration = Duration::from_secs(30);
/// A gap this big between two health ticks means the computer slept.
pub const RESUME_GAP: Duration = Duration::from_secs(10);
/// After waking, wait this long before checking the connection.
pub const RESUME_DELAY: Duration = Duration::from_secs(2);
/// Spacing of the queries sent by "Scan 0x0101–0x0120".
pub const SCAN_STEP: Duration = Duration::from_millis(80);
