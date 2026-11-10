//! Timing constants, named after the Swift code they come from (`BudsController.swift`).

use std::time::Duration;

/// Delays between connection retries; the last one repeats (`retryDelays`).
pub const RETRY_DELAYS: [Duration; 5] = [
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(3),
    Duration::from_secs(5),
    Duration::from_secs(8),
];
/// Periodic battery / noise refresh (`pollTimer`).
pub const POLL_INTERVAL: Duration = Duration::from_secs(30);
/// Device poll + safety net (upstream's `healthTimer` ran every 3 s; we also poll devices here).
pub const HEALTH_INTERVAL: Duration = Duration::from_secs(2);
/// Wait after the buds connect to this computer before opening RFCOMM: at 1.5 s the open hangs,
/// by ~3 s it succeeds in under 0.2 s (`deviceDidConnect`).
pub const CONNECT_AFTER_DEVICE_UP: Duration = Duration::from_secs(3);
/// A request with no reply for this long means the link is dead (`expectReply`).
pub const REPLY_WATCHDOG: Duration = Duration::from_secs(3);
/// Wait for the product ID before sending a mode requested while disconnected (`didOpen`).
pub const APPLY_QUEUED_MODE_DELAY: Duration = Duration::from_millis(1500);
/// The find-my-earbuds sound stops by itself after this long (`findBuds`).
pub const FIND_AUTO_STOP: Duration = Duration::from_secs(30);
/// At most one page (ask the buds to connect here) this often (`pageBuds`).
pub const PAGE_MIN_INTERVAL: Duration = Duration::from_secs(15);
/// A page that never reported back stops blocking new ones after this long.
pub const PAGE_INFLIGHT_EXPIRY: Duration = Duration::from_secs(30);
/// A gap this big between two health ticks means the computer slept.
pub const RESUME_GAP: Duration = Duration::from_secs(10);
/// After waking, wait this long before checking the connection.
pub const RESUME_DELAY: Duration = Duration::from_secs(2);
/// Spacing of the queries sent by "Scan 0x0101–0x0120".
pub const SCAN_STEP: Duration = Duration::from_millis(80);
