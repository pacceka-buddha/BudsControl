//! State shared by the commands, the tray and the windows.

use std::sync::Mutex;
use std::time::Instant;

use buds_core::controller::ControllerHandle;

use crate::positioning::Rect;

/// Everything about the flyout window that outlives a single show/hide.
pub struct FlyoutState {
    /// Where the tray icon was when the flyout was last shown (physical pixels).
    pub icon: Option<Rect>,
    /// The work area of the monitor the flyout is on, and its scale factor.
    pub work: Option<Rect>,
    pub scale: f64,
    /// Content height in logical pixels, as reported by the page.
    pub height: f64,
    /// When the flyout last hid itself because it lost focus. A tray click right after that is the
    /// click that caused it, and must not re-open the flyout.
    pub hidden_by_blur_at: Option<Instant>,
    /// A native menu opened from the flyout is showing; losing focus to it must not hide the flyout.
    pub menu_open: bool,
}

impl Default for FlyoutState {
    fn default() -> Self {
        FlyoutState {
            icon: None,
            work: None,
            scale: 1.0,
            height: crate::app_windows::INITIAL_HEIGHT,
            hidden_by_blur_at: None,
            menu_open: false,
        }
    }
}

pub struct AppState {
    pub handle: ControllerHandle,
    pub flyout: Mutex<FlyoutState>,
    /// Running against the simulated buds (`BUDS_FAKE=1`).
    pub fake: bool,
}
