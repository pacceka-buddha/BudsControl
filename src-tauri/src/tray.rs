//! The notification-area icon: left click opens the flyout, middle click cycles the mode, right click
//! shows a menu with the three modes, Settings, Reconnect and Quit. The tooltip and the menu's check
//! marks follow the controller's state.

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
    let menu = MenuBuilder::new(app)
        .item(&anc)
        .item(&transparency)
        .item(&off)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&settings)
        .item(&reconnect)
        .item(&PredefinedMenuItem::separator(app)?)
        .item(&quit)
        .build()?;

    let mut builder = TrayIconBuilder::with_id(TRAY_ID)
        .tooltip("Buds Control")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| menu_event(app, event.id().as_ref()))
        .on_tray_icon_event(|tray, event| tray_event(tray.app_handle(), &event));
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(Image::new_owned(
            icon.rgba().to_vec(),
            icon.width(),
            icon.height(),
        ));
    }
    let tray = builder.build(app)?;
    Ok(TrayState {
        tray,
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
    match id {
        "anc" => handle.set_mode(NoiseMode::Anc, None),
        "transparency" => handle.set_mode(NoiseMode::Transparency, None),
        "off" => handle.set_mode(NoiseMode::Off, None),
        "settings" => app_windows::open_settings(app),
        "reconnect" => handle.reconnect(),
        "quit" => crate::quit(app),
        _ => {}
    }
}

/// Brings the tooltip and the menu in line with the buds' state.
pub fn update(state: &TrayState, snapshot: &Snapshot) {
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
