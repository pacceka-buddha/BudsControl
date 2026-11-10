//! The app's windows: the tray flyout (created hidden at startup so it opens instantly) and the
//! settings window (created on demand).

use std::time::{Duration, Instant};

use tauri::window::{Effect, EffectsBuilder};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::positioning::{self, Rect};
use crate::state::AppState;

pub const FLYOUT: &str = "flyout";
pub const SETTINGS: &str = "settings";
pub const FLYOUT_WIDTH: f64 = 300.0;
pub const INITIAL_HEIGHT: f64 = 160.0;
/// Space between the flyout and the screen edges / taskbar, in logical pixels.
const MARGIN: f64 = 12.0;
const MIN_HEIGHT: f64 = 80.0;
/// A tray click this soon after a blur-hide is the same click (see `FlyoutState`).
const BLUR_CLICK_WINDOW: Duration = Duration::from_millis(300);

/// Creates the flyout window, hidden. Hides itself when it loses focus.
pub fn create_flyout(app: &AppHandle) -> tauri::Result<()> {
    let window = WebviewWindowBuilder::new(app, FLYOUT, WebviewUrl::App("flyout.html".into()))
        .title("Buds Control")
        .inner_size(FLYOUT_WIDTH, INITIAL_HEIGHT)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .visible(false)
        .focused(false)
        .effects(EffectsBuilder::new().effect(Effect::Acrylic).build())
        .build()?;

    let handle = app.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Focused(false) => {
            let menu_open = handle.state::<AppState>().flyout.lock().unwrap().menu_open;
            if !menu_open {
                hide_flyout(&handle, true);
            }
        }
        WindowEvent::CloseRequested { api, .. } => {
            api.prevent_close();
            hide_flyout(&handle, false);
        }
        _ => {}
    });
    Ok(())
}

fn flyout(app: &AppHandle) -> Option<WebviewWindow> {
    app.get_webview_window(FLYOUT)
}

pub fn hide_flyout(app: &AppHandle, by_blur: bool) {
    let Some(window) = flyout(app) else { return };
    if !window.is_visible().unwrap_or(false) {
        return;
    }
    let _ = window.hide();
    if by_blur {
        app.state::<AppState>()
            .flyout
            .lock()
            .unwrap()
            .hidden_by_blur_at = Some(Instant::now());
    }
}

/// Left click on the tray icon: open the flyout, or close it if it is open.
pub fn toggle_flyout(app: &AppHandle, icon: Option<tauri::Rect>) {
    let Some(window) = flyout(app) else { return };
    if window.is_visible().unwrap_or(false) {
        hide_flyout(app, false);
        return;
    }
    let recently_blurred = app
        .state::<AppState>()
        .flyout
        .lock()
        .unwrap()
        .hidden_by_blur_at
        .is_some_and(|t| t.elapsed() < BLUR_CLICK_WINDOW);
    if recently_blurred {
        return; // this click is what took the focus away; it means "close"
    }
    show_flyout(app, icon);
}

/// Shows the flyout next to the tray icon (`icon`, or the tray's own idea of where it is).
pub fn show_flyout(app: &AppHandle, icon: Option<tauri::Rect>) {
    let Some(window) = flyout(app) else { return };
    let state = app.state::<AppState>();

    // The icon's rectangle in physical pixels: the click's, the tray's, or the last known one.
    let icon = icon
        .or_else(|| {
            app.tray_by_id(crate::tray::TRAY_ID)
                .and_then(|t| t.rect().ok().flatten())
        })
        .map(|r| to_rect(&r))
        .or(state.flyout.lock().unwrap().icon);

    // The monitor the icon is on (the primary one when unknown).
    let monitor = icon
        .map(|r| r.center())
        .and_then(|(x, y)| {
            app.monitor_from_point(f64::from(x), f64::from(y))
                .ok()
                .flatten()
        })
        .or_else(|| app.primary_monitor().ok().flatten());
    let (work, scale) = match &monitor {
        Some(m) => {
            let w = m.work_area();
            (
                Rect::new(
                    w.position.x,
                    w.position.y,
                    w.size.width as i32,
                    w.size.height as i32,
                ),
                m.scale_factor(),
            )
        }
        None => (Rect::new(0, 0, 1920, 1040), 1.0),
    };

    let height = {
        let mut f = state.flyout.lock().unwrap();
        f.icon = icon;
        f.work = Some(work);
        f.scale = scale;
        f.height
    };
    apply_geometry(&window, icon, work, scale, height);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("flyout-shown", ());
}

/// The page reports its content height; follow it, keeping the anchored edge fixed.
pub fn resize_flyout(app: &AppHandle, requested: f64) {
    let Some(window) = flyout(app) else { return };
    let state = app.state::<AppState>();
    let (icon, work, scale, height) = {
        let mut f = state.flyout.lock().unwrap();
        let max = f
            .work
            .map_or(f64::MAX, |w| f64::from(w.h) / f.scale - 2.0 * MARGIN);
        f.height = requested.clamp(MIN_HEIGHT, max.max(MIN_HEIGHT));
        (f.icon, f.work, f.scale, f.height)
    };
    if window.is_visible().unwrap_or(false)
        && let Some(work) = work
    {
        apply_geometry(&window, icon, work, scale, height);
    } else {
        let _ = window.set_size(LogicalSize::new(FLYOUT_WIDTH, height));
    }
}

/// Sizes and positions the window (physical pixels, so mixed-DPI monitors behave).
fn apply_geometry(window: &WebviewWindow, icon: Option<Rect>, work: Rect, scale: f64, height: f64) {
    let size = (
        (FLYOUT_WIDTH * scale).round() as i32,
        (height * scale).round() as i32,
    );
    let placement = positioning::place(icon, work, size, (MARGIN * scale).round() as i32);
    let _ = window.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    let _ = window.set_position(PhysicalPosition::new(placement.x, placement.y));
}

fn to_rect(rect: &tauri::Rect) -> Rect {
    let p = rect.position.to_physical::<i32>(1.0);
    let s = rect.size.to_physical::<u32>(1.0);
    Rect::new(p.x, p.y, s.width as i32, s.height as i32)
}

/// Opens the settings window, or brings it to the front.
pub fn open_settings(app: &AppHandle) {
    hide_flyout(app, false); // the flyout has done its job once Settings is wanted
    if let Some(window) = app.get_webview_window(SETTINGS) {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
        .title("Buds Control Settings")
        .inner_size(600.0, 640.0)
        .min_inner_size(520.0, 480.0)
        .center()
        .build();
    match built {
        Ok(window) => {
            let _ = window.set_focus();
        }
        Err(error) => eprintln!("could not open the settings window: {error}"),
    }
}
