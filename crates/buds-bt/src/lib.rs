//! Per-OS Bluetooth backends implementing `buds_core::platform::Platform`.
//!
//! Windows (Winsock RFCOMM), Linux (BlueZ via `bluer`) and macOS (IOBluetooth). The candidate ordering
//! (`plan`) is shared and has no OS dependency.

pub mod plan;

#[cfg(windows)]
mod win;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(target_os = "linux")]
pub use linux::{LinuxPlatform, sdp_report};

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::{MacPlatform, run_main_loop_until, sdp_report};

#[cfg(windows)]
pub use win::{WindowsPlatform, audio_reconnect, audio_report, sdp_report};

/// The Bluetooth backend for the OS this was built for.
#[cfg(windows)]
pub type NativePlatform = WindowsPlatform;

/// The Bluetooth backend for the OS this was built for.
#[cfg(target_os = "linux")]
pub type NativePlatform = LinuxPlatform;

/// The Bluetooth backend for the OS this was built for.
#[cfg(target_os = "macos")]
pub type NativePlatform = MacPlatform;

/// Whether this build has a native Bluetooth backend.
pub const HAS_NATIVE_BACKEND: bool = cfg!(any(windows, target_os = "linux", target_os = "macos"));
