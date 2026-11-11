//! The app's windows: the tray flyout (created hidden at startup so it opens instantly) and the
//! settings window (created on demand).

use std::time::{Duration, Instant};

#[cfg(any(windows, target_os = "macos"))]
use tauri::window::{Effect, EffectsBuilder};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, PhysicalSize, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};

use crate::positioning::{self, Anchor, Rect};
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
    let builder = WebviewWindowBuilder::new(app, FLYOUT, WebviewUrl::App("flyout.html".into()))
        .title("Buds Control")
        .inner_size(FLYOUT_WIDTH, INITIAL_HEIGHT)
        .decorations(false)
        // Linux has no window effects and a transparent window needs a compositor: solid there.
        .transparent(cfg!(not(target_os = "linux")))
        .shadow(true)
        .always_on_top(true)
        .skip_taskbar(true)
        // Linux (X11): a non-resizable GTK window never shrinks below the largest size it has had, so
        // collapsing the panel would leave a tall window. It has no decorations, so the user can't resize it.
        .resizable(cfg!(target_os = "linux"))
        .maximizable(false)
        .minimizable(false)
        .visible(false)
        .focused(false);
    #[cfg(windows)]
    let builder = builder.effects(EffectsBuilder::new().effect(Effect::Acrylic).build());
    // macOS: the system popover material behind the translucent page, with rounded corners.
    #[cfg(target_os = "macos")]
    let builder = builder
        .effects(
            EffectsBuilder::new()
                .effect(Effect::Popover)
                .radius(10.0)
                .build(),
        )
        .visible_on_all_workspaces(true);
    let window = builder.build()?;

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

    // The icon's rectangle in physical pixels: the click's or the tray's. Without one (Linux, where the
    // tray reports neither) the pointer; failing all of that, the last known anchor.
    let icon = icon
        .or_else(|| {
            app.tray_by_id(crate::tray::TRAY_ID)
                .and_then(|t| t.rect().ok().flatten())
        })
        .map(|r| to_rect(&r));
    let anchor = match icon {
        Some(rect) => Anchor::Icon(rect),
        None => fresh_anchor(app).unwrap_or(state.flyout.lock().unwrap().anchor),
    };

    // The monitor the anchor is on (the primary one when unknown).
    let monitor = match anchor {
        Anchor::Icon(r) => Some(r.center()),
        Anchor::Point(x, y) => Some((x, y)),
        Anchor::Centered | Anchor::Corner => None,
    }
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

    // A bare pointer (Linux) is turned into a spot on the panel, so the flyout sits against the panel.
    let anchor = match (anchor, &monitor) {
        (Anchor::Point(x, y), Some(m)) => {
            let (p, s) = (m.position(), m.size());
            let mon = Rect::new(p.x, p.y, s.width as i32, s.height as i32);
            positioning::panel_strip((x, y), mon, work).map_or(anchor, Anchor::Icon)
        }
        _ => anchor,
    };

    let height = {
        let mut f = state.flyout.lock().unwrap();
        f.anchor = anchor;
        f.work = Some(work);
        f.scale = scale;
        f.height
    };
    apply_geometry(&window, anchor, work, scale, height);
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("flyout-shown", ());
}

/// The page reports its content height; follow it, keeping the anchored edge fixed.
pub fn resize_flyout(app: &AppHandle, requested: f64) {
    let Some(window) = flyout(app) else { return };
    let state = app.state::<AppState>();
    let (anchor, work, scale, height) = {
        let mut f = state.flyout.lock().unwrap();
        let max = f
            .work
            .map_or(f64::MAX, |w| f64::from(w.h) / f.scale - 2.0 * MARGIN);
        f.height = requested.clamp(MIN_HEIGHT, max.max(MIN_HEIGHT));
        (f.anchor, f.work, f.scale, f.height)
    };
    if window.is_visible().unwrap_or(false)
        && let Some(work) = work
    {
        apply_geometry(&window, anchor, work, scale, height);
    } else {
        let _ = window.set_size(LogicalSize::new(FLYOUT_WIDTH, height));
    }
}

/// Sizes and positions the window (physical pixels, so mixed-DPI monitors behave).
fn apply_geometry(window: &WebviewWindow, anchor: Anchor, work: Rect, scale: f64, height: f64) {
    let size = (
        (FLYOUT_WIDTH * scale).round() as i32,
        (height * scale).round() as i32,
    );
    let (x, y) = positioning::place_at(anchor, work, size, (MARGIN * scale).round() as i32);
    let _ = window.set_size(PhysicalSize::new(size.0 as u32, size.1 as u32));
    let _ = window.set_position(PhysicalPosition::new(x, y));
    // X11: the window manager clamps the move to the screen using the *old* (taller) size, because the
    // shrink has not happened yet; once it has, the window is left too high. Move again afterwards.
    #[cfg(target_os = "linux")]
    {
        let window = window.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(80));
            let _ = window.set_position(PhysicalPosition::new(x, y));
        });
    }
}

/// A Wayland session (also when the app itself runs through Xwayland).
#[cfg(target_os = "linux")]
pub fn is_wayland() -> bool {
    std::env::var("XDG_SESSION_TYPE").is_ok_and(|v| v.eq_ignore_ascii_case("wayland"))
        || std::env::var_os("WAYLAND_DISPLAY").is_some()
}

#[cfg(not(target_os = "linux"))]
pub fn is_wayland() -> bool {
    false
}

/// Linux: the tray gives no icon position and no click events (the flyout is opened from the tray menu),
/// so the flyout appears at the pointer. On Wayland the pointer is unknown (through Xwayland it is stale
/// whenever it is over a native window such as the shell's menu), so the flyout goes to the right end of
/// the panel, where the tray is on GNOME and KDE: a point far to the right, snapped to the panel edge.
#[cfg(target_os = "linux")]
fn fresh_anchor(app: &AppHandle) -> Option<Anchor> {
    if is_wayland() {
        return Some(Anchor::Point(1_000_000, 0));
    }
    Some(match app.cursor_position() {
        Ok(p) => Anchor::Point(p.x.round() as i32, p.y.round() as i32),
        Err(_) => Anchor::Centered,
    })
}

#[cfg(not(target_os = "linux"))]
fn fresh_anchor(_app: &AppHandle) -> Option<Anchor> {
    None
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
    let builder = || {
        WebviewWindowBuilder::new(app, SETTINGS, WebviewUrl::App("settings.html".into()))
            .title("Buds Control Settings")
            .inner_size(600.0, 640.0)
            .min_inner_size(520.0, 480.0)
            .center()
    };
    // A Mica backdrop (Windows 11) behind a translucent page; plain window where it isn't available.
    #[cfg(windows)]
    let built = builder()
        .transparent(true)
        .effects(EffectsBuilder::new().effect(Effect::Mica).build())
        .build()
        .or_else(|_| builder().build());
    #[cfg(not(windows))]
    let built = builder().build();
    match built {
        Ok(window) => {
            // If the window goes away while the recorder is listening, shortcuts must work again.
            let handle = app.clone();
            window.on_window_event(move |event| {
                if matches!(event, WindowEvent::Destroyed) {
                    let handle = handle.clone();
                    tauri::async_runtime::spawn(async move {
                        crate::hotkeys::set_recording(&handle, false).await;
                    });
                }
            });
            let _ = window.set_focus();
        }
        Err(error) => eprintln!("could not open the settings window: {error}"),
    }
}
