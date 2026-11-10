//! The on-screen popup shown when a shortcut changes the mode (like the system volume popup): a
//! transparent, click-through, non-activating window at the bottom centre of the monitor the mouse is on.
//! The page (`hud.html`) does the fading; this side shows, positions and finally hides the window.

use std::time::Duration;

use buds_core::controller::Snapshot;
use buds_core::types::{AncLevel, LinkState, NoiseMode};
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder,
};
use ts_rs::TS;

use crate::positioning::{Rect, hud_position};
use crate::state::AppState;

pub const HUD: &str = "hud";
/// Window size in logical pixels: the pill plus room for its shadow.
const WIDTH: f64 = 340.0;
const HEIGHT: f64 = 100.0;
/// Distance from the bottom of the work area to the bottom of the window, logical pixels.
const LIFT: f64 = 100.0;
/// If the page never reports back, hide the window after this long.
const SAFETY_HIDE: Duration = Duration::from_millis(3500);

#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HudPayload {
    /// Increases with every show, so a page can tell a repeat from something it already displayed.
    #[ts(type = "number")]
    pub id: u64,
    /// `mode`: the mode that was just set; `disconnected`: the buds weren't connected.
    pub kind: HudKind,
    pub mode: Option<NoiseMode>,
    pub level: Option<AncLevel>,
    pub device_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HudKind {
    Mode,
    Disconnected,
}

impl HudPayload {
    pub fn from_snapshot(s: &Snapshot) -> HudPayload {
        let connected = s.link == LinkState::Connected && s.mode.is_some();
        HudPayload {
            id: 0,
            kind: if connected {
                HudKind::Mode
            } else {
                HudKind::Disconnected
            },
            mode: s.mode.filter(|_| connected),
            level: if s.mode == Some(NoiseMode::Anc) {
                s.level
            } else {
                None
            },
            device_name: s.device_name.clone(),
        }
    }
}

/// Creates the (hidden) HUD window ahead of time. Creating a webview window takes the keyboard focus
/// for a moment, which must not happen while the user is typing in another app.
pub fn prepare(app: &AppHandle) {
    let _ = window(app);
}

fn window(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    if let Some(window) = app.get_webview_window(HUD) {
        return Ok(window);
    }
    let window = WebviewWindowBuilder::new(app, HUD, WebviewUrl::App("hud.html".into()))
        .title("Buds Control HUD")
        .inner_size(WIDTH, HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .focusable(false)
        .focused(false)
        .visible(false)
        .build()?;
    // Clicks go to whatever is underneath.
    window.set_ignore_cursor_events(true)?;
    Ok(window)
}

/// Shows the HUD (or restarts it if it is already showing).
pub fn show(app: &AppHandle, mut payload: HudPayload) {
    let state = app.state::<AppState>();
    let generation = {
        let mut hud = state.hud.lock().unwrap();
        hud.counter += 1;
        hud.generation += 1;
        payload.id = hud.counter;
        hud.pending = Some(payload.clone());
        hud.generation
    };
    let Ok(window) = window(app) else { return };
    place(app, &window);
    if !window.is_visible().unwrap_or(false) {
        // Shown without taking the focus (the window is non-focusable).
        let _ = window.show();
    }
    let _ = window.emit("hud://show", &payload);

    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SAFETY_HIDE).await;
        if app.state::<AppState>().hud.lock().unwrap().generation == generation {
            hide(&app);
        }
    });
}

/// The page finished fading out.
pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(HUD) {
        let _ = window.hide();
    }
}

/// The latest payload, for a page that loaded after it was emitted.
pub fn pending(app: &AppHandle) -> Option<HudPayload> {
    app.state::<AppState>().hud.lock().unwrap().pending.clone()
}

/// Bottom centre of the monitor the mouse pointer is on.
fn place(app: &AppHandle, window: &WebviewWindow) {
    let monitor = app
        .cursor_position()
        .ok()
        .and_then(|p| app.monitor_from_point(p.x, p.y).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten());
    let Some(monitor) = monitor else { return };
    let w = monitor.work_area();
    let work = Rect::new(
        w.position.x,
        w.position.y,
        w.size.width as i32,
        w.size.height as i32,
    );
    let scale = monitor.scale_factor();
    let size = (
        (WIDTH * scale).round() as i32,
        (HEIGHT * scale).round() as i32,
    );
    let (x, y) = hud_position(work, size, (LIFT * scale).round() as i32);
    let _ = window.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    let _ = window.set_position(PhysicalPosition::new(x, y));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn snapshot(link: LinkState, mode: Option<NoiseMode>, level: Option<AncLevel>) -> Snapshot {
        let mut s = Snapshot::initial("PC");
        s.link = link;
        s.mode = mode;
        s.level = level;
        s.device_name = "OnePlus Buds 4".into();
        s
    }

    #[test]
    fn a_connected_mode_is_shown_with_its_level_only_in_anc() {
        let anc = HudPayload::from_snapshot(&snapshot(
            LinkState::Connected,
            Some(NoiseMode::Anc),
            Some(AncLevel::Max),
        ));
        assert_eq!(anc.kind, HudKind::Mode);
        assert_eq!(anc.mode, Some(NoiseMode::Anc));
        assert_eq!(anc.level, Some(AncLevel::Max));

        // A stale level from earlier must not appear next to Transparency.
        let transparency = HudPayload::from_snapshot(&snapshot(
            LinkState::Connected,
            Some(NoiseMode::Transparency),
            Some(AncLevel::Max),
        ));
        assert_eq!(transparency.mode, Some(NoiseMode::Transparency));
        assert_eq!(transparency.level, None);
    }

    #[test]
    fn not_connected_or_unknown_shows_the_disconnected_message() {
        for s in [
            snapshot(LinkState::Searching, Some(NoiseMode::Anc), None),
            snapshot(LinkState::Connecting, None, None),
            snapshot(LinkState::Connected, None, None),
        ] {
            let payload = HudPayload::from_snapshot(&s);
            assert_eq!(payload.kind, HudKind::Disconnected);
            assert_eq!(payload.mode, None);
            assert_eq!(payload.device_name, "OnePlus Buds 4");
        }
    }

    #[test]
    fn the_payload_serialises_for_the_page() {
        let payload =
            HudPayload::from_snapshot(&snapshot(LinkState::Connected, Some(NoiseMode::Off), None));
        let json = serde_json::to_value(&payload).unwrap();
        assert_eq!(json["kind"], "mode");
        assert_eq!(json["mode"], "off");
        assert_eq!(json["deviceName"], "OnePlus Buds 4");
    }
}
