//! The notification-area icon: left click opens the flyout, middle click cycles the mode, right click
//! shows a menu with the three modes, Settings, Reconnect and Quit. The tooltip and the menu's check
//! marks follow the controller's state.

use std::sync::Mutex;

use buds_art::{TrayIconSpec, render_tray_icon};
use buds_core::controller::Snapshot;
use buds_core::types::{LinkState, NoiseMode};
use tauri::image::Image;
use tauri::menu::{
    CheckMenuItem, CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, PredefinedMenuItem,
};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::app_windows;
use crate::state::AppState;

pub const TRAY_ID: &str = "main";
/// Windows shows at most 127 characters of a tray tooltip.
const TOOLTIP_MAX: usize = 127;

/// Handles kept so the menu can be updated from state changes.
pub struct TrayState {
    pub tray: TrayIcon,
    /// What the icon currently shows; it is only redrawn when this changes.
    shown: Mutex<Option<IconSpec>>,
    anc: CheckMenuItem<Wry>,
    transparency: CheckMenuItem<Wry>,
    off: CheckMenuItem<Wry>,
}

pub fn build(app: &AppHandle) -> tauri::Result<TrayState> {
    let anc = CheckMenuItemBuilder::with_id("anc", "Noise Cancellation")
        .enabled(false)
        .build(app)?;
    let transparency = CheckMenuItemBuilder::with_id("transparency", "Transparency")
        .enabled(false)
        .build(app)?;
    let off = CheckMenuItemBuilder::with_id("off", "Off")
        .enabled(false)
        .build(app)?;
    let settings = MenuItemBuilder::with_id("settings", "Settings…").build(app)?;
    let reconnect = MenuItemBuilder::with_id("reconnect", "Reconnect").build(app)?;
    let quit = MenuItemBuilder::with_id("quit", "Quit Buds Control").build(app)?;
    let menu = MenuBuilder::new(app);
    // Linux trays report no clicks, so the menu is the way in: "Open" comes first.
    #[cfg(target_os = "linux")]
    let menu = {
        let open = MenuItemBuilder::with_id("open", "Open Buds Control").build(app)?;
        menu.item(&open).item(&PredefinedMenuItem::separator(app)?)
    };
    let menu = menu
        .item(&anc)
        .item(&transparency)
        .item(&off)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&settings)
        .item(&reconnect)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&quit)
        .build()?;

    let builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Buds Control")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| tray_event(tray.app_handle(), &event));
    // macOS: a template icon, so the menu bar tints it for light/dark menu bars and highlights.
    #[cfg(target_os = "macos")]
    let builder = builder.icon_as_template(true);
    let tray = builder.build(app)?;
    Ok(TrayState {
        tray,
        shown: Mutex::new(None),
        anc,
        transparency,
        off,
    })
}

fn tray_event(app: &AppHandle, event: &TrayIconEvent) {
    if let TrayIconEvent::Click {
        button,
        button_state: MouseButtonState::Up,
        rect,
        ..
    } = event
    {
        match button {
            MouseButton::Left => app_windows::toggle_flyout(app, Some(*rect)),
            MouseButton::Middle => app.state::<AppState>().handle.cycle_mode(),
            MouseButton::Right => {}
        }
    }
}

fn menu_event(app: &AppHandle, id: &str) {
    let handle = &app.state::<AppState>().handle;
    let mode = match id {
        "anc" => Some(NoiseMode::Anc),
        "transparency" => Some(NoiseMode::Transparency),
        "off" => Some(NoiseMode::Off),
        _ => None,
    };
    if let Some(mode) = mode {
        handle.set_mode(mode, None);
        // Windows flips a check item by itself when it is clicked. Clicking the mode that is already
        // on would leave it unchecked, and no state change would come to put the mark back, so set
        // the marks from the current state now; the new mode's state change follows.
        update(app, &app.state::<TrayState>(), &handle.snapshot());
        return;
    }
    match id {
        "open" => app_windows::show_flyout(app, None),
        "settings" => app_windows::open_settings(app),
        "reconnect" => handle.reconnect(),
        "quit" => crate::quit(app),
        _ => {}
    }
}

/// Pixels per side of a tray icon at 100% scaling.
const BASE_ICON_SIZE: f64 = 16.0;

/// What the icon should look like right now.
type IconSpec = TrayIconSpec;

/// The icon for `snapshot` on a display with `scale` and a light or dark taskbar.
pub fn icon_spec(snapshot: &Snapshot, scale: f64, light_taskbar: bool) -> IconSpec {
    let connected = snapshot.link == LinkState::Connected;
    TrayIconSpec {
        // Windows draws tray icons at 16 px at 100%, 20 at 125%, 24 at 150%, 32 at 200%: render at
        // exactly that size so nothing is resampled.
        size: (BASE_ICON_SIZE * scale).round().clamp(16.0, 64.0) as u32,
        level: connected.then_some(snapshot.buds_battery).flatten(),
        connected,
        light_taskbar,
    }
}

/// Brings the icon, the tooltip and the menu in line with the buds' state, the display scale and the
/// taskbar colour. The icon is only redrawn when it would look different.
pub fn update(app: &AppHandle, state: &TrayState, snapshot: &Snapshot) {
    let scale = app
        .primary_monitor()
        .ok()
        .flatten()
        .map_or(1.0, |m| m.scale_factor());
    let spec = icon_spec(snapshot, scale, crate::theme::taskbar_is_light());
    {
        let mut shown = state.shown.lock().unwrap();
        if *shown != Some(spec) {
            let image = render_tray_icon(spec);
            let icon = Image::new_owned(image.rgba, image.width, image.height);
            if state.tray.set_icon(Some(icon)).is_ok() {
                *shown = Some(spec);
            }
        }
    }
    let _ = state.tray.set_tooltip(Some(tooltip(snapshot)));
    let connected = snapshot.link == LinkState::Connected;
    for (item, mode) in [
        (&state.anc, NoiseMode::Anc),
        (&state.transparency, NoiseMode::Transparency),
        (&state.off, NoiseMode::Off),
    ] {
        let _ = item.set_enabled(connected && snapshot.available_modes.contains(&mode));
        let _ = item.set_checked(connected && snapshot.mode == Some(mode));
    }
}

/// `Buds Control · OnePlus Buds 4` / `L 80% · R 82% · Case 45%` / `ANC · Max`, at most 127 characters.
pub fn tooltip(s: &Snapshot) -> String {
    let mut lines = vec![format!("Buds Control · {}", s.device_name)];
    match s.link {
        LinkState::Connected => {
            let mut parts = Vec::new();
            if let Some(b) = s.left {
                parts.push(format!("L {}%", b.level));
            }
            if let Some(b) = s.right {
                parts.push(format!("R {}%", b.level));
            }
            if let Some(b) = s.case {
                parts.push(format!("Case {}%", b.level));
            }
            if !parts.is_empty() {
                lines.push(parts.join(" · "));
            }
            if let Some(mode) = s.mode {
                let mut text = mode.short_title().to_string();
                if mode == NoiseMode::Anc
                    && let Some(level) = s.level
                {
                    text = format!("{text} · {}", level.title());
                }
                lines.push(text);
            }
        }
        LinkState::Connecting => lines.push("Connecting…".into()),
        LinkState::Searching => lines.push("Not connected".into()),
    }
    let text = lines.join("\n");
    if text.chars().count() <= TOOLTIP_MAX {
        return text;
    }
    let mut cut: String = text.chars().take(TOOLTIP_MAX - 1).collect();
    cut.push('…');
    cut
}

#[cfg(test)]
mod tests {
    use super::*;
    use buds_core::types::{AncLevel, Battery};

    fn connected() -> Snapshot {
        let mut s = Snapshot::initial("PC");
        s.link = LinkState::Connected;
        s.device_name = "OnePlus Buds 4".into();
        s.left = Some(Battery {
            level: 80,
            charging: false,
        });
        s.right = Some(Battery {
            level: 82,
            charging: true,
        });
        s.case = Some(Battery {
            level: 45,
            charging: false,
        });
        s.mode = Some(NoiseMode::Anc);
        s.level = Some(AncLevel::Max);
        s
    }

    #[test]
    fn icon_follows_state_scale_and_taskbar() {
        let mut s = connected();
        s.buds_battery = Some(80);
        let spec = icon_spec(&s, 1.0, false);
        assert_eq!(
            (spec.size, spec.level, spec.connected, spec.light_taskbar),
            (16, Some(80), true, false)
        );
        assert_eq!(icon_spec(&s, 1.25, true).size, 20);
        assert_eq!(icon_spec(&s, 1.5, true).size, 24);
        assert_eq!(icon_spec(&s, 2.0, true).size, 32);
        assert!(icon_spec(&s, 1.5, true).light_taskbar);
        // Disconnected: dimmed, and the (stale) battery level is not shown.
        let mut off = s.clone();
        off.link = LinkState::Searching;
        let spec = icon_spec(&off, 1.0, false);
        assert!(!spec.connected);
        assert_eq!(spec.level, None);
        // Absurd scales stay in range.
        assert_eq!(icon_spec(&s, 0.5, false).size, 16);
        assert_eq!(icon_spec(&s, 10.0, false).size, 64);
    }

    #[test]
    fn tooltip_connected() {
        assert_eq!(
            tooltip(&connected()),
            "Buds Control · OnePlus Buds 4\nL 80% · R 82% · Case 45%\nANC · Max"
        );
    }

    #[test]
    fn tooltip_without_case_or_level() {
        let mut s = connected();
        s.case = None;
        s.mode = Some(NoiseMode::Transparency);
        assert_eq!(
            tooltip(&s),
            "Buds Control · OnePlus Buds 4\nL 80% · R 82%\nTransparency"
        );
    }

    #[test]
    fn tooltip_not_connected_and_connecting() {
        let mut s = Snapshot::initial("PC");
        assert_eq!(tooltip(&s), "Buds Control · Earbuds\nNot connected");
        s.link = LinkState::Connecting;
        assert_eq!(tooltip(&s), "Buds Control · Earbuds\nConnecting…");
    }

    #[test]
    fn tooltip_is_cut_at_127_characters() {
        let mut s = connected();
        s.device_name = "A very long Bluetooth name ".repeat(8);
        let text = tooltip(&s);
        assert_eq!(text.chars().count(), 127);
        assert!(text.ends_with('…'));
    }
}
