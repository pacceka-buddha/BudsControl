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

/// Merges `patch` (any subset of the settings, camelCase) into the settings and saves them.
#[tauri::command]
pub fn update_settings(state: State<AppState>, patch: serde_json::Value) -> Result<(), String> {
    let serde_json::Value::Object(changes) = patch else {
        return Err("patch must be an object".into());
    };
    let mut error = None;
    state.handle.settings().update(|settings| {
        let mut value = serde_json::to_value(&*settings).unwrap_or_default();
        if let Some(map) = value.as_object_mut() {
            map.extend(changes);
        }
        match serde_json::from_value(value) {
            Ok(updated) => *settings = updated,
            Err(e) => error = Some(e.to_string()),
        }
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

/// The log as text, for the Copy button.
#[tauri::command]
pub fn export_log(state: State<AppState>) -> String {
    state.handle.log().export_text()
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
