//! Where the app keeps its files. Shared by the tray app and `budsctl` so both see the same
//! settings and write the same log.

use std::path::PathBuf;

const APP_DIR: &str = "BudsControl";

/// Settings file: `%APPDATA%\BudsControl\settings.json` on Windows, `~/.config/BudsControl/` on
/// Linux, `~/Library/Application Support/BudsControl/` on macOS.
pub fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join(APP_DIR).join("settings.json"))
}

/// Log file: `%LOCALAPPDATA%\BudsControl\BudsControl.log` on Windows, `~/Library/Logs/` on macOS,
/// `~/.local/share/BudsControl/` on Linux.
pub fn log_path() -> Option<PathBuf> {
    if cfg!(target_os = "macos") {
        return dirs::home_dir().map(|h| h.join("Library").join("Logs").join("BudsControl.log"));
    }
    dirs::data_local_dir().map(|d| d.join(APP_DIR).join("BudsControl.log"))
}
