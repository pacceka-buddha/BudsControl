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
pub const FLYOUT_WIDTH: f64 = 272.0;
pub const INITIAL_HEIGHT: f64 = 160.0;
/// Space between the flyout and the screen edges / taskbar, in logical pixels.
const MARGIN: f64 = 12.0;
const MIN_HEIGHT: f64 = 80.0;
/// A tray click this soon after a blur-hide is the same click (see `FlyoutState`).
const BLUR_CLICK_WINDOW: Duration = Duration::from_millis(300);

/// Sets one of the window's DWM colours (a COLORREF, 0x00BBGGRR, or a special `DWMWA_COLOR_*`).
/// Windows 10 doesn't know these attributes and returns an error, which is ignored.
#[cfg(windows)]
fn set_dwm_color(
    window: &WebviewWindow,
    attribute: windows::Win32::Graphics::Dwm::DWMWINDOWATTRIBUTE,
    color: u32,
) {
    let Ok(hwnd) = window.hwnd() else { return };
    // SAFETY: a documented attribute with a u32 value, on a window this process owns.
    let _ = unsafe {
        windows::Win32::Graphics::Dwm::DwmSetWindowAttribute(
            hwnd,
            attribute,
            (&raw const color).cast(),
            std::mem::size_of::<u32>() as u32,
        )
    };
}

/// Windows 11 draws a one-pixel border round the window, in the accent colour when "Show accent colour
/// on title bars and window borders" is on. The flyout draws its own faint edge instead.
#[cfg(windows)]
fn remove_border(window: &WebviewWindow) {
    use windows::Win32::Graphics::Dwm::{DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE};
    set_dwm_color(window, DWMWA_BORDER_COLOR, DWMWA_COLOR_NONE);
}

/// Windows 11: for the borderless window's shadow, Tao keeps a one-pixel strip of title bar along the
/// top, which Windows paints in the title bar colour (the accent colour, with "Show accent colour on
/// title bars" on). The page works out the colour of its glass along that edge and passes it here
/// (0x00BBGGRR), so the strip reads as the glass's lit rim.
pub fn set_flyout_edge(app: &AppHandle, color: u32) {
    #[cfg(windows)]
    if let Some(window) = flyout(app) {
        set_dwm_color(
            &window,
            windows::Win32::Graphics::Dwm::DWMWA_CAPTION_COLOR,
            color,
        );
    }
    #[cfg(not(windows))]
    let _ = (app, color);
}

/// Where the page draws the snapshot behind the flyout (see `backdrop.rs`), in logical pixels
/// relative to the window.
#[cfg(windows)]
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackdropInfo {
    /// Matches `flyout_reveal`: the page reports when it has drawn this snapshot.
    pub generation: u64,
    /// The snapshot's size in samples.
    pub width: usize,
    pub height: usize,
    /// Where the snapshot's area lies, and its size.
    pub left: f64,
    pub top: f64,
    pub bottom: f64,
    pub css_width: f64,
    pub css_height: f64,
    /// The window grows upwards (its bottom edge stays put), so the page keeps the snapshot pinned to
    /// the bottom edge; otherwise to the top.
    pub pin_bottom: bool,
}

/// Extra snapshot on each side of the flyout, so the blur doesn't fade at the window's edges.
#[cfg(windows)]
const BACKDROP_PAD: f64 = 64.0;
/// How long the page gets to draw a new snapshot before the flyout opens anyway.
#[cfg(windows)]
const REVEAL_TIMEOUT: Duration = Duration::from_millis(250);

/// Creates the flyout window, hidden. Hides itself when it loses focus.
pub fn create_flyout(app: &AppHandle) -> tauri::Result<()> {
    let builder = WebviewWindowBuilder::new(app, FLYOUT, WebviewUrl::App("flyout.html".into()))
        .title("Buds Control")
        .inner_size(FLYOUT_WIDTH, INITIAL_HEIGHT)
        .decorations(false)
        // Linux has no window effects, so the page draws its own opaque, rounded sheet in the
        // transparent window (only the corners need a compositor, which every current desktop has).
        .transparent(true)
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
    // macOS: the system popover material behind the translucent page, with rounded corners.
    #[cfg(target_os = "macos")]
    let builder = builder
        .effects(
            EffectsBuilder::new()
                .effect(Effect::Popover)
                .radius(16.0)
                .build(),
        )
        .visible_on_all_workspaces(true);
    let window = builder.build()?;
    #[cfg(windows)]
    remove_border(&window);

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
    // A show still waiting for its snapshot is called off.
    app.state::<AppState>().flyout.lock().unwrap().generation += 1;
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
    let placed = apply_geometry(&window, anchor, work, scale, height);
    let generation = {
        let mut f = state.flyout.lock().unwrap();
        f.generation += 1;
        f.generation
    };

    // Windows: snapshot what the flyout will cover, let the page draw it, then show the window (or
    // show it anyway if the page is slow).
    #[cfg(windows)]
    if let Some(m) = &monitor {
        let (p, s) = (m.position(), m.size());
        let screen = Rect::new(p.x, p.y, s.width as i32, s.height as i32);
        let pad = (BACKDROP_PAD * scale).round() as i32;
        let left = (placed.x - pad).max(screen.x);
        let right = (placed.right() + pad).min(screen.right());
        let area = Rect::new(left, screen.y, right - left, screen.h);
        // Without a snapshot (the copy failed) the page falls back to a solid surface.
        let shot = crate::backdrop::capture(area, scale);
        let taller = positioning::place_at(
            anchor,
            work,
            (placed.w, placed.h + 40),
            (MARGIN * scale).round() as i32,
        );
        let info = BackdropInfo {
            generation,
            width: shot.as_ref().map_or(0, |s| s.width),
            height: shot.as_ref().map_or(0, |s| s.height),
            left: f64::from(area.x - placed.x) / scale,
            top: f64::from(area.y - placed.y) / scale,
            bottom: f64::from(placed.bottom() - area.bottom()) / scale,
            css_width: f64::from(area.w) / scale,
            css_height: f64::from(area.h) / scale,
            pin_bottom: taller.1 != placed.y,
        };
        state.flyout.lock().unwrap().backdrop = shot.map(|s| s.rgba).unwrap_or_default();
        let _ = window.emit("flyout-backdrop", info);
        let app = app.clone();
        std::thread::spawn(move || {
            std::thread::sleep(REVEAL_TIMEOUT);
            reveal_flyout(&app, generation);
        });
        return;
    }
    #[cfg(not(windows))]
    let _ = placed;
    reveal_flyout(app, generation);
}

/// Shows the flyout prepared by `show_flyout`, unless a later show or a hide has replaced it.
pub fn reveal_flyout(app: &AppHandle, generation: u64) {
    let Some(window) = flyout(app) else { return };
    {
        let state = app.state::<AppState>();
        let mut f = state.flyout.lock().unwrap();
        if f.generation != generation || f.revealed == generation {
            return;
        }
        f.revealed = generation;
    }
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

/// Sizes and positions the window (physical pixels, so mixed-DPI monitors behave). Returns where it put it.
fn apply_geometry(
    window: &WebviewWindow,
    anchor: Anchor,
    work: Rect,
    scale: f64,
    height: f64,
) -> Rect {
    let size = (
        (FLYOUT_WIDTH * scale).round() as i32,
        // Up, so a fractional scale never leaves the page a pixel short (which shows a scrollbar).
        (height * scale).ceil() as i32,
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
    Rect::new(x, y, size.0, size.1)
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
