//! Commands the web UI can call. Thin wrappers: the behaviour lives in `buds-core`.

use buds_core::controller::Snapshot;
use buds_core::packet_log::LogEntry;
use buds_core::protocol::parse_hex;
use buds_core::settings::Settings;
use buds_core::types::{AncLevel, EarSide, Gesture, GestureAction, NoiseMode};
use tauri::{AppHandle, State};

use crate::app_windows;
use crate::state::AppState;

#[tauri::command]
pub fn get_state(state: State<AppState>) -> Snapshot {
    state.handle.snapshot()
}

#[tauri::command]
pub fn get_settings(state: State<AppState>) -> Settings {
    state.handle.settings().get()
}

/// Applies a partial settings object (camelCase keys) to `current`. Keys the settings don't know are
/// ignored; a value of the wrong type is an error and changes nothing.
pub fn apply_patch(
    current: &Settings,
    patch: serde_json::Map<String, serde_json::Value>,
) -> Result<Settings, String> {
    let mut value = serde_json::to_value(current).map_err(|e| e.to_string())?;
    if let Some(map) = value.as_object_mut() {
        map.extend(patch);
    }
    serde_json::from_value(value).map_err(|e| e.to_string())
}

/// Merges `patch` (any subset of the settings, camelCase) into the settings and saves them.
#[tauri::command]
pub fn update_settings(state: State<AppState>, patch: serde_json::Value) -> Result<(), String> {
    let serde_json::Value::Object(changes) = patch else {
        return Err("patch must be an object".into());
    };
    let mut error = None;
    state
        .handle
        .settings()
        .update(|settings| match apply_patch(settings, changes) {
            Ok(updated) => *settings = updated,
            Err(e) => error = Some(e),
        });
    error.map_or(Ok(()), Err)
}

#[tauri::command]
pub fn set_mode(state: State<AppState>, mode: NoiseMode, level: Option<AncLevel>) {
    state.handle.set_mode(mode, level);
}

#[tauri::command]
pub fn cycle_mode(state: State<AppState>) {
    state.handle.cycle_mode();
}

#[tauri::command]
pub fn set_noise_bit(state: State<AppState>, bit: u8) {
    state.handle.set_noise_bit(bit);
}

#[tauri::command]
pub fn set_low_latency(state: State<AppState>, on: bool) {
    state.handle.set_low_latency(on);
}

#[tauri::command]
pub fn set_eq(state: State<AppState>, id: u8) {
    state.handle.set_eq(id);
}

#[tauri::command]
pub fn set_custom_eq(state: State<AppState>, gains: Vec<i32>) {
    state.handle.set_custom_eq(gains);
}

#[tauri::command]
pub fn find_buds(state: State<AppState>, start: bool) {
    state.handle.find_buds(start);
}

#[tauri::command]
pub fn set_gesture(state: State<AppState>, side: EarSide, gesture: Gesture, action: GestureAction) {
    state.handle.set_gesture(side, gesture, action);
}

#[tauri::command]
pub fn reconnect(state: State<AppState>) {
    state.handle.reconnect();
}

/// Developer: send a raw command; `payload_hex` is hex with any separators.
#[tauri::command]
pub fn send_raw(state: State<AppState>, command: u16, payload_hex: String) {
    state.handle.send_raw(command, parse_hex(&payload_hex));
}

#[tauri::command]
pub fn scan_queries(state: State<AppState>) {
    state.handle.scan_queries();
}

#[tauri::command]
pub fn get_log(state: State<AppState>) -> Vec<LogEntry> {
    state.handle.log().entries()
}

#[tauri::command]
pub fn clear_log(state: State<AppState>) {
    state.handle.log().clear();
}

#[tauri::command]
pub fn set_log_paused(state: State<AppState>, paused: bool) {
    state.handle.log().set_paused(paused);
}

#[tauri::command]
pub fn get_log_paused(state: State<AppState>) -> bool {
    state.handle.log().paused()
}

/// Whether the app starts when the user signs in (read from the system each time, not remembered).
#[tauri::command]
pub fn get_autostart(app: AppHandle) -> bool {
    use tauri_plugin_autostart::ManagerExt;
    app.autolaunch().is_enabled().unwrap_or(false)
}

/// Turns launch at login on or off and returns what the system now says.
#[tauri::command]
pub fn set_autostart(app: AppHandle, enabled: bool) -> Result<bool, String> {
    use tauri_plugin_autostart::ManagerExt;
    let launcher = app.autolaunch();
    if enabled {
        launcher.enable().map_err(|e| e.to_string())?;
    } else {
        launcher.disable().map_err(|e| e.to_string())?;
    }
    Ok(launcher.is_enabled().unwrap_or(enabled))
}

#[tauri::command]
pub fn app_version(app: AppHandle) -> String {
    app.package_info().version.to_string()
}

/// The shortcut recorder is listening (`true`) or finished (`false`).
#[tauri::command]
pub async fn set_recording(app: AppHandle, recording: bool) {
    crate::hotkeys::set_recording(&app, recording).await;
}

/// Whether an accelerator such as `Ctrl+Alt+KeyN` can be used as a shortcut.
#[tauri::command]
pub fn validate_shortcut(accelerator: String) -> bool {
    crate::hotkeys::is_valid(&accelerator)
}

/// Actions whose shortcut could not be registered.
#[tauri::command]
pub fn get_conflicts(app: AppHandle) -> Vec<String> {
    crate::hotkeys::conflicts(&app)
}

/// The HUD page finished fading out.
#[tauri::command]
pub fn hud_done(app: AppHandle) {
    crate::hud::hide(&app);
}

/// What the HUD should be showing (for a page that loaded after the event).
#[tauri::command]
pub fn get_hud_pending(app: AppHandle) -> Option<crate::hud::HudPayload> {
    crate::hud::pending(&app)
}

/// The log as text, for the Copy button.
#[tauri::command]
pub fn export_log(state: State<AppState>) -> String {
    state.handle.log().export_text()
}

/// Which OS the app runs on, so the pages can adapt (solid backgrounds on Linux, the Wayland hint).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PlatformInfo {
    /// `windows`, `linux` or `macos`.
    pub os: String,
    /// A Wayland session: no global shortcuts (the Shortcuts tab shows a hint instead).
    pub wayland: bool,
}

#[tauri::command]
pub fn get_platform() -> PlatformInfo {
    PlatformInfo {
        os: std::env::consts::OS.to_string(),
        wayland: app_windows::is_wayland(),
    }
}

#[tauri::command]
pub fn get_accent_color() -> Option<String> {
    crate::theme::accent_color()
}

/// The page reports its content height (logical pixels).
#[tauri::command]
pub fn flyout_resize(app: AppHandle, height: f64) {
    app_windows::resize_flyout(&app, height);
}

#[tauri::command]
pub fn flyout_hide(app: AppHandle) {
    app_windows::hide_flyout(&app, false);
}

/// Set while a native menu opened from the flyout is showing, so the flyout doesn't hide when the
/// menu takes the focus.
#[tauri::command]
pub fn set_menu_open(state: State<AppState>, open: bool) {
    state.flyout.lock().unwrap().menu_open = open;
}

#[tauri::command]
pub fn open_settings(app: AppHandle) {
    app_windows::open_settings(&app);
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    crate::quit(&app);
}

/// Whether the app runs against simulated buds.
#[tauri::command]
pub fn is_fake(state: State<AppState>) -> bool {
    state.fake
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn patch(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
        value.as_object().cloned().unwrap()
    }

    #[test]
    fn a_partial_patch_changes_only_what_it_names() {
        let before = Settings::default();
        let after = apply_patch(
            &before,
            patch(json!({ "rememberSettings": true, "gestureSide": "right" })),
        )
        .unwrap();
        assert!(after.remember_settings);
        assert_eq!(after.gesture_side, buds_core::types::EarSide::Right);
        assert_eq!(after.auto_connect, before.auto_connect);
        assert_eq!(after.show_hud, before.show_hud);
    }

    #[test]
    fn unknown_keys_are_ignored_and_wrong_types_rejected() {
        let before = Settings::default();
        assert_eq!(
            apply_patch(&before, patch(json!({ "somethingNew": 1 }))).unwrap(),
            before
        );
        assert!(apply_patch(&before, patch(json!({ "autoConnect": "yes" }))).is_err());
        assert!(apply_patch(&before, patch(json!({ "gestureSide": "middle" }))).is_err());
    }

    #[test]
    fn maps_can_be_replaced_whole() {
        let before = Settings::default();
        let after = apply_patch(
            &before,
            patch(json!({ "shortcuts": { "cycle": "Ctrl+Alt+N" } })),
        )
        .unwrap();
        assert_eq!(after.shortcuts["cycle"], "Ctrl+Alt+N");
    }
}
