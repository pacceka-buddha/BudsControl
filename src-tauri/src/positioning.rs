//! Where the flyout goes: next to the tray icon, on the taskbar's side of the screen, kept inside the
//! monitor's work area. Pure integer geometry in physical pixels, so it is unit-tested.

/// A rectangle in physical (device) pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }
    pub fn right(&self) -> i32 {
        self.x + self.w
    }
    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.w / 2, self.y + self.h / 2)
    }
}

/// Which screen edge the taskbar (and so the tray icon) is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Edge {
    Bottom,
    Top,
    Left,
    Right,
}

/// The taskbar edge: the side of the work area the icon lies outside of. An icon inside the work area
/// (for example in the tray overflow popup) is treated as a bottom taskbar.
pub fn taskbar_edge(icon: Rect, work: Rect) -> Edge {
    let (cx, cy) = icon.center();
    if cy >= work.bottom() {
        Edge::Bottom
    } else if cy < work.y {
        Edge::Top
    } else if cx < work.x {
        Edge::Left
    } else if cx >= work.right() {
        Edge::Right
    } else {
        Edge::Bottom
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    pub x: i32,
    pub y: i32,
    pub edge: Edge,
}

fn clamp_span(value: i32, lo: i32, hi: i32) -> i32 {
    // If the window is bigger than the space, stick to the low edge.
    if hi < lo { lo } else { value.clamp(lo, hi) }
}

/// Top-left position for a window of `size` (w, h) with `margin` to the work-area edges. With no icon
/// (unknown tray position) the bottom-right corner is used, which is where the tray usually is.
pub fn place(icon: Option<Rect>, work: Rect, size: (i32, i32), margin: i32) -> Placement {
    let (w, h) = size;
    let min_x = work.x + margin;
    let max_x = work.right() - w - margin;
    let min_y = work.y + margin;
    let max_y = work.bottom() - h - margin;
    let Some(icon) = icon else {
        return Placement {
            x: clamp_span(max_x, min_x, max_x),
            y: clamp_span(max_y, min_y, max_y),
            edge: Edge::Bottom,
        };
    };
    let edge = taskbar_edge(icon, work);
    let (cx, cy) = icon.center();
    let (x, y) = match edge {
        Edge::Bottom => (
            clamp_span(cx - w / 2, min_x, max_x),
            clamp_span(max_y, min_y, max_y),
        ),
        Edge::Top => (clamp_span(cx - w / 2, min_x, max_x), min_y),
        Edge::Left => (min_x, clamp_span(cy - h / 2, min_y, max_y)),
        Edge::Right => (
            clamp_span(max_x, min_x, max_x),
            clamp_span(cy - h / 2, min_y, max_y),
        ),
    };
    Placement { x, y, edge }
}

/// What the flyout is placed relative to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
    /// The tray icon's rectangle (Windows, macOS).
    Icon(Rect),
    /// The pointer when the tray gives no icon position (Linux, where the tray menu opens the flyout).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Point(i32, i32),
    /// Nowhere known, and windows can't be positioned (Wayland).
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    Centered,
    /// Nowhere known: the bottom-right corner, where a tray usually is.
    Corner,
}

/// Top-left of the flyout for `anchor`.
pub fn place_at(anchor: Anchor, work: Rect, size: (i32, i32), margin: i32) -> (i32, i32) {
    match anchor {
        Anchor::Icon(icon) => {
            let p = place(Some(icon), work, size, margin);
            (p.x, p.y)
        }
        Anchor::Corner => {
            let p = place(None, work, size, margin);
            (p.x, p.y)
        }
        Anchor::Point(x, y) => near_point((x, y), work, size, margin),
        Anchor::Centered => (
            work.x + (work.w - size.0) / 2,
            work.y + (work.h - size.1) / 2,
        ),
    }
}

/// Linux gives no tray icon position, only the pointer (which is on the tray menu, well away from the
/// panel). When the monitor reserves space for a panel, return a thin rectangle in that strip at the
/// pointer's position, so the flyout can be placed against the panel exactly as for a real icon.
pub fn panel_strip(point: (i32, i32), monitor: Rect, work: Rect) -> Option<Rect> {
    let (px, py) = point;
    let x = px.clamp(work.x, (work.right() - 1).max(work.x));
    let y = py.clamp(work.y, (work.bottom() - 1).max(work.y));
    if work.bottom() < monitor.bottom() {
        Some(Rect::new(
            x,
            work.bottom(),
            1,
            monitor.bottom() - work.bottom(),
        ))
    } else if work.y > monitor.y {
        Some(Rect::new(x, monitor.y, 1, work.y - monitor.y))
    } else if work.x > monitor.x {
        Some(Rect::new(monitor.x, y, work.x - monitor.x, 1))
    } else if work.right() < monitor.right() {
        Some(Rect::new(
            work.right(),
            y,
            monitor.right() - work.right(),
            1,
        ))
    } else {
        None
    }
}

/// A window of `size` opened at a point: centred on it horizontally, below it when there is room and
/// above it otherwise, kept inside the work area.
pub fn near_point(point: (i32, i32), work: Rect, size: (i32, i32), margin: i32) -> (i32, i32) {
    let (w, h) = size;
    let (min_x, max_x) = (work.x + margin, work.right() - w - margin);
    let (min_y, max_y) = (work.y + margin, work.bottom() - h - margin);
    let below = point.1 + margin;
    let y = if below <= max_y {
        below
    } else {
        point.1 - h - margin
    };
    (
        clamp_span(point.0 - w / 2, min_x, max_x),
        clamp_span(y, min_y, max_y),
    )
}

/// Top-left of the HUD window: horizontally centred in the work area, its bottom edge `lift` pixels
/// above the work area's bottom.
pub fn hud_position(work: Rect, size: (i32, i32), lift: i32) -> (i32, i32) {
    (
        work.x + (work.w - size.0) / 2,
        work.bottom() - size.1 - lift,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_window_opened_at_the_pointer_goes_below_it_or_above_when_there_is_no_room() {
        let work = Rect::new(0, 0, 1920, 1080);
        let size = (300, 400);
        // Pointer near the top (a top panel): the window hangs below it, centred on it.
        assert_eq!(near_point((960, 20), work, size, 12), (810, 32));
        // Pointer near the bottom: the window sits above it.
        assert_eq!(near_point((960, 1060), work, size, 12), (810, 648));
        // Pointer in a corner: kept inside the work area.
        assert_eq!(near_point((1915, 1075), work, size, 12), (1608, 663));
        assert_eq!(near_point((2, 5), work, size, 12), (12, 17));
    }

    #[test]
    fn anchors_map_to_positions() {
        let work = Rect::new(0, 0, 1920, 1080);
        let size = (300, 400);
        assert_eq!(place_at(Anchor::Centered, work, size, 12), (810, 340));
        let corner = place(None, work, size, 12);
        assert_eq!(
            place_at(Anchor::Corner, work, size, 12),
            (corner.x, corner.y)
        );
        let icon = Rect::new(1800, 1040, 16, 16);
        let p = place(Some(icon), work, size, 12);
        assert_eq!(place_at(Anchor::Icon(icon), work, size, 12), (p.x, p.y));
    }

    // A 1920x1080 monitor with a 48 px taskbar at the bottom.
    const WORK_BOTTOM: Rect = Rect {
        x: 0,
        y: 0,
        w: 1920,
        h: 1032,
    };

    #[test]
    fn bottom_taskbar_centres_above_the_icon() {
        let icon = Rect::new(1700, 1040, 24, 24);
        let p = place(Some(icon), WORK_BOTTOM, (300, 400), 12);
        assert_eq!(p.edge, Edge::Bottom);
        assert_eq!(p.x, 1712 - 150); // icon centre minus half the width
        assert_eq!(p.y, 1032 - 400 - 12); // bottom edge sits 12 px above the taskbar
    }

    #[test]
    fn clamps_inside_the_work_area() {
        // Icon at the far right: the window can't stick out past the margin.
        let icon = Rect::new(1890, 1040, 24, 24);
        let p = place(Some(icon), WORK_BOTTOM, (300, 400), 12);
        assert_eq!(p.x, 1920 - 300 - 12);
        // Icon at the far left.
        let p = place(
            Some(Rect::new(2, 1040, 24, 24)),
            WORK_BOTTOM,
            (300, 400),
            12,
        );
        assert_eq!(p.x, 12);
    }

    #[test]
    fn top_taskbar_hangs_below_it() {
        let work = Rect::new(0, 48, 1920, 1032);
        let icon = Rect::new(1700, 8, 24, 24);
        let p = place(Some(icon), work, (300, 400), 12);
        assert_eq!(p.edge, Edge::Top);
        assert_eq!(p.y, 48 + 12);
    }

    #[test]
    fn left_and_right_taskbars() {
        let work = Rect::new(60, 0, 1860, 1080);
        // Icon mid-screen: the window is centred on it.
        let p = place(Some(Rect::new(18, 500, 24, 24)), work, (300, 400), 12);
        assert_eq!(p.edge, Edge::Left);
        assert_eq!(p.x, 60 + 12);
        assert_eq!(p.y, 512 - 200);
        // Icon near the bottom: clamped so the window stays inside the work area.
        let p = place(Some(Rect::new(18, 900, 24, 24)), work, (300, 400), 12);
        assert_eq!(p.y, 1080 - 400 - 12);

        let work = Rect::new(0, 0, 1860, 1080);
        let p = place(Some(Rect::new(1878, 900, 24, 24)), work, (300, 400), 12);
        assert_eq!(p.edge, Edge::Right);
        assert_eq!(p.x, 1860 - 300 - 12);
    }

    #[test]
    fn secondary_monitor_with_negative_coordinates() {
        // A monitor to the left of the primary one.
        let work = Rect::new(-1920, 0, 1920, 1032);
        let icon = Rect::new(-300, 1040, 24, 24);
        let p = place(Some(icon), work, (300, 400), 12);
        assert_eq!(p.x, -288 - 150);
        assert!(p.x >= work.x + 12 && p.x + 300 <= work.right() - 12);
    }

    #[test]
    fn no_icon_uses_the_bottom_right_corner() {
        let p = place(None, WORK_BOTTOM, (300, 400), 12);
        assert_eq!((p.x, p.y), (1920 - 300 - 12, 1032 - 400 - 12));
    }

    #[test]
    fn pointer_on_a_tray_menu_snaps_to_the_panel_strip() {
        let monitor = Rect::new(0, 0, 1920, 1200);
        // 48 px panel at the bottom, like Dash to Panel.
        let work = Rect::new(0, 0, 1920, 1152);
        let strip = panel_strip((980, 640), monitor, work).unwrap();
        assert_eq!(strip, Rect::new(980, 1152, 1, 48));
        let p = place(Some(strip), work, (320, 420), 8);
        assert_eq!(p.edge, Edge::Bottom);
        assert_eq!(p.y, 1152 - 420 - 8);
        // Top bar only (stock GNOME).
        let work = Rect::new(0, 32, 1920, 1168);
        let strip = panel_strip((980, 640), monitor, work).unwrap();
        assert_eq!(place(Some(strip), work, (320, 420), 8).edge, Edge::Top);
        // No panel: fall back to the pointer.
        assert_eq!(panel_strip((980, 640), monitor, monitor), None);
    }

    #[test]
    fn growing_keeps_the_anchored_edge_fixed() {
        let icon = Rect::new(1700, 1040, 24, 24);
        let small = place(Some(icon), WORK_BOTTOM, (300, 200), 12);
        let big = place(Some(icon), WORK_BOTTOM, (300, 500), 12);
        assert_eq!(
            small.y + 200,
            big.y + 500,
            "bottom edge stays put, the window grows upward"
        );
        assert_eq!(small.x, big.x);
        let top = Rect::new(0, 48, 1920, 1032);
        let a = place(Some(Rect::new(1700, 8, 24, 24)), top, (300, 200), 12);
        let b = place(Some(Rect::new(1700, 8, 24, 24)), top, (300, 500), 12);
        assert_eq!(a.y, b.y, "top edge stays put, the window grows downward");
    }

    #[test]
    fn a_window_taller_than_the_screen_sticks_to_the_top() {
        let p = place(
            Some(Rect::new(1700, 1040, 24, 24)),
            WORK_BOTTOM,
            (300, 2000),
            12,
        );
        assert_eq!(p.y, 12);
    }

    #[test]
    fn overflow_popup_icons_count_as_bottom() {
        // The icon lives inside the work area (the hidden-icons popup).
        let icon = Rect::new(1650, 900, 24, 24);
        assert_eq!(taskbar_edge(icon, WORK_BOTTOM), Edge::Bottom);
    }

    #[test]
    fn the_hud_is_centred_above_the_bottom() {
        let work = Rect::new(0, 0, 1920, 1032);
        assert_eq!(hud_position(work, (340, 100), 120), (790, 812));
        // On a second monitor with an offset.
        let work = Rect::new(1920, -200, 2560, 1400);
        assert_eq!(
            hud_position(work, (340, 100), 120),
            (1920 + 1110, 1200 - 220)
        );
    }
}
